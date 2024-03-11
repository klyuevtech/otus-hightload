use actix_web::{error, get, post, web, App, Error, HttpResponse, HttpServer};
use deadpool_postgres::Pool;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

mod postgres;
mod user;
mod session;

const MAX_SIZE: usize = 262_144; // max payload size is 256k

#[get("/user")]
async fn list_users(pool: web::Data<Pool>) -> HttpResponse {
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

#[get("/user/get/{id}")]
async fn get_user(pool: web::Data<Pool>, path: web::Path<String>) -> HttpResponse {
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

#[post("/user/register")]
async fn register_user(pool: web::Data<Pool>, mut payload: web::Payload) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    log::debug!("register_user(): post body: {:?}", body);

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
    
    log::debug!("register_user(): parsed user object: {:?}", user_data);

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
        user_data.first_name,
        user_data.second_name,
        user_data.birthdate,
        user_data.biography,
        user_data.city
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

#[post("/login")]
async fn login(pool: web::Data<Pool>, mut payload: web::Payload) -> Result<HttpResponse, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }
        body.extend_from_slice(&chunk);
    }

    log::debug!("register_user: post body: {:?}", body);

    #[derive(Debug, Serialize, Deserialize)]
    struct LoginPayload {
        id: String,
        password: String,
    }

    let login_data = serde_json::from_slice::<LoginPayload>(&body)?;

    log::debug!("login(): login_data: {:?}", login_data);

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
        &session::Session::new(&login_data.id, String::from(""))
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

fn address() -> String {
    std::env::var("ADDRESS").unwrap_or_else(|_| "127.0.0.1:8000".into())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let pg_pool = postgres::create_pool();
    // postgres::migrate_down(&pg_pool).await;
    postgres::migrate_up(&pg_pool).await;

    let address = address();
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pg_pool.clone()))
            .service(list_users)
            .service(get_user)
            .service(register_user)
            .service(login)
    })
    .bind(&address)?
    .run()
    .await
}
