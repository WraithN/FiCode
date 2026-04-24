pub trait SlashCommandHandler {}

pub struct SlashCommand;

pub enum SlashCommandResult {}

pub fn parse(_input: &str) -> Option<SlashCommand> {
    None
}
