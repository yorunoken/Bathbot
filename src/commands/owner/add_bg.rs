use std::{str::FromStr, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::{BeatmapsetCompact, GameMode, OsuError};
use tokio::{
    fs::{remove_file, File},
    io::AsyncWriteExt,
};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    util::{
        builder::MessageBuilder,
        constants::{
            common_literals::{MANIA, OSU},
            GENERAL_ISSUE, OSU_API_ISSUE, OSU_BASE,
        },
    },
    BotResult, Context, CONFIG,
};

use super::OwnerAddBg;

pub async fn addbg(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    bg: OwnerAddBg,
) -> BotResult<()> {
    let OwnerAddBg { image, mode } = bg;

    let mode = mode.map_or(GameMode::STD, GameMode::from);

    // Check if attachement as proper name
    let mut filename_split = image.filename.split('.');

    let mapset_id = match filename_split.next().map(u32::from_str) {
        Some(Ok(id)) => id,
        None | Some(Err(_)) => {
            let content = "Provided image has no appropriate name. \
                Be sure to let the name be the mapset id, e.g. 948199.png";

            return command.error(&ctx, content).await;
        }
    };

    // Check if attachement has proper file type
    let valid_filetype_opt = filename_split
        .next()
        .filter(|&filetype| filetype == "jpg" || filetype == "png");

    if valid_filetype_opt.is_none() {
        let content = "Provided image has inappropriate type. Must be either `.jpg` or `.png`";

        return command.error(&ctx, content).await;
    }

    // Download attachement
    let path = match ctx.clients.custom.get_discord_attachment(&image).await {
        Ok(content) => {
            let mut path = CONFIG.get().unwrap().paths.backgrounds.clone();

            match mode {
                GameMode::STD => path.push(OSU),
                GameMode::MNA => path.push(MANIA),
                GameMode::TKO | GameMode::CTB => unreachable!(),
            }

            path.push(&image.filename);

            // Create file
            let mut file = match File::create(&path).await {
                Ok(file) => file,
                Err(why) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;

                    return Err(why.into());
                }
            };

            // Store in file
            if let Err(why) = file.write_all(&content).await {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
            path
        }
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    // Check if valid mapset id
    let content = match prepare_mapset(&ctx, mapset_id, &image.filename, mode).await {
        Ok(mapset) => format!(
            "Background for [{artist} - {title}]({base}s/{id}) successfully added ({mode})",
            artist = mapset.artist,
            title = mapset.title,
            base = OSU_BASE,
            id = mapset_id,
            mode = mode
        ),
        Err(err_msg) => {
            let _ = remove_file(path).await;

            err_msg.to_owned()
        }
    };

    let builder = MessageBuilder::new().embed(content);
    command.callback(&ctx, builder).await?;

    Ok(())
}

async fn prepare_mapset(
    ctx: &Context,
    mapset_id: u32,
    filename: &str,
    mode: GameMode,
) -> Result<BeatmapsetCompact, &'static str> {
    let db_fut = ctx.psql().get_beatmapset::<BeatmapsetCompact>(mapset_id);

    let mapset = match db_fut.await {
        Ok(mapset) => mapset,
        Err(_) => match ctx.osu().beatmapset(mapset_id).await {
            Ok(mapset) => {
                if let Err(err) = ctx.psql().insert_beatmapset(&mapset).await {
                    warn!("{:?}", Report::new(err));
                }

                mapset.into()
            }
            Err(OsuError::NotFound) => {
                return Err("No mapset found with the name of the given file as id")
            }
            Err(why) => {
                let report = Report::new(why).wrap_err("failed to request mapset");
                error!("{:?}", report);

                return Err(OSU_API_ISSUE);
            }
        },
    };

    if let Err(why) = ctx.psql().add_tag_mapset(mapset_id, filename, mode).await {
        let report = Report::new(why).wrap_err("error while adding mapset to tags table");
        warn!("{:?}", report);

        return Err("There is already an entry with this mapset id");
    }

    Ok(mapset)
}
