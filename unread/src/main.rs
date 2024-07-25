use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_httpauth::extractors::bearer::{
    self,
    // BearerAuth
};
use deadpool_postgres::Pool as PostgresPool;
use futures::StreamExt;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use serde::Deserialize;

mod dialog;
mod postgres;

const MAX_SIZE: usize = 262_144; // max payload size is 256k

fn log_request(req: &HttpRequest) {
    let unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let request_id = req
        .headers()
        .get("x-request-id")
        .unwrap_or_else(|| &unknown);
    log::debug!("[Request ID {:?}] {:?}", request_id, req);
}

async fn dialog_send(
    req: HttpRequest,
    pg_pool: web::Data<&'static PostgresPool>,
    mut payload: web::Payload,
) -> HttpResponse {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.unwrap();
        if (body.len() + chunk.len()) > MAX_SIZE {
            return HttpResponse::BadRequest().json("Payload data overflow");
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Deserialize)]
    struct DialogSendPayload {
        message_sender_user_id: String,
        message_receiver_user_id: String,
        text: String,
    }

    let payload_data = match serde_json::from_slice::<DialogSendPayload>(&body) {
        Ok(payload_data) => payload_data,
        Err(err) => {
            log::debug!("Unable to parse json data: {:?}", err);
            return HttpResponse::BadRequest().json("Unable to parse payload json data");
        }
    };

    let message_sender_user_id = match uuid::Uuid::parse_str(&payload_data.message_sender_user_id) {
        Ok(user_id2) => user_id2,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let message_receiver_user_id =
        match uuid::Uuid::parse_str(&payload_data.message_receiver_user_id) {
            Ok(user_id2) => user_id2,
            Err(_err) => {
                return HttpResponse::InternalServerError().json("User id is not specified")
            }
        };

    let pg_client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("Unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("Unable to get postgres client");
        }
    };

    match dialog::Message::save(
        &**pg_client,
        message_sender_user_id,
        message_receiver_user_id,
        payload_data.text,
    )
    .await
    {
        Ok(_) => log::debug!("Message saved"),
        Err(error) => {
            log::debug!("Unable to save message: {:?}", error);
            return HttpResponse::InternalServerError().json("Unable to save message");
        }
    }

    let header_value_unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let x_request_id = req
        .headers()
        .get("x-request-id")
        .unwrap_or_else(|| &header_value_unknown)
        .to_str()
        .unwrap();

    HttpResponse::Ok()
        .insert_header(("x-request-id", x_request_id))
        .insert_header((
            "x-server-instance",
            std::env::var("SELF_HOST_NAME").unwrap_or(String::from("unknown")),
        ))
        .json("ok")
}

async fn dialog_list(
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
        offset: Option<usize>,
        limit: Option<usize>,
    }

    let payload_data = match serde_json::from_slice::<DialogListPayload>(&body) {
        Ok(payload_data) => payload_data,
        Err(err) => {
            log::debug!("Unable to parse json data: {:?}", err);
            return HttpResponse::BadRequest().json("Unable to parse payload json data");
        }
    };

    let user_id1 = match uuid::Uuid::parse_str(&payload_data.user_id1) {
        Ok(user_id2) => user_id2,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let user_id2 = match uuid::Uuid::parse_str(&payload_data.user_id2) {
        Ok(user_id2) => user_id2,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let offset = match payload_data.offset {
        Some(offset) => offset,
        None => 0,
    };

    let limit = match payload_data.limit {
        Some(limit) => limit,
        None => 10,
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("Unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("Unable to get postgres client");
        }
    };
    log::debug!(
        "List messages with params: user_id1='{:?}' user_id2='{:?}' offset={:?} limit={:?}",
        user_id1,
        user_id2,
        offset,
        limit
    );

    let header_value_unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let x_request_id = req
        .headers()
        .get("x-request-id")
        .unwrap_or_else(|| &header_value_unknown)
        .to_str()
        .unwrap();

    HttpResponse::Ok()
        .insert_header(("x-request-id", x_request_id))
        .insert_header((
            "x-server-instance",
            std::env::var("SELF_HOST_NAME").unwrap_or(String::from("unknown")),
        ))
        .json(serde_json::json!(dialog::Message::list(
            &**client, user_id1, user_id2, offset, limit
        )
        .await
        .unwrap_or_else(|e| {
            log::debug!("Unable to get messages: {:?}", e);
            return Vec::new();
        })))
}

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

async fn dialog_messages_get_by_ids(
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
    struct DialogMessagesGetByIDsPayload {
        ids: Vec<String>,
    }

    let ids: Vec<uuid::Uuid> = match serde_json::from_slice::<DialogMessagesGetByIDsPayload>(&body)
    {
        Ok(payload) => payload
            .ids
            .into_iter()
            .map(|id| uuid::Uuid::parse_str(&id))
            .filter_map(Result::ok)
            .collect(),
        Err(err) => {
            log::debug!("Unable to parse json data: {:?}", err);
            return HttpResponse::BadRequest().json("Unable to parse payload json data");
        }
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("Unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("Unable to get postgres client");
        }
    };

    if let Ok(messages) = dialog::Message::get_by_ids(&**client, ids).await {
        return HttpResponse::Ok().json(messages);
    } else {
        return HttpResponse::InternalServerError().json("Unable to get messages");
    }
}

async fn dialog_messages_remove(
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
    struct DialogRemoveMessagesPayload {
        ids: Vec<String>,
    }

    let ids: Vec<uuid::Uuid> = match serde_json::from_slice::<DialogRemoveMessagesPayload>(&body) {
        Ok(payload) => payload
            .ids
            .into_iter()
            .map(|id| uuid::Uuid::parse_str(&id))
            .filter_map(Result::ok)
            .collect(),
        Err(err) => {
            log::debug!("Unable to parse json data: {:?}", err);
            return HttpResponse::BadRequest().json("Unable to parse payload json data");
        }
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("Unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("Unable to get postgres client");
        }
    };

    if let Ok(_) = dialog::Message::remove_by_ids(&**client, ids).await {
        return HttpResponse::Ok().json("ok");
    } else {
        return HttpResponse::InternalServerError().json("Unable to remove messages");
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    postgres::init().await;

    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key_file("key.pem", SslFiletype::PEM)
        .unwrap();

    builder.set_certificate_chain_file("cert.pem").unwrap();

    let http_address =
        std::env::var("HTTP_SERVER_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8000".into());
    let http_server = HttpServer::new(|| {
        App::new()
            .wrap(middleware::Compress::default())
            .service(
                web::resource("/send")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .app_data(
                        bearer::Config::default()
                            .realm("Restricted area")
                            .scope("post"),
                    )
                    .route(web::post().to(dialog_send)),
            )
            .service(
                web::resource("/list")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .app_data(
                        bearer::Config::default()
                            .realm("Restricted area")
                            .scope("post"),
                    )
                    .route(web::post().to(dialog_list)),
            )
            .service(
                web::resource("/count")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .app_data(
                        bearer::Config::default()
                            .realm("Restricted area")
                            .scope("post"),
                    )
                    .route(web::post().to(dialog_count)),
            )
            .service(
                web::resource("/messages/get")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .app_data(
                        bearer::Config::default()
                            .realm("Restricted area")
                            .scope("post"),
                    )
                    .route(web::post().to(dialog_messages_get_by_ids)),
            )
            .service(
                web::resource("/messages/remove")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .app_data(
                        bearer::Config::default()
                            .realm("Restricted area")
                            .scope("post"),
                    )
                    .route(web::post().to(dialog_messages_remove)),
            )
    })
    .bind_openssl(&http_address, builder)
    .unwrap()
    .run();

    http_server.await
}
