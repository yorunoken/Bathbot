use super::{
    constants::OSU_BASE,
    osu::{MapIdType, ModSelection},
};

use regex::Regex;
use rosu_v2::model::GameMods;
use std::{borrow::Cow, str::FromStr};

pub fn is_custom_emote(msg: &str) -> bool {
    EMOJI_MATCHER.is_match(msg)
}

enum MentionType {
    Channel,
    Role,
    User,
}

pub fn get_mention_channel(msg: &str) -> Option<u64> {
    get_mention(MentionType::Channel, msg)
}

pub fn get_mention_role(msg: &str) -> Option<u64> {
    get_mention(MentionType::Role, msg)
}

pub fn get_mention_user(msg: &str) -> Option<u64> {
    get_mention(MentionType::User, msg)
}

fn get_mention(mention_type: MentionType, msg: &str) -> Option<u64> {
    if let Ok(id) = msg.parse() {
        return Some(id);
    }

    let captures = match mention_type {
        MentionType::Channel => CHANNEL_ID_MATCHER.captures(msg),
        MentionType::Role => ROLE_ID_MATCHER.captures(msg),
        MentionType::User => MENTION_MATCHER.captures(msg),
    };

    captures
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse().ok())
}

pub fn get_osu_user_id(msg: &str) -> Option<u32> {
    OSU_URL_USER_MATCHER
        .captures(msg)
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u32>().ok())
}

pub fn get_osu_map_id(msg: &str) -> Option<MapIdType> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(MapIdType::Map(id));
    }

    if !msg.contains(OSU_BASE) {
        return None;
    }

    let matcher = if let Some(c) = OSU_URL_MAP_OLD_MATCHER.captures(msg) {
        c.get(1)
    } else {
        OSU_URL_MAP_NEW_MATCHER.captures(msg).and_then(|c| c.get(2))
    };

    matcher.and_then(|c| c.as_str().parse::<u32>().ok().map(MapIdType::Map))
}

pub fn get_osu_mapset_id(msg: &str) -> Option<MapIdType> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(MapIdType::Set(id));
    }

    if !msg.contains(OSU_BASE) {
        return None;
    }

    OSU_URL_MAPSET_OLD_MATCHER
        .captures(msg)
        .or_else(|| OSU_URL_MAP_NEW_MATCHER.captures(msg))
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u32>().ok())
        .map(MapIdType::Set)
}

pub fn get_osu_match_id(msg: &str) -> Option<u32> {
    if let Ok(id) = msg.parse::<u32>() {
        return Some(id);
    }

    OSU_URL_MATCH_MATCHER
        .captures(msg)
        .and_then(|c| c.get(1))
        .and_then(|c| c.as_str().parse::<u32>().ok())
}

pub fn get_mods(msg: &str) -> Option<ModSelection> {
    let selection = if let Some(captures) = MOD_PLUS_MATCHER.captures(msg) {
        let mods = GameMods::from_str(captures.get(1)?.as_str()).ok()?;

        if msg.ends_with('!') {
            ModSelection::Exact(mods)
        } else {
            ModSelection::Include(mods)
        }
    } else if let Some(captures) = MOD_MINUS_MATCHER.captures(msg) {
        let mods = GameMods::from_str(captures.get(1)?.as_str()).ok()?;

        ModSelection::Exclude(mods)
    } else {
        return None;
    };

    Some(selection)
}

#[allow(dead_code)]
pub fn is_hit_results(msg: &str) -> bool {
    HIT_RESULTS_MATCHER.is_match(msg)
}

pub fn is_guest_diff(msg: &str) -> bool {
    OSU_DIFF_MATCHER.is_match(msg)
}

pub fn tourney_badge(description: &str) -> bool {
    !IGNORE_BADGE_MATCHER.is_match_at(description, 0)
}

pub fn highlight_funny_numeral(content: &str) -> Cow<str> {
    SEVEN_TWO_SEVEN.replace_all(content, "__${num}__")
}

lazy_static! {
    static ref ROLE_ID_MATCHER: Regex = Regex::new(r"<@&(\d+)>").unwrap();

    static ref CHANNEL_ID_MATCHER: Regex = Regex::new(r"<#(\d+)>").unwrap();

    static ref MENTION_MATCHER: Regex = Regex::new(r"<@!?(\d+)>").unwrap();

    static ref OSU_URL_USER_MATCHER: Regex = Regex::new(r"https://osu.ppy.sh/users/(\d+)").unwrap();

    static ref OSU_URL_MAP_NEW_MATCHER: Regex = Regex::new(
        r"https://osu.ppy.sh/beatmapsets/(\d+)(?:(?:#(?:osu|mania|taiko|fruits)|<#\d+>)/(\d+))?"
    )
    .unwrap();

    static ref OSU_URL_MAP_OLD_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/b(?:eatmaps)?/(\d+)").unwrap();
    static ref OSU_URL_MAPSET_OLD_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/s/(\d+)").unwrap();

    static ref OSU_URL_MATCH_MATCHER: Regex =
        Regex::new(r"https://osu.ppy.sh/(?:community/matches|mp)/(\d+)").unwrap();

    static ref MOD_PLUS_MATCHER: Regex = Regex::new(r"^\+(\w+)!?$").unwrap();
    static ref MOD_MINUS_MATCHER: Regex = Regex::new(r"^-(\w+)!$").unwrap();

    static ref HIT_RESULTS_MATCHER: Regex = Regex::new(r".*\{(\d+/){2,}\d+}.*").unwrap();

    static ref OSU_DIFF_MATCHER: Regex =
        Regex::new(".*'s? (easy|normal|hard|insane|expert|extra|extreme|emotions|repetition)")
            .unwrap();

    static ref EMOJI_MATCHER: Regex = Regex::new(r"<(a?):([^:\n]+):(\d+)>").unwrap();

    static ref IGNORE_BADGE_MATCHER: Regex = Regex::new(r"^((?i)contrib|nomination|assessment|global|moderation|beatmap|spotlight|map|pending|aspire|elite|monthly|exemplary|outstanding|longstanding|idol[^@]+)").unwrap();

    static ref SEVEN_TWO_SEVEN: Regex = Regex::new("(?P<num>7[.,]?2[.,]?7)").unwrap();
}
