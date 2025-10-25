use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use std::io::{self, Write};

// dev-only generator: build with `cargo run --release --example genfixture -- [--count N] [--seed S]`

#[derive(Serialize)]
struct Root<'a> {
    count: usize,
    next: Option<serde_json::Value>,
    previous: Option<serde_json::Value>,
    #[serde(skip_serializing)]
    _phantom: std::marker::PhantomData<&'a ()>,
}

#[derive(Serialize)]
struct Item {
    name: String,
    url: String,
}

fn parse_arg(flag: &str) -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == flag {
            return args.next();
        }
    }
    None
}

fn main() -> anyhow::Result<()> {
    // Defaults
    let count: usize = parse_arg("--count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    let seed: u64 = parse_arg("--seed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(42);
    let base_url = parse_arg("--base-url")
        .unwrap_or_else(|| "https://example.com/api/v1/item/".to_string());

    let stdout = io::stdout();
    let mut w = io::BufWriter::with_capacity(1 << 20, stdout.lock());

    // Header
    let header = Root {
        count,
        next: None,
        previous: None,
        _phantom: std::marker::PhantomData,
    };
    write!(
        &mut w,
        "{{\"count\":{},\"next\":null,\"previous\":null,\"results\":[",
        header.count
    )?;

    // Deterministic RNG
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    for i in 0..count {
        // Deterministic ASCII lowercase "word" 5–10 chars
        let len = rng.random_range(5..=10);
        let mut name = String::with_capacity(len);
        for _ in 0..len {
            name.push(rng.random_range(b'a'..=b'z') as char);
        }
        let url = format!("{}{}{}", base_url, i + 1, "/");
        let it = Item { name, url };
        serde_json::to_writer(&mut w, &it)?;
        if i + 1 < count {
            w.write_all(b",")?;
        }
    }

    w.write_all(b"]}")?;
    w.flush()?;
    Ok(())
}
