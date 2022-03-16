use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    iter::{Copied, Map},
    path::PathBuf,
    slice::Iter,
};

use chrono::{DateTime, Utc};
use eyre::Report;
use futures::{stream::FuturesUnordered, TryFutureExt, TryStreamExt};
use hashbrown::HashMap;
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods, Grade, Score, UserStatistics};
use tokio::{fs::File, io::AsyncWriteExt};
use twilight_model::channel::{embed::Embed, Message};

use crate::{
    core::Context,
    custom_client::OsuTrackerCountryScore,
    error::MapFileError,
    pp::PpCalculator,
    util::{constants::OSU_BASE, matcher, numbers::round, BeatmapExt, Emote, ScoreExt},
    CONFIG,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ModSelection {
    Include(GameMods),
    Exclude(GameMods),
    Exact(GameMods),
}

impl ModSelection {
    pub fn mods(&self) -> GameMods {
        match self {
            Self::Include(m) | Self::Exclude(m) | Self::Exact(m) => *m,
        }
    }
}

pub fn flag_url(country_code: &str) -> String {
    // format!("{}/images/flags/{}.png", OSU_BASE, country_code) // from osu itself but outdated
    format!("https://osuflags.omkserver.nl/{country_code}-256.png") // kelderman
}

#[allow(dead_code)]
pub fn flag_url_svg(country_code: &str) -> String {
    assert_eq!(
        country_code.len(),
        2,
        "country code `{country_code}` is invalid",
    );

    const OFFSET: u32 = 0x1F1A5;
    let bytes = country_code.as_bytes();

    let url = format!(
        "{OSU_BASE}assets/images/flags/{:x}-{:x}.svg",
        bytes[0].to_ascii_uppercase() as u32 + OFFSET,
        bytes[1].to_ascii_uppercase() as u32 + OFFSET
    );

    url
}

pub fn grade_emote(grade: Grade) -> &'static str {
    CONFIG.get().unwrap().grade(grade)
}

pub fn mode_emote(mode: GameMode) -> Cow<'static, str> {
    let emote = match mode {
        GameMode::STD => Emote::Std,
        GameMode::TKO => Emote::Tko,
        GameMode::CTB => Emote::Ctb,
        GameMode::MNA => Emote::Mna,
    };

    emote.text()
}

pub fn grade_completion_mods(score: &dyn ScoreExt, map: &Beatmap) -> Cow<'static, str> {
    let mode = map.mode();
    let grade = CONFIG.get().unwrap().grade(score.grade(mode));
    let mods = score.mods();

    match (
        mods.is_empty(),
        score.grade(mode) == Grade::F && mode != GameMode::CTB,
    ) {
        (true, true) => format!("{grade} ({}%)", completion(score, map)).into(),
        (false, true) => format!("{grade} ({}%) +{mods}", completion(score, map)).into(),
        (true, false) => grade.into(),
        (false, false) => format!("{grade} +{mods}").into(),
    }
}

fn completion(score: &dyn ScoreExt, map: &Beatmap) -> u32 {
    let passed = score.hits(map.mode() as u8);
    let total = map.count_objects();

    100 * passed / total
}

pub async fn prepare_beatmap_file(ctx: &Context, map_id: u32) -> Result<PathBuf, MapFileError> {
    let mut map_path = CONFIG.get().unwrap().paths.maps.clone();
    map_path.push(format!("{map_id}.osu"));

    if !map_path.exists() {
        let bytes = ctx.clients.custom.get_map_file(map_id).await?;
        let mut file = File::create(&map_path).await?;
        file.write_all(&bytes).await?;
        info!("Downloaded {map_id}.osu successfully");
    }

    Ok(map_path)
}

pub trait IntoPpIter {
    type Inner: Iterator<Item = f32> + DoubleEndedIterator + ExactSizeIterator;

    fn into_pps(self) -> PpIter<Self::Inner>;
}

impl<'s> IntoPpIter for &'s [Score] {
    type Inner = Map<Iter<'s, Score>, fn(&Score) -> f32>;

    #[inline]
    fn into_pps(self) -> PpIter<Self::Inner> {
        PpIter {
            inner: self.iter().map(|score| score.pp.unwrap_or(0.0)),
        }
    }
}

impl<'f> IntoPpIter for &'f [f32] {
    type Inner = Copied<Iter<'f, f32>>;

    #[inline]
    fn into_pps(self) -> PpIter<Self::Inner> {
        PpIter {
            inner: self.iter().copied(),
        }
    }
}

pub struct PpIter<I> {
    inner: I,
}

impl<I: Iterator<Item = f32>> Iterator for PpIter<I> {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<I: Iterator<Item = f32> + DoubleEndedIterator> DoubleEndedIterator for PpIter<I> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<I: Iterator<Item = f32> + ExactSizeIterator> ExactSizeIterator for PpIter<I> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// First element: Weighted missing pp to reach goal from start
///
/// Second element: Index of hypothetical pp in pps
pub fn pp_missing(start: f32, goal: f32, pps: impl IntoPpIter) -> (f32, usize) {
    let pps = pps.into_pps();

    let mut top = start;
    let mut bot = 0.0;

    //     top + x * 0.95^i + bot = goal
    // <=> x = (goal - top - bot) / 0.95^i
    fn calculate_remaining(idx: usize, goal: f32, top: f32, bot: f32) -> (f32, usize) {
        let factor = 0.95_f32.powi(idx as i32);
        let required = (goal - top - bot) / factor;

        (required, idx)
    }

    for (i, last_pp) in pps.enumerate().rev() {
        let factor = 0.95_f32.powi(i as i32);
        let term = factor * last_pp;
        let bot_term = term * 0.95;

        if top + bot + bot_term >= goal {
            return calculate_remaining(i + 1, goal, top, bot);
        }

        bot += bot_term;
        top -= term;
    }

    calculate_remaining(0, goal, top, bot)
}

pub fn map_id_from_history(msgs: &[Message]) -> Option<MapIdType> {
    msgs.iter().find_map(map_id_from_msg)
}

pub fn map_id_from_msg(msg: &Message) -> Option<MapIdType> {
    if msg.content.chars().all(|c| c.is_numeric()) {
        return check_embeds_for_map_id(&msg.embeds);
    }

    matcher::get_osu_map_id(&msg.content)
        .or_else(|| matcher::get_osu_mapset_id(&msg.content))
        .or_else(|| check_embeds_for_map_id(&msg.embeds))
}

fn check_embeds_for_map_id(embeds: &[Embed]) -> Option<MapIdType> {
    embeds.iter().find_map(|embed| {
        let url = embed
            .author
            .as_ref()
            .and_then(|author| author.url.as_deref());

        url.and_then(matcher::get_osu_map_id)
            .or_else(|| url.and_then(matcher::get_osu_mapset_id))
            .or_else(|| embed.url.as_deref().and_then(matcher::get_osu_map_id))
            .or_else(|| embed.url.as_deref().and_then(matcher::get_osu_mapset_id))
    })
}

#[derive(Copy, Clone, Debug)]
pub enum MapIdType {
    Map(u32),
    Set(u32),
}

// Credits to https://github.com/RoanH/osu-BonusPP/blob/master/BonusPP/src/me/roan/bonuspp/BonusPP.java#L202
pub struct BonusPP {
    pp: f32,
    ys: [f32; 100],
    len: usize,

    sum_x: f32,
    avg_x: f32,
    avg_y: f32,
}

impl BonusPP {
    const MAX: f32 = 416.67;

    pub fn new() -> Self {
        Self {
            pp: 0.0,
            ys: [0.0; 100],
            len: 0,

            sum_x: 0.0,
            avg_x: 0.0,
            avg_y: 0.0,
        }
    }

    pub fn update(&mut self, weighted_pp: f32, idx: usize) {
        self.pp += weighted_pp;
        self.ys[idx] = weighted_pp.log(100.0);
        self.len += 1;

        let n = idx as f32 + 1.0;
        let weight = n.ln_1p();

        self.sum_x += weight;
        self.avg_x += n * weight;
        self.avg_y += self.ys[idx] * weight;
    }

    pub fn calculate(self, stats: &UserStatistics) -> f32 {
        let BonusPP {
            mut pp,
            len,
            ys,
            sum_x,
            mut avg_x,
            mut avg_y,
        } = self;

        if stats.pp.abs() < f32::EPSILON {
            let counts = &stats.grade_counts;
            let sum = counts.ssh + counts.ss + counts.sh + counts.s + counts.a;

            return round(Self::MAX * (1.0 - 0.9994_f32.powi(sum)));
        } else if self.len < 100 {
            return round(stats.pp - pp);
        }

        avg_x /= sum_x;
        avg_y /= sum_x;

        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        for n in 1..=len {
            let diff_x = n as f32 - avg_x;
            let ln_n = (n as f32).ln_1p();

            sum_xy += diff_x * (ys[n - 1] - avg_y) * ln_n;
            sum_x2 += diff_x * diff_x * ln_n;
        }

        let xy = sum_xy / sum_x;
        let x2 = sum_x2 / sum_x;

        let m = xy / x2;
        let b = avg_y - (xy / x2) * avg_x;

        for n in 100..=stats.playcount {
            let val = 100.0_f32.powf(m * n as f32 + b);

            if val <= 0.0 {
                break;
            }

            pp += val;
        }

        round(stats.pp - pp).clamp(0.0, Self::MAX)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ScoreOrder {
    Acc,
    Bpm,
    Combo,
    Date,
    Length,
    Misses,
    Pp,
    RankedDate,
    Score,
    Stars,
}

impl Default for ScoreOrder {
    fn default() -> Self {
        Self::Pp
    }
}

impl ScoreOrder {
    pub async fn apply<S: SortableScore>(self, ctx: &Context, scores: &mut [S]) {
        fn clock_rate(mods: GameMods) -> f32 {
            if mods.contains(GameMods::DoubleTime) {
                1.5
            } else if mods.contains(GameMods::HalfTime) {
                0.75
            } else {
                1.0
            }
        }

        match self {
            Self::Acc => {
                scores.sort_unstable_by(|a, b| {
                    b.acc().partial_cmp(&a.acc()).unwrap_or(Ordering::Equal)
                });
            }
            Self::Bpm => scores.sort_unstable_by(|a, b| {
                let a_bpm = a.bpm() * clock_rate(a.mods());
                let b_bpm = b.bpm() * clock_rate(b.mods());

                b_bpm.partial_cmp(&a_bpm).unwrap_or(Ordering::Equal)
            }),
            Self::Combo => scores.sort_unstable_by_key(|s| Reverse(s.max_combo())),
            Self::Date => scores.sort_unstable_by_key(|s| Reverse(s.created_at())),
            Self::Length => scores.sort_unstable_by(|a, b| {
                let a_len = a.seconds_drain() as f32 / clock_rate(a.mods());
                let b_len = b.seconds_drain() as f32 / clock_rate(b.mods());

                b_len.partial_cmp(&a_len).unwrap_or(Ordering::Equal)
            }),
            Self::Misses => scores.sort_unstable_by(|a, b| {
                b.n_misses().cmp(&a.n_misses()).then_with(|| {
                    let hits_a = a.total_hits_sort();
                    let hits_b = b.total_hits_sort();

                    let ratio_a = a.n_misses() as f32 / hits_a as f32;
                    let ratio_b = b.n_misses() as f32 / hits_b as f32;

                    ratio_b
                        .partial_cmp(&ratio_a)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| hits_b.cmp(&hits_a))
                })
            }),
            Self::Pp => scores
                .sort_unstable_by(|a, b| b.pp().partial_cmp(&a.pp()).unwrap_or(Ordering::Equal)),
            Self::RankedDate => {
                let mut mapsets = HashMap::new();
                let mut new_mapsets = HashMap::new();

                for score in scores.iter() {
                    let mapset_id = score.mapset_id();

                    match ctx.psql().get_beatmapset::<Beatmapset>(mapset_id).await {
                        Ok(Beatmapset {
                            ranked_date: Some(date),
                            ..
                        }) => {
                            mapsets.insert(mapset_id, date);
                        }
                        Ok(_) => {
                            warn!("Missing ranked date for top score DB mapset {mapset_id}");

                            continue;
                        }
                        Err(err) => {
                            let report = Report::new(err).wrap_err("failed to get mapset");
                            warn!("{report:?}");

                            match ctx.osu().beatmapset(mapset_id).await {
                                Ok(mapset) => {
                                    new_mapsets.insert(mapset_id, mapset);
                                }
                                Err(err) => {
                                    let report =
                                        Report::new(err).wrap_err("failed to request mapset");
                                    warn!("{report:?}");

                                    continue;
                                }
                            }
                        }
                    };
                }

                if !new_mapsets.is_empty() {
                    let result: Result<(), _> = new_mapsets
                        .values()
                        .map(|mapset| ctx.psql().insert_beatmapset(mapset).map_ok(|_| ()))
                        .collect::<FuturesUnordered<_>>()
                        .try_collect()
                        .await;

                    if let Err(err) = result {
                        let report = Report::new(err).wrap_err("failed to insert mapsets");
                        warn!("{report:?}");
                    } else {
                        info!("Inserted {} mapsets into the DB", new_mapsets.len());
                    }

                    let iter = new_mapsets
                        .into_iter()
                        .filter_map(|(id, mapset)| Some((id, mapset.ranked_date?)));

                    mapsets.extend(iter);
                }

                scores.sort_unstable_by(|a, b| {
                    let mapset_a = a.mapset_id();
                    let mapset_b = b.mapset_id();

                    let date_a = mapsets.get(&mapset_a).copied().unwrap_or_else(Utc::now);
                    let date_b = mapsets.get(&mapset_b).copied().unwrap_or_else(Utc::now);

                    date_a.cmp(&date_b)
                })
            }
            Self::Score => scores.sort_unstable_by_key(|score| Reverse(score.score())),
            Self::Stars => {
                let mut stars = HashMap::new();

                for score in scores.iter() {
                    let score_id = score.score_id();
                    let map_id = score.map_id();

                    if !score.mods().changes_stars(score.mode()) {
                        stars.insert(score_id, score.stars());

                        continue;
                    }

                    let stars_ = match PpCalculator::new(ctx, map_id).await {
                        Ok(mut calc) => calc.mods(score.mods()).stars() as f32,
                        Err(err) => {
                            warn!("{:?}", Report::new(err));

                            continue;
                        }
                    };

                    stars.insert(score_id, stars_);
                }

                scores.sort_unstable_by(|a, b| {
                    let stars_a = stars.get(&a.score_id()).unwrap_or(&0.0);
                    let stars_b = stars.get(&b.score_id()).unwrap_or(&0.0);

                    stars_b.partial_cmp(stars_a).unwrap_or(Ordering::Equal)
                })
            }
        }
    }
}

pub trait SortableScore {
    fn acc(&self) -> f32;
    fn bpm(&self) -> f32;
    fn created_at(&self) -> DateTime<Utc>;
    fn map_id(&self) -> u32;
    fn mapset_id(&self) -> u32;
    fn max_combo(&self) -> u32;
    fn mode(&self) -> GameMode;
    fn mods(&self) -> GameMods;
    fn n_misses(&self) -> u32;
    fn pp(&self) -> Option<f32>;
    fn score(&self) -> u32;
    fn score_id(&self) -> u64;
    fn seconds_drain(&self) -> u32;
    fn stars(&self) -> f32;
    fn total_hits_sort(&self) -> u32;
}

impl SortableScore for Score {
    fn acc(&self) -> f32 {
        self.accuracy
    }

    fn bpm(&self) -> f32 {
        self.map.as_ref().map_or(0.0, |map| map.bpm)
    }

    fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    fn map_id(&self) -> u32 {
        self.map.as_ref().map_or(0, |map| map.map_id)
    }

    fn mapset_id(&self) -> u32 {
        self.mapset.as_ref().map_or(0, |mapset| mapset.mapset_id)
    }

    fn max_combo(&self) -> u32 {
        self.max_combo
    }

    fn mode(&self) -> GameMode {
        self.mode
    }

    fn mods(&self) -> GameMods {
        self.mods
    }

    fn n_misses(&self) -> u32 {
        self.statistics.count_miss
    }

    fn pp(&self) -> Option<f32> {
        self.pp
    }

    fn score(&self) -> u32 {
        self.score
    }

    fn score_id(&self) -> u64 {
        self.score_id
    }

    fn seconds_drain(&self) -> u32 {
        self.map.as_ref().map_or(0, |map| map.seconds_drain)
    }

    fn stars(&self) -> f32 {
        self.map.as_ref().map_or(0.0, |map| map.stars)
    }

    fn total_hits_sort(&self) -> u32 {
        self.total_hits()
    }
}

macro_rules! impl_sortable_score_tuple {
    (($($ty:ty),*) => $idx:tt) => {
        impl SortableScore for ($($ty),*) {
            fn acc(&self) -> f32 {
                SortableScore::acc(&self.$idx)
            }

            fn bpm(&self) -> f32 {
                SortableScore::bpm(&self.$idx)
            }

            fn created_at(&self) -> DateTime<Utc> {
                SortableScore::created_at(&self.$idx)
            }

            fn map_id(&self) -> u32 {
                SortableScore::map_id(&self.$idx)
            }

            fn mapset_id(&self) -> u32 {
                SortableScore::mapset_id(&self.$idx)
            }

            fn max_combo(&self) -> u32 {
                SortableScore::max_combo(&self.$idx)
            }

            fn mode(&self) -> GameMode {
                SortableScore::mode(&self.$idx)
            }

            fn mods(&self) -> GameMods {
                SortableScore::mods(&self.$idx)
            }

            fn n_misses(&self) -> u32 {
                SortableScore::n_misses(&self.$idx)
            }

            fn pp(&self) -> Option<f32> {
                SortableScore::pp(&self.$idx)
            }

            fn score(&self) -> u32 {
                SortableScore::score(&self.$idx)
            }

            fn score_id(&self) -> u64 {
                SortableScore::score_id(&self.$idx)
            }

            fn seconds_drain(&self) -> u32 {
                SortableScore::seconds_drain(&self.$idx)
            }

            fn stars(&self) -> f32 {
                SortableScore::stars(&self.1)
            }

            fn total_hits_sort(&self) -> u32 {
                SortableScore::total_hits_sort(&self.$idx)
            }
        }
    };
}

impl_sortable_score_tuple!((usize, Score) => 1);
impl_sortable_score_tuple!((usize, Score, Option<f32>) => 1);

impl SortableScore for (OsuTrackerCountryScore, usize) {
    fn acc(&self) -> f32 {
        self.0.acc
    }

    fn bpm(&self) -> f32 {
        panic!("can't sort by bpm")
    }

    fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }

    fn map_id(&self) -> u32 {
        self.0.map_id
    }

    fn mapset_id(&self) -> u32 {
        self.0.mapset_id
    }

    fn max_combo(&self) -> u32 {
        panic!("can't sort by combo")
    }

    fn mode(&self) -> GameMode {
        GameMode::STD
    }

    fn mods(&self) -> GameMods {
        self.0.mods
    }

    fn n_misses(&self) -> u32 {
        self.0.n_misses
    }

    fn pp(&self) -> Option<f32> {
        Some(self.0.pp)
    }

    fn score(&self) -> u32 {
        panic!("can't sort by score")
    }

    fn score_id(&self) -> u64 {
        panic!("can't sort with score id")
    }

    fn seconds_drain(&self) -> u32 {
        self.0.seconds_total
    }

    fn stars(&self) -> f32 {
        panic!("can't sort by stars")
    }

    fn total_hits_sort(&self) -> u32 {
        self.0.n_misses + 1
    }
}
