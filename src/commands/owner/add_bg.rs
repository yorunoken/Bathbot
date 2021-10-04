use crate::{BotResult, CONFIG, CommandData, Context, MessageBuilder, util::{CowUtils, MessageExt, constants::{GENERAL_ISSUE, OSU_API_ISSUE, OSU_BASE, common_literals::{MANIA, OSU}}}};

use rosu_v2::prelude::{BeatmapsetCompact, GameMode, OsuError};
use std::{str::FromStr, sync::Arc};
use tokio::{
    fs::{remove_file, File},
    io::AsyncWriteExt,
};
use twilight_model::channel::Attachment;

#[command]
#[short_desc("Add background for the background game")]
#[aliases("bgadd")]
#[owner()]
async fn addbg(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    let (msg, mut args) = match data {
        CommandData::Message { msg, args, .. } => (msg, args),
        CommandData::Interaction { .. } => unreachable!(),
    };

    // Check if msg has an attachement
    let attachment = match msg.attachments.first() {
        Some(attachment) => attachment.to_owned(),
        None => {
            let content = "You must attach an image to the command that has the mapset id as name";

            return msg.error(&ctx, content).await;
        }
    };

    // Parse arguments as mode
    let mode = match args.next() {
        Some(arg) => match arg.cow_to_ascii_lowercase().as_ref() {
            "mna" | MANIA | "m" => GameMode::MNA,
            OSU | "std" | "standard" | "o" => GameMode::STD,
            _ => {
                let content = "Failed to parse first argument as mode. \
                    Provide either `mna`, or `std`";
                return msg.error(&ctx, content).await;
            }
        },
        None => GameMode::STD,
    };

    // Check if attachement as proper name
    let mut filename_split = attachment.filename.split('.');

    let mapset_id = match filename_split.next().map(u32::from_str) {
        Some(Ok(id)) => id,
        None | Some(Err(_)) => {
            let content = "Provided image has no appropriate name. \
                Be sure to let the name be the mapset id, e.g. 948199.png";

            return msg.error(&ctx, content).await;
        }
    };

    // Check if attachement has proper file type
    let valid_filetype_opt = filename_split
        .next()
        .filter(|&filetype| filetype == "jpg" || filetype == "jpeg" || filetype == "png");

    if valid_filetype_opt.is_none() {
        let content = "Provided image has no appropriate file type. \
            It must be either `.jpg`, `.jpeg`, or `.png`";

        return msg.error(&ctx, content).await;
    }

    // Download attachement
    let path = match download_attachment(&attachment).await {
        Ok(content) => {
            let mut path = CONFIG.get().unwrap().bg_path.clone();

            match mode {
                GameMode::STD => path.push(OSU),
                GameMode::MNA => path.push(MANIA),
                GameMode::TKO | GameMode::CTB => unreachable!(),
            }

            path.push(&attachment.filename);

            // Create file
            let mut file = match File::create(&path).await {
                Ok(file) => file,
                Err(why) => {
                    let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                    return Err(why.into());
                }
            };

            // Store in file
            if let Err(why) = file.write_all(&content).await {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
            path
        }
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    // Check if valid mapset id
    let content = match prepare_mapset(&ctx, mapset_id, &attachment.filename, mode).await {
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
    msg.create_message(&ctx, builder).await?;

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
                if let Err(why) = ctx.psql().insert_beatmapset(&mapset).await {
                    unwind_error!(warn, why, "Failed to insert mapset in DB: {}");
                }

                mapset.into()
            }
            Err(OsuError::NotFound) => {
                return Err("No mapset found with the name of the given file as id")
            }
            Err(why) => {
                unwind_error!(error, why, "Failed to request mapset: {}");

                return Err(OSU_API_ISSUE);
            }
        },
    };

    if let Err(why) = ctx.psql().add_tag_mapset(mapset_id, filename, mode).await {
        unwind_error!(warn, why, "Error while adding mapset to tags table: {}");

        return Err("There is already an entry with this mapset id");
    }

    Ok(mapset)
}

async fn download_attachment(attachment: &Attachment) -> BotResult<Vec<u8>> {
    Ok(reqwest::get(&attachment.url).await?.bytes().await?.to_vec())
}
