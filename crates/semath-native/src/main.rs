use std::io::{self, Read};

use semath_core::{ProjectSnapshot, QueryEnvelope, SemathEngine};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    let input: serde_json::Value = serde_json::from_slice(&input)?;
    let (snapshot, queries): (ProjectSnapshot, Vec<QueryEnvelope>) =
        if input.get("snapshot").is_some() {
            (
                serde_json::from_value(input["snapshot"].clone())?,
                serde_json::from_value(input["queries"].clone())?,
            )
        } else {
            (serde_json::from_value(input)?, Vec::new())
        };
    let mut engine = SemathEngine::default();
    let update = engine.reset(snapshot)?;
    if queries.is_empty() {
        println!("{}", serde_json::to_string(&update)?);
    } else {
        let results = queries
            .into_iter()
            .map(|query| engine.query(query))
            .collect::<Result<Vec<_>, _>>()?;
        println!("{}", serde_json::to_string(&results)?);
    }
    Ok(())
}
