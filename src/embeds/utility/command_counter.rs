use crate::{
    embeds::{Author, Footer},
    util::{constants::SYMBOLS, datetime::how_long_ago_text},
};

use chrono::{DateTime, Utc};
use std::fmt::Write;

pub struct CommandCounterEmbed {
    description: String,
    footer: Footer,
    author: Author,
}

impl CommandCounterEmbed {
    pub fn new(
        list: Vec<(&String, u32)>,
        booted_up: &DateTime<Utc>,
        idx: usize,
        pages: (usize, usize),
    ) -> Self {
        let len = list
            .iter()
            .fold(0, |max, (name, _)| max.max(name.chars().count()));

        let mut description = String::with_capacity(256);
        description.push_str("```\n");

        for (mut i, (name, amount)) in list.into_iter().enumerate() {
            i += idx;

            let _ = writeln!(
                description,
                "{:>2} {:1} # {:<len$} => {}",
                i,
                if i <= SYMBOLS.len() {
                    SYMBOLS[i - 1]
                } else {
                    ""
                },
                name,
                amount,
                len = len
            );
        }

        description.push_str("```");

        let footer_text = format!(
            "Page {}/{} ~ Started counting {}",
            pages.0,
            pages.1,
            how_long_ago_text(booted_up)
        );

        Self {
            description,
            footer: Footer::new(footer_text),
            author: Author::new("Most popular commands:"),
        }
    }
}

impl_builder!(CommandCounterEmbed {
    author,
    description,
    footer,
});
