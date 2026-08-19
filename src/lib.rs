mod scoring;

use wasm_bindgen::prelude::*;

/// Entry point called by the Telegraph validator.
/// Takes a ground truth payload and a miner response as JSON strings,
/// returns a score between 0.0 and 1.0.
#[wasm_bindgen]
pub fn evaluate(ground_truth: &str, miner_response: &str) -> f64 {
    let gt: scoring::GroundTruth = match serde_json::from_str(ground_truth) {
        Ok(v) => v,
        Err(_) => return 0.0,
    };
    let mr: scoring::MinerResponse = match serde_json::from_str(miner_response) {
        Ok(v) => v,
        Err(_) => return 0.0,
    };
    scoring::score(&gt, &mr)
}
