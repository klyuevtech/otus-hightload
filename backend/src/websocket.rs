use std::{
    collections::HashMap, env, io::Error as IoError, net::SocketAddr, sync::Arc, time::SystemTime,
};

use amqprs::{channel::{BasicAckArguments, BasicConsumeArguments, Channel}, consumer::AsyncConsumer, BasicProperties, Deliver};
use futures::{stream::SplitSink, StreamExt, lock::Mutex, SinkExt};

use serde::Deserialize;
use tokio::{net::{TcpListener, TcpStream}, sync::OnceCell};
use tokio_tungstenite::{tungstenite::protocol::Message, WebSocketStream};
use tonic::async_trait;

use crate::{post, rabbitmq, websocket};

type Wss = SplitSink<WebSocketStream<TcpStream>, Message>;
type PeerMap = Arc<Mutex<HashMap<SocketAddr, (Wss, String, SystemTime)>>>;

pub const FEED_WS_QUEUE_CONSUMER_TAG: &str = "ws_pub_sub";
pub const FEED_WS_QUEUE_NAME: &str = "feed.amqprs.ws";
pub const FEED_WS_QUEUE_EXCHANGE_NAME: &str = "feed.amq.topic.ws";

#[derive(Debug)]
pub struct WSConsumer {
    no_ack: bool,
    peer_addr: SocketAddr,
}

impl WSConsumer {
    pub fn new(no_ack: bool, peer_addr: SocketAddr) -> Self {
        Self {
            no_ack,
            peer_addr,
        }
    }
}

#[async_trait]
impl AsyncConsumer for WSConsumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        log::debug!("[WebSocket] WSConsumer: consume delivery {} on channel {}, content: {}",
            deliver, channel, String::from_utf8(content.clone()).unwrap(),
        );

        let message = String::from_utf8(content).unwrap();
        post_event_message(message, Some(self.peer_addr)).await;

        // ack explicitly if manual ack
        if !self.no_ack {
            log::info!("[WebSocket] WSConsumer: ack to delivery {} on channel {}", deliver, channel);
            let args = BasicAckArguments::new(deliver.delivery_tag(), false);
            channel.basic_ack(args).await.unwrap();
        }
    }
}

#[derive(Deserialize)]
struct WSIncomingPayload {
    user_id: String,
}

async fn handle_connection(raw_stream: TcpStream, addr: SocketAddr) {
    log::info!("[WebSocket] Incoming TCP connection from: {}", addr);
    let ws_stream = tokio_tungstenite::accept_async(raw_stream)
        .await
        .expect("[WebSocket] Error during the websocket handshake occurred");

    log::info!("[WebSocket] WebSocket connection established: {}", addr);
    let (outgoing, mut incoming ) = ws_stream.split();

    let ws_incoming_payload = serde_json::from_str::<WSIncomingPayload>(
        incoming.next().await.unwrap().unwrap().into_text().unwrap().as_str()
    ).unwrap();
    log::debug!("[WebSocket] Received User id: {}", ws_incoming_payload.user_id);

    // Queue binding: START
    let user_id = ws_incoming_payload.user_id.to_string();
    let user_id = user_id.as_str();
    let queue_name = self::FEED_WS_QUEUE_NAME.to_owned() + user_id;
    let queue_name = queue_name.as_str();

    {
        get_or_init_peer_map().await.lock().await.insert(addr, (outgoing, String::from(queue_name), SystemTime::now()));
    }

    rabbitmq::create_queue(queue_name).await;
    log::info!("[WebSocket] Binding queue: {}. Routing key: {:?}", &queue_name, (post::FEED_QUEUE_ROUTING_KEY_PREFIX.to_owned() + ws_incoming_payload.user_id.to_string().as_str()).as_str());
    rabbitmq::bind_queue(
        websocket::FEED_WS_QUEUE_EXCHANGE_NAME,
        queue_name,
        (post::FEED_QUEUE_ROUTING_KEY_PREFIX.to_owned() + user_id).as_str()
    ).await;

    let args = BasicConsumeArguments::new(
        queue_name,
        (self::FEED_WS_QUEUE_CONSUMER_TAG.to_owned() + user_id).as_str(),
    );

    {
        log::info!("[WebSocket] Adding consumer: {:?}", args);
        rabbitmq::add_consumer(
            rabbitmq::get_channel_ref().await.lock().await.first().unwrap(),
            WSConsumer::new(args.no_ack, addr),
            args
        ).await;
    }
    // Queue binding: END
}

async fn remove_disconnected_peers_and_queues() {
    log::debug!("remove_disconnected_peers_and_queues: START");
    let mut disconnected_peers = Vec::new();
    {
        let mut peer_map = get_or_init_peer_map().await.lock().await;
        for (addr, (outgoing, queue_name, time_connected)) in peer_map.iter_mut() {
            if time_connected.elapsed().unwrap().as_secs() < 5 {
                continue;
            }
            match outgoing.send(Message::from(String::from("watchdog"))).await {
                Ok(_) => {
                    log::debug!("[WebSocket] Watchdog: message sent to peer: {}", addr);
                },
                Err(e) => {
                    log::debug!("[WebSocket] Watchdog: Error sending watchdog message to peer: {}. Error: {:?}.", addr, e);
                    log::debug!("[WebSocket] Watchdog: Removing queue: {}", queue_name);
                    rabbitmq::delete_queue(queue_name).await;
                    disconnected_peers.push(*addr);
                }
            };
        }
    }

    {
        let mut peer_map = get_or_init_peer_map().await.lock().await;
        for addr in disconnected_peers.iter() {
            log::debug!("[WebSocket] Watchdog: Removing peer: {}", addr);
            peer_map.remove(addr);
        }
    }
    log::debug!("remove_disconnected_peers_and_queues: DONE");
}

static WS_STATE: OnceCell<PeerMap> = OnceCell::const_new();

async fn get_or_init_peer_map() -> &'static Arc<Mutex<HashMap<SocketAddr, (Wss, String, SystemTime)>>> {
    WS_STATE.get_or_init(|| async { PeerMap::new(Mutex::new(HashMap::new())) }).await
}

pub async fn post_event_message(msg: String, peer_addr: Option<SocketAddr>) {
    {
        let mut peer_map = get_or_init_peer_map().await.lock().await;
        if let Some(peer_addr) = peer_addr {
            log::info!("[WebSocket] Sending message to peer: {}", &peer_addr);
            peer_map.get_mut(&peer_addr).unwrap().0.send(Message::from(String::from(&msg))).await.unwrap_or_else(
                |e| {
                    log::debug!("[WebSocket] Error sending message to {}: {:?}", &msg, e);
                }
            );
            peer_map.get_mut(&peer_addr).unwrap().0.flush().await.unwrap();
        } else {
            log::info!("[WebSocket] Sending message to all peers: {}. Peer length: {:?}", &msg, peer_map.len());
            for (addr, (outgoing, _, _)) in peer_map.iter_mut() {
                log::info!("[WebSocket] Sending message to peer: {}", addr);
                outgoing.send(Message::from(String::from(&msg))).await.unwrap_or_else(
                    |e| {
                        log::debug!("[WebSocket] Error sending message to {}: {:?}", &msg, e);
                    }
                )
            }
        }
    }
    log::debug!("[WebSocket] post_event_message: DONE");
}

pub async fn serve() -> Result<(), IoError> {
    let addr = env::var("WS_SERVER_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8087".to_string());

    // Run task each 10 seconds to remove disconnected peers and queues
    tokio::spawn(async {
        loop {
            remove_disconnected_peers_and_queues().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    log::info!("[WebSocket] WS server listening on: {}", addr);

    // Let's spawn the handling of each connection in a separate task.
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(stream, addr));
    }

    Ok(())
}