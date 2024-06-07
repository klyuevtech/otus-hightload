use std::fmt;
use std::error;
use futures::io;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::OnceCell;
use uuid::Uuid;
use deadpool_redis::Connection;
use crate::post;

use crate::friend_storage::FriendStorage;

static STORAGE: OnceCell<Box<dyn FriendStorage + Send + Sync>> = OnceCell::const_new();

pub async fn init_storage(storage: Box<dyn FriendStorage + Send + Sync>) {
    if !STORAGE.initialized() {
        STORAGE.get_or_init(|| async {storage}).await;
    }
}

fn get_storage() -> &'static Box<dyn FriendStorage + Send + Sync> {
    STORAGE.get().expect("Storage must be initialized first")
}

#[derive(Debug)]
pub struct FriendDataError {
    details: String
}

impl FriendDataError {
    fn new(msg: &str) ->FriendDataError {
        FriendDataError{details: msg.to_string()}
    }
}

impl fmt::Display for FriendDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.details)
    }
}

impl error::Error for FriendDataError {
    fn description(&self) -> &str {
        &self.details
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Friend {
    id: Uuid,
    user_id: Uuid,
    friend_id: Uuid,
}

impl Friend {
    pub fn new(id: Option<Uuid>, user_id: Uuid, friend_id: Uuid) -> Friend {
        Friend {
            id: match id { Some(uuid) => uuid, None => Uuid::new_v4()},
            user_id,
            friend_id,
        }
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn get_user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn get_friend_id(&self) -> Uuid {
        self.friend_id
    }

    pub async fn get_by_id(id: &Uuid) -> Result<Option<Friend>, io::Error> {
        get_storage().get_by_id(id).await
    }

    pub async fn get_by_user_id(user_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        get_storage().get_by_user_id(user_id).await
    }

    pub async fn get_by_friend_id(friend_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        get_storage().get_by_friend_id(friend_id).await
    }

    pub async fn get_by_user_id_and_friend_id(user_id: &Uuid, friend_id: &Uuid) -> Result<Option<Friend>, io::Error> {
        get_storage().get_by_user_id_and_friend_id(user_id, friend_id).await
    }

    pub async fn create(friend: &Friend, redis_connection: &mut Connection) -> Result<Uuid, io::Error> {
        post::Post::cache_invalidate_by_friend_user_id(redis_connection, &friend.user_id).await.unwrap();

        get_storage().create(friend).await
    }

    pub async fn delete(friend: &Friend, redis_connection: &mut Connection) -> Result<bool, io::Error> {
        post::Post::cache_invalidate_by_friend_user_id(redis_connection, &friend.user_id).await.unwrap();

        get_storage().delete(friend).await
    }

    pub async fn is_persistant(friend: &Friend) -> Result<bool, io::Error> {
        get_storage().is_persistant(friend).await
    }
}