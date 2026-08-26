use super::student::{Student, StudentStatus};
use super::teacher::Teacher;
use uuid::Uuid;

#[cfg(test)]
mod domain_tests {
    use super::*;
    use crate::common::domain::clock::SystemClock;

    fn register_test_student(
        tenant_id: Uuid,
        nisn: &str,
        full_name: &str,
        guardian_id: Option<Uuid>,
        clock: &SystemClock,
    ) -> Result<Student, String> {
        Student::register(
            tenant_id,
            nisn.to_string(),
            full_name.to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            guardian_id,
            clock,
        )
    }

    #[test]
    fn test_create_teacher() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let teacher = Teacher::new(
            tenant_id,
            "Budi Santoso".to_string(),
            Some("19800101".to_string()),
            &clock,
        );

        assert_eq!(teacher.tenant_id, tenant_id);
        assert_eq!(teacher.full_name, "Budi Santoso");
        assert_eq!(teacher.nip.as_deref().unwrap(), "19800101");
        assert!(teacher.is_active);
    }

    #[test]
    fn test_register_student_success() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let result = register_test_student(
            tenant_id,
            "1234567890",
            "Andi Pratama",
            None,
            &clock,
        );

        assert!(result.is_ok());
        let student = result.unwrap();
        assert_eq!(student.tenant_id, tenant_id);
        assert_eq!(student.full_name, "Andi Pratama");
        assert_eq!(student.nisn, "1234567890");
        assert!(student.guardian_id.is_none());
        assert_eq!(student.status, StudentStatus::Pending);
    }

    #[test]
    #[ignore = "NISN strict length validation relaxed for Dapodik temporary NISN support"]
    fn test_register_student_invalid_nisn_too_short() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let result = register_test_student(
            tenant_id,
            "12345",
            "Andi",
            None,
            &clock,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("10 digits"));
    }

    #[test]
    #[ignore = "NISN strict numeric validation relaxed for Dapodik temporary NISN support"]
    fn test_register_student_invalid_nisn_non_numeric() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let result = register_test_student(
            tenant_id,
            "123456789A",
            "Andi",
            None,
            &clock,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_register_student_empty_name() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let result = register_test_student(
            tenant_id,
            "1234567890",
            "  ",
            None,
            &clock,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_register_student_nil_tenant() {
        let clock = SystemClock;
        let result = register_test_student(
            Uuid::nil(),
            "1234567890",
            "Andi",
            None,
            &clock,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Tenant ID"));
    }

    #[test]
    fn test_student_graduate() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let mut student = register_test_student(
            tenant_id,
            "1234567890",
            "Budi",
            None,
            &clock,
        )
        .unwrap();
        student.graduate(&clock);
        assert_eq!(student.status, StudentStatus::Graduated);
    }

    #[test]
    fn test_student_transfer() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let mut student = register_test_student(
            tenant_id,
            "1234567890",
            "Citra",
            None,
            &clock,
        )
        .unwrap();
        student.transfer(&clock);
        assert_eq!(student.status, StudentStatus::Transferred);
    }

    #[test]
    fn test_student_deactivate_only_from_active() {
        let tenant_id = Uuid::now_v7();
        let clock = SystemClock;
        let mut student = register_test_student(
            tenant_id,
            "1234567890",
            "Dani",
            None,
            &clock,
        )
        .unwrap();
        // Pending → deactivate should NOT change to Inactive (only Active can be deactivated)
        student.deactivate(&clock);
        assert_eq!(student.status, StudentStatus::Pending); // still Pending
    }
}

