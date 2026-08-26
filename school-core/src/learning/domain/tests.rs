#[cfg(test)]
mod tests {
    use crate::common::domain::clock::SystemClock;
    use crate::learning::domain::curriculum::Curriculum;
    use crate::learning::domain::syllabus::Syllabus;
    use uuid::Uuid;

    #[test]
    fn test_create_curriculum_success() {
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let curriculum = Curriculum::new(
            tenant_id,
            "KUR-MERDEKA".to_string(),
            "Kurikulum Merdeka 2026".to_string(),
            Some("Kurikulum Nasional".to_string()),
            &clock,
        );

        assert_eq!(curriculum.tenant_id, tenant_id);
        assert_eq!(curriculum.code, "KUR-MERDEKA");
        assert_eq!(curriculum.name, "Kurikulum Merdeka 2026");
        assert_eq!(
            curriculum.description.as_deref(),
            Some("Kurikulum Nasional")
        );
        assert!(curriculum.is_active);
        assert_eq!(curriculum.version, 1);
    }

    #[test]
    #[should_panic(expected = "tenant_id must not be nil")]
    fn test_create_curriculum_nil_tenant() {
        let clock = SystemClock;
        Curriculum::new(
            Uuid::nil(),
            "KUR-1".to_string(),
            "Kurikulum".to_string(),
            None,
            &clock,
        );
    }

    #[test]
    #[should_panic(expected = "code must not be empty")]
    fn test_create_curriculum_empty_code() {
        let clock = SystemClock;
        Curriculum::new(
            Uuid::now_v7(),
            "".to_string(),
            "Kurikulum".to_string(),
            None,
            &clock,
        );
    }

    #[test]
    fn test_create_syllabus_success() {
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let curriculum_id = Uuid::now_v7();
        let subject_id = Uuid::now_v7();

        let syllabus = Syllabus::new(
            tenant_id,
            curriculum_id,
            subject_id,
            None,
            "SIL-MTK-10".to_string(),
            "Silabus Matematika Kelas 10".to_string(),
            Some("Silabus Matematika Wajib".to_string()),
            &clock,
        );

        assert_eq!(syllabus.tenant_id, tenant_id);
        assert_eq!(syllabus.curriculum_id, curriculum_id);
        assert_eq!(syllabus.subject_id, subject_id);
        assert_eq!(syllabus.code, "SIL-MTK-10");
        assert_eq!(syllabus.name, "Silabus Matematika Kelas 10");
        assert!(syllabus.is_active);
    }

    #[test]
    #[should_panic(expected = "curriculum_id must not be nil")]
    fn test_create_syllabus_nil_curriculum() {
        let clock = SystemClock;
        Syllabus::new(
            Uuid::now_v7(),
            Uuid::nil(),
            Uuid::now_v7(),
            None,
            "SIL-1".to_string(),
            "Silabus".to_string(),
            None,
            &clock,
        );
    }

    #[test]
    fn test_create_learning_material_success() {
        use crate::learning::domain::learning_material::LearningMaterial;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();

        let material = LearningMaterial::new(
            tenant_id,
            Some(lesson_id),
            "PDF".to_string(),
            "Modul Aljabar Linier".to_string(),
            Some("Bahan ajar bab 1".to_string()),
            Some("storage/modul_1.pdf".to_string()),
            None,
            1,
            "published".to_string(),
            &clock,
        );

        assert_eq!(material.tenant_id, tenant_id);
        assert_eq!(material.lesson_id, Some(lesson_id));
        assert_eq!(material.material_type, "PDF");
        assert_eq!(material.title, "Modul Aljabar Linier");
        assert_eq!(material.order_index, 1);
        assert!(material.is_active);
    }

    #[test]
    #[should_panic(expected = "title must not be empty")]
    fn test_create_learning_material_empty_title() {
        use crate::learning::domain::learning_material::LearningMaterial;
        let clock = SystemClock;
        LearningMaterial::new(
            Uuid::now_v7(),
            None,
            "Video".to_string(),
            "".to_string(),
            None,
            None,
            None,
            1,
            "draft".to_string(),
            &clock,
        );
    }

    #[test]
    fn test_lesson_aggregate_publish_without_materials_fails() {
        use crate::learning::domain::lesson::Lesson;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let syllabus_id = Uuid::now_v7();

        let mut lesson = Lesson::new(
            tenant_id,
            syllabus_id,
            "LES-01".to_string(),
            "Persamaan Kuadrat".to_string(),
            None,
            None,
            45,
            1,
            "draft".to_string(),
            &clock,
        );

        let res = lesson.publish(0, &clock);
        assert!(res.is_err());
        assert_eq!(lesson.status, "draft");
    }

    #[test]
    fn test_lesson_aggregate_publish_with_materials_succeeds() {
        use crate::common::domain::aggregate::AggregateRoot;
        use crate::learning::domain::lesson::Lesson;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let syllabus_id = Uuid::now_v7();

        let mut lesson = Lesson::new(
            tenant_id,
            syllabus_id,
            "LES-02".to_string(),
            "Fungsi Kuadrat".to_string(),
            None,
            None,
            45,
            2,
            "draft".to_string(),
            &clock,
        );

        let res = lesson.publish(1, &clock);
        assert!(res.is_ok());
        assert_eq!(lesson.status, "published");

        let events = lesson.take_events();
        assert!(!events.is_empty());
    }

    #[test]
    fn test_lesson_aggregate_cannot_modify_archived() {
        use crate::learning::domain::lesson::Lesson;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let syllabus_id = Uuid::now_v7();

        let mut lesson = Lesson::new(
            tenant_id,
            syllabus_id,
            "LES-03".to_string(),
            "Materi Lama".to_string(),
            None,
            None,
            45,
            3,
            "draft".to_string(),
            &clock,
        );

        let _ = lesson.archive(&clock);
        assert_eq!(lesson.status, "archived");

        let update_res = lesson.update(Some("Materi Baru".to_string()), None, None, None, &clock);
        assert!(update_res.is_err());
    }

    #[test]
    fn test_assignment_due_date_must_be_in_future() {
        use crate::common::domain::clock::Clock;
        use crate::learning::domain::assignment::Assignment;
        use chrono::Duration;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();
        let past_due = clock.now() - Duration::hours(1);

        let res = Assignment::new(
            tenant_id,
            lesson_id,
            "Tugas Aljabar".to_string(),
            None,
            None,
            100,
            Some(past_due),
            "individual".to_string(),
            &clock,
        );

        assert!(res.is_err());
    }

    #[test]
    fn test_assignment_cannot_publish_if_lesson_not_published() {
        use crate::common::domain::clock::Clock;
        use crate::learning::domain::assignment::Assignment;
        use chrono::Duration;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();
        let future_due = clock.now() + Duration::days(3);

        let mut assignment = Assignment::new(
            tenant_id,
            lesson_id,
            "Tugas Matriks".to_string(),
            None,
            None,
            100,
            Some(future_due),
            "individual".to_string(),
            &clock,
        )
        .unwrap();

        let publish_res = assignment.publish("draft", &clock);
        assert!(publish_res.is_err());
        assert_eq!(assignment.status, "draft");
    }

    #[test]
    fn test_assignment_publish_and_close_flow_succeeds() {
        use crate::common::domain::aggregate::AggregateRoot;
        use crate::common::domain::clock::Clock;
        use crate::learning::domain::assignment::Assignment;
        use chrono::Duration;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();
        let future_due = clock.now() + Duration::days(5);

        let mut assignment = Assignment::new(
            tenant_id,
            lesson_id,
            "Tugas Geometri".to_string(),
            None,
            None,
            100,
            Some(future_due),
            "individual".to_string(),
            &clock,
        )
        .unwrap();

        let publish_res = assignment.publish("published", &clock);
        assert!(publish_res.is_ok());
        assert_eq!(assignment.status, "published");

        let close_res = assignment.close(&clock);
        assert!(close_res.is_ok());
        assert_eq!(assignment.status, "closed");

        let events = assignment.take_events();
        assert!(!events.is_empty());
    }

    #[test]
    fn test_assignment_cannot_update_when_closed() {
        use crate::common::domain::clock::Clock;
        use crate::learning::domain::assignment::Assignment;
        use chrono::Duration;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();
        let future_due = clock.now() + Duration::days(2);

        let mut assignment = Assignment::new(
            tenant_id,
            lesson_id,
            "Tugas Vektor".to_string(),
            None,
            None,
            100,
            Some(future_due),
            "individual".to_string(),
            &clock,
        )
        .unwrap();

        let _ = assignment.publish("published", &clock);
        let _ = assignment.close(&clock);

        let update_res = assignment.update(
            Some("Judul Baru".to_string()),
            None,
            None,
            None,
            None,
            &clock,
        );
        assert!(update_res.is_err());
    }

    #[test]
    fn test_submission_aggregate_creation_and_attempt_tracking() {
        use crate::common::domain::clock::Clock;
        use crate::learning::domain::assignment_submission::AssignmentSubmission;
        use chrono::Duration;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let assignment_id = Uuid::now_v7();
        let student_id = Uuid::now_v7();
        let future_due = clock.now() + Duration::days(2);

        let mut submission = AssignmentSubmission::new(
            tenant_id,
            assignment_id,
            student_id,
            Some("Jawaban Percobaan 1".to_string()),
            None,
            &clock,
        );

        assert_eq!(submission.attempts.len(), 1);
        assert_eq!(submission.status, "submitted");

        let attempt_res = submission.add_attempt(
            Some("Jawaban Revisi Percobaan 2".to_string()),
            None,
            None,
            "published",
            Some(future_due),
            3,
            &clock,
        );

        assert!(attempt_res.is_ok());
        assert_eq!(submission.attempts.len(), 2);
        assert!(!attempt_res.unwrap().is_late);
    }

    #[test]
    fn test_submission_cannot_add_attempt_when_assignment_not_published() {
        use crate::learning::domain::assignment_submission::AssignmentSubmission;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let assignment_id = Uuid::now_v7();
        let student_id = Uuid::now_v7();

        let mut submission = AssignmentSubmission::new(
            tenant_id,
            assignment_id,
            student_id,
            Some("Draft".to_string()),
            None,
            &clock,
        );

        let res = submission.add_attempt(
            Some("Draft 2".to_string()),
            None,
            None,
            "draft",
            None,
            3,
            &clock,
        );

        assert!(res.is_err());
    }

    #[test]
    fn test_submission_grade_validation() {
        use crate::learning::domain::assignment_submission::AssignmentSubmission;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let assignment_id = Uuid::now_v7();
        let student_id = Uuid::now_v7();
        let grader_id = Uuid::now_v7();

        let mut submission = AssignmentSubmission::new(
            tenant_id,
            assignment_id,
            student_id,
            Some("Jawaban Final".to_string()),
            None,
            &clock,
        );

        let invalid_grade = submission.grade(
            150,
            100,
            Some("Nilai kebesaran".to_string()),
            grader_id,
            &clock,
        );
        assert!(invalid_grade.is_err());

        let valid_grade =
            submission.grade(95, 100, Some("Sangat baik!".to_string()), grader_id, &clock);
        assert!(valid_grade.is_ok());
        assert_eq!(submission.status, "graded");
        assert_eq!(submission.score, Some(95));
    }

    #[test]
    fn test_quiz_cannot_publish_without_questions() {
        use crate::learning::domain::quiz::Quiz;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();

        let mut quiz = Quiz::new(
            tenant_id,
            lesson_id,
            "Kuis Matematika".to_string(),
            None,
            30,
            70,
            2,
            false,
            false,
            None,
            None,
            &clock,
        )
        .unwrap();

        let publish_res = quiz.publish("published", &clock);
        assert!(publish_res.is_err());
    }

    #[test]
    fn test_quiz_publish_with_questions_succeeds() {
        use crate::learning::domain::quiz::Quiz;
        use crate::learning::domain::quiz_choice::QuizChoice;
        use crate::learning::domain::quiz_question::QuizQuestion;

        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();

        let mut quiz = Quiz::new(
            tenant_id,
            lesson_id,
            "Kuis Fisika".to_string(),
            None,
            45,
            75,
            1,
            true,
            true,
            None,
            None,
            &clock,
        )
        .unwrap();

        let q_id = Uuid::now_v7();
        let c1 = QuizChoice::new(q_id, "Jawaban Benar".to_string(), true, 1);
        let c2 = QuizChoice::new(q_id, "Jawaban Salah".to_string(), false, 2);
        let question = QuizQuestion::new(
            quiz.id,
            "Berapa kecepatan cahaya?".to_string(),
            "multiple_choice".to_string(),
            10,
            1,
            vec![c1, c2],
        );

        let add_res = quiz.add_question(question, &clock);
        assert!(add_res.is_ok());
        assert_eq!(quiz.questions_count, 1);
        assert_eq!(quiz.max_score, 10);

        let publish_res = quiz.publish("published", &clock);
        assert!(publish_res.is_ok());
        assert_eq!(quiz.status, "published");
    }

    #[test]
    fn test_quiz_attempt_auto_grade_and_pass_fail() {
        use crate::learning::domain::attempt_answer::AttemptAnswer;
        use crate::learning::domain::quiz::Quiz;
        use crate::learning::domain::quiz_attempt::QuizAttempt;
        use crate::learning::domain::quiz_choice::QuizChoice;
        use crate::learning::domain::quiz_question::QuizQuestion;

        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let lesson_id = Uuid::now_v7();
        let student_id = Uuid::now_v7();

        let mut quiz = Quiz::new(
            tenant_id,
            lesson_id,
            "Kuis Biologi".to_string(),
            None,
            30,
            60,
            2,
            false,
            false,
            None,
            None,
            &clock,
        )
        .unwrap();

        let q1_id = Uuid::now_v7();
        let c1_correct = QuizChoice::new(q1_id, "Sel".to_string(), true, 1);
        let c1_wrong = QuizChoice::new(q1_id, "Atom".to_string(), false, 2);
        let q1 = QuizQuestion::new(
            quiz.id,
            "Unit terkecil kehidupan?".to_string(),
            "multiple_choice".to_string(),
            100,
            1,
            vec![c1_correct.clone(), c1_wrong],
        );

        let _ = quiz.add_question(q1.clone(), &clock);
        let _ = quiz.publish("published", &clock);

        let mut attempt = QuizAttempt::start_new(tenant_id, &quiz, student_id, 0, &clock).unwrap();

        let answer = AttemptAnswer::new(attempt.id, q1.id, Some(c1_correct.id), None);
        attempt.add_answer(answer);

        let grade_res = attempt.auto_grade(&quiz, &clock);
        assert!(grade_res.is_ok());

        let (score, passed) = grade_res.unwrap();
        assert_eq!(score, 100);
        assert!(passed);
        assert_eq!(attempt.status, "auto_graded");
    }

    #[test]
    fn test_assessment_rule_100_percent_weight_invariant() {
        use crate::learning::domain::assessment_rule::AssessmentRule;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let class_id = Uuid::now_v7();
        let subject_id = Uuid::now_v7();

        let mut rule =
            AssessmentRule::new(tenant_id, class_id, subject_id, None, 70.0, &clock).unwrap();

        let _ = rule.add_component(
            "Assignment".to_string(),
            "assignment".to_string(),
            40.0,
            true,
            1,
        );
        let _ = rule.add_component("Quiz".to_string(), "quiz".to_string(), 40.0, true, 2);

        // Sum = 80.0%, should fail activation
        let activate_fail = rule.activate(&clock);
        assert!(activate_fail.is_err());

        // Add 20.0% Midterm component to reach 100%
        let _ = rule.add_component("Midterm".to_string(), "midterm".to_string(), 20.0, true, 3);
        let activate_ok = rule.activate(&clock);
        assert!(activate_ok.is_ok());
        assert_eq!(rule.status, "active");
        assert!(rule.is_active);
    }

    #[test]
    fn test_assessment_rule_unique_component_types() {
        use crate::learning::domain::assessment_rule::AssessmentRule;
        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let class_id = Uuid::now_v7();
        let subject_id = Uuid::now_v7();

        let mut rule =
            AssessmentRule::new(tenant_id, class_id, subject_id, None, 75.0, &clock).unwrap();

        let res1 = rule.add_component(
            "Tugas 1".to_string(),
            "assignment".to_string(),
            50.0,
            true,
            1,
        );
        assert!(res1.is_ok());

        // Adding duplicate component_type 'assignment' should fail
        let res2 = rule.add_component(
            "Tugas 2".to_string(),
            "assignment".to_string(),
            50.0,
            true,
            2,
        );
        assert!(res2.is_err());
    }

    #[test]
    fn test_final_grade_calculator_weighted_score_calculation() {
        use crate::learning::domain::assessment_rule::AssessmentRule;
        use crate::learning::domain::final_grade_calculator::FinalGradeCalculator;
        use crate::learning::domain::gradebook::GradeBook;

        let clock = SystemClock;
        let tenant_id = Uuid::now_v7();
        let student_id = Uuid::now_v7();
        let class_id = Uuid::now_v7();
        let subject_id = Uuid::now_v7();

        let mut rule =
            AssessmentRule::new(tenant_id, class_id, subject_id, None, 70.0, &clock).unwrap();
        let _ = rule.add_component(
            "Assignment".to_string(),
            "assignment".to_string(),
            30.0,
            true,
            1,
        );
        let _ = rule.add_component("Quiz".to_string(), "quiz".to_string(), 30.0, true, 2);
        let _ = rule.add_component("Final Exam".to_string(), "final".to_string(), 40.0, true, 3);
        let _ = rule.activate(&clock);

        let mut gradebook =
            GradeBook::new(tenant_id, student_id, class_id, subject_id, None, &clock).unwrap();

        // Assignment: 90 / 100 (30% weight) => 27 points
        let _ = gradebook.record_grade(
            "assignment".to_string(),
            None,
            "Assignment".to_string(),
            90.0,
            100.0,
            30.0,
            &clock,
        );
        // Quiz: 80 / 100 (30% weight) => 24 points
        let _ = gradebook.record_grade(
            "quiz".to_string(),
            None,
            "Quiz".to_string(),
            80.0,
            100.0,
            30.0,
            &clock,
        );
        // Final: 85 / 100 (40% weight) => 34 points
        let _ = gradebook.record_grade(
            "final".to_string(),
            None,
            "Final Exam".to_string(),
            85.0,
            100.0,
            40.0,
            &clock,
        );

        // Total Expected = 27 + 24 + 34 = 85.0 (Letter Grade B, Passed)
        let calc_res = FinalGradeCalculator::calculate(&rule, &mut gradebook, &clock);
        assert!(calc_res.is_ok());

        let (final_score, letter_grade, passed) = calc_res.unwrap();
        assert_eq!(final_score, 85.0);
        assert_eq!(letter_grade, "B");
        assert!(passed);
        assert_eq!(gradebook.status, "calculated");
    }
}
