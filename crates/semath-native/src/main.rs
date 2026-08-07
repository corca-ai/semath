use std::io::{self, Read};

use semath_core::{ProjectSnapshot, SemathEngine};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    let snapshot: ProjectSnapshot = serde_json::from_slice(&input)?;
    let mut engine = SemathEngine::default();
    println!("{}", serde_json::to_string(&engine.reset(snapshot)?)?);
    Ok(())
}
