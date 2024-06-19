use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use actix_web_httpauth::extractors::bearer::{
    self,
    // BearerAuth
};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use deadpool_postgres::Pool as PostgresPool;
use serde::Deserialize;
use futures::StreamExt;

mod postgres;
mod dialog;

const MAX_SIZE: usize = 262_144; // max payload size is 256k

fn log_request(req: HttpRequest) {
    let unknown = actix_web::http::header::HeaderValue::from_str("unknown").unwrap();
    let request_id = req.headers().get("x-request-id").unwrap_or_else(|| &unknown);
    log::debug!("[Request ID {:?}] {:?}", request_id, req);
}

async fn dialog_send(
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

    let message_receiver_user_id = match uuid::Uuid::parse_str(&payload_data.message_receiver_user_id) {
        Ok(user_id2) => user_id2,
        Err(_err) => return HttpResponse::InternalServerError().json("User id is not specified"),
    };

    let pg_client = match pg_pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("Unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("Unable to get postgres client");
        }
    };

    match dialog::Dialog::save_message(
        &**pg_client,
        message_sender_user_id,
        message_receiver_user_id,
        payload_data.text
    ).await {
        Ok(_) => log::debug!("Message saved"),
        Err(error) => {
            log::debug!("Unable to save message: {:?}", error);
            return HttpResponse::InternalServerError().json("Unable to save message");
        }
    }

    HttpResponse::Ok().json("ok")
}

async fn dialog_list(
    req: HttpRequest,
    pool: web::Data<&'static PostgresPool>,
    mut payload: web::Payload,
) -> HttpResponse {
    log_request(req);

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
    log::debug!("List messages with params: user_id1='{:?}' user_id2='{:?}' offset={:?} limit={:?}", user_id1, user_id2, offset, limit);
    HttpResponse::Ok().json(serde_json::json!(dialog::Dialog::list_messages(&**client, user_id1, user_id2, offset, limit).await.unwrap()))
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

    let http_address = std::env::var("HTTP_SERVER_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8000".into());
    let http_server = HttpServer::new(|| {
        App::new()
            .wrap(middleware::Compress::default())
            .service(
                web::resource("/send")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .route(web::post().to(dialog_send))
            )
            .service(
                web::resource("/list")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .app_data(bearer::Config::default().realm("Restricted area").scope("post"))
                    .route(web::post().to(dialog_list))
            )
    })
    .bind_openssl(&http_address, builder).unwrap()
    .run();

    http_server.await
}
