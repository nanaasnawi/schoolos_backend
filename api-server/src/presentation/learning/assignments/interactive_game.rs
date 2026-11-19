use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use uuid::Uuid;

/// Outcome of automatically grading an interactive game question (`MATCHING`, `ORDERING`, `FILL_BLANK`, `CROSSWORD`).
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
        "MATCHING" | "ORDERING" | "FILL_BLANK" | "CROSSWORD"
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CwDir {
    Across,
    Down,
}

impl CwDir {
    fn as_str(self) -> &'static str {
        match self {
            CwDir::Across => "ACROSS",
            CwDir::Down => "DOWN",
        }
    }
}

struct PlacedCwWord {
    word: String,
    clue: String,
    row: usize,
    col: usize,
    dir: CwDir,
}

fn normalize_crossword_answer(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn can_place_cw(grid: &[Vec<u8>], bytes: &[u8], r0: usize, c0: usize, dir: CwDir) -> bool {
    let rows = grid.len();
    let cols = grid.first().map(|r| r.len()).unwrap_or(0);
    let len = bytes.len();
    match dir {
        CwDir::Across => {
            if r0 >= rows || c0 + len > cols {
                return false;
            }
            if c0 > 0 && grid[r0][c0 - 1] != b'#' {
                return false;
            }
            if c0 + len < cols && grid[r0][c0 + len] != b'#' {
                return false;
            }
            for (k, &ch) in bytes.iter().enumerate() {
                let cc = c0 + k;
                let cell = grid[r0][cc];
                if cell == ch {
                    continue;
                }
                if cell != b'#' {
                    return false;
                }
                if r0 > 0 && grid[r0 - 1][cc] != b'#' {
                    return false;
                }
                if r0 + 1 < rows && grid[r0 + 1][cc] != b'#' {
                    return false;
                }
            }
            true
        }
        CwDir::Down => {
            if c0 >= cols || r0 + len > rows {
                return false;
            }
            if r0 > 0 && grid[r0 - 1][c0] != b'#' {
                return false;
            }
            if r0 + len < rows && grid[r0 + len][c0] != b'#' {
                return false;
            }
            for (k, &ch) in bytes.iter().enumerate() {
                let rr = r0 + k;
                let cell = grid[rr][c0];
                if cell == ch {
                    continue;
                }
                if cell != b'#' {
                    return false;
                }
                if c0 > 0 && grid[rr][c0 - 1] != b'#' {
                    return false;
                }
                if c0 + 1 < cols && grid[rr][c0 + 1] != b'#' {
                    return false;
                }
            }
            true
        }
    }
}

fn commit_cw_word(
    grid: &mut [Vec<u8>],
    placed: &mut Vec<PlacedCwWord>,
    word: String,
    clue: String,
    r0: usize,
    c0: usize,
    dir: CwDir,
) {
    let bytes = word.as_bytes();
    match dir {
        CwDir::Across => {
            for (k, &ch) in bytes.iter().enumerate() {
                grid[r0][c0 + k] = ch;
            }
        }
        CwDir::Down => {
            for (k, &ch) in bytes.iter().enumerate() {
                grid[r0 + k][c0] = ch;
            }
        }
    }
    placed.push(PlacedCwWord {
        word,
        clue,
        row: r0,
        col: c0,
        dir,
    });
}

/// Compiles a `CROSSWORD` configuration (`{ words: [{ answer, clue }] }`) into a full
/// compacted 2D crossword layout (`rows`, `cols`, `grid`, `clues`, `words`).
/// Guaranteed to place every valid normalized word (2..=15 A-Z chars), preferring
/// high-intersection crossings and falling back to an isolated separator row if needed.
pub fn compile_crossword_interaction_config(config: &Value) -> Value {
    let mut raw_entries: Vec<(String, String)> = Vec::new();
    let mut seen_answers: HashSet<String> = HashSet::new();

    let push_entry = |ans_raw: &str,
                      clue_raw: &str,
                      entries: &mut Vec<(String, String)>,
                      seen: &mut HashSet<String>| {
        let ans = normalize_crossword_answer(ans_raw);
        let clue = clue_raw.trim().to_string();
        if ans.len() < 2 || ans.len() > 15 || clue.is_empty() {
            return;
        }
        if seen.insert(ans.clone()) {
            entries.push((ans, clue));
        }
    };

    if let Some(words) = config.get("words").and_then(|v| v.as_array()) {
        for item in words {
            let ans = item
                .get("answer")
                .or_else(|| item.get("word"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let clue = item
                .get("clue")
                .or_else(|| item.get("hint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            push_entry(ans, clue, &mut raw_entries, &mut seen_answers);
        }
    }

    if raw_entries.is_empty() {
        if let Some(clues) = config.get("clues").and_then(|v| v.as_array()) {
            for item in clues {
                let ans = item.get("answer").and_then(|v| v.as_str()).unwrap_or("");
                let clue = item.get("clue").and_then(|v| v.as_str()).unwrap_or("");
                push_entry(ans, clue, &mut raw_entries, &mut seen_answers);
            }
        }
    }

    if raw_entries.is_empty() {
        return json!({
            "words": [],
            "rows": 0,
            "cols": 0,
            "grid": [],
            "clues": []
        });
    }

    // Preserve normalized words for teacher editing while sorting a copy longest-first for spine layout.
    let normalized_words_json: Vec<Value> = raw_entries
        .iter()
        .map(|(ans, clue)| json!({ "answer": ans, "clue": clue }))
        .collect();

    let mut candidates = raw_entries;
    candidates.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));

    let max_len = candidates.iter().map(|(w, _)| w.len()).max().unwrap_or(8);
    let canvas_size = (max_len + 8).clamp(15, 21);
    let mut grid = vec![vec![b'#'; canvas_size]; canvas_size];
    let mut placed: Vec<PlacedCwWord> = Vec::with_capacity(candidates.len());
    let mut used = vec![false; candidates.len()];

    // Place longest anchor word horizontally in center of canvas.
    let (first_word, first_clue) = candidates[0].clone();
    let r0 = canvas_size / 2;
    let c0 = (canvas_size.saturating_sub(first_word.len())) / 2;
    commit_cw_word(&mut grid, &mut placed, first_word, first_clue, r0, c0, CwDir::Across);
    used[0] = true;

    // Multi-pass crossing search so later words can intersect words placed in earlier passes.
    for _pass in 0..candidates.len() {
        let mut progress = false;
        for idx in 1..candidates.len() {
            if used[idx] {
                continue;
            }
            let (ref word, ref clue) = candidates[idx];
            let bytes = word.as_bytes();
            let mut best: Option<(usize, usize, usize, CwDir)> = None; // (score, r0, c0, dir)

            for r in 0..canvas_size {
                for c in 0..canvas_size {
                    let existing = grid[r][c];
                    if existing == b'#' {
                        continue;
                    }
                    for (k, &ch) in bytes.iter().enumerate() {
                        if ch != existing {
                            continue;
                        }
                        if c >= k {
                            let start_c = c - k;
                            if can_place_cw(&grid, bytes, r, start_c, CwDir::Across) {
                                let crosses: usize = (0..bytes.len())
                                    .filter(|&i| grid[r][start_c + i] == bytes[i])
                                    .count();
                                if crosses >= 1 {
                                    let dist = r.abs_diff(canvas_size / 2)
                                        + (start_c + bytes.len() / 2).abs_diff(canvas_size / 2);
                                    let score = crosses * 100 + (40usize.saturating_sub(dist));
                                    if best.map(|b| score > b.0).unwrap_or(true) {
                                        best = Some((score, r, start_c, CwDir::Across));
                                    }
                                }
                            }
                        }
                        if r >= k {
                            let start_r = r - k;
                            if can_place_cw(&grid, bytes, start_r, c, CwDir::Down) {
                                let crosses: usize = (0..bytes.len())
                                    .filter(|&i| grid[start_r + i][c] == bytes[i])
                                    .count();
                                if crosses >= 1 {
                                    let dist = (start_r + bytes.len() / 2).abs_diff(canvas_size / 2)
                                        + c.abs_diff(canvas_size / 2);
                                    let score = crosses * 100 + (40usize.saturating_sub(dist));
                                    if best.map(|b| score > b.0).unwrap_or(true) {
                                        best = Some((score, start_r, c, CwDir::Down));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some((_, best_r, best_c, best_dir)) = best {
                commit_cw_word(
                    &mut grid,
                    &mut placed,
                    word.clone(),
                    clue.clone(),
                    best_r,
                    best_c,
                    best_dir,
                );
                used[idx] = true;
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }

    // Fallback for any word that has zero letter overlap with placed grid: append cleanly below with a '#' gap row.
    for idx in 0..candidates.len() {
        if used[idx] {
            continue;
        }
        let (ref word, ref clue) = candidates[idx];
        let bytes = word.as_bytes();
        let mut fallback_placed = false;
        'scan: for rr in 0..grid.len() {
            for cc in 0..grid[0].len() {
                if cc + bytes.len() <= grid[0].len()
                    && can_place_cw(&grid, bytes, rr, cc, CwDir::Across)
                {
                    commit_cw_word(
                        &mut grid,
                        &mut placed,
                        word.clone(),
                        clue.clone(),
                        rr,
                        cc,
                        CwDir::Across,
                    );
                    fallback_placed = true;
                    break 'scan;
                }
            }
        }
        if !fallback_placed {
            let needed_cols = grid[0].len().max(bytes.len() + 2);
            for row_vec in &mut grid {
                row_vec.resize(needed_cols, b'#');
            }
            grid.push(vec![b'#'; needed_cols]);
            let target_row = grid.len();
            grid.push(vec![b'#'; needed_cols]);
            let start_c = (needed_cols.saturating_sub(bytes.len())) / 2;
            commit_cw_word(
                &mut grid,
                &mut placed,
                word.clone(),
                clue.clone(),
                target_row,
                start_c,
                CwDir::Across,
            );
        }
        used[idx] = true;
    }

    // Trim bounding box around occupied cells.
    let mut min_r = grid.len();
    let mut max_r = 0usize;
    let mut min_c = grid[0].len();
    let mut max_c = 0usize;
    for (r, row_vec) in grid.iter().enumerate() {
        for (c, &ch) in row_vec.iter().enumerate() {
            if ch != b'#' {
                min_r = min_r.min(r);
                max_r = max_r.max(r);
                min_c = min_c.min(c);
                max_c = max_c.max(c);
            }
        }
    }

    let trimmed_rows = max_r.saturating_sub(min_r) + 1;
    let trimmed_cols = max_c.saturating_sub(min_c) + 1;
    let grid_strings: Vec<String> = (min_r..=max_r)
        .map(|r| {
            (min_c..=max_c)
                .map(|c| grid[r][c] as char)
                .collect::<String>()
        })
        .collect();

    let mut starts: BTreeSet<(usize, usize)> = BTreeSet::new();
    for p in &placed {
        starts.insert((p.row - min_r, p.col - min_c));
    }
    let mut number_map: HashMap<(usize, usize), usize> = HashMap::new();
    for (idx, pos) in starts.into_iter().enumerate() {
        number_map.insert(pos, idx + 1);
    }

    placed.sort_by(|a, b| {
        let dir_ord = (a.dir as u8).cmp(&(b.dir as u8));
        if dir_ord != std::cmp::Ordering::Equal {
            return dir_ord;
        }
        let na = number_map.get(&(a.row - min_r, a.col - min_c)).copied().unwrap_or(0);
        let nb = number_map.get(&(b.row - min_r, b.col - min_c)).copied().unwrap_or(0);
        na.cmp(&nb)
    });

    let clues_json: Vec<Value> = placed
        .iter()
        .map(|p| {
            let rr = p.row - min_r;
            let cc = p.col - min_c;
            let number = number_map.get(&(rr, cc)).copied().unwrap_or(1);
            json!({
                "number": number,
                "direction": p.dir.as_str(),
                "row": rr,
                "col": cc,
                "length": p.word.len(),
                "answer": p.word,
                "clue": p.clue,
            })
        })
        .collect();

    json!({
        "words": normalized_words_json,
        "rows": trimmed_rows,
        "cols": trimmed_cols,
        "grid": grid_strings,
        "clues": clues_json,
    })
}

fn ensure_compiled_crossword(config: &Value) -> Value {
    let has_grid = config
        .get("grid")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let has_clues = config
        .get("clues")
        .and_then(|v| v.as_array())
        .map(|a| {
            !a.is_empty()
                && a.iter()
                    .all(|c| c.get("answer").and_then(|x| x.as_str()).is_some())
        })
        .unwrap_or(false);
    if has_grid && has_clues {
        config.clone()
    } else {
        compile_crossword_interaction_config(config)
    }
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
        "CROSSWORD" => {
            let compiled = ensure_compiled_crossword(config);
            let rows = compiled.get("rows").and_then(|v| v.as_u64()).unwrap_or(0);
            let cols = compiled.get("cols").and_then(|v| v.as_u64()).unwrap_or(0);
            let masked_grid: Vec<String> = compiled
                .get("grid")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|row| row.as_str())
                        .map(|row_str| {
                            row_str
                                .chars()
                                .map(|ch| if ch == '#' { '#' } else { '.' })
                                .collect::<String>()
                        })
                        .collect()
                })
                .unwrap_or_default();

            let safe_clues: Vec<Value> = compiled
                .get("clues")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| {
                            let number = c.get("number").and_then(|v| v.as_u64())?;
                            let direction = c.get("direction").and_then(|v| v.as_str())?;
                            let row = c.get("row").and_then(|v| v.as_u64())?;
                            let col = c.get("col").and_then(|v| v.as_u64())?;
                            let length = c.get("length").and_then(|v| v.as_u64())?;
                            let clue = c.get("clue").and_then(|v| v.as_str())?;
                            Some(json!({
                                "number": number,
                                "direction": direction,
                                "row": row,
                                "col": col,
                                "length": length,
                                "clue": clue,
                            }))
                        })
                        .collect()
                })
                .unwrap_or_default();

            json!({
                "rows": rows,
                "cols": cols,
                "grid": masked_grid,
                "clues": safe_clues,
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
        "CROSSWORD" => {
            let compiled = ensure_compiled_crossword(config);
            let clues = compiled
                .get("clues")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let board: Vec<Vec<char>> = parsed_ans
                .get("board")
                .or_else(|| parsed_ans.get("grid"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.chars().collect::<Vec<char>>())
                        .collect()
                })
                .unwrap_or_default();

            let words_map = parsed_ans.get("words").and_then(|v| v.as_object());

            let mut correct = 0usize;
            for clue in &clues {
                let expected = normalize_crossword_answer(
                    clue.get("answer").and_then(|v| v.as_str()).unwrap_or(""),
                );
                if expected.is_empty() {
                    continue;
                }
                let row = clue.get("row").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let col = clue.get("col").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let dir = clue
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ACROSS");
                let len = clue
                    .get("length")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(expected.len() as u64) as usize;
                let number = clue.get("number").and_then(|v| v.as_u64()).unwrap_or(0);

                let mut from_board = String::with_capacity(len);
                if !board.is_empty() {
                    for k in 0..len {
                        let (rr, cc) = if dir.eq_ignore_ascii_case("DOWN") {
                            (row + k, col)
                        } else {
                            (row, col + k)
                        };
                        if let Some(ch) = board.get(rr).and_then(|r| r.get(cc)) {
                            if ch.is_ascii_alphabetic() {
                                from_board.push(ch.to_ascii_uppercase());
                            } else {
                                from_board.push('.');
                            }
                        }
                    }
                }

                let mut matched = from_board.len() == expected.len() && from_board == expected;
                if !matched {
                    if let Some(map) = words_map {
                        let key_u = format!("{}_{}", number, dir.to_ascii_uppercase());
                        let key_d = format!("{}-{}", number, dir.to_ascii_uppercase());
                        if let Some(raw_val) = map
                            .get(&key_u)
                            .or_else(|| map.get(&key_d))
                            .and_then(|v| v.as_str())
                        {
                            if normalize_crossword_answer(raw_val) == expected {
                                matched = true;
                            }
                        }
                    }
                }

                if matched {
                    correct += 1;
                }
            }

            let total = clues.len();
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

    #[test]
    fn crossword_compiles_masks_student_payload_and_grades_proportionally() {
        let cfg = json!({
            "words": [
                { "answer": "EVAPORASI", "clue": "Proses penguapan air dari permukaan bumi" },
                { "answer": "KONDENSASI", "clue": "Perubahan uap air menjadi titik air di awan" },
                { "answer": "HUJAN", "clue": "Titik air yang jatuh dari awan ke permukaan bumi" },
                { "answer": "MATAHARI", "clue": "Sumber energi utama siklus air" }
            ]
        });

        let compiled = compile_crossword_interaction_config(&cfg);
        let clues = compiled["clues"].as_array().unwrap();
        assert_eq!(clues.len(), 4);
        assert!(compiled["rows"].as_u64().unwrap() > 0);
        assert!(compiled["cols"].as_u64().unwrap() > 0);

        // Student view must mask every letter cell to '.' and omit 'words' & clue 'answer'
        let student_cfg = shuffle_interaction_for_student("CROSSWORD", &compiled);
        assert!(student_cfg.get("words").is_none());
        for row_val in student_cfg["grid"].as_array().unwrap() {
            let row_str = row_val.as_str().unwrap();
            assert!(row_str.chars().all(|c| c == '#' || c == '.'));
        }
        for c in student_cfg["clues"].as_array().unwrap() {
            assert!(c.get("answer").is_none());
            assert!(c.get("clue").and_then(|v| v.as_str()).is_some());
        }

        // Simulate student solving half (2 of 4) via board cells + 1 wrong
        let teacher_grid: Vec<Vec<char>> = compiled["grid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap().chars().collect())
            .collect();
        let mut board_chars: Vec<Vec<char>> = student_cfg["grid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap().chars().collect())
            .collect();

        for clue in clues.iter().take(2) {
            let r0 = clue["row"].as_u64().unwrap() as usize;
            let c0 = clue["col"].as_u64().unwrap() as usize;
            let len = clue["length"].as_u64().unwrap() as usize;
            let dir = clue["direction"].as_str().unwrap();
            for k in 0..len {
                let (rr, cc) = if dir == "DOWN" { (r0 + k, c0) } else { (r0, c0 + k) };
                board_chars[rr][cc] = teacher_grid[rr][cc];
            }
        }
        let partial_board: Vec<String> = board_chars
            .iter()
            .map(|row| row.iter().collect::<String>())
            .collect();
        let res_half = grade_interactive_answer(
            "CROSSWORD",
            &compiled,
            Some(&json!({ "board": partial_board }).to_string()),
            100,
        );
        assert_eq!(res_half.correct_count, 2);
        assert_eq!(res_half.total_count, 4);
        assert_eq!(res_half.points_earned, 50);
        assert!(!res_half.is_fully_correct);

        // Full solved grid scores 100
        let full_board: Vec<String> = teacher_grid
            .iter()
            .map(|row| row.iter().collect::<String>())
            .collect();
        let res_full = grade_interactive_answer(
            "CROSSWORD",
            &compiled,
            Some(&json!({ "board": full_board }).to_string()),
            100,
        );
        assert_eq!(res_full.correct_count, 4);
        assert_eq!(res_full.points_earned, 100);
        assert!(res_full.is_fully_correct);
    }

    #[test]
    fn crossword_preserves_words_without_common_letters() {
        let cfg = json!({
            "words": [
                { "answer": "BBB", "clue": "Clue 1" },
                { "answer": "CCC", "clue": "Clue 2" },
                { "answer": "DDD", "clue": "Clue 3" }
            ]
        });
        let compiled = compile_crossword_interaction_config(&cfg);
        assert_eq!(compiled["clues"].as_array().unwrap().len(), 3);
    }
}
