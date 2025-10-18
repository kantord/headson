use std::io::{self, Read};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "headson", version, about = "Parse JSON from stdin and echo it")] 
struct Cli {
    #[arg(short = 'n', long = "budget", default_value_t = 500)]
    budget: usize,
    #[arg(short = 'f', long = "template", value_enum, default_value_t = Template::Pseudo)]
    template: Template,
    #[arg(long = "indent", default_value = "  ")]
    indent: String,
    #[arg(long = "no-space", default_value_t = false)]
    no_space: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Template {
    Json,
    Pseudo,
    Js,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("failed to read from stdin")?;

    let template = match cli.template {
        Template::Json => headson::OutputTemplate::Json,
        Template::Pseudo => headson::OutputTemplate::Pseudo,
        Template::Js => headson::OutputTemplate::Js,
    };
    let space = if cli.no_space { "".to_string() } else { " ".to_string() };
    let config = headson::RenderConfig { template, indent_unit: cli.indent.clone(), space };

    let output = headson::headson(&buffer, config, cli.budget)?;
    println!("{}", output);

    Ok(())
}
