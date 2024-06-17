use serde::Serialize;
use tokio_postgres::GenericClient;
use uuid::Uuid;

pub struct Dialog {
    id: Uuid,
    user_id1: Uuid,
    user_id2: Uuid,
}

impl Dialog {
    pub fn get_id(&self) -> Uuid {
        self.id
    }
    pub async fn save_message<C: GenericClient>(pg_client: &C, message_owner_user_id: Uuid, message_receiver_user_id: Uuid, content: String) -> Result<(), tokio_postgres::Error> {
        let dialog = Dialog::get_dialog(pg_client, message_owner_user_id, message_receiver_user_id).await?;
        let message = Message::new(dialog.get_id(), message_owner_user_id, content).await;
        Message::create(pg_client, &message).await?;
        Ok(())
    }
    async fn get_dialog<C: GenericClient>(pg_client: &C, user_id1: Uuid, user_id2: Uuid) -> Result<Dialog, tokio_postgres::Error> {
        let stmt = pg_client.prepare(
            "SELECT id, user_id1, user_id2 FROM dialogs WHERE (user_id1 = $1 AND user_id2 = $2) OR (user_id1 = $2 AND user_id2 = $1)"
        ).await?;

        if let Ok(row) = pg_client.query_one(&stmt, &[&user_id1, &user_id2]).await {
            Ok(Dialog {
                id: row.get(0),
                user_id1: row.get(1),
                user_id2: row.get(2),
            })
        } else {
            let dialog = Dialog {
                id: Uuid::new_v4(),
                user_id1,
                user_id2,
            };
            Dialog::create(pg_client, &dialog).await?;
            Ok(dialog)
        }
    }

    async fn create<C: GenericClient>(pg_client: &C, dialog: &Dialog) -> Result<(), tokio_postgres::Error> {
        let stmt = pg_client.prepare(
            "INSERT INTO dialogs (id, user_id1, user_id2) VALUES ($1, $2, $3)"
        ).await?;

        pg_client.execute(
            &stmt,
            &[&dialog.id, &dialog.user_id1, &dialog.user_id2]
        ).await?;

        Ok(())
    }

    pub async fn list_messages<C: GenericClient>(
        pg_client: &C,
        user_id1: Uuid,
        user_id2: Uuid,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Message>, tokio_postgres::Error> {
        let stmt = pg_client.prepare(
            "SELECT id FROM dialogs WHERE (user_id1 = $1 AND user_id2 = $2) OR (user_id1 = $2 AND user_id2 = $1)"
        ).await?;

        let row = pg_client.query_one(&stmt, &[&user_id1, &user_id2]).await?;
        if row.is_empty() {
            return Ok(Vec::new());
        }

        let dialog_id: uuid::Uuid = row.get(0);

        let stmt = pg_client.prepare(
            "SELECT id, dialog_id, user_id, content FROM dialog_messages WHERE dialog_id = $1 ORDER BY id OFFSET $2 LIMIT $3"
        ).await?;

        let rows = pg_client.query(&stmt, &[&dialog_id, &(offset as i64), &(limit as i64)]).await?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(Message {
                id: row.get(0),
                dialog_id: row.get(1),
                user_id: row.get(2),
                content: row.get(3),
            });
        }

        Ok(messages)
    }
}

#[derive(Serialize)]
pub struct Message {
    id: Uuid,
    dialog_id: Uuid,
    user_id: Uuid,
    content: String,
}

impl Message {
    pub async fn new(dialog_id: Uuid, user_id: Uuid, content: String) -> Message {
        Message {
            id: Uuid::new_v4(),
            dialog_id,
            user_id,
            content,
        }
    }
    pub async fn create<C: GenericClient>(pg_client: &C, message: &Message) -> Result<(), tokio_postgres::Error> {
        let stmt = pg_client.prepare(
            "INSERT INTO dialog_messages (id, dialog_id, user_id, content) VALUES ($1, $2, $3, $4)"
        ).await?;

        pg_client.execute(
            &stmt,
            &[&message.id, &message.dialog_id, &message.user_id, &message.content]
        ).await?;

        Ok(())
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