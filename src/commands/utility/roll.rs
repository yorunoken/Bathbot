use crate::{
    commands::{MyCommand, MyCommandOption},
    util::{ApplicationCommandExt, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use rand::Rng;
use std::sync::Arc;
use twilight_model::application::interaction::{
    application_command::CommandDataOption, ApplicationCommand,
};

const DEFAULT_LIMIT: u64 = 100;

#[command]
#[short_desc("Get a random number")]
#[long_desc(
    "Get a random number.\n\
    If no upper limit is specified, it defaults to 100."
)]
#[usage("[upper limit]")]
#[no_typing()]
async fn roll(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let limit = match num {
                Some(n) => n as u64,
                None => match args.next().map(|arg| arg.parse()) {
                    Some(Ok(n)) => n,
                    None | Some(Err(_)) => DEFAULT_LIMIT,
                },
            };

            _roll(ctx, CommandData::Message { msg, args, num }, limit).await
        }
        CommandData::Interaction { command } => slash_roll(ctx, *command).await,
    }
}

async fn _roll(ctx: Arc<Context>, data: CommandData<'_>, limit: u64) -> BotResult<()> {
    let num = rand::thread_rng().gen_range(1..(limit + 1).max(2));

    let author_id = data.author()?.id;

    let description = format!(
        "<@{}> rolls {} point{} :game_die:",
        author_id,
        num,
        if num == 1 { "" } else { "s" }
    );

    let builder = MessageBuilder::new().embed(description);
    data.create_message(&ctx, builder).await?;

    Ok(())
}

pub async fn slash_roll(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let mut limit = None;

    for option in command.yoink_options() {
        match option {
            CommandDataOption::String { name, .. } => bail_cmd_option!("roll", string, name),
            CommandDataOption::Integer { name, value } => match name.as_str() {
                "limit" => limit = Some(value.max(0) as u64),
                _ => bail_cmd_option!("roll", integer, name),
            },
            CommandDataOption::Boolean { name, .. } => bail_cmd_option!("roll", boolean, name),
            CommandDataOption::SubCommand { name, .. } => {
                bail_cmd_option!("roll", subcommand, name)
            }
        }
    }

    _roll(ctx, command.into(), limit.unwrap_or(DEFAULT_LIMIT)).await
}

pub fn define_roll() -> MyCommand {
    let limit = MyCommandOption::builder("limit", "Specify an upper limit, defaults to 100")
        .integer(Vec::new(), false);

    MyCommand::new("roll", "Roll a random number").options(vec![limit])
}
