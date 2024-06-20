use std::str::FromStr;

use actix_web::http::header::ContentType;
use actix_web::{error,
    // http,
    middleware, web, App, Error, HttpResponse, HttpRequest, HttpServer
};
use deadpool_postgres::Pool as PostgresPool;
use deadpool_redis::Pool as RedisPool;
use futures::{future, StreamExt};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use postgres_friend_storage::PostgresFriendStorage;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tarantool::{TarantoolClientConfig, TarantoolClientManager};
use tarantool_friend_storage::TarantoolFriendStorage;
use tarantool_session_storage::TarantoolSessionStorage;
use tonic::IntoRequest;
use user_search::user_search_server::{
    UserSearch,
    // UserSearchServer
};
use user_search::{UserSearchResponse, UserSearchRequest};
// use tonic::codec::CompressionEncoding;
use actix_web_httpauth::extractors::bearer::{self, BearerAuth};
// use amqprs::channel::Channel;
use reqwest;

mod user_search;
mod postgres;
mod tarantool;
mod user;
mod session;
mod friend;
mod post;
mod redis;
mod rabbitmq;
mod websocket;
mod session_storage;
mod postgres_session_storage;
mod tarantool_session_storage;
mod friend_storage;
mod postgres_friend_storage;
mod tarantool_friend_storage;

const MAX_SIZE: usize = 262_144; // max payload size is 256k

async fn list_users(pool: web::Data<&'static PostgresPool>) -> HttpResponse {
    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to get postgres client");
        }
    };
    match user::User::get_all(&**client).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(err) => {
            log::debug!("unable to fetch users: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to fetch users");
        }
    }
}

async fn get_user(pool: web::Data<&'static PostgresPool>, path: web::Path<String>) -> HttpResponse {
    let id = path.parse::<String>().unwrap();
    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to get postgres client");
        }
    };
    match user::User::get_by_id(&**client, &id).await {
        Ok(user) => HttpResponse::Ok().json(user),
        Err(err) => {
            log::debug!("unable to fetch users: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to fetch users");
        }
    }
}

#[derive(Deserialize)]
struct UserSearchRequestQuery {
    first_name: String,
    last_name: String,
}

async fn search_user(pool: web::Data<&'static PostgresPool>, search: web::Query<UserSearchRequestQuery>) -> HttpResponse {
    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to get postgres client");
        }
    };
    let use_fast_search = pool.status().available < 0;
    match user::User::search_by_first_name_and_last_name(&**client, &search.first_name, &search.last_name, use_fast_search).await {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(err) => {
            log::debug!("unable to find users: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to find users");
        }
    }
}

async fn register_user(pool: web::Data<&'static PostgresPool>, mut payload: web::Payload) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct UserRegisterPayload {
        first_name: String,
        second_name: String,
        birthdate: String,
        biography: String,
        city: String,
        password: String,
    }

    let user_data = match serde_json::from_slice::<UserRegisterPayload>(&body) {
        Ok(user_data) => user_data,
        Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }
    };

    match user::User::is_password_correct(&user_data.password) {
        Ok(result) => result,
        Err(err) => {
            log::debug!("unable to register user: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("password format is incorrect: ".to_owned() + &err.to_string()));
        }
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    let user = match user::User::new(
        &user_data.first_name,
        &user_data.second_name,
        &user_data.birthdate,
        &user_data.biography,
        &user_data.city
    ) {
        Ok(user) => user,
        Err(e) => {
            log::debug!("user data is incorrect: {:?}", e);
            return Ok(HttpResponse::InternalServerError().json("user data is incorrect: ".to_owned() + &e.to_string()));
        }
    };

    match user::User::create(
        &**client,
        &user,
        &user_data.password
    ).await {
        Ok(token) => {
            #[derive(Debug, Serialize)]
            struct UserRegisterResponse {
                user_id: String,
            }
            Ok(HttpResponse::Ok().json(UserRegisterResponse{user_id: token.to_string()}))
        },
        Err(err) => {
            log::debug!("unable to register user: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to register user"));
        }
    }
}

async fn login(pool: web::Data<&'static PostgresPool>, mut payload: web::Payload) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct LoginPayload {
        id: String,
        password: String,
    }

    let login_data = match serde_json::from_slice::<LoginPayload>(&body) {
        Ok(user_data) => user_data,
        Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    if true != user::User::authenticate(&**client, &login_data.id, &login_data.password).await {
        log::debug!("unable to authenticate user");
        return Ok(HttpResponse::InternalServerError().json("unable to authenticate user"));
    }

    match session::Session::create(
        &session::Session::new(
            uuid::Uuid::new_v4().to_string(),
            login_data.id,
            serde_json::json!({}).to_string(),
        )
    ).await {
        Ok(session_id) => {
            #[derive(Debug, Serialize)]
            struct UserLoginResponse {
                token: String,
            }
            Ok(HttpResponse::Ok().json(UserLoginResponse{token: session_id.to_string()}))
        },
        Err(err) => {
            log::debug!("unable to register user: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to register user"));
        }
    }
}

struct UserSearchService {
    pg_pool: &'static PostgresPool,
}

#[tonic::async_trait]
impl UserSearch for UserSearchService {
    async fn search(
        &self,
        request: tonic::Request<UserSearchRequest>,
    ) -> std::result::Result<
        tonic::Response<UserSearchResponse>,
        tonic::Status,
    > {
        let client = match self.pg_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::debug!("unable to get postgres client: {:?}", err);
                return Err(tonic::Status::not_found("unable to get postgres client"));
            }
        };

        let req = request.into_inner().to_owned();

        let first_name = &req.first_name;
        let second_name = &req.last_name;

        match user::User::search_by_first_name_and_last_name(&**client, &first_name, &second_name, true).await {
            Ok(users) => {
                let user_data = users.into_iter().map(|user: user::User| user_search::User {
                    id: user.id().to_string(),
                    first_name: user.first_name().to_string(),
                    second_name: user.second_name().to_string(),
                    birthdate: user.birthdate().to_string(),
                    biography: user.biography().to_string(),
                    city: user.city().to_string(),
                }).collect();
                Ok(tonic::Response::new(UserSearchResponse{ data: user_data, page_number: 1, results_per_page: 50, }))
            },
            Err(err) => {
                log::debug!("unable to find users: {:?}", err);
                return Err(tonic::Status::not_found(format!("Couldn't find users by query: {}, {}", &first_name, &second_name)));
            }
        }
    }
}

async fn friend_set(path: web::Path<String>, auth: BearerAuth, redis_pool: web::Data<&'static RedisPool>) -> Result<HttpResponse, Error> {
    let friend_user_id = match uuid::Uuid::from_str(&path.parse::<String>().unwrap()) {
        Ok(val) => val,
        Err(_err) => return Ok(HttpResponse::InternalServerError().json("internal error")),
    };

    let mut redis_connection = match redis_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get redis client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get redis client"));
        }
    };

    if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        let user_id = session.get_user_id();
        let friend = friend::Friend::new(None, user_id, friend_user_id);

        let is_persistant = match friend::Friend::is_persistant(&friend).await {
            Ok(res) => res,
            Err(err) => {
                log::debug!("unable to add friend: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to add friend"));
            }
        };
    
        if is_persistant {
            log::debug!("unable to add friend: record already exists");
            return Ok(HttpResponse::InternalServerError().json("unable to add friend"));   
        }
        
        match friend::Friend::create(&friend, &mut redis_connection).await {
            Ok(_res) => Ok(HttpResponse::Ok().json("ok")),
            Err(err) => {
                log::debug!("unable to add friend: {:?}", err);
                Ok(HttpResponse::InternalServerError().json("unable to add friend"))
            }
        }
    } else {
        log::debug!("unable to add friend: unauthorized");
        return Ok(HttpResponse::InternalServerError().json("unable to add friend"));
    }
}

async fn friend_search(pg_pool: web::Data<&'static PostgresPool>, auth: BearerAuth) -> Result<HttpResponse, Error> {
    if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        let user_id = session.get_user_id();
        let friends = match friend::Friend::get_by_user_id(&user_id).await {
            Ok(friends) => friends,
            Err(err) => {
                log::debug!("unable to search friend: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to search friends"));
            }
        };

        Ok(HttpResponse::Ok().json(friends))

        // let pg_client = match pg_pool.get().await {
        //     Ok(client) => client,
        //     Err(err) => {
        //         log::debug!("unable to get postgres client: {:?}", err);
        //         return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        //     }
        // };
    
        // let users = user::User::search_by_ids(
        //     &**pg_client,
        //     &friends.into_iter().map(|friend| friend.get_friend_id().to_string()).collect(),
        // ).await.unwrap();

        // Ok(HttpResponse::Ok().json(users))
    } else {
        log::debug!("unable to search friend: unauthorized");
        return Ok(HttpResponse::InternalServerError().json("unable to search friends"));
    }
}

async fn friend_delete(path: web::Path<String>, mut payload: web::Payload, redis_pool: web::Data<&'static RedisPool>) -> Result<HttpResponse, Error> {
    let user_id = match uuid::Uuid::from_str(&path.parse::<String>().unwrap()) {
        Ok(val) => val,
        Err(_err) => return Ok(HttpResponse::InternalServerError().json("internal error")),
    };
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct FriendDeletePayload {
        user_id: String,
    }

    let friend_user_id = match serde_json::from_slice::<FriendDeletePayload>(&body) {
        Ok(friend_data) => match uuid::Uuid::from_str(&friend_data.user_id) { Ok(user_id) => user_id, Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }},
        Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }
    };

    let mut redis_connection = match redis_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get redis client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get redis client"));
        }
    };

    let friend_option = match friend::Friend::get_by_user_id_and_friend_id(&user_id, &friend_user_id).await {
        Ok(friend_option) => friend_option,
        Err(err) => {
            log::debug!("unable to delete friend: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to delete friend"));
        }
    };

    match friend_option {
        Some(friend) => 
            match friend::Friend::delete(&friend, &mut redis_connection).await {
                Ok(_res) => Ok(HttpResponse::Ok().json("ok")),
                Err(err) => {
                    log::debug!("unable to add friend: {:?}", err);
                    Ok(HttpResponse::InternalServerError().json("unable to delete friend"))
                }
            },
        None => {
            log::debug!("unable to delete friend: record not found");
            return Ok(HttpResponse::InternalServerError().json("unable to delete friend: record not found"));
        }
    }
}

#[derive(Deserialize)]
struct PostFeedRequestQuery {
    offset: usize,
    limit: usize,
}

async fn post_feed(
    pg_pool: web::Data<&'static PostgresPool>,
    search: web::Query<PostFeedRequestQuery>,
    auth: BearerAuth,
    redis_pool: web::Data<&'static RedisPool>,
) -> Result<HttpResponse, Error> {
    let pg_client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    let mut redis_connection = match redis_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get redis client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get redis client"));
        }
    };

    if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        let user_id = session.get_user_id();
        let feed = match post::Post::get_feed(&**pg_client, &mut redis_connection, &user_id, &search.offset, &search.limit).await {
            Ok(feed) => feed,
            Err(err) => {
                log::debug!("unable to get feed: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to get feed"));
            }
        };
    
        Ok(HttpResponse::Ok().json(feed))
    } else {
        log::debug!("unable to get user id: unauthorized");
        return Ok(HttpResponse::InternalServerError().json("unable to get user id"));
    }
}

async fn post_create(
    pg_pool: web::Data<&'static PostgresPool>,
    mut payload: web::Payload,
    auth: BearerAuth,
) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct PostCreatePayload {
        text: String,
    }

    let post_data = match serde_json::from_slice::<PostCreatePayload>(&body) {
        Ok(post_data) => post_data,
        Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }
    };

    let pg_client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        let user_id = session.get_user_id();
        let post = match post::Post::new(None, &post_data.text, &user_id) {
            Ok(post) => post,
            Err(err) => {
                log::debug!("unable to create post: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to create post"));
            }
        };
    
        match post::Post::create(&**pg_client, &post).await {
            Ok(_res) => Ok(HttpResponse::Ok().json("ok")),
            Err(err) => {
                log::debug!("unable to create post: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to create post"));
            }
        }
    } else {
        log::debug!("unable to create post: unauthorized");
        return Ok(HttpResponse::InternalServerError().json("unable to create post"));
    }
}

async fn post_get(pg_pool: web::Data<&'static PostgresPool>, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let post_id = match uuid::Uuid::parse_str(&path) {
        Ok(val) => val,
        Err(_err) => return Ok(HttpResponse::InternalServerError().json("internal error")),
    };

    let pg_client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    let post = match post::Post::get_by_id(&**pg_client, &post_id).await {
        Ok(post) => post,
        Err(err) => {
            log::debug!("unable to get post: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get post"));
        }
    };

    Ok(HttpResponse::Ok().json(post))
}

async fn post_update(
    pg_pool: web::Data<&'static PostgresPool>,
    path: web::Path<String>,
    mut payload: web::Payload,
    auth: BearerAuth,
) -> Result<HttpResponse, Error> {
    let post_id = match uuid::Uuid::from_str(&path.parse::<String>().unwrap()) {
        Ok(val) => val,
        Err(_err) => return Ok(HttpResponse::InternalServerError().json("internal error")),
    };

    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct PostUpdatePayload {
        text: String,
    }

    let pg_client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    let post_data = match serde_json::from_slice::<PostUpdatePayload>(&body) {
        Ok(post_data) => post_data,
        Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }
    };

    if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        let user_id = session.get_user_id();
        let mut post = match post::Post::get_by_id(&**pg_client, &uuid::Uuid::from(post_id)).await {
            Ok(post) => post,
            Err(err) => {
                log::debug!("unable to update post: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to update post"));
            }
        };
    
        if user_id != post.get_user_id() {
            log::debug!("unable to update post: user is not owner");
            return Ok(HttpResponse::InternalServerError().json("unable to update post: user is not owner"));
        }
    
        post.set_content(&post_data.text);
    
        match post::Post::update(&**pg_client, &post).await {
            Ok(_res) => Ok(HttpResponse::Ok().json("ok")),
            Err(err) => {
                log::debug!("unable to update post: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to update post"));
            }
        }
    } else {
        log::debug!("unable to update post: unauthorized");
        return Ok(HttpResponse::InternalServerError().json("unable to update post"));
    }
}

async fn post_delete(
    pg_pool: web::Data<&'static PostgresPool>,
    path: web::Path<String>,
    auth: BearerAuth,
) -> Result<HttpResponse, Error> {
    let post_id = match uuid::Uuid::from_str(&path.parse::<String>().unwrap()) {
        Ok(val) => val,
        Err(_err) => return Ok(HttpResponse::InternalServerError().json("internal error")),
    };

    let pg_client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        let user_id = session.get_user_id();
        let post = match post::Post::get_by_id(&**pg_client, &post_id).await {
            Ok(post) => post,
            Err(err) => {
                log::debug!("unable to get post: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to get post"));
            }
        };
    
        if user_id != post.get_user_id() {
            log::debug!("unable to delete post: user is not owner");
            return Ok(HttpResponse::InternalServerError().json("unable to delete post: user is not owner"));
        }
    
        match post::Post::delete(&**pg_client, &post).await {
            Ok(_res) => Ok(HttpResponse::Ok().json("ok")),
            Err(err) => {
                log::debug!("unable to delete post: {:?}", err);
                return Ok(HttpResponse::InternalServerError().json("unable to delete post"));
            }
        }
    } else {
        log::debug!("unable to delete post: unauthorized");
        return Ok(HttpResponse::InternalServerError().json("unable to delete post"));
    }
}

fn log_request(req: &HttpRequest) {
    let unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let request_id = req.headers().get("x-request-id").unwrap_or_else(|| &unknown);
    log::debug!("[Request ID {:?}] {:?}", request_id, req);
}

async fn dialog_send(
    req: HttpRequest,
    path: web::Path<String>,
    mut payload: web::Payload,
    auth: BearerAuth,
) -> HttpResponse {
    log_request(&req);

    let message_sender_user_id: uuid::Uuid;
    if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        message_sender_user_id = session.get_user_id();
    } else {
        log::debug!("Unable to send dialog message: unauthorized");
        return HttpResponse::InternalServerError().json("Unable to send dialog message: unauthorized");
    }

    let message_receiver_user_id = match uuid::Uuid::parse_str(&path) {
        Ok(user_id2) => user_id2,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.unwrap();
        if (body.len() + chunk.len()) > MAX_SIZE {
            return HttpResponse::BadRequest().json("Payload data overflow");
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Deserialize)]
    struct DialogSendPayload {
        text: String,
    }

    let payload_data = match serde_json::from_slice::<DialogSendPayload>(&body) {
        Ok(payload_data) => payload_data,
        Err(err) => {
            log::debug!("Unable to parse json data: {:?}", err);
            return HttpResponse::BadRequest().json("Unable to parse payload json data");
        }
    };

    let dialog_service_url = std::env::var("DIALOGS_SERVICE_URL").unwrap_or_else(|_| String::from("localhost:8001"));
    let dialogs_body = serde_json::json!({
        "message_sender_user_id": message_sender_user_id,
        "message_receiver_user_id": message_receiver_user_id,
        "text": payload_data.text
    });
    let header_value_unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let request_id_str = req.headers().get("x-request-id").unwrap_or_else(|| &header_value_unknown).to_str().unwrap();
    let res = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build().unwrap()
        .post(dialog_service_url + "/send")
        .header("x-request-id", request_id_str)
        .json(&dialogs_body)
        .send().await.unwrap();

    let header_value_unknown = reqwest::header::HeaderValue::from_str("unknown").unwrap();
    
    let headers = res.headers().clone();
    let x_request_id = headers.get("x-request-id").unwrap_or_else(|| &header_value_unknown).to_str().unwrap();
    let x_server_instance = headers.get("x-server-instance").unwrap_or_else(|| &header_value_unknown).to_str().unwrap();

    let res_text = res.text().await.unwrap();
    HttpResponse::Ok()
        .content_type(ContentType::json())
        .insert_header(("x-request-id", x_request_id))
        .insert_header(("x-server-instance", x_server_instance))
        .body(res_text)
}

#[derive(Deserialize)]
struct DialogListRequestQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

async fn dialog_list(
    req: HttpRequest,
    path: web::Path<String>,
    search: web::Query<DialogListRequestQuery>,
    auth: BearerAuth,
) -> HttpResponse {
    log_request(&req);

    let user_id1: uuid::Uuid = if let Ok(Some(session)) = session::Session::get_by_id(auth.token()).await {
        session.get_user_id()
    } else {
        log::debug!("Unable to send dialog message: unauthorized");
        return HttpResponse::InternalServerError().json("Unable to send dialog message: unauthorized");
    };

    let user_id2 = match uuid::Uuid::parse_str(&path) {
        Ok(user_id2) => user_id2,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let dialog_service_url = std::env::var("DIALOGS_SERVICE_URL").unwrap_or_else(|_| String::from("localhost:8001"));
    let dialogs_body = serde_json::json!({
        "user_id1": user_id1,
        "user_id2": user_id2,
        "offset": search.offset.unwrap_or(0),
        "limit": search.limit.unwrap_or(50),
    });
    let header_value_unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let x_request_id = req.headers().get("x-request-id").unwrap_or_else(|| &header_value_unknown).to_str().unwrap();

    let res = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build().unwrap()
        .post(dialog_service_url + "/list")
        .header("x-request-id", x_request_id)
        .json(&dialogs_body)
        .send().await.unwrap();

    let header_value_unknown = reqwest::header::HeaderValue::from_str("unknown").unwrap();
    
    let headers = res.headers().clone();
    let x_request_id = headers.get("x-request-id").unwrap_or_else(|| &header_value_unknown).to_str().unwrap();
    let x_server_instance = headers.get("x-server-instance").unwrap_or_else(|| &header_value_unknown).to_str().unwrap();

    let res_text = res.text().await.unwrap();

    HttpResponse::Ok()
        .content_type(ContentType::json())
        .insert_header(("x-request-id", x_request_id))
        .insert_header(("x-server-instance", x_server_instance))
        .body(res_text.clone())
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    postgres::init_pools().await;
    redis::init_pool().await;
    rabbitmq::create_pub_sub().await;
    post::create_pub_sub().await;

    session::init_storage(Box::new(
        // postgres_session_storage::PostgresSessionStorage::new(
        //     postgres::get_master_pool_ref(),
        //     postgres::get_replica_pool_ref()
        // )
        TarantoolSessionStorage::new(tarantool::TarantoolClientManager::new().await)
    )).await;

    friend::init_storage(Box::new(
        // PostgresFriendStorage::new(
        //     postgres::get_master_pool_ref(),
        //     postgres::get_replica_pool_ref()
        // )
        TarantoolFriendStorage::new(tarantool::TarantoolClientManager::new().await)
    )).await;

    // let grpc_address = std::env::var("GRPC_SERVER_ADDRESS")
    //     .unwrap_or_else(|_| "127.0.0.1:9000".into())
    //     .parse::<std::net::SocketAddr>()
    //     .unwrap_or_else(|_| std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 9000));

    // let grpc_server = tonic::transport::Server::builder()
    //     .tls_config(tonic::transport::server::ServerTlsConfig::new().identity(
    //         tonic::transport::Identity::from_pem(&std::fs::read_to_string("cert.pem")?,
    //         &std::fs::read_to_string("key.pem")?))
    //     )
    //     .unwrap()
    //     .add_service(
    //         UserSearchServer::new(UserSearchService { pg_pool: postgres::get_replica_pool_ref() })
    //             .send_compressed(CompressionEncoding::Gzip)
    //             .accept_compressed(CompressionEncoding::Gzip)
    //     )
    //     .serve(grpc_address);

    // postgres::migrate_down(postgres::get_master_pool_ref()).await;
    postgres::migrate_up(
        postgres::get_master_pool_ref(),
        redis::get_pool_ref().get().await.unwrap(),
    ).await;

    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key_file("key.pem", SslFiletype::PEM)
        .unwrap();

    builder.set_certificate_chain_file("cert.pem").unwrap();

    let http_address = std::env::var("HTTP_SERVER_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8000".into());
    let http_server = HttpServer::new(|| {
        App::new()
            .wrap(middleware::Compress::default())
            .service(
                web::resource("/user")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(list_users))
            )
            .service(
                web::resource("/user/get/{id}")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(get_user))
            )
            .service(
                web::resource("/user/search")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(search_user))
            )
            .service(
                web::resource("/user/register")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .route(web::post().to(register_user))
            )
            .service(
                web::resource("/login")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .route(web::post().to(login))
            )
            .service(
                web::resource("/friend/set/{user_id}")
                    .app_data(bearer::Config::default().realm("Restricted area").scope("friend"))
                    .app_data(web::Data::new(redis::get_pool_ref()))
                    .route(web::put().to(friend_set))
            )
            .service(
                web::resource("/friend/search")
                    .app_data(bearer::Config::default().realm("Restricted area").scope("friend"))
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(friend_search))
            )
            .service(
                web::resource("/friend/delete/{user_id}")
                    .app_data(bearer::Config::default().realm("Restricted area").scope("friend"))
                    .app_data(web::Data::new(redis::get_pool_ref()))
                    .route(web::put().to(friend_delete))
            )
            .service(
                web::resource("/post/feed")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .app_data(web::Data::new(redis::get_pool_ref()))
                    .route(web::get().to(post_feed))
            )
            .service(
                web::resource("/post/create")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .route(web::post().to(post_create))
            )
            .service(
                web::resource("/post/get/{id}")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(post_get))
            )
            .service(
                web::resource("/post/update/{id}")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .route(web::put().to(post_update))
            )
            .service(
                web::resource("/post/delete/{id}")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .route(web::delete().to(post_delete))
            )
            .service(
                web::resource("/dialog/{user_id}/send")
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .route(web::post().to(dialog_send))
            )
            .service(
                web::resource("/dialog/{user_id}/list")
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .route(web::get().to(dialog_list))
            )
    })
    .bind_openssl(&http_address, builder)?
    .run();

    let _ = future::try_join(
        tokio::spawn(http_server),
        tokio::spawn(websocket::serve()),
    ).await?;

    Ok(())

}
