use std::str::FromStr;
use serde::{Deserialize, Serialize};
use tokio_postgres::{Error, GenericClient, Row};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    id: Uuid,
    user_id: Uuid,
    data: serde_json::Value,
}

impl From<Row> for Session {
    fn from(row: Row) -> Self {
        let data: serde_json::Value = row.get(2);
        Self {
            id: row.get(0),
            user_id: row.get(1),
            data,
        }
    }
}

impl Session {
    pub fn new(user_id: &String, data: serde_json::Value) -> Session {
        Session {
            id: Uuid::new_v4(),
            user_id: Uuid::from_str(&user_id).unwrap(),
            data
        }
    }

    pub async fn get_by_id<C: GenericClient,>(client: &C, id: &str) -> Result<Session, Error> {
        let stmt = client.prepare("SELECT * FROM sessions WHERE id = $1").await?;

        let row = client.query_one(&stmt, &[&Uuid::from_str(id).unwrap()]).await?;

        Ok(Session::from(row))
    }

    pub async fn create<C: GenericClient>(client: &C, session: &Session) -> Result<Uuid, Error> {
        let id = if "" == session.id.to_string() { Uuid::new_v4() } else { session.id };

        let stmt = client.prepare(
            "INSERT INTO sessions (id, user_id, data) VALUES ($1, $2, $3)"
        ).await?;

        client.execute(
            &stmt,
            &[&id, &session.user_id, &session.data]
        ).await?;

        Ok(id)
    }

    pub fn get_user_id(&self) -> Uuid {
        self.user_id
    }
}