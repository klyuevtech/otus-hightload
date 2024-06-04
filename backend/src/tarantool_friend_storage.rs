use futures::io;
use rusty_tarantool::tarantool::{Client, ClientConfig, ExecWithParamaters as _};
use tonic::async_trait;
use uuid::Uuid;

use crate::friend::Friend;
use crate::friend_storage::FriendStorage;

pub struct TarantoolClientConfig {
    addr: String,
    login: String,
    password: String,
}

impl TarantoolClientConfig {
    pub fn new(addr: String, login: String, password: String) -> Self {
        Self { addr, login, password }
    }
}

pub struct TarantoolFriendStorage {
    client: Client,
}

impl TarantoolFriendStorage {
    pub fn new(client_config: TarantoolClientConfig) -> Self {
        Self {
            client: ClientConfig::new(client_config.addr, client_config.login, client_config.password)
                .set_timeout_time_ms(2000)
                .set_reconnect_time_ms(2000)
                .build()
        }
    }
}

#[async_trait]
impl FriendStorage for TarantoolFriendStorage {
    async fn get_by_id(&self, id: &Uuid) -> Result<Friend, io::Error> {
        let friend_tuple: (String, String, String) = self.client
            .prepare_fn_call("friend_get_by_id")
            .bind(id).unwrap()
            .execute().await.unwrap()
            .decode_single().expect("Friend not found");

        Ok(
            Friend::new(
                Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
                Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
                Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
            )
        )
    }

    async fn get_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        let friend_tuple_vec: Vec<(String, String, String)> = self.client
            .prepare_fn_call("friend_get_by_user_id")
            .bind(user_id).unwrap()
            .execute().await.unwrap()
            .decode_result_set().unwrap();

        let friends = friend_tuple_vec.into_iter().map(|friend_tuple|  Friend::new(
            Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
            Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
            Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
        )).collect();

        Ok(friends)
    }

    async fn get_by_friend_id(&self, friend_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        let friend_tuple_vec: Vec<(String, String, String)> = self.client
            .prepare_fn_call("friend_get_by_friend_id")
            .bind(friend_id).unwrap()
            .execute().await.unwrap()
            .decode_result_set().unwrap();

        let friends = friend_tuple_vec.into_iter().map(|friend_tuple|  Friend::new(
            Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
            Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
            Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
        )).collect();

        Ok(friends)
    }

    async fn get_by_user_id_and_friend_id(&self, user_id: &Uuid, friend_id: &Uuid) -> Result<Friend, io::Error> {
        let friend_tuple: (String, String, String) = self.client
            .prepare_fn_call("friend_get_by_user_id_and_friend_id")
            .bind(user_id).unwrap()
            .bind(friend_id).unwrap()
            .execute().await.unwrap()
            .decode_single().expect("Friend not found");

        Ok(
            Friend::new(
                Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
                Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
                Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
            )
        )
    }

    async fn create(&self, friend: &Friend) -> Result<Uuid, io::Error> {
        let friend_tuple: (String, String, String) = self.client
            .prepare_fn_call("friend_create")
            .bind(friend.get_id().to_string().as_str()).unwrap()
            .bind(friend.get_user_id().to_string().as_str()).unwrap()
            .bind(friend.get_friend_id().to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_single().unwrap();

        Ok(uuid::Uuid::parse_str(friend_tuple.0.as_str()).unwrap())
    }

    async fn delete(&self, friend: &Friend) -> Result<bool, io::Error> {
        self.client
            .prepare_fn_call("friend_delete")
            .bind(friend.get_id().to_string().as_str()).unwrap()
            .bind(friend.get_user_id().to_string().as_str()).unwrap()
            .bind(friend.get_friend_id().to_string().as_str()).unwrap()
            .execute().await.unwrap();

        Ok(true)
    }

    async fn is_persistant(&self, friend: &Friend) -> Result<bool, io::Error> {
        let is_persistant: String = self.client
            .prepare_fn_call("friend_get_by_id")
            .bind(friend.get_id()).unwrap()
            .execute().await.unwrap()
            .decode().expect("Friend not found");

        Ok("true" == is_persistant)
    }
}
