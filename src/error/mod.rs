use plotters::drawing::DrawingAreaErrorKind;
use twilight_model::application::interaction::{ApplicationCommand, MessageComponentInteraction};
use twilight_validate::message::MessageValidationError;

pub use self::{
    bg_game::{BgGameError, InvalidBgState},
    graph::GraphError,
    help::InvalidHelpState,
    map_file::MapFileError,
    pp::PpError,
};

mod bg_game;
mod graph;
mod help;
mod map_file;
mod pp;

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => {
        return Err($crate::Error::Custom(format!("{}", format_args!($($arg)*))))
    };
}

// TODO: remove unused variants

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error while checking authority status")]
    Authority(#[source] Box<Error>),
    #[error("background game error")]
    BgGame(#[from] BgGameError),
    #[error("missing value in cache")]
    Cache(#[from] crate::core::CacheMiss),
    #[error("serde cbor error")]
    Cbor(#[from] serde_cbor::Error),
    #[error("error occured on cluster request")]
    ClusterCommand(#[from] twilight_gateway::cluster::ClusterCommandError),
    #[error("failed to start cluster")]
    ClusterStart(#[from] twilight_gateway::cluster::ClusterStartError),
    #[error("chrono parse error")]
    ChronoParse(#[from] chrono::format::ParseError),
    #[error("command error: {1}")]
    Command(#[source] Box<Error>, String),
    #[error("{0}")]
    Custom(String),
    #[error("custom client error")]
    CustomClient(#[from] crate::custom_client::CustomClientError),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("fmt error")]
    Fmt(#[from] std::fmt::Error),
    #[error("image error")]
    Image(#[from] image::ImageError),
    #[error("received invalid options for command")]
    InvalidCommandOptions,
    #[error("invalid bg state")]
    InvalidBgState(#[from] InvalidBgState),
    #[error("invalid help state")]
    InvalidHelpState(#[from] InvalidHelpState),
    #[error("io error")]
    Io(#[from] tokio::io::Error),
    #[error("error while preparing beatmap file")]
    MapFile(#[from] MapFileError),
    #[error("failed to validate message")]
    MessageValidation(#[from] MessageValidationError),
    #[error("missing env variable `{0}`")]
    MissingEnvVariable(&'static str),
    #[error("event was expected to contain member or user but contained neither")]
    MissingAuthor,
    #[error("osu error")]
    Osu(#[from] rosu_v2::error::OsuError),
    #[error("failed to parse env variable `{name}={value}`; expected {expected}")]
    ParsingEnvVariable {
        name: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error("received invalid options for command")]
    ParseSlashOptions(#[from] twilight_interactions::error::ParseError),
    #[error("error while calculating pp")]
    Pp(#[from] PpError),
    #[error("failed to send reaction after {0} retries")]
    ReactionRatelimit(usize),
    #[error("error while communicating with redis")]
    Redis(#[from] bb8_redis::redis::RedisError),
    #[error("serde json error")]
    Json(#[from] serde_json::Error),
    #[error("shard command error")]
    ShardCommand(#[from] twilight_gateway::shard::CommandError),
    #[error("twilight failed to deserialize response")]
    TwilightDeserialize(#[from] twilight_http::response::DeserializeBodyError),
    #[error("error while making discord request")]
    TwilightHttp(#[from] twilight_http::Error),
    #[error("unknown message component: {component:#?}")]
    UnknownMessageComponent {
        component: Box<MessageComponentInteraction>,
    },
    #[error("unexpected autocomplete for slash command `{0}`")]
    UnknownSlashAutocomplete(String),
    #[error("unknown slash command `{name}`: {command:#?}")]
    UnknownSlashCommand {
        name: String,
        command: Box<ApplicationCommand>,
    },
}

impl<E: std::error::Error + Send + Sync> From<DrawingAreaErrorKind<E>> for GraphError {
    fn from(err: DrawingAreaErrorKind<E>) -> Self {
        Self::Plotter(err.to_string())
    }
}
