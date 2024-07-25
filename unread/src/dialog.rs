use serde::Serialize;
use tokio_postgres::GenericClient;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Message {
    id: Uuid,
    sender_user_id: Uuid,
    receiver_user_id: Uuid,
    content: String,
}

impl Message {
    pub fn get_id(&self) -> Uuid {
        self.id
    }
    pub async fn save<C: GenericClient>(
        pg_client: &C,
        sender_user_id: Uuid,
        receiver_user_id: Uuid,
        content: String,
    ) -> Result<(), tokio_postgres::Error> {
        let message = Message::new(sender_user_id, receiver_user_id, content).await;
        Message::create(pg_client, &message).await?;
        Ok(())
    }

    pub async fn list<C: GenericClient>(
        pg_client: &C,
        user_id1: Uuid,
        user_id2: Uuid,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Message>, tokio_postgres::Error> {
        let stmt = pg_client.prepare(
            "SELECT id, sender_user_id, receiver_user_id, content FROM dialog_messages WHERE (sender_user_id = $1 AND receiver_user_id = $2) OR (sender_user_id = $2 AND receiver_user_id = $1) ORDER BY time_created OFFSET $2 LIMIT $3"
        ).await?;
        let rows = pg_client
            .query(
                &stmt,
                &[&user_id1, &user_id2, &(offset as i64), &(limit as i64)],
            )
            .await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        Ok(rows
            .into_iter()
            .map(|row| Message {
                id: row.get(0),
                sender_user_id: row.get(1),
                receiver_user_id: row.get(2),
                content: row.get(3),
            })
            .collect::<Vec<Message>>())
    }

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

    pub async fn new(sender_user_id: Uuid, receiver_user_id: Uuid, content: String) -> Message {
        Self {
            id: Uuid::new_v4(),
            sender_user_id,
            receiver_user_id,
            content,
        }
    }

    pub async fn create<C: GenericClient>(
        pg_client: &C,
        message: &Message,
    ) -> Result<(), tokio_postgres::Error> {
        let stmt = pg_client.prepare(
            "INSERT INTO dialog_messages (id, sender_user_id, receiver_user_id, content) VALUES ($1, $2, $3, $4)"
        ).await?;

        pg_client
            .execute(
                &stmt,
                &[
                    &message.id,
                    &message.sender_user_id,
                    &message.receiver_user_id,
                    &message.content,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn get_by_ids<C: GenericClient>(
        pg_client: &C,
        ids: Vec<Uuid>,
    ) -> Result<Vec<Message>, tokio_postgres::Error> {
        let stmt = pg_client
            .prepare(
                "SELECT id, sender_user_id, receiver_user_id, content FROM dialog_messages WHERE id = ANY($1)"
            )
            .await
            .unwrap();

        let rows = pg_client.query(&stmt, &[&ids]).await.unwrap();

        Ok(rows
            .into_iter()
            .map(|row| Message {
                id: row.get(0),
                sender_user_id: row.get(1),
                receiver_user_id: row.get(2),
                content: row.get(3),
            })
            .collect::<Vec<Message>>())
    }

    pub async fn remove_by_ids<C: GenericClient>(
        pg_client: &C,
        ids: Vec<Uuid>,
    ) -> Result<u64, tokio_postgres::Error> {
        let stmt = pg_client
            .prepare("DELETE FROM dialog_messages WHERE id = ANY($1)")
            .await
            .unwrap();

        pg_client.execute(&stmt, &[&ids]).await
    }
    // async fn read<C: GenericClient>(pg_client: &C, message_id: Uuid) -> Result<Message, tokio_postgres::Error> {
    //     let stmt = pg_client.prepare(
    //         "SELECT id, dialog_id, user_id, content FROM dialog_messages WHERE id = $1"
    //     ).await?;

    //     let row = pg_client.query_one(&stmt, &[&message_id]).await?;

    //     Ok(Message {
    //         id: row.get(0),
    //         dialog_id: row.get(1),
    //         user_id: row.get(2),
    //         content: row.get(3),
    //     })
    // }
    // async fn update<C: GenericClient>(pg_client: &C, message: Message) -> Result<(), tokio_postgres::Error> {
    //     let stmt = pg_client.prepare(
    //         "UPDATE dialog_messages SET dialog_id = $2, user_id = $3, content = $4 WHERE id = $1"
    //     ).await?;

    //     pg_client.execute(
    //         &stmt,
    //         &[&message.id, &message.dialog_id, &message.user_id, &message.content]
    //     ).await?;

    //     Ok(())
    // }
    // async fn delete<C: GenericClient>(pg_client: &C, message_id: Uuid) -> Result<(), tokio_postgres::Error> {
    //     let stmt = pg_client.prepare(
    //         "DELETE FROM dialog_messages WHERE id = $1"
    //     ).await?;

    //     pg_client.execute(&stmt, &[&message_id]).await?;

    //     Ok(())
    // }
}
