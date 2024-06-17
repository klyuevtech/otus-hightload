use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::{config::LoadBalanceHosts, NoTls};
use tokio_postgres_migration::Migration;
use tokio::sync::OnceCell;

const SCRIPTS_UP: [(&str, &str); 5] = [(
    "0001_create_extension-uuid-ossp_up",
    include_str!("../migrations/0001_create_extension-uuid-ossp_up.sql"),
),(
    "0001_create_extension_citus_up",
    include_str!("../migrations/0001_create_extension_citus_up.sql"),
),(
    "0001_create_dialog-messages_up",
    include_str!("../migrations/0001_create_dialog-messages_up.sql"),
),(
    "0001_create_distributed_table_dialog_messages_up",
    include_str!("../migrations/0001_create_distributed_table_dialog_messages_up.sql"),
),(
    "0001_create_dialogs_up",
    include_str!("../migrations/0001_create_dialogs_up.sql"),
),];

const SCRIPTS_DOWN: [(&str, &str); 4] = [(
    "0001_create_extension-uuid-ossp_down",
    include_str!("../migrations/0001_create_extension-uuid-ossp_down.sql"),
),(
    "0001_create_extension_citus_down",
    include_str!("../migrations/0001_create_extension_citus_down.sql"),
),(
    "0001_create_dialog-messages_down",
    include_str!("../migrations/0001_create_dialog-messages_down.sql"),
),(
    "0001_create_dialogs_down",
    include_str!("../migrations/0001_create_dialogs_down.sql"),
)];

static MASTER_POOL: OnceCell<Pool> = OnceCell::const_new();
static REPLICA_POOL: OnceCell<Pool> = OnceCell::const_new();

pub async fn init() {
    init_master_pool_ref().await;
    init_replica_pool_ref().await;
    // migrate_down().await;
    migrate_up().await;
}

async fn init_master_pool_ref() -> &'static Pool {
    MASTER_POOL.get_or_init(|| async {
        create_pool(
            get_master_pool_max_size(),
            create_config(
                std::env::var("PG_USER").expect("Postgres user is not specified").as_str(),
                std::env::var("PG_PASSWORD").expect("Postgres password is not specified").as_str(),
                vec!(std::env::var("PG_AUTHORITY_MASTER").expect("Postgres host is not specified").as_str()),
                std::env::var("PG_DBNAME").expect("Postgres dbname is not specified").as_str(),
            )
        )
    }).await
}

async fn init_replica_pool_ref() -> &'static Pool {
    REPLICA_POOL.get_or_init(|| async {
        create_pool(
            get_replica_pool_max_size(),
            create_config(
                std::env::var("PG_USER").expect("Postgres user is not specified").as_str(),
                std::env::var("PG_PASSWORD").expect("Postgres password is not specified").as_str(),
                std::env::var("PG_AUTHORITY_REPLICA").expect("Postgres host is not specified").split(',').collect(),
                std::env::var("PG_DBNAME").expect("Postgres dbhame is not specified").as_str(),
            )
        )
    }).await
}

pub fn get_master_pool_ref() -> &'static Pool {
    MASTER_POOL.get().expect("Postgres master pool is not initialized")
}

pub fn get_replica_pool_ref() -> &'static Pool {
    REPLICA_POOL.get().expect("Postgres replica pool is not initialized")
}

fn get_master_pool_max_size() -> usize {
    usize::from_str_radix(std::env::var("PG_MASTER_POOL_MAX_SIZE").unwrap_or("100".to_owned()).as_str(), 10).unwrap_or(100)
}

fn get_replica_pool_max_size() -> usize {
    usize::from_str_radix(std::env::var("PG_REPLICA_POOL_MAX_SIZE").unwrap_or("100".to_owned()).as_str(), 10).unwrap_or(100)
}

fn create_config(user: &str, password: &str, hosts: Vec<&str>, dbname: &str) -> tokio_postgres::Config {
    let mut cfg = tokio_postgres::Config::new();
    hosts.into_iter().for_each(|authority| {
        let (host, port) = authority.split_once(':')
            .expect(&("Resource authority is specified incorrectly. Correct format should be: 'host1:port1,host2:port2...'. Got: ".to_owned() + &authority),);

        cfg.host(host);
        cfg.port(u16::from_str_radix(port, 10).expect("Port is not specified or incorrect"));
    });

    cfg
        .dbname(dbname)
        .user(user)
        .password(password)
        .load_balance_hosts(LoadBalanceHosts::Random)
        .to_owned()
}

fn create_pool(max_size: usize, config: tokio_postgres::Config) -> Pool {
    Pool::new(
        Manager::from_config(config, NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast
            }
        ),
        max_size
    )
}

pub async fn migrate_up() {
    let mut client = self::get_master_pool_ref().get().await.expect("couldn't get postgres client");

    let migration = Migration::new("migrations".to_string());
    migration
        .up(&mut **client, &SCRIPTS_UP)
        .await
        .expect("couldn't run migrations");
}

pub async fn migrate_down() {
    let mut client = self::get_master_pool_ref().get().await.expect("couldn't get postgres client");
    let migration = Migration::new("migrations".to_string());
    migration
        .down(&mut **client, &SCRIPTS_DOWN)
        .await
        .expect("couldn't run migrations");
}
