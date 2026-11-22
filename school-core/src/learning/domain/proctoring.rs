//! Domain logic for CBT Proctoring, Schedule Phase Evaluation, Conflict Detection,
//! and Proctor Token / Extension invariants (ADR-0001, ADR-0011, ADR-0013).
//!
//! Strictly pure domain logic: no infrastructure (`sqlx`) or HTTP (`axum`) imports allowed.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Canonical backend-determined execution phase of an assessment / CBT quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExamPhase {
    Draft,
    Upcoming,
    Live,
    Ended,
}

impl ExamPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Upcoming => "UPCOMING",
            Self::Live => "LIVE",
            Self::Ended => "ENDED",
        }
    }
}

/// Canonical backend-determined status for each rostered student in a live CBT monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProctorParticipantStatus {
    NotStarted,
    InProgress,
    NeedGrading,
    Completed,
}

impl ProctorParticipantStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotStarted => "NOT_STARTED",
            Self::InProgress => "IN_PROGRESS",
            Self::NeedGrading => "NEED_GRADING",
            Self::Completed => "COMPLETED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuizScheduleInput<'a> {
    pub status: &'a str,
    pub is_active: bool,
    pub questions_count: i32,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub release_at: Option<DateTime<Utc>>,
    pub student_attempt_status: Option<&'a str>,
    pub student_has_completed: bool,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExamWindowEvaluation {
    pub phase: ExamPhase,
    pub is_open_for_student: bool,
    pub seconds_until_start: Option<i64>,
    pub seconds_remaining: Option<i64>,
}

/// Computes the deterministic schedule phase and student availability strictly on the server.
pub fn evaluate_quiz_schedule(input: QuizScheduleInput<'_>) -> ExamWindowEvaluation {
    let status_lower = input.status.trim().to_lowercase();
    let effective_start = input.release_at.or(input.start_at);

    let phase = if !input.is_active || status_lower == "closed" || status_lower == "archived" {
        ExamPhase::Ended
    } else if status_lower == "published" || status_lower == "active" {
        if effective_start.is_some_and(|start| input.now < start) {
            ExamPhase::Upcoming
        } else if input.end_at.is_some_and(|end| input.now > end) {
            ExamPhase::Ended
        } else {
            ExamPhase::Live
        }
    } else if status_lower == "scheduled"
        || (status_lower == "draft"
            && input.questions_count > 0
            && effective_start.is_some_and(|start| input.now < start))
    {
        ExamPhase::Upcoming
    } else {
        ExamPhase::Draft
    };

    let is_in_progress = input
        .student_attempt_status
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("in_progress"));

    let is_published_open =
        input.is_active && (status_lower == "published" || status_lower == "active");

    let is_open_for_student = is_published_open
        && !input.student_has_completed
        && (phase == ExamPhase::Live || (is_in_progress && phase != ExamPhase::Draft));

    let seconds_until_start = if phase == ExamPhase::Upcoming {
        effective_start.map(|start| (start - input.now).num_seconds().max(0))
    } else {
        None
    };

    let seconds_remaining = if phase == ExamPhase::Live {
        input
            .end_at
            .map(|end| (end - input.now).num_seconds().max(0))
    } else {
        None
    };

    ExamWindowEvaluation {
        phase,
        is_open_for_student,
        seconds_until_start,
        seconds_remaining,
    }
}

/// Maps raw `quiz_attempts.status` (or `None` for students who have not started)
/// into a canonical `ProctorParticipantStatus`.
pub fn classify_proctor_participant_status(
    attempt_status: Option<&str>,
) -> ProctorParticipantStatus {
    match attempt_status.map(|s| s.trim().to_lowercase()).as_deref() {
        None => ProctorParticipantStatus::NotStarted,
        Some("created" | "started" | "in_progress") => ProctorParticipantStatus::InProgress,
        Some("graded" | "reviewed") => ProctorParticipantStatus::Completed,
        Some(_) => ProctorParticipantStatus::NeedGrading,
    }
}

/// Determines whether a recorded telemetry event should increment student violation count
/// (vs harmless periodic heartbeat pings).
pub fn is_violation_event_type(event_type: &str) -> bool {
    matches!(
        event_type.trim().to_uppercase().as_str(),
        "APP_BACKGROUND" | "TAB_SWITCH" | "WINDOW_BLUR" | "EXIT_ATTEMPT" | "SCREEN_SPLIT"
    )
}

#[derive(Debug, Clone)]
pub struct ScheduleConflictInput {
    pub quiz_id: Uuid,
    pub title: String,
    pub class_id: Option<Uuid>,
    pub class_name: Option<String>,
    pub teacher_id: Option<Uuid>,
    pub teacher_name: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: Option<DateTime<Utc>>,
    pub duration_minutes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProctorScheduleConflict {
    pub conflict_type: String,
    pub primary_quiz_id: Uuid,
    pub primary_title: String,
    pub conflicting_quiz_id: Uuid,
    pub conflicting_title: String,
    pub scope_label: String,
    pub overlap_start: DateTime<Utc>,
    pub overlap_end: DateTime<Utc>,
}

/// Detects overlapping scheduled CBT windows within the same class or teacher.
pub fn detect_proctor_schedule_conflicts(
    sessions: &[ScheduleConflictInput],
) -> Vec<ProctorScheduleConflict> {
    let resolved_end = |item: &ScheduleConflictInput| -> DateTime<Utc> {
        let dur_mins = i64::from(item.duration_minutes.max(15));
        item.end_at
            .filter(|e| *e > item.start_at)
            .unwrap_or_else(|| item.start_at + Duration::minutes(dur_mins))
    };

    let mut conflicts = Vec::new();
    for i in 0..sessions.len() {
        let a = &sessions[i];
        let a_end = resolved_end(a);

        for b in sessions.iter().skip(i + 1) {
            let b_end = resolved_end(b);
            let overlap_start = a.start_at.max(b.start_at);
            let overlap_end = a_end.min(b_end);
            if overlap_start >= overlap_end {
                continue;
            }

            if let (Some(cid_a), Some(cid_b)) = (a.class_id, b.class_id) {
                if cid_a == cid_b {
                    conflicts.push(ProctorScheduleConflict {
                        conflict_type: "CLASS_OVERLAP".to_string(),
                        primary_quiz_id: a.quiz_id,
                        primary_title: a.title.clone(),
                        conflicting_quiz_id: b.quiz_id,
                        conflicting_title: b.title.clone(),
                        scope_label: a
                            .class_name
                            .clone()
                            .unwrap_or_else(|| "Rombel Sama".to_string()),
                        overlap_start,
                        overlap_end,
                    });
                    continue;
                }
            }

            if let (Some(tid_a), Some(tid_b)) = (a.teacher_id, b.teacher_id) {
                if tid_a == tid_b {
                    conflicts.push(ProctorScheduleConflict {
                        conflict_type: "TEACHER_OVERLAP".to_string(),
                        primary_quiz_id: a.quiz_id,
                        primary_title: a.title.clone(),
                        conflicting_quiz_id: b.quiz_id,
                        conflicting_title: b.title.clone(),
                        scope_label: a
                            .teacher_name
                            .clone()
                            .unwrap_or_else(|| "Guru Pengawas Sama".to_string()),
                        overlap_start,
                        overlap_end,
                    });
                }
            }
        }
    }

    conflicts
}

/// Computes an extended session end timestamp, anchored at `max(current_end_at, now)`.
pub fn compute_extended_end_at(
    current_end_at: Option<DateTime<Utc>>,
    extra_minutes: i32,
    now: DateTime<Utc>,
) -> Result<DateTime<Utc>, &'static str> {
    if !(1..=180).contains(&extra_minutes) {
        return Err("Perpanjangan waktu ujian harus berada di antara 1 sampai 180 menit.");
    }
    let anchor = current_end_at.map_or(now, |existing| existing.max(now));
    Ok(anchor + Duration::minutes(i64::from(extra_minutes)))
}

/// Generates a 6-character ambiguity-free uppercase exam token (excludes 0/O and 1/I/L).
pub fn generate_ambiguity_free_exam_token(custom: Option<&str>) -> Result<String, &'static str> {
    if let Some(raw) = custom.map(str::trim).filter(|s| !s.is_empty()) {
        let normalized: String = raw
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if normalized.len() < 4 || normalized.len() > 10 {
            return Err("Token ujian kustom harus terdiri dari 4 sampai 10 karakter.");
        }
        return Ok(normalized);
    }

    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let seed = Uuid::now_v7();
    let digest = Sha256::digest(seed.as_bytes());
    let mut token = String::with_capacity(6);
    for idx in 0..6 {
        let byte = digest[idx];
        let ch = ALPHABET[(byte as usize) % ALPHABET.len()] as char;
        token.push(ch);
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn evaluate_quiz_schedule_handles_upcoming_live_ended_and_draft() {
        let now = Utc.with_ymd_and_hms(2026, 11, 23, 2, 0, 0).unwrap();
        let future_start = now + Duration::minutes(45);
        let past_start = now - Duration::minutes(30);
        let future_end = now + Duration::minutes(60);
        let past_end = now - Duration::minutes(5);

        let upcoming_scheduled = evaluate_quiz_schedule(QuizScheduleInput {
            status: "draft",
            is_active: true,
            questions_count: 10,
            start_at: Some(future_start),
            end_at: Some(future_start + Duration::minutes(90)),
            release_at: Some(future_start),
            student_attempt_status: None,
            student_has_completed: false,
            now,
        });
        assert_eq!(upcoming_scheduled.phase, ExamPhase::Upcoming);
        assert!(!upcoming_scheduled.is_open_for_student);
        assert_eq!(upcoming_scheduled.seconds_until_start, Some(2700));

        let live_eval = evaluate_quiz_schedule(QuizScheduleInput {
            status: "published",
            is_active: true,
            questions_count: 10,
            start_at: Some(past_start),
            end_at: Some(future_end),
            release_at: Some(past_start),
            student_attempt_status: None,
            student_has_completed: false,
            now,
        });
        assert_eq!(live_eval.phase, ExamPhase::Live);
        assert!(live_eval.is_open_for_student);
        assert_eq!(live_eval.seconds_remaining, Some(3600));

        let ended_eval = evaluate_quiz_schedule(QuizScheduleInput {
            status: "published",
            is_active: true,
            questions_count: 10,
            start_at: Some(past_start),
            end_at: Some(past_end),
            release_at: Some(past_start),
            student_attempt_status: None,
            student_has_completed: false,
            now,
        });
        assert_eq!(ended_eval.phase, ExamPhase::Ended);
        assert!(!ended_eval.is_open_for_student);

        let pure_draft = evaluate_quiz_schedule(QuizScheduleInput {
            status: "draft",
            is_active: true,
            questions_count: 0,
            start_at: Some(future_start),
            end_at: None,
            release_at: Some(future_start),
            student_attempt_status: None,
            student_has_completed: false,
            now,
        });
        assert_eq!(pure_draft.phase, ExamPhase::Draft);
    }

    #[test]
    fn detect_proctor_schedule_conflicts_flags_same_class_and_same_teacher() {
        let base = Utc.with_ymd_and_hms(2026, 11, 23, 1, 0, 0).unwrap();
        let class_a = Uuid::now_v7();
        let class_b = Uuid::now_v7();
        let teacher_1 = Uuid::now_v7();
        let teacher_2 = Uuid::now_v7();

        let conflicts = detect_proctor_schedule_conflicts(&[
            ScheduleConflictInput {
                quiz_id: Uuid::now_v7(),
                title: "SAS Matematika".to_string(),
                class_id: Some(class_a),
                class_name: Some("Paket A 4".to_string()),
                teacher_id: Some(teacher_1),
                teacher_name: Some("Budi".to_string()),
                start_at: base,
                end_at: Some(base + Duration::minutes(90)),
                duration_minutes: 90,
            },
            ScheduleConflictInput {
                quiz_id: Uuid::now_v7(),
                title: "SAS IPAS".to_string(),
                class_id: Some(class_a),
                class_name: Some("Paket A 4".to_string()),
                teacher_id: Some(teacher_2),
                teacher_name: Some("Siti".to_string()),
                start_at: base + Duration::minutes(30),
                end_at: Some(base + Duration::minutes(120)),
                duration_minutes: 90,
            },
            ScheduleConflictInput {
                quiz_id: Uuid::now_v7(),
                title: "SAS IPS".to_string(),
                class_id: Some(class_b),
                class_name: Some("Paket B 7".to_string()),
                teacher_id: Some(teacher_1),
                teacher_name: Some("Budi".to_string()),
                start_at: base + Duration::minutes(45),
                end_at: Some(base + Duration::minutes(105)),
                duration_minutes: 60,
            },
        ]);

        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].conflict_type, "CLASS_OVERLAP");
        assert_eq!(conflicts[1].conflict_type, "TEACHER_OVERLAP");
    }

    #[test]
    fn token_generation_and_time_extension_respect_bounds() {
        let tok = generate_ambiguity_free_exam_token(None).unwrap();
        assert_eq!(tok.len(), 6);
        assert!(!tok.contains(['0', 'O', '1', 'I', 'L']));

        assert!(generate_ambiguity_free_exam_token(Some("ab")).is_err());
        assert_eq!(
            generate_ambiguity_free_exam_token(Some(" pas26 ")).unwrap(),
            "PAS26"
        );

        let now = Utc.with_ymd_and_hms(2026, 11, 23, 3, 0, 0).unwrap();
        let past_end = now - Duration::minutes(10);
        let extended = compute_extended_end_at(Some(past_end), 15, now).unwrap();
        assert_eq!(extended, now + Duration::minutes(15));
        assert!(compute_extended_end_at(None, 0, now).is_err());
    }
}
