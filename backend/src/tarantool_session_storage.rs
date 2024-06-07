use futures::io;
use rusty_tarantool::tarantool::{Client, ExecWithParamaters as _};
use tonic::async_trait;
use uuid::Uuid;

use crate::session::Session;
use crate::session_storage::SessionStorage;
use crate::tarantool::TarantoolClientManager;

pub struct TarantoolSessionStorage {
    manager: TarantoolClientManager,
}

impl TarantoolSessionStorage {
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
impl SessionStorage for TarantoolSessionStorage {
    async fn create(&self, session: &Session) -> Result<Uuid, io::Error> {
        let session_tuple: (String, String, String) = self.get_leader_client().await
            .prepare_fn_call("session_create")
            .bind(session.get_id().to_string().as_str()).unwrap()
            .bind(session.get_user_id().to_string().as_str()).unwrap()
            .bind(session.get_data().to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_single().unwrap();

        Ok(uuid::Uuid::parse_str(session_tuple.0.as_str()).unwrap())
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Session>, io::Error> {
        if let Ok(session_tuple) = self.get_client()
            .prepare_fn_call("session_get_by_id")
            .bind(id).unwrap()
            .execute().await.unwrap()
            .decode_single::<(String, String, String)>() {
                Ok(Some(
                    Session::new(
                        String::from(session_tuple.0),
                        String::from(session_tuple.1), 
                        String::from(session_tuple.2),
                    )
                ))
            } else {
                Ok(None)
            }
    }
}