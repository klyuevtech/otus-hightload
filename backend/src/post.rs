use std::{cmp, fmt};
use std::error::Error;
use amqprs::channel::{BasicAckArguments, BasicConsumeArguments, Channel};
use amqprs::consumer::AsyncConsumer;
use amqprs::{BasicProperties, Deliver};
use deadpool_redis::Connection;
use serde::{Deserialize, Serialize};
use tokio_postgres::{Error as PostgresError, GenericClient, Row};
use tonic::async_trait;
use uuid::Uuid;
use lazy_static::lazy_static;

use crate::{friend, postgres, rabbitmq, redis, websocket};

pub const FEED_LENGTH: i64 = 1000;
pub const FEED_CACHE_KEY_PREFIX: &str = "feed:";

pub const FEED_QUEUE_NAME: &str = "feed.amqprs.post";
pub const FEED_QUEUE_EXCHANGE_NAME: &str = "feed.amq.topic.post";
pub const FEED_QUEUE_ROUTING_KEY_PREFIX: &str = "feed.userid.";
pub const FEED_QUEUE_CONSUMER_TAG: &str = "feed_sub_pub";

lazy_static! {
    pub static ref FEED_ONE_POST_PER_USER: bool = std::env::var("POSTS_FEED_ONE_POST_PER_USER").unwrap_or_else(|_| "false".to_string()).as_str() == "true";
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Post {
    id: Uuid,
    content: String,
    user_id: Uuid,
    time_created: chrono::NaiveDateTime,
    time_updated: chrono::NaiveDateTime,
}

impl From<Row> for Post {
    fn from(row: Row) -> Self {
        Self {
            id: row.get(0),
            content: row.get(1),
            user_id: row.get(2),
            time_created: row.get::<usize, chrono::NaiveDateTime>(3),
            time_updated: row.get::<usize, chrono::NaiveDateTime>(4),
        }
    }
}

impl From<&Row> for Post {
    fn from(row: &Row) -> Self {
        Self {
            id: row.get(0),
            content: row.get(1),
            user_id: row.get(2),
            time_created: row.get::<usize, chrono::NaiveDateTime>(3),
            time_updated: row.get::<usize, chrono::NaiveDateTime>(4),
        }
    }
}

#[derive(Debug)]
pub struct PostDataError {
    details: String
}

impl PostDataError {
    fn new(msg: &str) ->PostDataError {
        PostDataError{details: msg.to_string()}
    }
}

impl fmt::Display for PostDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.details)
    }
}

impl Error for PostDataError {
    fn description(&self) -> &str {
        &self.details
    }
}

#[derive(Serialize, Deserialize)]
enum PostEvent {
    CREATED,
    UPDATED,
    DELETED,
}

#[derive(Serialize, Deserialize)]
struct PostEventMessage {
    event: PostEvent,
    post_id: Uuid,
    post: Post,
}

impl Post {
    pub fn new(id: Option<&Uuid>, content: &String, user_id: &Uuid) -> Result<Post, PostDataError> {
        Ok(Post {
            id: match id {
                Some(uuid) => uuid.to_owned(),
                None => Uuid::new_v4()
            },
            content: content.to_string(),
            user_id: user_id.to_owned(),
            time_created: chrono::Utc::now().naive_utc(),
            time_updated: chrono::Utc::now().naive_utc(),
        })
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }

    pub fn set_content(&mut self, content: &String) {
        self.content = content.to_string();
    }

    pub fn get_user_id(&self) -> Uuid {
        self.user_id
    }

    pub async fn get_feed<C: GenericClient>(
        pg_client: &C,
        redis_connection: &mut Connection,
        user_id: &Uuid,
        offset: &usize,
        limit: &usize
    ) -> Result<Vec<Post>, PostgresError> {
        let feed_offset = if self::FEED_LENGTH < (*offset).try_into().unwrap() {
            self::FEED_LENGTH as usize
        } else {
            offset.to_owned()
        };
        let feed_limit = if self::FEED_LENGTH < (*limit).try_into().unwrap() {
            self::FEED_LENGTH as usize
        } else {
            limit.to_owned()
        };
        log::debug!("Feed offset: {}, feed limit: {}", feed_offset, feed_limit);

        let mut is_cache_available = true;
        let cache_key = self::FEED_CACHE_KEY_PREFIX.to_string() + user_id.to_string().as_str();
        let has_cached_result = redis::exists(cache_key.as_str(), redis_connection).await
            .unwrap_or_else(|_| { is_cache_available = false; is_cache_available });

        log::debug!("Cache key: '{}'", cache_key);
        log::debug!("Has cached result: {}", has_cached_result);
        log::debug!("Is cache available: {}", is_cache_available);

        if !has_cached_result {
            log::debug!("Cache miss for user_id: '{}'. Cache key: '{}'", user_id, cache_key);
            let stmt = pg_client.prepare(
                "SELECT * FROM posts WHERE user_id IN (SELECT friend_id FROM friends WHERE user_id=$1) ORDER BY time_updated DESC LIMIT $2"
            ).await?;

            let rows = pg_client.query(
                &stmt,
                &[user_id, &self::FEED_LENGTH]
            ).await?;

            let mut posts: Vec<Post> = rows
                .iter()
                .map(Post::from)
                .collect::<Vec<Post>>();
            if true == *FEED_ONE_POST_PER_USER {
                posts.dedup_by(|post1, post2| post1.user_id == post2.user_id);
            }

            if is_cache_available {
                for post in posts.iter() {
                    redis::r_push(
                        &cache_key,
                        serde_json::to_value(post).unwrap().to_string().as_str(),
                        redis_connection
                    ).await
                    .unwrap();
                }
            }

            let posts_len = posts.len();

            if posts_len < feed_offset {
                return Ok(Vec::new());
            }

            Ok(posts[feed_offset..cmp::min(feed_offset+feed_limit, posts_len)].to_vec())
        } else {
            log::debug!("Cache hit for user_id: '{}'. Cache key: {}", user_id, cache_key);
            let posts: Vec<Post> = redis::l_range(
                cache_key.as_str(),
                &feed_offset,
                &(feed_offset+feed_limit-1),
                redis_connection
            ).await
            .unwrap()
            .iter()
            .map(|post| serde_json::from_str(post).unwrap())
            .collect();

            Ok(posts)
        }
    }

    // pub async fn get_feed<C: GenericClient>(
    //     pg_client: &C,
    //     redis_connection: &mut Connection,
    //     user_id: &Uuid,
    //     offset: &i64,
    //     limit: &i64
    // ) -> Result<Vec<Post>, PostgresError> {
    //     let mut is_cache_available = true;
    //     let cache_hash_key = redis::FEED_HASH_PREFIX.to_string() + user_id.to_string().as_str();
    //     let cache_field_key = offset.to_string() + ":" + limit.to_string().as_str();
    //     let has_cached_result = redis::h_exists(
    //         cache_hash_key.as_str(),
    //         cache_field_key.as_str(),
    //         redis_connection).await.unwrap_or_else(|_| { is_cache_available = false; is_cache_available }
    //     );

    //     if !has_cached_result {
    //         log::debug!("Cache miss for user_id: '{}'. Cache hash key: {}", user_id, cache_hash_key);
    //         let stmt = pg_client.prepare(
    //             "SELECT * FROM posts WHERE user_id IN (SELECT friend_id FROM friends WHERE user_id=$1) ORDER BY id DESC OFFSET $2 LIMIT $3"
    //         ).await?;

    //         let rows = pg_client.query(
    //             &stmt,
    //             &[user_id, &offset, &limit]
    //         ).await?;

    //         let posts: Vec<Post> = rows.into_iter().map(Post::from).collect();
    //         if is_cache_available {
    //             redis::h_set(
    //                 cache_hash_key.as_str(),
    //                 cache_field_key.as_str(),
    //                 serde_json::to_value(&posts).unwrap().to_string().as_str(),
    //                 redis_connection).await.unwrap();
    //         }
    //         return Ok(posts);
    //     } else {
    //         log::debug!("Cache hit for user_id: '{}'. Cache hash key: {}", user_id, cache_hash_key);
    //         let posts: Vec<Post> = serde_json::from_str(redis::h_get(cache_hash_key.as_str(), cache_field_key.as_str(), redis_connection).await.unwrap().as_str()).unwrap();
    //         return Ok(posts);
    //     }
    // }

    pub async fn get_by_user_id<C: GenericClient>(client: &C, user_id: &Uuid) -> Result<Vec<Post>, PostgresError> {
        let stmt = client.prepare(
            "SELECT * FROM posts WHERE user_id=$1"
        ).await?;

        let rows = client.query(
            &stmt,
            &[user_id]
        ).await?;

        Ok(rows.into_iter().map(Post::from).collect())
    }

    pub async fn get_by_id<C: GenericClient>(client: &C, id: &Uuid) -> Result<Post, PostgresError> {
        let stmt = client.prepare(
            "SELECT * FROM posts WHERE id=$1"
        ).await?;

        let row = client.query_one(
            &stmt,
            &[id]
        ).await?;

        Ok(row.try_into().unwrap())
    }

    pub async fn create<C: GenericClient>(client: &C, post: &Post) -> Result<Uuid, PostgresError> {
        let stmt = client.prepare(
            "INSERT INTO posts (content, user_id) VALUES ($1, $2) RETURNING id"
        ).await?;

        let rows = client.query(
            &stmt,
            &[&post.content, &post.user_id]
        ).await?;

        let post_id = rows.iter().next().unwrap().get(0);

        self::publish_message(
            post.user_id.to_string().as_str(),
            serde_json::to_value(PostEventMessage {
                event: PostEvent::CREATED,
                post_id,
                post: post.clone(),
            }).unwrap().to_string().as_str(),
        ).await;

        Ok(post_id)
    }

    pub async fn update<C: GenericClient>(client: &C, post: &Post) -> Result<bool, PostgresError> {
        let stmt = client.prepare(
            "UPDATE posts SET content=$2, user_id=$3, time_updated=$4 WHERE id=$1"
        ).await?;

        let rows_count = client.execute(
            &stmt,
            &[&post.id, &post.content, &post.user_id, &chrono::Utc::now().naive_utc()]
        ).await?;

        self::publish_message(
            post.user_id.to_string().as_str(),
            serde_json::to_value(PostEventMessage {
                event: PostEvent::UPDATED,
                post_id: post.id,
                post: post.clone(),
            }).unwrap().to_string().as_str(),
        ).await;

        Ok(0 < rows_count)
    }

    pub async fn delete<C: GenericClient>(client: &C, post: &Post) -> Result<Uuid, PostgresError> {
        let stmt = client.prepare(
            "DELETE FROM posts WHERE id=$1"
        ).await?;

        client.execute(
            &stmt,
            &[&post.id]
        ).await?;

        self::publish_message(
            post.user_id.to_string().as_str(),
            serde_json::to_value(PostEventMessage {
                event: PostEvent::DELETED,
                post_id: post.id,
                post: post.clone(),
            }).unwrap().to_string().as_str(),
        ).await;

        Ok(post.id)
    }

    pub async fn cache_invalidate_by_friend_user_id(redis_connection: &mut Connection, user_id: &Uuid) -> Result<(), PostgresError> {
        let friends: Vec<friend::Friend> = friend::Friend::get_by_user_id(user_id).await.unwrap();
        for friend in friends.iter() {
            let cache_hash_key = self::FEED_CACHE_KEY_PREFIX.to_string() + friend.get_friend_id().to_string().as_str();
            redis::del(
                cache_hash_key.as_str(),
                redis_connection
            ).await.unwrap();
        }
        let cache_hash_key = self::FEED_CACHE_KEY_PREFIX.to_string() + user_id.to_string().as_str();
        redis::del(
            cache_hash_key.as_str(),
            redis_connection
        ).await.unwrap();
        Ok(())
    }
}

#[derive(Debug)]
pub struct FeedConsumer {
    no_ack: bool,
}

impl FeedConsumer {
    pub fn new(no_ack: bool) -> Self {
        Self {
            no_ack
        }
    }
}

#[async_trait]
impl AsyncConsumer for FeedConsumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        log::info!(
            "FeedConsumer: consume delivery {} on channel {}, content size: {}",
            deliver,
            channel,
            content.len(),
        );

        let pg_pool = postgres::get_replica_pool_ref();
        let pg_client = match pg_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::debug!("unable to get postgres client: {:?}", err);
                return;
            }
        };
    
        let redis_pool = redis::get_pool_ref();
        let mut redis_connection = match redis_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::debug!("unable to get redis client: {:?}", err);
                return;
            }
        };

        // Process message
        let post_event_message: PostEventMessage = serde_json::from_str(String::from_utf8(content).unwrap().as_str()).unwrap();
        match post_event_message.event {
            PostEvent::CREATED => {
                // let post = Post::get_by_id(&**pg_client, &post_event_message.post_id).await.unwrap();
                let post: Post = post_event_message.post;
                log::debug!("Adding post to cache: {:?}", post);
                let friends = friend::Friend::get_by_friend_id(&post.user_id).await.unwrap();
                for friend in friends.iter() {
                    let cache_key = self::FEED_CACHE_KEY_PREFIX.to_string() + friend.get_user_id().to_string().as_str();
                    log::debug!("Updating cache key: '{}'", cache_key);
                    if true == *FEED_ONE_POST_PER_USER {
                        let cache_key = self::FEED_CACHE_KEY_PREFIX.to_string() + friend.get_user_id().to_string().as_str();
                        log::debug!("Deleting cache key: '{}'", cache_key);
                        redis::del(&cache_key, &mut redis_connection).await.unwrap();
                    } else {
                        redis::l_push(&cache_key, serde_json::to_value(&post).unwrap().to_string().as_str(), &mut redis_connection).await.unwrap();
                        redis::l_trim(&cache_key, &(0 as usize), &(self::FEED_LENGTH as usize), &mut redis_connection).await.unwrap();
                    }
                }
            },
            PostEvent::UPDATED => {
                let post: Post = post_event_message.post;
                log::debug!("Updating post to cache: {:?}", post);
                let friends = friend::Friend::get_by_friend_id(&post.user_id).await.unwrap();
                for friend in friends.iter() {
                    let cache_key = self::FEED_CACHE_KEY_PREFIX.to_string() + friend.get_user_id().to_string().as_str();
                    log::debug!("Deleting cache key: '{}'", cache_key);
                    redis::del(&cache_key, &mut redis_connection).await.unwrap();
                }
            },
            PostEvent::DELETED => {
                let post: Post = post_event_message.post;
                log::debug!("Removing post to cache: {:?}", post);
                let friends = friend::Friend::get_by_friend_id(&post.user_id).await.unwrap();
                for friend in friends.iter() {
                    let cache_key = self::FEED_CACHE_KEY_PREFIX.to_string() + friend.get_user_id().to_string().as_str();
                    log::debug!("Deleting cache key: '{}'", cache_key);
                    redis::del(&cache_key, &mut redis_connection).await.unwrap();
                }
            }
        }

        // ack explicitly if manual ack
        if !self.no_ack {
            log::info!("FeedConsumer: ack to delivery {} on channel {}", deliver, channel);
            let args = BasicAckArguments::new(deliver.delivery_tag(), false);
            channel.basic_ack(args).await.unwrap();
        }
    }
}

pub async fn create_pub_sub() {
    rabbitmq::create_queue(self::FEED_QUEUE_NAME).await;

    rabbitmq::bind_queue(
        self::FEED_QUEUE_EXCHANGE_NAME,
        self::FEED_QUEUE_NAME,
        (self::FEED_QUEUE_ROUTING_KEY_PREFIX.to_owned() + "all").as_str()
    ).await;

    {
        let args = BasicConsumeArguments::new(FEED_QUEUE_NAME, FEED_QUEUE_CONSUMER_TAG);
        rabbitmq::add_consumer(
            rabbitmq::get_channel_ref().await.lock().await.first().unwrap(),
            FeedConsumer::new(args.no_ack),
            args
        ).await;
    }
}

pub async fn publish_message(entity_id: &str, message: &str) {
    {
        rabbitmq::publish_message(
            rabbitmq::get_channel_ref().await.lock().await.first().unwrap(),
            message,
            self::FEED_QUEUE_EXCHANGE_NAME,
            (self::FEED_QUEUE_ROUTING_KEY_PREFIX.to_owned() + "all").as_str()
        ).await;
    }

    {
        let user_id = Uuid::parse_str(entity_id).unwrap();
        let users = friend::Friend::get_by_friend_id(&user_id).await.unwrap();
        for user in users.iter() {
            rabbitmq::publish_message(
                rabbitmq::get_channel_ref().await.lock().await.first().unwrap(),
                message,
                websocket::FEED_WS_QUEUE_EXCHANGE_NAME,
                (self::FEED_QUEUE_ROUTING_KEY_PREFIX.to_owned() + user.get_user_id().to_string().as_str()).as_str()
            ).await;
        }
    }
}
