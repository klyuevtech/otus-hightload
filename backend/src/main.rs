use actix_web::{middleware, error, web, App, Error, HttpResponse, HttpServer};
use deadpool_postgres::Pool;
use futures::{future, StreamExt};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use serde::{Deserialize, Serialize};
use user_search::user_search_server::{UserSearch, UserSearchServer};
use user_search::{UserSearchResponse, UserSearchRequest};
use tonic::codec::CompressionEncoding;

mod user_search;
mod postgres;
mod user;
mod session;

const MAX_SIZE: usize = 262_144; // max payload size is 256k

async fn list_users(pool: web::Data<&'static Pool>) -> HttpResponse {
    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to get postgres client");
        }
    };
    match user::User::get_all(&**client).await {
        Ok(list) => HttpResponse::Ok().json(list),
        Err(err) => {
            log::debug!("unable to fetch users: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to fetch users");
        }
    }
}

async fn get_user(pool: web::Data<&'static Pool>, path: web::Path<String>) -> HttpResponse {
    let id = path.parse::<String>().unwrap();
    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to get postgres client");
        }
    };
    match user::User::get_by_id(&**client, &id).await {
        Ok(user) => HttpResponse::Ok().json(user),
        Err(err) => {
            log::debug!("unable to fetch users: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to fetch users");
        }
    }
}

#[derive(Deserialize)]
struct UserSearchRequestQuery {
    first_name: String,
    last_name: String,
}

async fn search_user(pool: web::Data<&'static Pool>, search: web::Query<UserSearchRequestQuery>) -> HttpResponse {
    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to get postgres client");
        }
    };
    let use_fast_search = pool.status().available < 0;
    match user::User::search_by_first_name_and_last_name(&**client, &search.first_name, &search.last_name, use_fast_search).await {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(err) => {
            log::debug!("unable to find users: {:?}", err);
            return HttpResponse::InternalServerError().json("unable to find users");
        }
    }
}

async fn register_user(pool: web::Data<&'static Pool>, mut payload: web::Payload) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct UserRegisterPayload {
        first_name: String,
        second_name: String,
        birthdate: String,
        biography: String,
        city: String,
        password: String,
    }

    let user_data = match serde_json::from_slice::<UserRegisterPayload>(&body) {
        Ok(user_data) => user_data,
        Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }
    };

    match user::User::is_password_correct(&user_data.password) {
        Ok(result) => result,
        Err(err) => {
            log::debug!("unable to register user: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("password format is incorrect: ".to_owned() + &err.to_string()));
        }
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    let user = match user::User::new(
        &user_data.first_name,
        &user_data.second_name,
        &user_data.birthdate,
        &user_data.biography,
        &user_data.city
    ) {
        Ok(user) => user,
        Err(e) => {
            log::debug!("user data is incorrect: {:?}", e);
            return Ok(HttpResponse::InternalServerError().json("user data is incorrect: ".to_owned() + &e.to_string()));
        }
    };

    match user::User::create(
        &**client,
        &user,
        &user_data.password
    ).await {
        Ok(token) => {
            #[derive(Debug, Serialize)]
            struct UserRegisterResponse {
                user_id: String,
            }
            Ok(HttpResponse::Ok().json(UserRegisterResponse{user_id: token.to_string()}))
        },
        Err(err) => {
            log::debug!("unable to register user: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to register user"));
        }
    }
}

async fn login(pool: web::Data<&'static Pool>, mut payload: web::Payload) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct LoginPayload {
        id: String,
        password: String,
    }

    let login_data = match serde_json::from_slice::<LoginPayload>(&body) {
        Ok(user_data) => user_data,
        Err(err) => {
            log::debug!("unable to parse json data: {:?}", err);
            return Ok(HttpResponse::BadRequest().json("unable to parse json data"));
        }
    };

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    if true != user::User::authenticate(&**client, &login_data.id, &login_data.password).await {
        log::debug!("unable to authenticate user");
        return Ok(HttpResponse::InternalServerError().json("unable to authenticate user"));
    }

    let client = match pool.get().await {
        Ok(client) => client,
        Err(err) => {
            log::debug!("unable to get postgres client: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to get postgres client"));
        }
    };

    match session::Session::create(
        &**client,
        &session::Session::new(&login_data.id, serde_json::json!({}))
    ).await {
        Ok(session_id) => {
            #[derive(Debug, Serialize)]
            struct UserLoginResponse {
                token: String,
            }
            Ok(HttpResponse::Ok().json(UserLoginResponse{token: session_id.to_string()}))
        },
        Err(err) => {
            log::debug!("unable to register user: {:?}", err);
            return Ok(HttpResponse::InternalServerError().json("unable to register user"));
        }
    }
}

struct UserSearchService {
    pg_pool: &'static Pool,
}

#[tonic::async_trait]
impl UserSearch for UserSearchService {
    async fn search(
        &self,
        request: tonic::Request<UserSearchRequest>,
    ) -> std::result::Result<
        tonic::Response<UserSearchResponse>,
        tonic::Status,
    > {
        let client = match self.pg_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::debug!("unable to get postgres client: {:?}", err);
                return Err(tonic::Status::not_found("unable to get postgres client"));
            }
        };

        let req = request.into_inner().to_owned();

        let first_name = &req.first_name;
        let second_name = &req.last_name;

        match user::User::search_by_first_name_and_last_name(&**client, &first_name, &second_name, true).await {
            Ok(users) => {
                let user_data = users.into_iter().map(|user: user::User| user_search::User {
                    id: user.id().to_string(),
                    first_name: user.first_name().to_string(),
                    second_name: user.second_name().to_string(),
                    birthdate: user.birthdate().to_string(),
                    biography: user.biography().to_string(),
                    city: user.city().to_string(),
                }).collect();
                Ok(tonic::Response::new(UserSearchResponse{ data: user_data, page_number: 1, results_per_page: 50, }))
            },
            Err(err) => {
                log::debug!("unable to find users: {:?}", err);
                return Err(tonic::Status::not_found(format!("Couldn't find users by query: {}, {}", &first_name, &second_name)));
            }
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    postgres::init_pools().await;

    let grpc_address = std::env::var("GRPC_SERVER_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:9000".into())
        .parse::<std::net::SocketAddr>()
        .unwrap_or_else(|_| std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 9000));

    let grpc_server = tonic::transport::Server::builder()
        .tls_config(tonic::transport::server::ServerTlsConfig::new().identity(
            tonic::transport::Identity::from_pem(&std::fs::read_to_string("cert.pem")?,
            &std::fs::read_to_string("key.pem")?))
        )
        .unwrap()
        .add_service(
            UserSearchServer::new(UserSearchService { pg_pool: postgres::get_replica_pool_ref() })
                .send_compressed(CompressionEncoding::Gzip)
                .accept_compressed(CompressionEncoding::Gzip)
        )
        .serve(grpc_address);

    // postgres::migrate_down(postgres::get_master_pool_ref()).await;
    postgres::migrate_up(postgres::get_master_pool_ref()).await;

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
                web::resource("/user")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(list_users))
            )
            .service(
                web::resource("/user/get/{id}")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(get_user))
            )
            .service(
                web::resource("/user/search")
                    .app_data(web::Data::new(postgres::get_replica_pool_ref()))
                    .route(web::get().to(search_user))
            )
            .service(
                web::resource("/user/register")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .route(web::post().to(register_user))
            )
            .service(
                web::resource("/login")
                    .app_data(web::Data::new(postgres::get_master_pool_ref()))
                    .route(web::post().to(login))
            )
    })
    .bind_openssl(&http_address, builder)?
    .run();

    let _ = future::try_join(tokio::spawn(http_server), tokio::spawn(grpc_server)).await?;

    Ok(())

}
