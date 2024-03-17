use deadpool_postgres::{Config, Pool};
use tokio_postgres::NoTls;
use tokio_postgres_migration::Migration;

const SCRIPTS_UP: [(&str, &str); 6] = [(
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
)];

const SCRIPTS_DOWN: [(&str, &str); 2] = [(
    "0001_create-users",
    include_str!("../migrations/0001_create-users_down.sql"),
),(
    "0001_create-sessions",
    include_str!("../migrations/0001_create-sessions_down.sql"),
)];

fn create_config() -> Config {
    let mut cfg = Config::new();
    if let Ok(host) = std::env::var("PG_HOST") {
        cfg.host = Some(host);
    }
    if let Ok(dbname) = std::env::var("PG_DBNAME") {
        cfg.dbname = Some(dbname);
    }
    if let Ok(user) = std::env::var("PG_USER") {
        cfg.user = Some(user);
    }
    if let Ok(password) = std::env::var("PG_PASSWORD") {
        cfg.password = Some(password);
    }
    cfg
}

pub fn create_pool() -> Pool {
    create_config()
        .create_pool(NoTls)
        .expect("couldn't create postgres pool")
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
