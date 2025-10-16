use anyhow::Result;
use serde_json::Value;

pub fn parse_json(input: &str) -> Result<Value> {
    let parsed_value: Value = serde_json::from_str(input)?;
    Ok(parsed_value)
}
