# Домашнее задание
## Разделение монолита на сервисы

Функционал диалогов вынесен в отдельный сервис.
Код и конфигурация в папке dialogs .

Сервис dialogs доступен только внутри сети 'server-side'.
REST API запросы приходят в основное монолитное приложение которое выполняет роль шлюза. При получении запроса шлюз производит аутентификацию и если пользователь определен, то запрос перенаправляется в сервис диалогов. Результат полученный от сервиса диалогов отправляется обратно клиенту.

Перенаправление запроса происходит в функциях `dialog_send` и `dialog_list` в backend/src/main.rs файле:

```rust
    let res = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build().unwrap()
        .post(dialog_service_url + "/list")
        .header("x-request-id", request_id_str)
        .json(&dialogs_body)
        .send().await.unwrap()
        .text().await.unwrap();
```

Результат выполнения запроса сохраняется в переменной `res` и передается в ответе клиенту:

```rust
    HttpResponse::Ok()
        .content_type(ContentType::json())
        .insert_header(("x-request-id", request_id_str))
        .body(res)
```

Как видим, также происходит проброс хедера x-request-id, значение которого вместе со всеми данными запроса логируется на сервисе dialogs (файл dialogs/src/main.rs):

```rust
fn log_request(req: HttpRequest) {
    let unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let request_id = req.headers().get("x-request-id").unwrap_or_else(|| &unknown);
    log::debug!("[Request ID {:?}] {:?}", request_id, req);
}
```