use std::str::FromStr;
use futures::io;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tokio::sync::OnceCell;

use crate::session_storage::SessionStorage;

static STORAGE: OnceCell<Box<dyn SessionStorage + Send + Sync>> = OnceCell::const_new();

pub async fn init_storage(storage: Box<dyn SessionStorage + Send + Sync>) {
    if !STORAGE.initialized() {
        STORAGE.get_or_init(|| async {storage}).await;
    }
}

fn get_storage() -> &'static Box<dyn SessionStorage + Send + Sync> {
    STORAGE.get().unwrap()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    id: Uuid,
    user_id: Uuid,
    data: serde_json::Value,
}

impl Session {
    pub fn new(id: String, user_id: String, data: String) -> Session {
        Session {
            id: Uuid::parse_str(&id).unwrap(),
            user_id: Uuid::from_str(&user_id).unwrap(),
            data: serde_json::Value::String(data),
        }
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn get_user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn get_data(&self) -> &serde_json::Value {
        &self.data
    }

    pub async fn create(session: &Session) -> Result<Uuid, io::Error> {
        get_storage().create(session).await
    }

    pub async fn get_by_id(id: &str) -> Result<Option<Session>, io::Error> {
        get_storage().get_by_id(id).await
    }
}
