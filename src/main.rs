use std::io::{self, Read};

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "headson", version, about = "Parse JSON from stdin and echo it")] 
struct Cli {}

fn main() -> Result<()> {
    let _ = Cli::parse();

    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("failed to read from stdin")?;

    let value = headson::parse_json(&buffer).context("failed to parse JSON from stdin")?;
    println!("{}", serde_json::to_string(&value)?);

    Ok(())
}
