use deadpool_postgres::Pool;
use futures::io;
use tokio_postgres::Row;
use tonic::async_trait;
use uuid::Uuid;

use crate::{friend::Friend, friend_storage::FriendStorage};

impl From<Row> for Friend {
    fn from(row: Row) -> Friend {
        Friend::new(row.get(0), row.get(1), row.get(2))
    }
}

pub struct PostgresFriendStorage {
    master_pool: &'static Pool,
    replica_pool: &'static Pool,
}

impl PostgresFriendStorage {
    pub fn new(master_pool: &'static Pool, replica_pool: &'static Pool) -> Self {
        Self {
            master_pool,
            replica_pool,
        }
    }
}

#[async_trait]
impl FriendStorage for PostgresFriendStorage {
    async fn get_by_id(&self, id: &Uuid) -> Result<Friend, io::Error> {
        let client = self.replica_pool.get().await.unwrap();

        let stmt = client.prepare("SELECT * FROM friends WHERE id = $1").await.unwrap();

        let row = client.query_one(&stmt, &[&id]).await.unwrap();

        Ok(Friend::from(row))
    }


    async fn get_by_user_id(&self, user_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        let client = self.replica_pool.get().await.unwrap();

        let stmt = client.prepare("SELECT * FROM friends WHERE user_id = $1").await.unwrap();

        let rows = client.query(&stmt, &[user_id]).await.unwrap();

        let mut friends = Vec::new();
        for row in rows {
            friends.push(Friend::from(row));
        }

        Ok(friends)
    }

    async fn get_by_friend_id(&self, friend_id: &Uuid) -> Result<Vec<Friend>, io::Error> {
        let client = self.replica_pool.get().await.unwrap();

        let stmt = client.prepare("SELECT * FROM friends WHERE friend_id = $1").await.unwrap();

        let rows = client.query(&stmt, &[friend_id]).await.unwrap();

        let mut friends = Vec::new();
        for row in rows {
            friends.push(Friend::from(row));
        }

        Ok(friends)
    }

    async fn get_by_user_id_and_friend_id(&self, user_id: &Uuid, friend_id: &Uuid) -> Result<Friend, io::Error> {
        let client = self.replica_pool.get().await.unwrap();

        let stmt = client.prepare("SELECT * FROM friends WHERE user_id = $1 AND friend_id = $2").await.unwrap();

        let row = client.query_one(&stmt, &[user_id, friend_id]).await.unwrap();

        Ok(Friend::from(row))
    }

    async fn create(&self, friend: &Friend) -> Result<Uuid, io::Error> {
        let client = self.master_pool.get().await.unwrap();

        let stmt = client.prepare(
            "INSERT INTO friends (id, user_id, friend_id) VALUES ($1, $2, $3)"
        ).await.unwrap();

        client.execute(
            &stmt,
            &[&friend.get_id(), &friend.get_user_id(), &friend.get_friend_id()]
        ).await.unwrap();

        Ok(friend.get_id())
    }

    async fn delete(&self, friend: &Friend) -> Result<bool, io::Error> {
        let client = self.master_pool.get().await.unwrap();

        let stmt = client.prepare(
            "DELETE FROM friends WHERE user_id = $1 AND friend_id = $2"
        ).await.unwrap();

        client.execute(
            &stmt,
            &[&friend.get_user_id(), &friend.get_friend_id()]
        ).await.unwrap();

        Ok(true)
    }

    async fn is_persistant(&self, friend: &Friend) -> Result<bool, io::Error> {
        let client = self.replica_pool.get().await.unwrap();

        let stmt = client.prepare(
            "SELECT id FROM friends WHERE user_id = $1 AND friend_id = $2"
        ).await.unwrap();

        match client.query_one(
            &stmt,
            &[&friend.get_user_id(), &friend.get_friend_id()]
        ).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}