use std::io::Error;
use tonic::async_trait;
use uuid::Uuid;

use crate::friend::Friend;

#[async_trait]
pub trait FriendStorage {
    async fn get_by_id(&self, id: &Uuid) -> Result<Option<Friend>, Error>;
    async fn get_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Friend>, Error>;
    async fn get_by_friend_id(&self, friend_id: &Uuid) -> Result<Vec<Friend>, Error>;
    async fn get_by_user_id_and_friend_id(&self, user_id: &Uuid, friend_id: &Uuid) -> Result<Option<Friend>, Error>;
    async fn create(&self, friend: &Friend) -> Result<Uuid, Error>;
    async fn delete(&self, friend: &Friend) -> Result<bool, Error>;
    async fn is_persistant(&self, friend: &Friend) -> Result<bool, Error>;
}
