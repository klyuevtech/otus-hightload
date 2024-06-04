use deadpool_postgres::Pool;
use futures::io;
use tokio_postgres::Row;
use tonic::async_trait;
use uuid::Uuid;

use crate::{session::Session, session_storage::SessionStorage};

impl From<Row> for Session {
    fn from(row: Row) -> Session {
        let data: serde_json::Value = row.get(2);
        Session::new(row.get(0), row.get(1), data.to_string())
    }
}

pub struct PostgresSessionStorage {
    master_pool: &'static Pool,
    replica_pool: &'static Pool,
}

impl PostgresSessionStorage {
    pub fn new(master_pool: &'static Pool, replica_pool: &'static Pool) -> Self {
        Self {
            master_pool,
            replica_pool,
        }
    }
}

#[async_trait]
impl SessionStorage for PostgresSessionStorage {
    async fn get_by_id(&self, id: &str) -> Result<Session, io::Error> {
        let client = self.replica_pool.get().await.unwrap();

        let stmt = client.prepare("SELECT * FROM sessions WHERE id = $1").await.unwrap();

        let row = client.query_one(&stmt, &[&Uuid::parse_str(id).unwrap()]).await.unwrap();

        Ok(Session::from(row))
    }

    async fn create(&self, session: &Session) -> Result<Uuid, io::Error> {
        let client = self.master_pool.get().await.unwrap();
        
        let stmt = client.prepare(
            "INSERT INTO sessions (id, user_id, data) VALUES ($1, $2, $3)"
        ).await.unwrap();

        client.execute(
            &stmt,
            &[&session.get_id(), &session.get_user_id(), &session.get_data()]
        ).await.unwrap();

        Ok(session.get_id())
    }
}