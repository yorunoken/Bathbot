use crate::embeds::{EmbedData, EmbedFields};

use rosu_v2::model::GameMode;
use std::fmt::Write;

pub struct TrackEmbed {
    title: Option<String>,
    fields: EmbedFields,
}

impl TrackEmbed {
    pub fn new(
        mode: GameMode,
        success: Vec<String>,
        failure: Vec<String>,
        failed: Option<String>,
        limit: usize,
    ) -> Self {
        let title = format!("Top score tracking | mode={} | limit={}", mode, limit);
        let mut fields = EmbedFields::new();
        let mut iter = success.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);

            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }

            fields.push(("Now tracking:".to_owned(), value, false));
        }

        let mut iter = failure.iter();

        if let Some(first) = iter.next() {
            let names_len: usize = success.iter().map(|name| name.len() + 4).sum();
            let mut value = String::with_capacity(names_len);
            let _ = write!(value, "`{}`", first);

            for name in iter {
                let _ = write!(value, ", `{}`", name);
            }

            fields.push(("Already tracked:".to_owned(), value, false));
        }

        if let Some(failed) = failed {
            fields.push((
                "Failed to track:".to_owned(),
                format!("`{}`", failed),
                false,
            ));
        }

        Self {
            title: Some(title),
            fields,
        }
    }
}

impl EmbedData for TrackEmbed {
    fn title_owned(&mut self) -> Option<String> {
        self.title.take()
    }
    fn fields_owned(self) -> Option<EmbedFields> {
        Some(self.fields)
    }
}
