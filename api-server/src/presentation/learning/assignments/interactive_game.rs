use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

/// Outcome of automatically grading an interactive game question (`MATCHING`, `ORDERING`, `FILL_BLANK`).
#[derive(Debug, Clone, PartialEq)]
pub struct InteractiveGradeResult {
    pub points_earned: i32,
    pub correct_count: usize,
    pub total_count: usize,
    pub is_fully_correct: bool,
}

/// Returns true for interactive game question types supported in Misi Game Tugas.
pub fn is_interactive_game_type(question_type: &str) -> bool {
    matches!(
        question_type.trim().to_uppercase().as_str(),
        "MATCHING" | "ORDERING" | "FILL_BLANK"
    )
}

/// Deterministic-free Fisher-Yates shuffle seeded from UUID v4 entropy (no `rand` crate needed).
fn shuffle_vec<T>(items: &mut [T]) {
    let n = items.len();
    if n <= 1 {
        return;
    }
    let mut seed = Uuid::new_v4().as_u128();
    if seed == 0 {
        seed = 0x9E37_79B9_7F4A_7C15_1234_5678_9ABC_DEF0;
    }
    for i in (1..n).rev() {
        // Xorshift128-style step
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let j = (seed as usize) % (i + 1);
        items.swap(i, j);
    }
}

/// Sanitizes and shuffles interactive question `interaction_config` before sending to a student
/// so correct answer order / key mappings are never exposed in client payloads.
pub fn shuffle_interaction_for_student(question_type: &str, config: &Value) -> Value {
    match question_type.trim().to_uppercase().as_str() {
        "MATCHING" => {
            let pairs = config
                .get("pairs")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let mut left_items = Vec::with_capacity(pairs.len());
            let mut right_options: Vec<String> = Vec::with_capacity(pairs.len());

            for (idx, p) in pairs.iter().enumerate() {
                let id = p
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("p{}", idx + 1));
                let left = p
                    .get("left")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let right = p
                    .get("right")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();

                if !left.is_empty() && !right.is_empty() {
                    left_items.push(json!({ "id": id, "text": left }));
                    right_options.push(right);
                }
            }

            let original_right = right_options.clone();
            shuffle_vec(&mut right_options);
            // Guarantee shuffled right side isn't accidentally identical when >=2 distinct items exist
            if right_options.len() >= 2 && right_options == original_right {
                let all_same = original_right.windows(2).all(|w| w[0] == w[1]);
                if !all_same {
                    right_options.rotate_left(1);
                }
            }

            json!({
                "left_items": left_items,
                "right_options": right_options,
            })
        }
        "ORDERING" => {
            let raw_items = config
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let mut clean_items: Vec<Value> = raw_items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    let (id, text) = if let Some(s) = item.as_str() {
                        (format!("o{}", idx + 1), s.trim().to_string())
                    } else {
                        let id = item
                            .get("id")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("o{}", idx + 1));
                        let text = item
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        (id, text)
                    };
                    if text.is_empty() {
                        None
                    } else {
                        Some(json!({ "id": id, "text": text }))
                    }
                })
                .collect();

            let original_ids: Vec<String> = clean_items
                .iter()
                .map(|v| v.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string())
                .collect();
            shuffle_vec(&mut clean_items);
            let shuffled_ids: Vec<String> = clean_items
                .iter()
                .map(|v| v.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string())
                .collect();
            if clean_items.len() >= 2 && shuffled_ids == original_ids {
                clean_items.rotate_left(1);
            }

            json!({ "items": clean_items })
        }
        "FILL_BLANK" => {
            let template = config
                .get("template")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let blanks: Vec<String> = config
                .get("blanks")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let distractors: Vec<String> = config
                .get("distractors")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();

            let blank_count = blanks.len();
            let mut word_bank = blanks;
            for d in distractors {
                if !word_bank.iter().any(|w| w.eq_ignore_ascii_case(&d)) {
                    word_bank.push(d);
                }
            }
            shuffle_vec(&mut word_bank);

            json!({
                "template": template,
                "blank_count": blank_count,
                "word_bank": word_bank,
            })
        }
        _ => config.clone(),
    }
}

/// Computes proportional points from `(correct_count / total_count) * max_points`, rounded cleanly.
pub fn compute_proportional_points(
    correct_count: usize,
    total_count: usize,
    max_points: i32,
) -> i32 {
    if total_count == 0 || max_points <= 0 || correct_count == 0 {
        return 0;
    }
    let clamped_correct = correct_count.min(total_count);
    ((clamped_correct as f64 / total_count as f64) * (max_points as f64))
        .round()
        .clamp(0.0, max_points as f64) as i32
}

/// Computes normalized 0..=100 score percentage across questions.
pub fn calculate_total_percentage_score(earned_points: i32, total_max_points: i32) -> i32 {
    if total_max_points <= 0 || earned_points <= 0 {
        return 0;
    }
    ((earned_points as f64 / total_max_points as f64) * 100.0)
        .round()
        .clamp(0.0, 100.0) as i32
}

/// Automatically grades a student's JSON submission string (`text_answer`) against teacher `interaction_config`.
pub fn grade_interactive_answer(
    question_type: &str,
    config: &Value,
    student_text_answer: Option<&str>,
    max_points: i32,
) -> InteractiveGradeResult {
    let raw_ans = student_text_answer.unwrap_or("").trim();
    let parsed_ans: Value = serde_json::from_str(raw_ans).unwrap_or(Value::Null);

    match question_type.trim().to_uppercase().as_str() {
        "MATCHING" => {
            let pairs = config
                .get("pairs")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if pairs.is_empty() {
                return InteractiveGradeResult {
                    points_earned: 0,
                    correct_count: 0,
                    total_count: 0,
                    is_fully_correct: false,
                };
            }

            let mut submitted_map: HashMap<String, String> = HashMap::new();
            if let Some(obj) = parsed_ans.get("matches").and_then(|v| v.as_object()) {
                for (k, v) in obj {
                    if let Some(val_str) = v.as_str() {
                        submitted_map.insert(k.trim().to_string(), val_str.trim().to_string());
                    }
                }
            } else if let Some(arr) = parsed_ans.get("pairs").and_then(|v| v.as_array()) {
                for item in arr {
                    let lid = item
                        .get("left_id")
                        .or_else(|| item.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    let rtxt = item
                        .get("right")
                        .or_else(|| item.get("right_text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim();
                    if !lid.is_empty() {
                        submitted_map.insert(lid.to_string(), rtxt.to_string());
                    }
                }
            }

            let mut correct = 0usize;
            for (idx, pair) in pairs.iter().enumerate() {
                let id = pair
                    .get("id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("p{}", idx + 1));
                let expected_right = pair
                    .get("right")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim();

                if let Some(chosen_right) = submitted_map.get(&id) {
                    if !expected_right.is_empty() && chosen_right.eq_ignore_ascii_case(expected_right) {
                        correct += 1;
                    }
                }
            }

            let total = pairs.len();
            let points_earned = compute_proportional_points(correct, total, max_points);
            InteractiveGradeResult {
                points_earned,
                correct_count: correct,
                total_count: total,
                is_fully_correct: correct == total && total > 0,
            }
        }
        "ORDERING" => {
            let items = config
                .get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if items.is_empty() {
                return InteractiveGradeResult {
                    points_earned: 0,
                    correct_count: 0,
                    total_count: 0,
                    is_fully_correct: false,
                };
            }

            let expected_ids: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    if item.is_string() {
                        format!("o{}", idx + 1)
                    } else {
                        item.get("id")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("o{}", idx + 1))
                    }
                })
                .collect();

            let submitted_ids: Vec<String> = parsed_ans
                .get("ordered_ids")
                .or_else(|| parsed_ans.get("order"))
                .and_then(|v| v.as_array())
                .or_else(|| parsed_ans.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let mut correct = 0usize;
            for (idx, exp_id) in expected_ids.iter().enumerate() {
                if submitted_ids.get(idx).map(|s| s == exp_id).unwrap_or(false) {
                    correct += 1;
                }
            }

            let total = expected_ids.len();
            let points_earned = compute_proportional_points(correct, total, max_points);
            InteractiveGradeResult {
                points_earned,
                correct_count: correct,
                total_count: total,
                is_fully_correct: correct == total && total > 0,
            }
        }
        "FILL_BLANK" => {
            let blanks: Vec<String> = config
                .get("blanks")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                        .collect()
                })
                .unwrap_or_default();

            if blanks.is_empty() {
                return InteractiveGradeResult {
                    points_earned: 0,
                    correct_count: 0,
                    total_count: 0,
                    is_fully_correct: false,
                };
            }

            let submitted_answers: Vec<String> = parsed_ans
                .get("answers")
                .or_else(|| parsed_ans.get("fills"))
                .and_then(|v| v.as_array())
                .or_else(|| parsed_ans.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let mut correct = 0usize;
            for (idx, exp) in blanks.iter().enumerate() {
                if let Some(ans_val) = submitted_answers.get(idx) {
                    if !exp.is_empty() && ans_val.eq_ignore_ascii_case(exp) {
                        correct += 1;
                    }
                }
            }

            let total = blanks.len();
            let points_earned = compute_proportional_points(correct, total, max_points);
            InteractiveGradeResult {
                points_earned,
                correct_count: correct,
                total_count: total,
                is_fully_correct: correct == total && total > 0,
            }
        }
        _ => InteractiveGradeResult {
            points_earned: 0,
            correct_count: 0,
            total_count: 0,
            is_fully_correct: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matching_two_of_four_correct_scores_50_not_100() {
        let config = json!({
            "pairs": [
                { "id": "p1", "left": "Fotosintesis", "right": "Proses membuat makanan pada daun" },
                { "id": "p2", "left": "Klorofil", "right": "Zat hijau daun" },
                { "id": "p3", "left": "Stomata", "right": "Mulut daun" },
                { "id": "p4", "left": "Xilem", "right": "Pembuluh angkut air" }
            ]
        });

        // Student answers p1 and p2 correctly, but swaps p3 and p4
        let student_answer = json!({
            "matches": {
                "p1": "Proses membuat makanan pada daun",
                "p2": "Zat hijau daun",
                "p3": "Pembuluh angkut air",
                "p4": "Mulut daun"
            }
        })
        .to_string();

        // Direct 100-point question must yield exactly 50 (not 100)
        let res_100 = grade_interactive_answer("MATCHING", &config, Some(&student_answer), 100);
        assert_eq!(res_100.correct_count, 2);
        assert_eq!(res_100.total_count, 4);
        assert_eq!(res_100.points_earned, 50);
        assert!(!res_100.is_fully_correct);

        // And with a 20-point question, total assignment percentage must also normalize to 50
        let res_20 = grade_interactive_answer("MATCHING", &config, Some(&student_answer), 20);
        assert_eq!(res_20.points_earned, 10);
        assert_eq!(calculate_total_percentage_score(res_20.points_earned, 20), 50);
    }

    #[test]
    fn matching_all_correct_and_all_wrong() {
        let config = json!({
            "pairs": [
                { "id": "p1", "left": "I", "right": "1" },
                { "id": "p2", "left": "V", "right": "5" },
                { "id": "p3", "left": "X", "right": "10" },
                { "id": "p4", "left": "L", "right": "50" }
            ]
        });

        let perfect_ans = json!({
            "matches": { "p1": "1", "p2": "5", "p3": "10", "p4": "50" }
        })
        .to_string();
        let res_perfect = grade_interactive_answer("MATCHING", &config, Some(&perfect_ans), 100);
        assert_eq!(res_perfect.points_earned, 100);
        assert!(res_perfect.is_fully_correct);

        let zero_ans = json!({
            "matches": { "p1": "50", "p2": "10", "p3": "5", "p4": "1" }
        })
        .to_string();
        let res_zero = grade_interactive_answer("MATCHING", &config, Some(&zero_ans), 100);
        assert_eq!(res_zero.points_earned, 0);
        assert!(!res_zero.is_fully_correct);
    }

    #[test]
    fn ordering_partial_positions_graded_proportionally() {
        let config = json!({
            "items": [
                { "id": "o1", "text": "Langkah 1" },
                { "id": "o2", "text": "Langkah 2" },
                { "id": "o3", "text": "Langkah 3" },
                { "id": "o4", "text": "Langkah 4" }
            ]
        });

        // o1 and o4 are in right index (0 and 3), o2/o3 swapped -> 2/4 = 50%
        let partial_ans = json!({
            "ordered_ids": ["o1", "o3", "o2", "o4"]
        })
        .to_string();

        let res = grade_interactive_answer("ORDERING", &config, Some(&partial_ans), 100);
        assert_eq!(res.correct_count, 2);
        assert_eq!(res.total_count, 4);
        assert_eq!(res.points_earned, 50);
        assert!(!res.is_fully_correct);

        // Android app submits `{ "order": [...] }`
        let android_order = json!({
            "order": ["o1", "o2", "o3", "o4"]
        })
        .to_string();
        let res_android = grade_interactive_answer("ORDERING", &config, Some(&android_order), 100);
        assert_eq!(res_android.points_earned, 100);
        assert!(res_android.is_fully_correct);
    }

    #[test]
    fn fill_blank_partial_and_case_insensitive_grading() {
        let config = json!({
            "template": "Air membeku pada {{1}} C dan mendidih pada {{2}} C.",
            "blanks": ["0", "100"],
            "distractors": ["50", "37"]
        });

        let one_of_two = json!({ "answers": ["0", "50"] }).to_string();
        let res_half = grade_interactive_answer("FILL_BLANK", &config, Some(&one_of_two), 100);
        assert_eq!(res_half.correct_count, 1);
        assert_eq!(res_half.total_count, 2);
        assert_eq!(res_half.points_earned, 50);

        let full = json!({ "answers": [" 0 ", "100"] }).to_string();
        let res_full = grade_interactive_answer("FILL_BLANK", &config, Some(&full), 100);
        assert_eq!(res_full.points_earned, 100);
        assert!(res_full.is_fully_correct);

        // Android app submits `{ "fills": [...] }`
        let android_fills = json!({ "fills": ["0", "100"] }).to_string();
        let res_android = grade_interactive_answer("FILL_BLANK", &config, Some(&android_fills), 100);
        assert_eq!(res_android.points_earned, 100);
        assert!(res_android.is_fully_correct);
    }

    #[test]
    fn shuffle_for_student_strips_keys_and_shuffles_ordering() {
        let ordering_cfg = json!({
            "items": [
                { "id": "o1", "text": "Satu" },
                { "id": "o2", "text": "Dua" },
                { "id": "o3", "text": "Tiga" },
                { "id": "o4", "text": "Empat" }
            ]
        });
        let student_ordering = shuffle_interaction_for_student("ORDERING", &ordering_cfg);
        let ids: Vec<&str> = student_ordering["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap())
            .collect();
        assert_ne!(ids, vec!["o1", "o2", "o3", "o4"]);
        assert_eq!(ids.len(), 4);

        let fill_cfg = json!({
            "template": "Ibu kota Indonesia adalah {{1}}.",
            "blanks": ["Jakarta"],
            "distractors": ["Bandung", "Surabaya"]
        });
        let student_fill = shuffle_interaction_for_student("FILL_BLANK", &fill_cfg);
        assert!(student_fill.get("blanks").is_none());
        assert!(student_fill.get("distractors").is_none());
        assert_eq!(student_fill["blank_count"], 1);
        assert_eq!(student_fill["word_bank"].as_array().unwrap().len(), 3);
    }
}
