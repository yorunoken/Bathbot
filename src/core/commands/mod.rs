mod command;
mod data;
mod group;
mod handle_interaction;
mod handle_message;
pub mod parse;

pub use command::Command;
pub use data::{CommandData, CommandDataCompact};
pub use group::{CommandGroup, CommandGroups, CMD_GROUPS};
pub use handle_interaction::{handle_command, handle_component};
pub use handle_message::handle_message;
pub use parse::Invoke;

use crate::{core::buckets::BucketName, util::Authored, BotResult, Context, Error};

use std::fmt::{Display, Formatter, Result as FmtResult, Write};
use twilight_model::{
    guild::Permissions,
    id::{GuildId, RoleId, UserId},
};

#[derive(Debug)]
enum ProcessResult {
    Success,
    NoDM,
    NoSendPermission,
    Ratelimited(BucketName),
    NoOwner,
    NoAuthority,
}

impl ProcessResult {
    fn success(_: ()) -> Self {
        Self::Success
    }
}

impl Display for ProcessResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Ratelimited(bucket) => write!(f, "Ratelimited ({:?})", bucket),
            _ => write!(f, "{:?}", self),
        }
    }
}

// Is authority -> Ok(None)
// No authority -> Ok(Some(message to user))
// Couldn't figure out -> Err()
async fn check_authority(ctx: &Context, authored: &impl Authored) -> BotResult<Option<String>> {
    let author_id = authored.author().ok_or(Error::MissingInteractionAuthor)?.id;
    let guild_id = authored.guild_id();

    _check_authority(ctx, author_id, guild_id).await
}

async fn _check_authority(
    ctx: &Context,
    author_id: UserId,
    guild_id: Option<GuildId>,
) -> BotResult<Option<String>> {
    let (guild_id, permissions) = match guild_id {
        Some(id) => (id, ctx.cache.permissions().root(author_id, id)),
        None => return Ok(Some(String::new())),
    };

    let permissions = permissions.ok().unwrap_or_else(Permissions::empty);

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Ok(None);
    }

    let to_role = |role_id| RoleId::new(role_id).unwrap();
    let auth_roles = ctx.config_authorities_collect(guild_id, to_role).await;

    if auth_roles.is_empty() {
        let content = "You need admin permissions to use this command.\n\
            (`/authorities` to adjust authority status for this server)";

        return Ok(Some(content.to_owned()));
    } else if let Some(member) = ctx.cache.member(guild_id, author_id) {
        if !member.roles().iter().any(|role| auth_roles.contains(role)) {
            let mut content = String::from(
                "You need either admin permissions or \
                any of these roles to use this command:\n",
            );

            content.reserve(auth_roles.len() * 5);
            let mut roles = auth_roles.into_iter();

            if let Some(first) = roles.next() {
                let _ = write!(content, "<@&{}>", first);

                for role in roles {
                    let _ = write!(content, ", <@&{}>", role);
                }
            }

            content.push_str("\n(`/authorities` to adjust authority status for this server)");

            return Ok(Some(content));
        }
    } else {
        bail!("member {} not cached for guild {}", author_id, guild_id);
    }

    Ok(None)
}

async fn check_ratelimit(
    ctx: &Context,
    authored: &impl Authored,
    bucket: impl AsRef<str>,
) -> Option<(i64, BucketName)> {
    // * Note: Dangerous `?` if author is None and ratelimit should apply.
    // * Should be caught elsewhere though so this is likely fine
    let author_id = authored.author()?.id;
    let guild_id = authored.guild_id();

    _check_ratelimit(ctx, author_id, guild_id, bucket.as_ref().parse().unwrap()).await
}

async fn _check_ratelimit(
    ctx: &Context,
    author_id: UserId,
    guild_id: Option<GuildId>,
    bucket: BucketName,
) -> Option<(i64, BucketName)> {
    let (ratelimit, bucket) = {
        let guard = ctx.buckets.get(&bucket).unwrap();
        let mutex = guard.value();
        let mut bucket_elem = mutex.lock();

        match bucket {
            BucketName::Snipe => (bucket_elem.take(0), bucket), // same bucket for everyone
            BucketName::Songs => {
                let id = guild_id.map_or_else(|| author_id.get(), |guild_id| guild_id.get()); // same bucket for guilds

                (bucket_elem.take(id), bucket)
            }
            _ => (bucket_elem.take(author_id.get()), bucket),
        }
    };

    if ratelimit > 0 {
        return Some((ratelimit, bucket));
    }

    None
}
