use log::{error, info, warn};
use redis::{
    AsyncCommands, FromRedisValue,
    aio::ConnectionManager,
    streams::{StreamReadOptions, StreamReadReply},
};
use shared::message::StreamMessage;
use sqlx::{PgPool, QueryBuilder};

pub struct MessageConsumer {
    redis_conn: ConnectionManager,
    db_pool: PgPool,
    consumer_name: String,
    opts: StreamReadOptions,
}

struct StreamEntry {
    redis_id: String,
    message: StreamMessage,
}

impl MessageConsumer {
    pub fn new(
        redis_conn: ConnectionManager,
        db_pool: PgPool,
        consumer_name: impl Into<String>,
    ) -> Self {
        let consumer_name = consumer_name.into();
        let opts = StreamReadOptions::default()
            .group(StreamMessage::GROUP, &consumer_name)
            .count(500)
            .block(300);

        Self {
            redis_conn,
            db_pool,
            consumer_name,
            opts,
        }
    }

    pub async fn run(mut self) {
        info!(
            "Consumer '{}' started on stream '{}'",
            self.consumer_name,
            StreamMessage::STREAM
        );

        loop {
            match self.read_batch().await {
                Ok((valid, invalid_ids)) => {
                    if !invalid_ids.is_empty() {
                        warn!("ACKing {} invalid/poison messages", invalid_ids.len());
                        if let Err(e) = self.ack(&invalid_ids).await {
                            error!("Failed to ACK invalid messages: {}", e);
                        }
                    }

                    if valid.is_empty() {
                        continue;
                    }
 
                    let valid_ids: Vec<String> = valid.iter().map(|e| e.redis_id.clone()).collect();

                    if let Err(e) = self.insert_batch(&valid).await {
                        error!("Batch insert failed ({} messages): {}", valid.len(), e);
                        // Do NOT ACK — Redis will redeliver
                        continue;
                    }

                    if let Err(e) = self.ack(&valid_ids).await {
                        error!("Failed to ACK {} valid messages: {}", valid_ids.len(), e);
                    } else {
                        info!("Processed & ACKed {} messages", valid_ids.len());
                    }
                }
                Err(e) => {
                    error!("Redis read error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn read_batch(&mut self) -> redis::RedisResult<(Vec<StreamEntry>, Vec<String>)> {
        let reply: StreamReadReply = self
            .redis_conn
            .xread_options(&[StreamMessage::STREAM], &[">"], &self.opts)
            .await?;

        let mut valid = Vec::new();
        let mut invalid_ids = Vec::new();

        for key in reply.keys {
            for entry in key.ids {
                let maybe_msg: Option<StreamMessage> = match entry.map.get("data") {
                    Some(v) => match String::from_redis_value(v.clone()) {
                        Ok(json_str) => match serde_json::from_str(&json_str) {
                            Ok(msg) => Some(msg),
                            Err(e) => {
                                warn!("Failed to deserialize message {}: {}", entry.id, e);
                                None
                            }
                        },
                        Err(e) => {
                            warn!("Failed to convert Redis value for {}: {}", entry.id, e);
                            None
                        }
                    },
                    None => {
                        warn!("Message {} has no 'data' field", entry.id);
                        None
                    }
                };

                match maybe_msg {
                    Some(msg) => {
                        valid.push(StreamEntry {
                            redis_id: entry.id,
                            message: msg,
                        });
                    }
                    None => {
                        invalid_ids.push(entry.id);
                    }
                }
            }
        }

        Ok((valid, invalid_ids))
    }

    async fn insert_batch(&self, entries: &[StreamEntry]) -> Result<(), sqlx::Error> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut qb =
            QueryBuilder::new(r#"INSERT INTO messages (id, "from", "to", message, created_at) "#);

        qb.push_values(entries, |mut b, entry| {
            b.push_bind(entry.message.id)
                .push_bind(&entry.message.from)
                .push_bind(&entry.message.to)
                .push_bind(&entry.message.message)
                .push_bind(entry.message.created_at);
        });

        qb.push(" ON CONFLICT (id) DO NOTHING");

        qb.build().execute(&self.db_pool).await?;

        Ok(())
    }

    async fn ack(&mut self, ids: &[String]) -> redis::RedisResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        let _: () = self
            .redis_conn
            .xack(StreamMessage::STREAM, StreamMessage::GROUP, &refs)
            .await?;
        Ok(())
    }
}
