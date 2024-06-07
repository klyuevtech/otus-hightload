use std::io::Error;
use tonic::async_trait;
use uuid::Uuid;

use crate::session::Session;

#[async_trait]
pub trait SessionStorage {
    async fn get_by_id(&self, id: &str) -> Result<Option<Session>, Error>;
    async fn create(&self, session: &Session) -> Result<Uuid, Error>;
}
