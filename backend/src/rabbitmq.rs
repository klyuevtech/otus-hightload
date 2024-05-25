use amqprs::{
    callbacks::{DefaultChannelCallback, DefaultConnectionCallback},
    channel::{
        BasicConsumeArguments, BasicPublishArguments, Channel, QueueBindArguments, QueueDeclareArguments
    },
    connection::{ Connection, OpenConnectionArguments},
    consumer::AsyncConsumer, BasicProperties,
};
use tokio::sync::OnceCell;

static RABBITMQ_CONNECTION: OnceCell<Connection> = OnceCell::const_new();
static RABBITMQ_CHANNEL: OnceCell<Channel> = OnceCell::const_new();

pub async fn init_connection() {
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
    .await;
}

pub async fn open_channel() -> Channel {
    RABBITMQ_CHANNEL.get_or_init(|| async {
        let connection = get_connection_ref();
        let channel = connection.open_channel(None).await.unwrap();
        channel
            .register_callback(DefaultChannelCallback)
            .await
            .unwrap();
        channel
    }).await.to_owned()
}

pub fn get_channel_ref() -> &'static Channel {
    RABBITMQ_CHANNEL.get().expect("RabbitMQ channel is not available")
}

pub fn get_connection_ref() -> &'static Connection {
    RABBITMQ_CONNECTION.get().expect("RabbitMQ connection is not available")
}

pub async fn create_pub_sub(
    exchange_name: &str,
    routing_key: &str,
    consumer: impl AsyncConsumer + std::marker::Send + 'static,
    args: BasicConsumeArguments
) {
    init_connection().await;
    let channel = self::open_channel().await;
    channel
        .queue_declare(QueueDeclareArguments::durable_client_named(&args.queue,))
        // .queue_declare(QueueDeclareArguments::default())
        .await
        .unwrap()
        .unwrap();

    channel
        .queue_bind(QueueBindArguments::new(
            &args.queue,
            exchange_name,
            routing_key,
        ))
        .await
        .unwrap();

    channel
        .basic_consume(consumer, args)
        .await
        .unwrap();
}

pub async fn publish_message(message: &str, exchange_name: &str, routing_key: &str) {
    let content = message.as_bytes().to_vec();
    let args = BasicPublishArguments::new(exchange_name, routing_key);

    self::get_channel_ref()
        .basic_publish(BasicProperties::default(), content, args)
        .await
        .unwrap();
}