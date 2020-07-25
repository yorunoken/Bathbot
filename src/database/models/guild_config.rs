use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgRow, types::Json, Error, FromRow, Row};
use std::ops::Deref;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GuildConfig {
    pub with_lyrics: bool,
    pub prefixes: Vec<String>,
    pub authorities: Vec<u64>,
    #[serde(default, skip_serializing)]
    pub modified: bool,
}

impl<'c> FromRow<'c, PgRow> for GuildConfig {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        serde_json::from_value(row.get("config")).map_err(|why| Error::Decode(Box::new(why)))
    }
}

impl Default for GuildConfig {
    fn default() -> Self {
        GuildConfig {
            with_lyrics: true,
            prefixes: vec!["<".to_owned(), "!!".to_owned()],
            authorities: vec![],
            modified: true,
        }
    }
}
