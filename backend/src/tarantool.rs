use rusty_tarantool::tarantool::{Client, ClientConfig};
use rand::Rng as _;
use tokio::sync::OnceCell;

static REPLICA_AUTHORITY: OnceCell<Vec<TarantoolClientConfig>> = OnceCell::const_new();

#[derive(Clone)]
pub struct TarantoolClientManager {
    clients: Vec<Client>,
}

impl TarantoolClientManager {
    pub async fn new() -> Self {
        let configs = REPLICA_AUTHORITY.get_or_init(|| async {
            let auth: Vec<String> = std::env::var("TARANTOOL_AUTHORITY")
                .unwrap_or_else(|_| String::from("tarantool:3301,tarantool:3302,tarantool:3303,tarantool:3304,tarantool:3305,tarantool:3306,tarantool:3307"))
                .split(',')
                .map(|s| s.to_string())
                .collect();

            auth
                .into_iter()
                .map(|authority| {
                    TarantoolClientConfig::new(
                        String::from(authority),
                        std::env::var("TARANTOOL_LOGIN").unwrap_or(String::from("appuser")),
                        std::env::var("TARANTOOL_PASSWORD").unwrap_or(String::from("topsecret")),
                    )
                })
                .collect::<Vec<_>>()
        }).await;

        Self {
            clients: configs.into_iter().map(|config| ClientConfig::new(
                    config.addr.clone(),
                    config.login.clone(),
                    config.password.clone(),
            )
                .set_timeout_time_ms(2000)
                .set_reconnect_time_ms(2000)
                .build()).collect(),
        }
    }

    pub fn get_client(&self) -> &Client {
        let range_end = self.clients.len()-1;
        if range_end == 0 {
            return &self.clients[0];
        }
        &self.clients[rand::thread_rng().gen_range(0..range_end)]
    }

    pub async fn get_leader_client(&self) -> &Client {
        &self.clients[self.get_leader_index().await as usize]
    }

    async fn get_leader_index(&self) -> usize {
        let leader_name: String = self.get_client().prepare_fn_call("get_leader_name").execute().await.unwrap().decode_single().unwrap();
        let leader_index = leader_name.chars().last().unwrap().to_digit(10).unwrap() as usize - 1;
        leader_index
    }
}

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
