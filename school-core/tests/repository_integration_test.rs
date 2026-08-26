use school_core::academic::domain::enrollment::Enrollment;
use school_core::academic::infrastructure::pg_academic_repository::PgAcademicRepository;
use school_core::academic::infrastructure::repository_traits::EnrollmentRepository;
use school_core::common::domain::clock::SystemClock;
use school_core::common::infrastructure::pg_uow::PgUnitOfWorkFactory;
use school_core::common::infrastructure::uow::UnitOfWorkFactory;
use school_core::people::domain::{student::Student, teacher::Teacher};
use school_core::people::infrastructure::pg_people_repository::PgPeopleRepository;
use school_core::people::infrastructure::repository_traits::{
    StudentRepository, TeacherRepository,
};
use sqlx::PgPool;
use uuid::Uuid;

// NOTE: To run this test, you need to set up a PostgreSQL database and provide DATABASE_URL.
#[sqlx::test]
async fn test_create_and_find_teacher(pool: PgPool) {
    let repo = PgPeopleRepository::new(pool.clone());

    let tenant_id = Uuid::now_v7();
    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, 'Test Tenant')")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();

    let clock = SystemClock;
    let teacher = Teacher::new(
        tenant_id,
        "Bapak Budi".to_string(),
        Some("12345678".to_string()),
        &clock,
    );

    let uow_factory = PgUnitOfWorkFactory::new(pool.clone());
    let mut uow = uow_factory.begin().await.unwrap();

    let result = TeacherRepository::create(&repo, &teacher, &mut *uow).await;
    uow.commit().await.unwrap();
    assert!(result.is_ok(), "Failed to create teacher");

    let found = TeacherRepository::find_by_id(&repo, teacher.id)
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().full_name, "Bapak Budi");
}

#[sqlx::test]
async fn test_constraint_duplicate_nisn(pool: PgPool) {
    let repo = PgPeopleRepository::new(pool.clone());

    let tenant_id = Uuid::now_v7();
    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, 'Test Tenant')")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();

    let clock = SystemClock;
    let student1 = Student::register(
        tenant_id,
        "1234567890".to_string(),
        "Andi".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &clock,
    )
    .unwrap();
    let student2 = Student::register(
        tenant_id,
        "0987654321".to_string(),
        "Budi".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &clock,
    )
    .unwrap();

    let mut uow = PgUnitOfWorkFactory::new(pool.clone())
        .begin()
        .await
        .unwrap();

    // First insert should succeed
    StudentRepository::create(&repo, &student1, &mut *uow)
        .await
        .unwrap();
    uow.commit().await.unwrap();

    // Second insert should fail due to UNIQUE(tenant_id, nisn)
    let mut uow2 = PgUnitOfWorkFactory::new(pool.clone())
        .begin()
        .await
        .unwrap();
    let result = StudentRepository::create(&repo, &student2, &mut *uow2).await;
    assert!(
        result.is_err(),
        "Expected database error due to duplicate NISN"
    );
}

#[sqlx::test]
async fn test_constraint_duplicate_enrollment(pool: PgPool) {
    let people_repo = PgPeopleRepository::new(pool.clone());
    let academic_repo = PgAcademicRepository::new(pool.clone());

    let tenant_id = Uuid::now_v7();
    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, 'Test Tenant')")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();

    let academic_year_id = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_years (id, tenant_id, name, start_date, end_date) VALUES ($1, $2, '2024/2025', '2024-07-01', '2025-06-30')")
        .bind(academic_year_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();

    let grade_level_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO grade_levels (id, tenant_id, level, name) VALUES ($1, $2, 1, 'Kelas 1')",
    )
    .bind(grade_level_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();

    let class_id = Uuid::now_v7();
    sqlx::query("INSERT INTO classes (id, tenant_id, academic_year_id, grade_level_id, name) VALUES ($1, $2, $3, $4, '1A')")
        .bind(class_id)
        .bind(tenant_id)
        .bind(academic_year_id)
        .bind(grade_level_id)
        .execute(&pool)
        .await
        .unwrap();

    let clock = SystemClock;
    let student = Student::register(
        tenant_id,
        "9999900000".to_string(),
        "Andi".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &clock,
    )
    .unwrap();

    let uow_factory = PgUnitOfWorkFactory::new(pool.clone());
    let mut uow = uow_factory.begin().await.unwrap();
    StudentRepository::create(&people_repo, &student, &mut *uow)
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let enrollment1 = Enrollment::new(tenant_id, student.id, class_id, academic_year_id, &clock);
    let enrollment2 = Enrollment::new(tenant_id, student.id, class_id, academic_year_id, &clock);

    // First enrollment succeeds
    EnrollmentRepository::create(&academic_repo, &enrollment1)
        .await
        .unwrap();

    // Second enrollment fails due to UNIQUE(student_id, academic_year_id) WHERE status = 'Active'
    let result = EnrollmentRepository::create(&academic_repo, &enrollment2).await;
    assert!(
        result.is_err(),
        "Expected database error due to duplicate active enrollment"
    );
}

#[sqlx::test]
async fn test_constraint_delete_homeroom_teacher(pool: PgPool) {
    let repo = PgPeopleRepository::new(pool.clone());

    let tenant_id = Uuid::now_v7();
    sqlx::query("INSERT INTO tenants (id, name) VALUES ($1, 'Test Tenant')")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();

    let clock = SystemClock;
    let teacher = Teacher::new(tenant_id, "Guru".to_string(), None, &clock);
    let uow_factory = PgUnitOfWorkFactory::new(pool.clone());
    let mut uow = uow_factory.begin().await.unwrap();
    TeacherRepository::create(&repo, &teacher, &mut *uow)
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let academic_year_id = Uuid::now_v7();
    sqlx::query("INSERT INTO academic_years (id, tenant_id, name, start_date, end_date) VALUES ($1, $2, '2024/2025', '2024-07-01', '2025-06-30')")
        .bind(academic_year_id)
        .bind(tenant_id)
        .execute(&pool)
        .await
        .unwrap();

    let grade_level_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO grade_levels (id, tenant_id, level, name) VALUES ($1, $2, 1, 'Kelas 1')",
    )
    .bind(grade_level_id)
    .bind(tenant_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO classes (id, tenant_id, academic_year_id, grade_level_id, homeroom_teacher_id, name) VALUES ($1, $2, $3, $4, $5, '1A')")
        .bind(Uuid::now_v7())
        .bind(tenant_id)
        .bind(academic_year_id)
        .bind(grade_level_id)
        .bind(teacher.id)
        .execute(&pool)
        .await
        .unwrap();

    // Try to delete the teacher, should fail due to RESTRICT FK constraint
    let result = sqlx::query("DELETE FROM teachers WHERE id = $1")
        .bind(teacher.id)
        .execute(&pool)
        .await;

    assert!(
        result.is_err(),
        "Expected database error due to RESTRICT FK on homeroom_teacher_id"
    );
}
