use futures::io;
use rusty_tarantool::tarantool::{Client, ClientConfig, ExecWithParamaters as _};
use tonic::async_trait;
use uuid::Uuid;

use crate::session::Session;
use crate::session_storage::SessionStorage;

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

pub struct TarantoolSessionStorage {
    client: Client,
}

impl TarantoolSessionStorage {
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
impl SessionStorage for TarantoolSessionStorage {
    async fn create(&self, session: &Session) -> Result<Uuid, io::Error> {
        let session_tuple: (String, String, String) = self.client
            .prepare_fn_call("session_create")
            .bind(session.get_id().to_string().as_str()).unwrap()
            .bind(session.get_user_id().to_string().as_str()).unwrap()
            .bind(session.get_data().to_string().as_str()).unwrap()
            .execute().await.unwrap()
            .decode_single().unwrap();

        Ok(uuid::Uuid::parse_str(session_tuple.0.as_str()).unwrap())
    }

    async fn get_by_id(&self, id: &str) -> Result<Session, io::Error> {
        let session_tuple: (String, String, String) = self.client
            .prepare_fn_call("session_get_by_id")
            .bind(id).unwrap()
            .execute().await.unwrap()
            .decode_single().expect("Session not found");

        Ok(
            Session::new(
                String::from(session_tuple.0),
                String::from(session_tuple.1), 
                String::from(session_tuple.2),
            )
        )
    }
}