#[macro_use]
extern crate eyre;

mod builder;
mod cow;
mod exp_backoff;
mod ext;
mod hasher;
mod html_to_png;
mod matrix;

pub mod constants;
pub mod datetime;
pub mod matcher;
pub mod numbers;
pub mod osu;
pub mod string_cmp;

pub use self::{
    builder::{modal, AuthorBuilder, EmbedBuilder, FooterBuilder, MessageBuilder},
    cow::CowUtils,
    exp_backoff::ExponentialBackoff,
    ext::*,
    hasher::{IntHash, IntHasher},
    html_to_png::HtmlToPng,
    matrix::Matrix,
};
