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

    let value = headson::parse_json(&buffer, cli.budget).context("failed to parse JSON from stdin")?;

    match cli.template {
        Template::Pseudo => {
            if matches!(value, serde_json::Value::String(ref s) if s.is_empty()) {
                println!("[ … ]");
            } else {
                println!("{}", serde_json::to_string(&value)?);
            }
        }
        Template::Json | Template::Js => {
            println!("{}", serde_json::to_string(&value)?);
        }
    }

    Ok(())
}
