use std::fmt;
use std::error::Error;
use uuid::Uuid;
use tokio_postgres::{Error as PostgresError, GenericClient, Row};
use deadpool_redis::Connection;
use crate::post;

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

impl Error for FriendDataError {
    fn description(&self) -> &str {
        &self.details
    }
}

pub struct Friend {
    id: Uuid,
    user_id: Uuid,
    friend_id: Uuid,
}

impl From<Row> for Friend {
    fn from(row: Row) -> Self {
        Self {
            id: row.get(0),
            user_id: row.get(1),
            friend_id: row.get(2),
        }
    }
}

impl Friend {
    pub fn new(id: Option<Uuid>, user_id: Uuid, friend_id: Uuid) -> Result<Friend, FriendDataError> {
        Ok(Friend {
            id: match id { Some(uuid) => uuid, None => Uuid::new_v4()},
            user_id,
            friend_id,
        })
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

    pub async fn get_by_id<C: GenericClient>(client: &C, id: &Uuid) -> Result<Friend, PostgresError> {
        let stmt = client.prepare("SELECT * FROM friends WHERE id = $1").await?;

        let row = client.query_one(&stmt, &[id]).await?;

        Ok(Friend::from(row))
    }

    pub async fn get_by_user_id<C: GenericClient>(client: &C, user_id: &Uuid) -> Result<Vec<Friend>, PostgresError> {
        let stmt = client.prepare("SELECT * FROM friends WHERE user_id = $1").await?;

        let rows = client.query(&stmt, &[user_id]).await?;

        let mut friends = Vec::new();
        for row in rows {
            friends.push(Friend::from(row));
        }

        Ok(friends)
    }

    pub async fn get_by_friend_id(client: &impl GenericClient, friend_id: &Uuid) -> Result<Vec<Friend>, PostgresError> {
        let stmt = client.prepare("SELECT * FROM friends WHERE friend_id = $1").await?;

        let rows = client.query(&stmt, &[friend_id]).await?;

        let mut friends = Vec::new();
        for row in rows {
            friends.push(Friend::from(row));
        }

        Ok(friends)
    }

    pub async fn get_by_user_id_and_friend_id<C: GenericClient>(client: &C, user_id: &Uuid, friend_id: &Uuid) -> Result<Friend, PostgresError> {
        let stmt = client.prepare("SELECT * FROM friends WHERE user_id = $1 AND friend_id = $2").await?;

        let row = client.query_one(&stmt, &[user_id, friend_id]).await?;

        Ok(Friend::from(row))
    }

    pub async fn add<C: GenericClient>(&self, client: &C, redis_connection: &mut Connection) -> Result<Uuid, PostgresError> {
        let stmt = client.prepare(
            "INSERT INTO friends (id, user_id, friend_id) VALUES ($1, $2, $3)"
        ).await?;

        client.execute(
            &stmt,
            &[&self.id, &self.user_id, &self.friend_id]
        ).await?;

        post::Post::cache_invalidate_by_friend_user_id(client, redis_connection, &self.user_id).await.unwrap();

        Ok(self.id)
    }

    pub async fn delete<C: GenericClient>(&self, client: &C, redis_connection: &mut Connection) -> Result<bool, PostgresError> {
        let stmt = client.prepare(
            "DELETE FROM friends WHERE user_id = $1 AND friend_id = $2"
        ).await?;

        client.execute(
            &stmt,
            &[&self.user_id, &self.friend_id]
        ).await?;

        post::Post::cache_invalidate_by_friend_user_id(client, redis_connection, &self.user_id).await.unwrap();

        Ok(true)
    }

    pub async fn is_persistant<C: GenericClient>(&self, client: &C) -> Result<bool, PostgresError> {
        let stmt = client.prepare(
            "SELECT id FROM friends WHERE user_id = $1 AND friend_id = $2"
        ).await?;

        match client.query_one(
            &stmt,
            &[&self.user_id, &self.friend_id]
        ).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}