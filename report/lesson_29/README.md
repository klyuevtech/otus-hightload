# Домашнее задание

## Реализация счетчика непрочитанных сообщений

### Описание архитектуры

При разработаке архитектуры счетчика непрочитанных сообщений возникла идея хранить непрочитанные сообщения в отдельном микросервисе. После прочтения сообщения, оно будет перемещено в микросервис прочитанных сообщений.

Для реализации управления сервисами прочитанных и непрочитанных сообщений был выбран шаблон Повествование (Saga) с оркестрацией. Роль оркестратора было решено делегировать шлюзу (gateway) микросервисов.

Взаимодействие сервисов продемонстрировано на следующей диаграмме последовательностей:

![](./assets/MessagesDiagram.svg)

Идея заключается в том, чтобы сделать хранилище новых сообщений как можно более легковесным. Так как количество непрочитанных сообщений у пользователя обычно не большое, то и выборка таких сообщений и побсчет количества будет быстрой операцией. В таком случае возможно даже можно будет обойтись запросом SELECT COUNT(id) для счетчика непрочитанных сообщений.

Приложения пользователей будут подключаться и отсылать запросы к шлюзу, шлюз выполняя оркестрацию будет производить необходимые операции с микросервисами и отдавать уже агрегированный результат приложениям пользователей.

Код микросервиса непрочитанных сообщений находится в файлах
`unread/src/main.rs`
`unread/src/dialogs.rs`

Код микросервиса прочитанных сообщений находится в файлах
`dialogs/src/main.rs`
`dialogs/src/dialogs.rs`

В роли шлюза выступает основное приложение-монолит
`backend/src/main.rs`

Рассмотрим 4 основных варианта использования описанных на диаграмме.

1. Размешение новых сообщений. Приложение пользователя отправляет новое сообщение в шлюз, сообщение в приложении отправителя в этот момент помечается как неотправленное.
   Шлюз после получения сообщение отправляет его микросервис непрочитанных сообщений (реализовано в файле `backend/src/main.rs`, функция `dialog_send`).
   При успешном ответе от микросервиса непрочитанных сообщений шлюз возвращает ответ с подтверждением приложению пользователя, то в свою очередь помечает сообщение как "полученное".
   Если получатель в это время подключен к шлюзу по WebSoket, то приложение получателя оповещается о получении нового сообщения.

2. Запрос новых сообщений. При запросе новых сообщений приложением получателя, шлюз вычитывает их из сервиса непрочитанных сообщений и отправляет получателю. При этом сообщения помечаются как "доставленное".
   Файл `backend/src/main.rs`, функция `dialog_get_unread`

3. При просмотре сообщений получателем, приложение получателя отсылает специальный запрос шлюзу содержащий ID просмотренных сообщений. При получении этого запроса, шлюз вычитывает сообщения по ID из сервиса непрочитанных сообщений, пересылает эти сообщения в сервис прочитанных сообщений и при получении положительного ответа от сервиса прочитанных сообщений об успешном сохранении сообщений, отсылает запрос в сервис непрочитанных сообщений на удаление этих сообщений по ID. При этом если клиентый подключены по WS, то у получателя и отправителя сообщения помечаются как прочитанные.
   Файл `backend/src/main.rs`, функция `dialog_mark_as_read`.

4. Последнияя диаграмма отображает процесс запроса всех сообщений приложением получателя. При запросе, например, 5-и сообщений шлюз выполняет запросы в сервисы непрочитанных и прочитанных сообщений. Если непрочитанных сообщений 5 или более, то запрос в сервит прочитанных сообщений можно не производить. Агрегированный ответ отправляется в приложение получателя. При простомтре сообщений получателем, приложение отошлет в шлюз специальный запрос содержащий ID просмотренных сообщений и шлюз, как описано в пункте 3, перешлет сообщений в сервис прочитанных и оповестит об этом клиентские приложения по WS.

При описанной архитектуре хранения непрочитанных сообщений когда сервис непрочитанных сообщений достаточно легковесный, для реализации счетчика непрочитанных сообщений достаточно выпонить запрос `SELECT COUNT(id)` в базу сервиса. Реализовано в файле `backend/src/main.rs`, функция `dialog_count` в шлюзе. Код запроса в микросервисе:

unread/src/main.rs

```rust
async fn dialog_count(
    req: HttpRequest,
    pool: web::Data<&'static PostgresPool>,
    mut payload: web::Payload,
) -> HttpResponse {
    log_request(&req);

    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.unwrap();
        if (body.len() + chunk.len()) > MAX_SIZE {
            return HttpResponse::BadRequest().json("Payload data overflow");
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Deserialize)]
    struct DialogListPayload {
        user_id1: String,
        user_id2: String,
    }

    let payload_data = match serde_json::from_slice::<DialogListPayload>(&body) {
        Ok(payload_data) => payload_data,
        Err(err) => {
            log::debug!("Unable to parse json data: {:?}", err);
            return HttpResponse::BadRequest().json("Unable to parse payload json data");
        }
    };

    let user_id1 = match uuid::Uuid::parse_str(&payload_data.user_id1) {
        Ok(uid) => uid,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let user_id2 = match uuid::Uuid::parse_str(&payload_data.user_id2) {
        Ok(uid) => uid,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("Unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("Unable to get postgres client");
        }
    };

    let header_value_unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let x_request_id = req
        .headers()
        .get("x-request-id")
        .unwrap_or_else(|| &header_value_unknown)
        .to_str()
        .unwrap();

    if let Ok(messages_count) = dialog::Message::count(&**client, user_id1, user_id2).await {
        return HttpResponse::Ok()
            .insert_header(("x-request-id", x_request_id))
            .insert_header((
                "x-server-instance",
                std::env::var("SELF_HOST_NAME").unwrap_or(String::from("unknown")),
            ))
            .json(messages_count);
    } else {
        return HttpResponse::InternalServerError().json("Unable to get messages");
    }
}
```

unread/src/dialog.rs

```rust
    pub async fn count<C: GenericClient>(
        pg_client: &C,
        user_id1: Uuid,
        user_id2: Uuid,
    ) -> Result<i64, tokio_postgres::Error> {
        let stmt = pg_client.prepare(
            "SELECT COUNT(*) FROM dialog_messages WHERE (sender_user_id = $1 AND receiver_user_id = $2) OR (sender_user_id = $2 AND receiver_user_id = $1)"
        ).await?;
        let row = pg_client.query_one(&stmt, &[&user_id1, &user_id2]).await?;
        Ok(row.get(0))
    }
```
