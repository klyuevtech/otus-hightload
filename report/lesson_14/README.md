Домашнее задание

Онлайн обновление ленты новостей

Для выполнения условия: "При добавлении нового поста друга подписчику websocket'а должно приходить событие о новом посте, тем самым обеспечивая обновление ленты в реальном времени. При реализации обязательно обеспечить отправку только целевым пользователям это событие (можно применить Routing Key из RabbitMQ)." нам нужно будет создать дополнительный модуль для функционала web sockets:
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/websocket.rs
Этот модуль будет обеспечивать управление вебсокет соединениями.
Для того, чтобы обеспечить отправку сообщений только целевым пользователям, для транспортировки и маршрутизации сообщений которые будут отправляться по вебсокет соединениям нужно создать дополнительный RabbitMQ exchange типа 'direct', который позволяет отправлять сообщения в определенную очередь с использованием маршрутизации по "routing key".

Приложение работает таким образом:

В главной функции `main` вызываем:

```rust
    rabbitmq::create_pub_sub().await;
```

для создания нужных Exchange:

```rust
pub async fn create_pub_sub() {
    self::create_exchange(post::FEED_QUEUE_EXCHANGE_NAME, "fanout").await;
    self::create_exchange(websocket::FEED_WS_QUEUE_EXCHANGE_NAME, "direct").await;
}
```

В главной функции `main` вызываем `websocket::serve()` в отдельной задаче. 

```rust
    // WebSocket server
    let ws_server = websocket::serve();

    let _ = future::try_join(
        tokio::spawn(http_server),
        tokio::spawn(ws_server),
    ).await?;
```

Функция `websocket::serve()` стартуем TCP socket сервер и принимаем входяще соединения:

```rust
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
```

При получении входящего соединения сохраняем информацию о соединении и очереди вызовом `get_or_init_peer_map().await.lock().await.insert(addr, (outgoing, String::from(queue_name), SystemTime::now()))` , создаем саму очередь и подключаем к ней consumer:

```rust
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
```

Клиент web socket сразу после создания соединения (на localhost:8087 в тестовой конфигурации) должен отправить сообщение в которм содержится его ID пользователя, например:

```json
{"user_id":"91cb89ec-ae54-4c1b-84c6-6b46497d4dcb"}
```

Тут наверное стоит переделать на session ID, а User ID уже брать из сессии. Эта информация нужна для создания очереди с нужным routing key.

В объект WSConsumer сохраняется информация об адресе peer которому нужно будет переслать полученное в подключенную очередь сообщение.

Consumer будет отправлять сообщения, полученные в свою подключенную очередь, своему peer:

```rust
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
```

В модуле post.rs при добавлении/изменении/удалении поста вызывается функция `publish_message`:
https://github.com/klyuevtech/otus-hightload/blob/main/backend/src/post.rs#L494

которая принимает ID пользователя которому принадлежит post, находит пользователей, у которых пользователь с данным ID назодится в друзьях, и отправляет сообщение для каждого пользователя в его очередь используя "routing key" сгенеренный как "prefix + user_id" (такой же routing key использовался при привязке очереди при подключении пользователя к серверу web socket):
```rust
    {
        let user_id = Uuid::parse_str(entity_id).unwrap();
        let users = friend::Friend::get_by_friend_id(pg_client, &user_id).await.unwrap();
        for user in users.iter() {
            rabbitmq::publish_message(
                rabbitmq::get_channel_ref().await.lock().await.first().unwrap(),
                message,
                websocket::FEED_WS_QUEUE_EXCHANGE_NAME,
                (self::FEED_QUEUE_ROUTING_KEY_PREFIX.to_owned() + user.get_user_id().to_string().as_str()).as_str()
            ).await;
        }
    }
```

Таким образом пользователи получают только обновления о постах друзей.


В функции `websocket::serve` также запускается задача по удалению информации об отключенных соединениях и относящимся к ним очередям:

```rust
    // Run task each 10 seconds to remove disconnected peers and queues
    tokio::spawn(async {
        loop {
            remove_disconnected_peers_and_queues().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });
```

Текст функции `remove_disconnected_peers_and_queues`:

```rust
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
```