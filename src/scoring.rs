use serde::{Deserialize, Serialize};
/// Default relative error tolerance — if error is below this, score = 1.0.
const DEFAULT_NUMERIC_TOLERANCE: f64 = 0.001;

/// Penalty multiplier for overconfident wrong answers.
/// Applied as: `score = answer_score * (1.0 - confidence * PENALTY)` when answer is wrong.
const CONFIDENCE_PENALTY: f64 = 0.5;

/// The known-correct answer provided by the protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruth {
    /// The canonical answer or expected output.
    pub answer: serde_json::Value,
    /// Optional metadata (e.g. scoring hints, tolerance, expected type).
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
        score_values(&gt.answer, &mr.answer, &gt.metadata)
    };

    // Apply confidence penalty for wrong answers (anti-gaming)
    let final_score = if answer_score < 1.0 {
        apply_confidence_penalty(answer_score, mr.confidence)
    } else {
        answer_score
    };

    // Clamp to [0.0, 1.0]
    final_score.clamp(0.0, 1.0)
}

/// Apply confidence-based penalty for wrong answers.
/// A miner that is highly confident and wrong gets penalized more.
fn apply_confidence_penalty(answer_score: f64, confidence: Option<f64>) -> f64 {
    match confidence {
        Some(c) if c > 0.0 && c <= 1.0 => {
            answer_score * (1.0 - c * CONFIDENCE_PENALTY)
        }
        _ => answer_score,
    }
}

/// Score two JSON values with type coercion and fuzzy matching.
fn score_values(gt: &serde_json::Value, mr: &serde_json::Value, metadata: &Option<serde_json::Value>) -> f64 {
    // --- Boolean handling ---
    // Both booleans: exact match
    if let (Some(gt_bool), Some(mr_bool)) = (gt.as_bool(), mr.as_bool()) {
        return if gt_bool == mr_bool { 1.0 } else { 0.0 };
    }
    // Boolean vs number: coerce bool to 0/1
    if let (Some(gt_bool), Some(mr_num)) = (gt.as_bool(), mr.as_f64()) {
        let gt_num = if gt_bool { 1.0 } else { 0.0 };
        return score_numeric(gt_num, mr_num, metadata);
    }
    if let (Some(gt_num), Some(mr_bool)) = (gt.as_f64(), mr.as_bool()) {
        let mr_num = if mr_bool { 1.0 } else { 0.0 };
        return score_numeric(gt_num, mr_num, metadata);
    }

    // --- Numeric comparison (with type coercion) ---
    let gt_num = gt.as_f64().or_else(|| try_coerce_to_f64(gt));
    let mr_num = mr.as_f64().or_else(|| try_coerce_to_f64(mr));

    if let (Some(gt_n), Some(mr_n)) = (gt_num, mr_num) {
        return score_numeric(gt_n, mr_n, metadata);
    }

    // --- Text comparison (with normalization) ---
    let gt_str = gt.as_str().or_else(|| try_coerce_to_str(gt));
    let mr_str = mr.as_str().or_else(|| try_coerce_to_str(mr));

    if let (Some(gt_s), Some(mr_s)) = (gt_str, mr_str) {
        return text_similarity(gt_s, mr_s);
    }

    // --- Fuzzy object/array scoring ---
    if gt.is_object() && mr.is_object() {
        return score_objects(gt, mr);
    }
    if gt.is_array() && mr.is_array() {
        return score_arrays(gt, mr);
    }

    // Type mismatch or unparseable
    0.0
}

/// Score two numeric values with tolerance support.
fn score_numeric(gt_num: f64, mr_num: f64, metadata: &Option<serde_json::Value>) -> f64 {
    if gt_num == mr_num {
        return 1.0;
    }
    if gt_num == 0.0 {
        return if mr_num == 0.0 { 1.0 } else { 0.0 };
    }

    let relative_error = ((gt_num - mr_num) / gt_num).abs();

    // Check for absolute tolerance (near-miss = full credit)
    let tolerance = get_numeric_tolerance(metadata);
    if relative_error <= tolerance {
        return 1.0;
    }

    (1.0 - relative_error).max(0.0)
}

/// Extract numeric tolerance from metadata, or use default.
fn get_numeric_tolerance(metadata: &Option<serde_json::Value>) -> f64 {
    metadata
        .as_ref()
        .and_then(|m| m.get("tolerance"))
        .and_then(|t| t.as_f64())
        .unwrap_or(DEFAULT_NUMERIC_TOLERANCE)
}

/// Try to coerce a JSON value to f64 (handles string-encoded numbers).
fn try_coerce_to_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_str().and_then(|s| s.parse::<f64>().ok())
}

/// Try to coerce a JSON value to &str (handles stringified values).
fn try_coerce_to_str(v: &serde_json::Value) -> Option<&str> {
    // For non-string, non-null values, we don't coerce to string
    // to avoid false matches like number 42 vs string "42" in text mode
    // (that's handled by numeric coercion above)
    v.as_str()
}

/// Normalized text similarity (lowercase + punctuation stripped, then Jaccard).
fn text_similarity(a: &str, b: &str) -> f64 {
    let a_words: std::collections::HashSet<String> = tokenize(a);
    let b_words: std::collections::HashSet<String> = tokenize(b);

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

/// Tokenize a string: lowercase, strip punctuation, split on whitespace.
fn tokenize(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect()
}

/// Fuzzy scoring for JSON objects: match keys, compare values recursively.
fn score_objects(gt: &serde_json::Value, mr: &serde_json::Value) -> f64 {
    let gt_obj = match gt.as_object() {
        Some(o) => o,
        None => return 0.0,
    };
    let mr_obj = match mr.as_object() {
        Some(o) => o,
        None => return 0.0,
    };

    if gt_obj.is_empty() && mr_obj.is_empty() {
        return 1.0;
    }

    // Collect all unique keys
    let all_keys: Vec<&String> = {
        let mut keys: Vec<&String> = gt_obj.keys().chain(mr_obj.keys()).collect();
        keys.sort();
        keys.dedup();
        keys
    };

    let mut total_score = 0.0;
    let mut count = 0;

    for key in &all_keys {
        match (gt_obj.get(*key), mr_obj.get(*key)) {
            (Some(gt_val), Some(mr_val)) => {
                // Both have the key — score the values
                total_score += score_values(gt_val, mr_val, &None);
            }
            (Some(_), None) | (None, Some(_)) => {
                // One side missing the key — 0 credit for this key
            }
            (None, None) => unreachable!(),
        }
        count += 1;
    }

    if count == 0 {
        0.0
    } else {
        total_score / count as f64
    }
}

/// Fuzzy scoring for JSON arrays: match elements by index, compare recursively.
fn score_arrays(gt: &serde_json::Value, mr: &serde_json::Value) -> f64 {
    let gt_arr = match gt.as_array() {
        Some(a) => a,
        None => return 0.0,
    };
    let mr_arr = match mr.as_array() {
        Some(a) => a,
        None => return 0.0,
    };

    if gt_arr.is_empty() && mr_arr.is_empty() {
        return 1.0;
    }
    if gt_arr.is_empty() || mr_arr.is_empty() {
        return 0.0;
    }

    let len = gt_arr.len().max(mr_arr.len());
    let mut total_score = 0.0;

    for i in 0..len {
        if i < gt_arr.len() && i < mr_arr.len() {
            total_score += score_values(&gt_arr[i], &mr_arr[i], &None);
        }
        // Missing elements get 0
    }

    total_score / len as f64
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn gt(answer: &str) -> GroundTruth {
        GroundTruth {
            answer: serde_json::from_str(answer).unwrap(),
            metadata: None,
        }
    }

    fn gt_with_meta(answer: &str, meta: &str) -> GroundTruth {
        GroundTruth {
            answer: serde_json::from_str(answer).unwrap(),
            metadata: Some(serde_json::from_str(meta).unwrap()),
        }
    }

    fn mr(answer: &str) -> MinerResponse {
        MinerResponse {
            answer: serde_json::from_str(answer).unwrap(),
            confidence: None,
            reasoning: None,
        }
    }

    fn mr_with_conf(answer: &str, conf: f64) -> MinerResponse {
        MinerResponse {
            answer: serde_json::from_str(answer).unwrap(),
            confidence: Some(conf),
            reasoning: None,
        }
    }

    // Existing tests (unchanged)

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

    // ─── Gap 1: Boolean handling ─────────────────────────────────────

    #[test]
    fn bool_exact_match_true() {
        assert_eq!(score(&gt("true"), &mr("true")), 1.0);
    }

    #[test]
    fn bool_exact_match_false() {
        assert_eq!(score(&gt("false"), &mr("false")), 1.0);
    }

    #[test]
    fn bool_mismatch() {
        assert_eq!(score(&gt("true"), &mr("false")), 0.0);
    }

    #[test]
    fn bool_vs_number_coercion() {
        // true == 1, false == 0
        assert_eq!(score(&gt("true"), &mr("1")), 1.0);
        assert_eq!(score(&gt("false"), &mr("0")), 1.0);
    }

    #[test]
    fn number_vs_bool_coercion() {
        assert_eq!(score(&gt("1"), &mr("true")), 1.0);
        assert_eq!(score(&gt("0"), &mr("false")), 1.0);
    }

    // Gap 2: Case-insensitive + punctuation-stripped text

    #[test]
    fn case_insensitive_match() {
        assert_eq!(score(&gt("\"Hello\""), &mr("\"hello\"")), 1.0);
    }

    #[test]
    fn punctuation_stripped_match() {
        assert_eq!(score(&gt("\"hello.\""), &mr("\"hello\"")), 1.0);
    }

    #[test]
    fn mixed_case_punctuation_match() {
        assert_eq!(score(&gt("\"Hello, World!\""), &mr("\"hello world\"")), 1.0);
    }

    #[test]
    fn text_partial_after_normalization() {
        let s = score(&gt("\"the quick brown fox\""), &mr("\"The quick red Fox.\""));
        assert!(s > 0.5, "expected > 0.5, got {s}");
    }

    // ─── Gap 3: Confidence-based scoring ─────────────────────────────

    #[test]
    fn confident_wrong_answer_penalized() {
        // Use answers with partial word overlap so base score > 0
        let s_no_conf = score(&gt("\"the quick brown fox\""), &mr("\"the slow brown fox\""));
        let s_high_conf = score(&gt("\"the quick brown fox\""), &mr_with_conf("\"the slow brown fox\"", 1.0));
        // High confidence wrong answer should score lower than no confidence
        assert!(s_no_conf > 0.0, "base score should be > 0, got {s_no_conf}");
        assert!(s_high_conf < s_no_conf, "confident wrong ({s_high_conf}) should be < no conf ({s_no_conf})");
    }

    #[test]
    fn uncertain_wrong_answer_less_penalized() {
        let s_low_conf = score(&gt("\"the quick brown fox\""), &mr_with_conf("\"the slow brown fox\"", 0.1));
        let s_high_conf = score(&gt("\"the quick brown fox\""), &mr_with_conf("\"the slow brown fox\"", 1.0));
        assert!(s_low_conf > s_high_conf, "uncertain wrong ({s_low_conf}) should be > confident wrong ({s_high_conf})");
    }

    #[test]
    fn correct_answer_no_penalty() {
        let s = score(&gt("\"yes\""), &mr_with_conf("\"yes\"", 1.0));
        assert_eq!(s, 1.0);
    }

    // Gap 4: Type coercion

    #[test]
    fn string_number_coercion() {
        // "42" (string) should match 42 (number)
        assert_eq!(score(&gt("42"), &mr("\"42\"")), 1.0);
    }

    #[test]
    fn number_to_string_coercion() {
        assert_eq!(score(&gt("\"42\""), &mr("42")), 1.0);
    }

    #[test]
    fn string_number_coercion_close() {
        let s = score(&gt("100"), &mr("\"99.5\""));
        assert!(s > 0.9, "expected > 0.9, got {s}");
    }

    #[test]
    fn non_numeric_string_no_coercion() {
        // "hello" should not coerce to a number
        assert_eq!(score(&gt("42"), &mr("\"hello\"")), 0.0);
    }

    // ─── Gap 5: Numeric tolerance ────────────────────────────────────

    #[test]
    fn near_miss_within_default_tolerance() {
        // 99.95 vs 100 → relative error = 0.0005 < 0.001 tolerance → 1.0
        assert_eq!(score(&gt("100"), &mr("99.95")), 1.0);
    }

    #[test]
    fn near_miss_outside_tolerance() {
        // 99 vs 100 → relative error = 0.01 > 0.001 tolerance → 0.99
        let s = score(&gt("100"), &mr("99"));
        assert!(s > 0.98 && s < 1.0, "expected ~0.99, got {s}");
    }

    #[test]
    fn metadata_tolerance_override() {
        // With tolerance=0.1 (10%), 90 vs 100 should be perfect
        let gt = gt_with_meta("100", "{\"tolerance\": 0.1}");
        assert_eq!(score(&gt, &mr("90")), 1.0);
    }

    #[test]
    fn zero_ground_truth_handling() {
        // gt=0, mr=0 → 1.0
        assert_eq!(score(&gt("0"), &mr("0")), 1.0);
        // gt=0, mr=1 → 0.0
        assert_eq!(score(&gt("0"), &mr("1")), 0.0);
    }

    // Gap 6: Fuzzy object scoring

    #[test]
    fn object_exact_match() {
        let gt = gt("{\"name\": \"Alice\", \"age\": 30}");
        let mr = mr("{\"name\": \"Alice\", \"age\": 30}");
        assert_eq!(score(&gt, &mr), 1.0);
    }

    #[test]
    fn object_partial_match() {
        let gt = gt("{\"name\": \"Alice\", \"age\": 30}");
        let mr = mr("{\"name\": \"Alice\", \"age\": 25}");
        let s = score(&gt, &mr);
        // name matches (1.0), age is close (0.75) → ~0.875
        assert!(s > 0.8 && s < 1.0, "expected ~0.875, got {s}");
    }

    #[test]
    fn object_missing_key() {
        let gt = gt("{\"name\": \"Alice\", \"age\": 30}");
        let mr = mr("{\"name\": \"Alice\"}");
        let s = score(&gt, &mr);
        // name matches (1.0), age missing (0.0) → 0.5
        assert!(s > 0.4 && s < 0.6, "expected ~0.5, got {s}");
    }

    #[test]
    fn object_empty_both() {
        let gt = gt("{}");
        let mr = mr("{}");
        assert_eq!(score(&gt, &mr), 1.0);
    }

    #[test]
    fn array_partial_match() {
        let gt = gt("[1, 2, 3]");
        let mr = mr("[1, 2, 4]");
        let s = score(&gt, &mr);
        // (1.0 + 1.0 + 0.667) / 3 = 0.889
        assert!(s > 0.88 && s < 0.90, "expected ~0.889, got {s}");
    }

    #[test]
    fn array_exact_match() {
        assert_eq!(score(&gt("[1, 2, 3]"), &mr("[1, 2, 3]")), 1.0);
    }

    // Subnet-specific integration tests

    #[test]
    fn itsai_integer_classification() {
        // ItsAI (subnet 32): { answer: 0 | 1 }
        assert_eq!(
            score(
                &gt("{\"answer\": 1}"),
                &mr("{\"answer\": 1}")
            ),
            1.0
        );
        assert_eq!(
            score(
                &gt("{\"answer\": 0}"),
                &mr("{\"answer\": 1}")
            ),
            0.0
        );
    }

    #[test]
    fn bitmind_bool_classification() {
        // Bitmind (subnet 34): { isAI: true/false }
        assert_eq!(
            score(
                &gt("{\"isAI\": true}"),
                &mr("{\"isAI\": true}")
            ),
            1.0
        );
        assert_eq!(
            score(
                &gt("{\"isAI\": false}"),
                &mr("{\"isAI\": true}")
            ),
            0.0
        );
    }

    #[test]
    fn groq_llm_text_response() {
        // Groq LLM (subnet 102): { choices: [{ message: { content: "..." } }] }
        let gt = gt("{\"choices\": [{\"message\": {\"content\": \"The capital of France is Paris.\"}}]}");
        let mr = mr("{\"choices\": [{\"message\": {\"content\": \"The capital of France is Paris.\"}}]}");
        assert_eq!(score(&gt, &mr), 1.0);
    }

    #[test]
    fn desearch_articles_partial() {
        // DeSearch (subnet 101): { articles: [{ title, snippet, source }] }
        let gt = gt("{\"articles\": [{\"title\": \"AI Advances\", \"snippet\": \"New breakthrough\", \"source\": \"TechCrunch\"}]}");
        let mr = mr("{\"articles\": [{\"title\": \"AI Advances\", \"snippet\": \"New breakthrough in AI\", \"source\": \"TechCrunch\"}]}");
        let s = score(&gt, &mr);
        assert!(s > 0.8, "expected > 0.8, got {s}");
    }

    // Edge cases 

    #[test]
    fn both_empty_strings() {
        assert_eq!(score(&gt("\"\""), &mr("\"\"")), 1.0);
    }

    #[test]
    fn empty_vs_nonempty() {
        assert_eq!(score(&gt("\"\""), &mr("\"hello\"")), 0.0);
    }

    #[test]
    fn null_vs_null() {
        assert_eq!(score(&gt("null"), &mr("null")), 1.0);
    }

    #[test]
    fn null_vs_value() {
        assert_eq!(score(&gt("null"), &mr("\"hello\"")), 0.0);
    }
}
