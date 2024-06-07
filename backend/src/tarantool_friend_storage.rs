use futures::io;
use rusty_tarantool::tarantool::{Client, ExecWithParamaters as _};
use tonic::async_trait;
use uuid::Uuid;

use crate::friend::Friend;
use crate::friend_storage::FriendStorage;
use crate::tarantool::TarantoolClientManager;

pub struct TarantoolFriendStorage {
    manager: TarantoolClientManager,
}

impl TarantoolFriendStorage {
    pub fn new(manager: TarantoolClientManager) -> Self {
        Self {
            manager,
        }
    }

    pub fn get_client(&self) -> &Client {
        self.manager.get_client()
    }

    pub async fn get_leader_client(&self) -> &Client {
        self.manager.get_leader_client().await
    }
}

#[async_trait]
impl FriendStorage for TarantoolFriendStorage {
    async fn get_by_id(&self, id: &Uuid) -> Result<Option<Friend>, io::Error> {
        if let Ok(friend_tuple) = self.get_client()
                .prepare_fn_call("friend_get_by_id")
                .bind(id.to_string().as_str()).unwrap()
                .execute().await.unwrap()
                .decode_single::<(String, String, String)>() {

            Ok(Some(
                Friend::new(
                    Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
                    Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
                    Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
                )
            ))
        } else {
            Ok(None)
        }
    }

    async fn get_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        let friend_tuple_vec: Vec<Vec<(String, String, String)>> = self.get_client()
            .prepare_fn_call("friend_get_by_user_id")
            .bind(user_id.to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_result_set().unwrap_or_else(|_| Vec::new());

        let friend_tuple_vec = friend_tuple_vec.into_iter().flatten().collect::<Vec<(String, String, String)>>();

        let friends = friend_tuple_vec.into_iter().map(|friend_tuple|  Friend::new(
            Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
            Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
            Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
        )).collect();

        Ok(friends)
    }

    async fn get_by_friend_id(&self, friend_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        let friend_tuple_vec: Vec<(String, String, String)> = self.get_client()
            .prepare_fn_call("friend_get_by_friend_id")
            .bind(friend_id.to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_result_set().unwrap();

        let friends = friend_tuple_vec.into_iter().map(|friend_tuple|  Friend::new(
            Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
            Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
            Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
        )).collect();

        Ok(friends)
    }

    async fn get_by_user_id_and_friend_id(&self, user_id: &Uuid, friend_id: &Uuid) -> Result<Option<Friend>, io::Error> {
        if let Ok(friend_tuple) = self.get_client()
                .prepare_fn_call("friend_get_by_user_id_and_friend_id")
                .bind(user_id.to_string().as_str()).unwrap()
                .bind(friend_id.to_string().as_str()).unwrap()
                .execute().await.unwrap()
                .decode_single::<(String, String, String)>() {

            Ok(Some(
                Friend::new(
                    Some(Uuid::parse_str(String::from(friend_tuple.0).as_str()).unwrap()),
                    Uuid::parse_str(String::from(friend_tuple.1).as_str()).unwrap(),
                    Uuid::parse_str(String::from(friend_tuple.2).as_str()).unwrap(),
                )
            ))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, friend: &Friend) -> Result<Uuid, io::Error> {
        let friend_tuple: (String, String, String) = self.get_leader_client().await
            .prepare_fn_call("friend_create")
            .bind(friend.get_id().to_string().as_str()).unwrap()
            .bind(friend.get_user_id().to_string().as_str()).unwrap()
            .bind(friend.get_friend_id().to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_single().unwrap();

        Ok(uuid::Uuid::parse_str(friend_tuple.0.as_str()).unwrap())
    }

    async fn delete(&self, friend: &Friend) -> Result<bool, io::Error> {
        self.get_leader_client().await
            .prepare_fn_call("friend_delete")
            .bind(friend.get_id().to_string().as_str()).unwrap()
            .execute().await.unwrap();

        Ok(true)
    }

    async fn is_persistant(&self, friend: &Friend) -> Result<bool, io::Error> {
        let is_persistant: bool = self.get_client()
            .prepare_fn_call("friend_is_persistant")
            .bind(friend.get_id().to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_single().unwrap();

        Ok(is_persistant)
    }
}
