use std::sync::Arc;

use amqprs::{
    callbacks::{DefaultChannelCallback, DefaultConnectionCallback},
    channel::{
        BasicCancelArguments, BasicConsumeArguments, BasicPublishArguments, Channel, ExchangeDeclareArguments, QueueBindArguments, QueueDeclareArguments, QueueDeleteArguments
    },
    connection::{ Connection, OpenConnectionArguments},
    consumer::AsyncConsumer, BasicProperties,
};
use futures::lock::Mutex;
use tokio::sync::OnceCell;

use crate::websocket;
use crate::post;

type ChannelVec = Arc<Mutex<Vec<Channel>>>;

static RABBITMQ_CONNECTION: OnceCell<Connection> = OnceCell::const_new();
static RABBITMQ_CHANNEL_VEC: OnceCell<ChannelVec> = OnceCell::const_new();

async fn get_or_create_connection() -> &'static Connection {
    RABBITMQ_CONNECTION.get_or_init(|| async {
        let connection = Connection::open(&OpenConnectionArguments::new(
            std::env::var("RABBITMQ_CONNECTION_HOST").unwrap_or_else(|_| String::from("rabbitmq")).as_str(),
            std::env::var("RABBITMQ_CONNECTION_PORT").unwrap_or_else(|_| String::from("5672")).parse::<u16>().unwrap(),
            std::env::var("RABBITMQ_CONNECTION_USERNAME").unwrap_or_else(|_| String::from("guest")).as_str(),
            std::env::var("RABBITMQ_CONNECTION_PASSWORD").unwrap_or_else(|_| String::from("guest")).as_str(),
        ))
        .await
        .unwrap();

        connection
            .register_callback(DefaultConnectionCallback)
            .await
            .unwrap();

        connection
    })
    .await
}

pub async fn get_connection_ref() -> &'static Connection {
    get_or_create_connection().await
}

async fn get_or_create_channel_vec() -> &'static ChannelVec {
    RABBITMQ_CHANNEL_VEC.get_or_init(|| async {
        let connection = get_connection_ref().await;
        let channel = connection.open_channel(None).await.unwrap();
        channel
            .register_callback(DefaultChannelCallback)
            .await
            .unwrap();

        Arc::new(Mutex::new(vec![channel]))
    }).await
}

pub async fn get_channel_ref() -> &'static ChannelVec {
    let mut channel_vec = get_or_create_channel_vec().await.lock().await;
    channel_vec.retain(|channel| channel.is_open());
    if channel_vec.is_empty() {
        let connection = get_connection_ref().await;
        let channel = connection.open_channel(None).await.unwrap();
        channel
            .register_callback(DefaultChannelCallback)
            .await
            .unwrap();
        channel_vec.push(channel);
    }
    
    get_or_create_channel_vec().await
}

pub async fn create_pub_sub() {
    self::create_exchange(post::FEED_QUEUE_EXCHANGE_NAME, "fanout").await;
    self::create_exchange(websocket::FEED_WS_QUEUE_EXCHANGE_NAME, "direct").await;
}

pub async fn create_exchange(
    exchange_name: &str,
    exchange_type: &str,
) {
    let channel_vec = self::get_channel_ref().await.lock().await;
    let channel = channel_vec.first().unwrap();

    let args = ExchangeDeclareArguments::new(exchange_name, exchange_type);
    channel.exchange_declare(args).await.unwrap();
}

pub async fn create_queue(
    queue_name: &str,
) {
    let channel_vec = self::get_channel_ref().await.lock().await;
    let channel = channel_vec.first().unwrap();

    channel.queue_declare(QueueDeclareArguments::new(queue_name))
        .await
        .unwrap_or_else(|e| {log::debug!("Error creating queue: {}. Error: {:?}", queue_name, e);Some((String::from(""), 0, 0))});
}

pub async fn bind_queue(
    exchange_name: &str,
    queue_name: &str,
    routing_key: &str,
) {
    let channel_vec = self::get_channel_ref().await.lock().await;
    let channel = channel_vec.first().unwrap();

    let args = QueueBindArguments::new(queue_name, exchange_name, routing_key);
    channel.queue_bind(args)
        .await
        .unwrap_or_else(|e| {log::debug!("Error binding queue: {}. Error: {:?}", queue_name, e);});
}

pub async fn delete_queue(
    queue_name: &str,
) {
    let channel_vec = self::get_channel_ref().await.lock().await;
    let channel = channel_vec.first().unwrap();

    let args = QueueDeleteArguments::new(queue_name);
    channel.queue_delete(args).await.unwrap();
}

pub async fn add_consumer(channel: &Channel, consumer: impl AsyncConsumer + std::marker::Send + 'static + std::fmt::Debug, args: BasicConsumeArguments) {
    log::debug!("Adding consumer: {:?}. Args: {:?}", consumer, args);
    let consumer_tag = args.consumer_tag.clone();
    remove_consumer(channel, &consumer_tag).await;
    channel
        .basic_consume(consumer, args)
        .await
        .unwrap_or_else(|e| {log::debug!("Error adding consumer: {:?}. Error: {:?}", consumer_tag, e);String::from("")});
}

pub async fn remove_consumer(channel: &Channel, consumer_tag: &str) {
    log::debug!("Removing consumer: {}", consumer_tag);
    let args = BasicCancelArguments::new(consumer_tag);
    channel
        .basic_cancel(args)
        .await
        .unwrap_or_else(|e| {log::debug!("Error removing consumer: {}. Error: {:?}", consumer_tag, e);String::from("")});
}

pub async fn publish_message(channel: &Channel, message: &str, exchange_name: &str, routing_key: &str) {
    let content = message.as_bytes().to_vec();
    let args = BasicPublishArguments::new(exchange_name, routing_key);
    log::debug!("Publishing message: \"{:?}\". Publish args: \"{:?}\"", message, args);
    channel
        .basic_publish(BasicProperties::default(), content, args)
        .await
        .unwrap();
}
