use std::{fmt, vec};
use std::str::FromStr;
use std::error::Error;
use chrono::NaiveDate;
use tokio_postgres::{Error as PostgresError, GenericClient, Row, RowStream};
use futures_util::{pin_mut, TryStreamExt};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};

pub const MAX_COUNT: usize = 16;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    id: Uuid,
    first_name: String,
    second_name: String,
    birthdate: chrono::NaiveDate,
    biography: String,
    city: String,
}

impl From<Row> for User {
    fn from(row: Row) -> Self {
        let birthdate: NaiveDate = row.get(3);
        Self {
            id: row.get(0),
            first_name: row.get(1),
            second_name: row.get(2),
            birthdate,
            biography: row.get(4),
            city: row.get(5),
        }
    }
}

#[derive(Debug)]
pub struct UserDataError {
    details: String
}

impl UserDataError {
    fn new(msg: &str) ->UserDataError {
        UserDataError{details: msg.to_string()}
    }
}

impl fmt::Display for UserDataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.details)
    }
}

impl Error for UserDataError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl User {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn first_name(&self) -> &str {
        &self.first_name
    }

    pub fn second_name(&self) -> &str {
        &self.second_name
    }

    pub fn birthdate(&self) -> &chrono::NaiveDate {
        &self.birthdate
    }

    pub fn biography(&self) -> &str {
        &self.biography
    }

    pub fn city(&self) -> &str {
        &self.city
    }

    pub fn new(
        first_name: &String,
        second_name: &String,
        birthdate: &String,
        biography: &String,
        city: &String,
    ) -> Result<User, UserDataError> {
        Ok(User {
            id: Uuid::new_v4(),
            first_name: if first_name.graphemes(true).count() <= 32 { first_name.to_string() } else { return Err(UserDataError::new("first_name is too long")) },
            second_name: if second_name.graphemes(true).count() <= 32 { second_name.to_string() } else { return Err(UserDataError::new("second_name is too long")) },
            birthdate: match chrono::NaiveDate::parse_from_str(&birthdate, "%Y-%m-%d") {
                Ok(birthdate) => birthdate,
                Err(e) => {
                    log::debug!("birthdate format is incorrect: {:?}", e);
                    return Err(UserDataError::new("birthdate format is incorrect, should be %Y-%m-%d"))
                }
            },
            biography: if biography.graphemes(true).count() <= 2048 { biography.to_string() } else { return Err(UserDataError::new("biography is too long")) },
            city: if city.graphemes(true).count() <= 32 { city.to_string() } else { return Err(UserDataError::new("city is too long")) },
        })
    }

    pub async fn get_all<C: GenericClient>(client: &C) -> Result<Vec<User>, PostgresError> {
        let stmt = client.prepare("SELECT id, first_name, second_name, birthdate, biography, city FROM users").await?;
        let rows = client.query(&stmt, &[]).await?;
        Ok(rows.into_iter().map(User::from).collect())
    }

    pub async fn get_by_id<C: GenericClient>(client: &C, id: &String) -> Result<User, PostgresError> {
        let stmt = client.prepare("SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE id = $1").await?;
        let row = client.query_one(&stmt, &[&Uuid::from_str(&id).unwrap()]).await?;
        Ok(User::from(row))
    }

    pub async fn search_by_ids<C: GenericClient>(client: &C, ids: &Vec<String>) -> Result<Vec<User>, PostgresError> {
        let arr_params = ids.iter().enumerate().map(|(i, _)| "$".to_owned() + (i+1).to_string().as_str());
        let string_params = arr_params.reduce(|acc, x| acc + "," + &x).unwrap();
        let str_params: &str = string_params.as_str();

        let stmt = client.prepare(
            ("SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE id::text IN (".to_owned() + &str_params + ")").as_str()
        ).await?;

        let rows_stream: RowStream = client.query_raw(&stmt, ids).await?.try_into().unwrap();

        pin_mut!(rows_stream);
        let mut users: Vec<User> = vec![];
        while let Some(row) = rows_stream.try_next().await? {
            users.push(User::from(row));
        }
        Ok(users)
    }

    pub async fn search_by_first_name_and_last_name<C: GenericClient>(
        client: &C,
        first_name: &String,
        second_name: &String,
        use_fast_search: bool
    // ) -> Result<[User; MAX_COUNT], PostgresError> {
    ) -> Result<Vec<User>, PostgresError> {
        let rows: Result<Vec<Row>, _>;
        // let arr: Result<[Row; MAX_COUNT], _>;
        if use_fast_search {
            let stmt = client.prepare(
                // "SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE (to_tsvector('russian', first_name) @@ to_tsquery('russian', $1)) AND (to_tsvector('russian', second_name) @@ to_tsquery('russian', $2)) ORDER BY id"
                &("SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE textsearchable_first_name @@ to_tsquery('russian', $1) AND textsearchable_second_name @@ to_tsquery('russian', $2) ORDER BY id LIMIT ".to_owned() + MAX_COUNT.to_string().as_str())
            ).await?;
            rows = client.query(
                &stmt,
                &[&(first_name.to_owned() + ":*"), &(second_name.to_owned() + ":*")]
            ).await;
        } else {
            let stmt = client.prepare(
                &("SELECT id, first_name, second_name, birthdate, biography, city FROM users WHERE first_name ILIKE $1 AND second_name ILIKE $2 ORDER BY id LIMIT ".to_owned() + MAX_COUNT.to_string().as_str())
            ).await?;
            rows = client.query(
                &stmt,
                &[&("%".to_owned() + &first_name + "%"), &("%".to_owned() + &second_name.to_owned() + "%")]
            ).await;
        }
        // arr = rows.expect("SQL query execution failed").try_into();
        // Ok(arr.expect("Unable to create User array").map(User::from))
        Ok(rows.expect("SQL query execution failed").into_iter().map(User::from).collect())
    }

    pub async fn create<C: GenericClient>(client: &C, user: &User, password: &String) -> Result<Uuid, PostgresError> {
        let id = if "" == user.id.to_string() { Uuid::new_v4() } else { user.id };
        let (password_hash, salt) = User::encrypt_password(&password);

        let stmt = client.prepare(
            "INSERT INTO users (id, first_name, second_name, birthdate, biography, city, password_hash, salt) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        ).await?;

        client.execute(
            &stmt,
            &[&id, &user.first_name, &user.second_name, &user.birthdate, &user.biography, &user.city, &password_hash, &salt]
        ).await?;

        Ok(id)
    }

    pub fn is_password_correct(password: &String) -> Result<bool, UserDataError> {
        let password_length = password.graphemes(true).count();
        if 32 < password_length {
            return Err(UserDataError::new("password is too long"));
        } else if password_length < 8 {
            return Err(UserDataError::new("password is too short"));
        }
        Ok(true)
    }

    pub async fn authenticate<C: GenericClient>(client: &C, id: &String, password: &String) -> bool {
        let (password_hash, salt) = User::fetch_password_hash_and_salt(client, &id).await.unwrap();
        User::check_password(password, &password_hash)
    }

    async fn fetch_password_hash_and_salt<C: GenericClient>(client: &C, id: &String) -> Result<(String, String), PostgresError> {
        let stmt = client.prepare("SELECT password_hash, salt FROM users WHERE id = $1").await?;
        let row = client.query_one(&stmt, &[&Uuid::from_str(&id).unwrap()]).await?;
        Ok((row.get(0), row.get(1)))
    }

    fn encrypt_password(password: &String) -> (String, String) {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        (
            argon2.hash_password(password.as_ref(), &salt).unwrap().to_string(),
            salt.to_string()
        )
    }

    fn check_password(password: &String, password_hash: &String) -> bool {
        let parsed_hash = PasswordHash::new(&password_hash).unwrap();
        Argon2::default().verify_password(password.as_ref(), &parsed_hash).is_ok()
    }
}
