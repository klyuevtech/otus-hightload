use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::{config::LoadBalanceHosts, NoTls};
use tokio_postgres_migration::Migration;
use tokio::sync::OnceCell;

const SCRIPTS_UP: [(&str, &str); 7] = [(
    "0001_create-extension-uuid-ossp",
    include_str!("../migrations/0001_create-extension-uuid-ossp_up.sql"),
),(
    "0001_create-extension-pg_trgm",
    include_str!("../migrations/0001_create-extension-pg_trgm_up.sql"),
),(
    "0001_create-users",
    include_str!("../migrations/0001_create-users_up.sql"),
),(
    "0001_create-sessions",
    include_str!("../migrations/0001_create-sessions_up.sql"),
),(
    "0001_create-index-users-f_name_s_name_idx_up",
    include_str!("../migrations/0001_create-index-users-f_name_s_name_idx_up.sql"),
),(
    "0001_create-index-users-id_idx_up",
    include_str!("../migrations/0001_create_index-users-id_idx_up.sql"),
),(
    "/0001_create_index-users_names_gin_tsvector_up",
    include_str!("../migrations/0001_create_index-users_names_gin_tsvector_up.sql"),
)];

const SCRIPTS_DOWN: [(&str, &str); 2] = [(
    "0001_create-users",
    include_str!("../migrations/0001_create-users_down.sql"),
),(
    "0001_create-sessions",
    include_str!("../migrations/0001_create-sessions_down.sql"),
)];

static MASTER_POOL: OnceCell<Pool> = OnceCell::const_new();
static REPLICA_POOL: OnceCell<Pool> = OnceCell::const_new();

pub async fn init_pools() {
    MASTER_POOL.get_or_init(|| async {create_master_pool()}).await;
    REPLICA_POOL.get_or_init(|| async {create_replica_pool()}).await;
}

pub fn get_master_pool_ref() -> &'static Pool {
    MASTER_POOL.get().expect("Master pool is not avaliable")
}

pub fn get_replica_pool_ref() -> &'static Pool {
    REPLICA_POOL.get().expect("Replica pool is not avaliable")
}

pub fn create_master_pool() -> Pool {
    create_pool(get_master_pool_max_size(),
        create_config(
            std::env::var("PG_USER").expect("Postgres user is not specified").as_str(),
            std::env::var("PG_PASSWORD").expect("Postgres password is not specified").as_str(),
            vec!(std::env::var("PG_AUTHORITY_MASTER").expect("Postgres host is not specified").as_str()),
            std::env::var("PG_DBNAME").expect("Postgres dbname is not specified").as_str(),
        )
    )
}

pub fn create_replica_pool() -> Pool {
    create_pool(get_replica_pool_max_size(),
        create_config(
            std::env::var("PG_USER").expect("Postgres user is not specified").as_str(),
            std::env::var("PG_PASSWORD").expect("Postgres password is not specified").as_str(),
            std::env::var("PG_AUTHORITY_REPLICA").expect("Postgres host is not specified").split(',').collect(),
            std::env::var("PG_DBNAME").expect("Postgres dbhame is not specified").as_str(),
        )
    )
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
            .expect("Resource authority is specified incorrectly. Correct format should be: host1:port1,host2:port2...");

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

pub async fn migrate_up(pool: &Pool) {
    let mut client = pool.get().await.expect("couldn't get postgres client");
    let migration = Migration::new("migrations".to_string());
    migration
        .up(&mut **client, &SCRIPTS_UP)
        .await
        .expect("couldn't run migrations");
}

pub async fn migrate_down(pool: &Pool) {
    let mut client = pool.get().await.expect("couldn't get postgres client");
    let migration = Migration::new("migrations".to_string());
    migration
        .down(&mut **client, &SCRIPTS_DOWN)
        .await
        .expect("couldn't run migrations");
}
