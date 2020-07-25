use crate::{database::GuildConfig, BotResult, Database};

use dashmap::DashMap;
use sqlx::{types::Json, FromRow, Row};
use twilight::model::id::GuildId;

impl Database {
    pub async fn get_guilds(&self) -> BotResult<DashMap<GuildId, GuildConfig>> {
        let guilds = sqlx::query("SELECT * FROM guilds")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| {
                let id: i64 = row.get(0);
                let config = GuildConfig::from_row(&row).unwrap();
                (GuildId(id as u64), config)
            })
            .collect();
        Ok(guilds)
    }

    pub async fn insert_guilds(&self, configs: &DashMap<GuildId, GuildConfig>) -> BotResult<usize> {
        configs.retain(|_, config| config.modified);
        let mut txn = self.pool.begin().await?;
        let mut counter = 0;
        for guard in configs.iter() {
            let query = format!(
                "
INSERT INTO
    guilds
VALUES
    ({},$1)
ON CONFLICT DO
    UPDATE
        SET config=$1",
                guard.key()
            );
            sqlx::query(&query)
                .bind(Json(guard.value()))
                .execute(&mut *txn)
                .await?;
            counter += 1;
        }
        txn.commit().await?;
        Ok(counter)
    }
}
