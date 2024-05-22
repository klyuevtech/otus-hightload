use deadpool_redis::{Pool, Config, Runtime, Connection};
use redis::cmd;
use tokio::sync::OnceCell;

static REDIS_POOL: OnceCell<Pool> = OnceCell::const_new();

pub async fn init_pool() {
    REDIS_POOL.get_or_init(|| async {
        Config::from_url(std::env::var("POSTS_FEED_CACHE_REDIS_URL").unwrap_or_else(|_| String::from("redis://redis:6379")))
            .create_pool(Some(Runtime::Tokio1)).unwrap()
    }).await;
}

pub fn get_pool_ref() -> &'static Pool {
    REDIS_POOL.get().expect("Redis pool is not avaliable")
}

pub async fn exists(key: &str, conn: &mut Connection) -> Result<bool, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("EXISTS")
            .arg(&[key])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn get(key: &str, conn: &mut Connection) -> Result<String, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("GET")
            .arg(&[key])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn set(key: &str, value: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("SET")
            .arg(&[key, value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn del(key: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    log::debug!("Deleting key: {}", key);
    Ok(
        cmd("DEL")
            .arg(&[key])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn s_is_member(key: &str, value: &str, conn: &mut Connection) -> Result<bool, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("SISMEMBER")
            .arg(&[key, value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn s_add(key: &str, value: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("SADD")
            .arg(&[key, value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn s_members(key: &str, conn: &mut Connection) -> Result<Vec<String>, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("SMEMBERS")
            .arg(&[key])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn s_remove(key: &str, value: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("SREM")
            .arg(&[key, value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn s_card(key: &str, conn: &mut Connection) -> Result<usize, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("SCARD")
            .arg(&[key])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn l_push(key: &str, value: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("LPUSH")
            .arg(&[key, value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn r_push(key: &str, value: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("RPUSH")
            .arg(&[key, value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn l_trim(key: &str, start: &usize, stop: &usize, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("LTRIM")
            .arg(&[key, start.to_string().as_str(), stop.to_string().as_str()])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn l_range(key: &str, start: &usize, stop: &usize, conn: &mut Connection) -> Result<Vec<String>, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("LRANGE")
            .arg(&[key, start.to_string().as_str(), stop.to_string().as_str()])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn l_remove(key: &str, value: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("LREM")
            .arg(&[key, "0", value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn l_range_all(key: &str, conn: &mut Connection) -> Result<Vec<String>, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("LRANGE")
            .arg(&[key, "0", "-1"])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn h_exists(key: &str, field: &str, conn: &mut Connection) -> Result<bool, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("HEXISTS")
            .arg(&[key, field])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn h_get(key: &str, field: &str, conn: &mut Connection) -> Result<String, deadpool_redis::redis::RedisError> {
    Ok(
        cmd("HGET")
            .arg(&[key, field])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn h_set(key: &str, field: &str, value: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("HSET")
            .arg(&[key, field, value])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn h_del(key: &str, field: &str, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("HDEL")
            .arg(&[key, field])
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn del_all(conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("FLUSHALL")
            .query_async(conn)
            .await.unwrap()
    )
}

pub async fn h_trim(key: &str, maxlen: &i64, conn: &mut Connection) -> Result<(), deadpool_redis::redis::RedisError> {
    Ok(
        cmd("HTRIM")
            .arg(&[key, "MAXLEN", maxlen.to_string().as_str()])
            .query_async(conn)
            .await.unwrap()
    )
}