use serde::{Deserialize, Serialize};

/// The known-correct answer provided by the protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruth {
    /// The canonical answer or expected output.
    pub answer: serde_json::Value,
    /// Optional metadata (e.g. confidence, source).
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// A response returned by a miner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerResponse {
    /// The miner's answer.
    pub answer: serde_json::Value,
    /// Optional confidence score from the miner (0.0–1.0).
    #[serde(default)]
    pub confidence: Option<f64>,
    /// Optional reasoning or explanation.
    #[serde(default)]
    pub reasoning: Option<String>,
}

/// Score a miner response against ground truth.
/// Returns a value between 0.0 (completely wrong) and 1.0 (perfect match).
pub fn score(gt: &GroundTruth, mr: &MinerResponse) -> f64 {
    // Exact match on the answer field
    let answer_score = if gt.answer == mr.answer {
        1.0
    } else {
        // Partial scoring for numeric values
        score_numeric_or_text(&gt.answer, &mr.answer)
    };

    // Clamp to [0.0, 1.0]
    answer_score.clamp(0.0, 1.0)
}

/// Score numeric or text answers.
fn score_numeric_or_text(gt: &serde_json::Value, mr: &serde_json::Value) -> f64 {
    // Both are numbers — compute relative error
    if let (Some(gt_num), Some(mr_num)) = (gt.as_f64(), mr.as_f64()) {
        if gt_num == 0.0 {
            return if mr_num == 0.0 { 1.0 } else { 0.0 };
        }
        let relative_error = ((gt_num - mr_num) / gt_num).abs();
        return (1.0 - relative_error).max(0.0);
    }

    // Both are strings — use fuzzy text similarity
    if let (Some(gt_str), Some(mr_str)) = (gt.as_str(), mr.as_str()) {
        return text_similarity(gt_str, mr_str);
    }

    // Type mismatch or unparseable
    0.0
}

/// Simple word-overlap text similarity (Jaccard index).
fn text_similarity(a: &str, b: &str) -> f64 {
    let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();

    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }
    if a_words.is_empty() || b_words.is_empty() {
        return 0.0;
    }

    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();

    intersection as f64 / union as f64
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn gt(answer: &str) -> GroundTruth {
        GroundTruth {
            answer: serde_json::from_str(answer).unwrap(),
            metadata: None,
        }
    }

    fn mr(answer: &str) -> MinerResponse {
        MinerResponse {
            answer: serde_json::from_str(answer).unwrap(),
            confidence: None,
            reasoning: None,
        }
    }

    #[test]
    fn exact_string_match() {
        assert_eq!(score(&gt("\"hello\""), &mr("\"hello\"")), 1.0);
    }

    #[test]
    fn exact_number_match() {
        assert_eq!(score(&gt("42"), &mr("42")), 1.0);
    }

    #[test]
    fn wrong_answer() {
        assert_eq!(score(&gt("\"yes\""), &mr("\"no\"")), 0.0);
    }

    #[test]
    fn numeric_close() {
        let s = score(&gt("100"), &mr("95"));
        assert!(s > 0.9, "expected > 0.9, got {s}");
    }

    #[test]
    fn numeric_far_off() {
        let s = score(&gt("100"), &mr("50"));
        assert!(s < 0.6, "expected < 0.6, got {s}");
    }

    #[test]
    fn text_similarity_partial() {
        let s = score(&gt("\"the quick brown fox\""), &mr("\"the quick red fox\""));
        assert!(s > 0.5, "expected > 0.5, got {s}");
    }

    #[test]
    fn malformed_response_returns_zero() {
        let mr = MinerResponse {
            answer: serde_json::Value::Null,
            confidence: None,
            reasoning: None,
        };
        let gt = gt("\"hello\"");
        assert_eq!(score(&gt, &mr), 0.0);
    }
}
