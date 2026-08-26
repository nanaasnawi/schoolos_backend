use crate::common::error::InfrastructureError;
use crate::learning::domain::assessment_rule::{AssessmentComponent, AssessmentRule};
use crate::learning::domain::gradebook::{GradeBook, GradeEntry};
use crate::learning::domain::gradebook_entry::GradebookEntry;
use crate::learning::infrastructure::repository_traits::{
    AssessmentRuleRepository, GradebookRepository,
};
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn text_to_f64(val: Option<&str>) -> Option<f64> {
    val.and_then(|s| s.parse::<f64>().ok())
}

pub struct PgAssessmentRepository {
    pool: PgPool,
}

impl PgAssessmentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AssessmentRuleRepository for PgAssessmentRepository {
    async fn save(&self, rule: &AssessmentRule) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO assessment_rules (id, tenant_id, class_id, subject_id, academic_term_id, minimum_passing_grade, status, rounding_policy, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6::TEXT::NUMERIC, $7, $8, $9, $10, $11)
            ON CONFLICT (tenant_id, class_id, subject_id) DO UPDATE
            SET is_active = $9, status = $7, minimum_passing_grade = $6::TEXT::NUMERIC, academic_term_id = COALESCE($5, assessment_rules.academic_term_id), updated_at = $11
            "#
        )
        .bind(rule.id)
        .bind(rule.tenant_id)
        .bind(rule.class_id)
        .bind(rule.subject_id)
        .bind(rule.academic_term_id)
        .bind(rule.minimum_passing_grade.to_string())
        .bind(&rule.status)
        .bind(&rule.rounding_policy)
        .bind(rule.is_active)
        .bind(rule.created_at)
        .bind(rule.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        self.clear_components(rule.id).await?;

        for comp in &rule.components {
            self.save_component(comp).await?;
        }

        Ok(())
    }

    async fn update(&self, rule: &AssessmentRule) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            UPDATE assessment_rules
            SET minimum_passing_grade = $1::TEXT::NUMERIC, status = $2, rounding_policy = $3, is_active = $4, updated_at = $5
            WHERE id = $6 AND deleted_at IS NULL
            "#
        )
        .bind(rule.minimum_passing_grade.to_string())
        .bind(&rule.status)
        .bind(&rule.rounding_policy)
        .bind(rule.is_active)
        .bind(rule.updated_at)
        .bind(rule.id)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        self.clear_components(rule.id).await?;
        for comp in &rule.components {
            self.save_component(comp).await?;
        }

        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<AssessmentRule>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, class_id, subject_id, academic_term_id, minimum_passing_grade::TEXT AS mpg, status, rounding_policy, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM assessment_rules WHERE id = $1 AND deleted_at IS NULL"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        if let Some(r) = record {
            let rule_id: Uuid = r.get("id");
            let comp_rows = sqlx::query(
                r#"SELECT id, rule_id, name, source_type, weight_percentage::TEXT AS wp, is_required, order_index, created_at
                   FROM assessment_rule_components WHERE rule_id = $1
                   ORDER BY order_index ASC"#
            )
            .bind(rule_id)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

            let components = comp_rows
                .into_iter()
                .map(|cr| {
                    let wp_str: Option<String> = cr.get("wp");
                    AssessmentComponent {
                        id: cr.get("id"),
                        rule_id: cr.get("rule_id"),
                        name: cr.get("name"),
                        component_type: cr.get("source_type"),
                        weight_percentage: text_to_f64(wp_str.as_deref()).unwrap_or(0.0),
                        is_required: cr.get::<Option<bool>, _>("is_required").unwrap_or(true),
                        order_index: cr.get("order_index"),
                    }
                })
                .collect();

            let mpg_str: Option<String> = r.get("mpg");

            Ok(Some(AssessmentRule {
                id: rule_id,
                tenant_id: r.get("tenant_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                academic_term_id: r.get("academic_term_id"),
                minimum_passing_grade: text_to_f64(mpg_str.as_deref()).unwrap_or(70.0),
                status: r
                    .get::<Option<String>, _>("status")
                    .unwrap_or_else(|| "draft".to_string()),
                rounding_policy: r
                    .get::<Option<String>, _>("rounding_policy")
                    .unwrap_or_else(|| "half_up".to_string()),
                is_active: r.get("is_active"),
                components,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_by_class_subject(
        &self,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Option<AssessmentRule>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, class_id, subject_id, academic_term_id, minimum_passing_grade::TEXT AS mpg, status, rounding_policy, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM assessment_rules WHERE class_id = $1 AND subject_id = $2 AND deleted_at IS NULL"#
        )
        .bind(class_id)
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        if let Some(r) = record {
            let rule_id: Uuid = r.get("id");
            let comp_rows = sqlx::query(
                r#"SELECT id, rule_id, name, source_type, weight_percentage::TEXT AS wp, is_required, order_index, created_at
                   FROM assessment_rule_components WHERE rule_id = $1
                   ORDER BY order_index ASC"#
            )
            .bind(rule_id)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

            let components = comp_rows
                .into_iter()
                .map(|cr| {
                    let wp_str: Option<String> = cr.get("wp");
                    AssessmentComponent {
                        id: cr.get("id"),
                        rule_id: cr.get("rule_id"),
                        name: cr.get("name"),
                        component_type: cr.get("source_type"),
                        weight_percentage: text_to_f64(wp_str.as_deref()).unwrap_or(0.0),
                        is_required: cr.get::<Option<bool>, _>("is_required").unwrap_or(true),
                        order_index: cr.get("order_index"),
                    }
                })
                .collect();

            let mpg_str: Option<String> = r.get("mpg");

            Ok(Some(AssessmentRule {
                id: rule_id,
                tenant_id: r.get("tenant_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                academic_term_id: r.get("academic_term_id"),
                minimum_passing_grade: text_to_f64(mpg_str.as_deref()).unwrap_or(70.0),
                status: r
                    .get::<Option<String>, _>("status")
                    .unwrap_or_else(|| "draft".to_string()),
                rounding_policy: r
                    .get::<Option<String>, _>("rounding_policy")
                    .unwrap_or_else(|| "half_up".to_string()),
                is_active: r.get("is_active"),
                components,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_by_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<AssessmentRule>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, class_id, subject_id, academic_term_id, minimum_passing_grade::TEXT AS mpg, status, rounding_policy, is_active, created_at, updated_at, deleted_at, deleted_by
               FROM assessment_rules WHERE tenant_id = $1 AND deleted_at IS NULL
               ORDER BY created_at DESC"#
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let mut rules = Vec::new();
        for r in records {
            let rule_id: Uuid = r.get("id");
            let comp_rows = sqlx::query(
                r#"SELECT id, rule_id, name, source_type, weight_percentage::TEXT AS wp, is_required, order_index, created_at
                   FROM assessment_rule_components WHERE rule_id = $1
                   ORDER BY order_index ASC"#
            )
            .bind(rule_id)
            .fetch_all(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;

            let components = comp_rows
                .into_iter()
                .map(|cr| {
                    let wp_str: Option<String> = cr.get("wp");
                    AssessmentComponent {
                        id: cr.get("id"),
                        rule_id: cr.get("rule_id"),
                        name: cr.get("name"),
                        component_type: cr.get("source_type"),
                        weight_percentage: text_to_f64(wp_str.as_deref()).unwrap_or(0.0),
                        is_required: cr.get::<Option<bool>, _>("is_required").unwrap_or(true),
                        order_index: cr.get("order_index"),
                    }
                })
                .collect();

            let mpg_str: Option<String> = r.get("mpg");

            rules.push(AssessmentRule {
                id: rule_id,
                tenant_id: r.get("tenant_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                academic_term_id: r.get("academic_term_id"),
                minimum_passing_grade: text_to_f64(mpg_str.as_deref()).unwrap_or(70.0),
                status: r
                    .get::<Option<String>, _>("status")
                    .unwrap_or_else(|| "draft".to_string()),
                rounding_policy: r
                    .get::<Option<String>, _>("rounding_policy")
                    .unwrap_or_else(|| "half_up".to_string()),
                is_active: r.get("is_active"),
                components,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                deleted_at: r.get("deleted_at"),
                deleted_by: r.get("deleted_by"),
                domain_events: Vec::new(),
                version: 1,
            });
        }

        Ok(rules)
    }

    async fn save_component(
        &self,
        component: &AssessmentComponent,
    ) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO assessment_rule_components (id, rule_id, name, source_type, weight_percentage, is_required, order_index, created_at)
            VALUES ($1, $2, $3, $4, $5::TEXT::NUMERIC, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE
            SET name = $3, source_type = $4, weight_percentage = $5::TEXT::NUMERIC, is_required = $6, order_index = $7
            "#
        )
        .bind(component.id)
        .bind(component.rule_id)
        .bind(&component.name)
        .bind(&component.component_type)
        .bind(component.weight_percentage.to_string())
        .bind(component.is_required)
        .bind(component.order_index)
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn clear_components(&self, rule_id: Uuid) -> Result<(), InfrastructureError> {
        sqlx::query("DELETE FROM assessment_rule_components WHERE rule_id = $1")
            .bind(rule_id)
            .execute(&self.pool)
            .await
            .map_err(InfrastructureError::Database)?;
        Ok(())
    }
}

#[async_trait]
impl GradebookRepository for PgAssessmentRepository {
    async fn save_gradebook(&self, gradebook: &GradeBook) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO gradebooks (id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score, letter_grade, passed, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7::TEXT::NUMERIC, $8, $9, $10, $11, $12)
            ON CONFLICT (student_id, class_id, subject_id) DO UPDATE
            SET final_score = COALESCE($7::TEXT::NUMERIC, gradebooks.final_score),
                letter_grade = COALESCE($8, gradebooks.letter_grade),
                passed = COALESCE($9, gradebooks.passed),
                status = $10,
                updated_at = $12
            "#
        )
        .bind(gradebook.id)
        .bind(gradebook.tenant_id)
        .bind(gradebook.student_id)
        .bind(gradebook.class_id)
        .bind(gradebook.subject_id)
        .bind(gradebook.academic_year_id)
        .bind(gradebook.final_score.map(|s| s.to_string()))
        .bind(&gradebook.letter_grade)
        .bind(gradebook.passed)
        .bind(&gradebook.status)
        .bind(gradebook.created_at)
        .bind(gradebook.updated_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        for entry in &gradebook.entries {
            self.save_entry(entry).await?;
        }

        Ok(())
    }

    async fn update_gradebook(&self, gradebook: &GradeBook) -> Result<(), InfrastructureError> {
        self.save_gradebook(gradebook).await
    }

    async fn find_gradebook_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<GradeBook>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score::TEXT AS fs, letter_grade, passed, status, created_at, updated_at
               FROM gradebooks WHERE id = $1"#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        if let Some(r) = record {
            let gb_id: Uuid = r.get("id");
            let entries = self.find_entries(gb_id).await?;
            let fs_str: Option<String> = r.get("fs");

            Ok(Some(GradeBook {
                id: gb_id,
                tenant_id: r.get("tenant_id"),
                student_id: r.get("student_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                academic_year_id: r.get("academic_year_id"),
                final_score: text_to_f64(fs_str.as_deref()),
                letter_grade: r.get("letter_grade"),
                passed: r.get("passed"),
                status: r.get("status"),
                entries,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_gradebook_by_student_subject(
        &self,
        student_id: Uuid,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Option<GradeBook>, InfrastructureError> {
        let record = sqlx::query(
            r#"SELECT id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score::TEXT AS fs, letter_grade, passed, status, created_at, updated_at
               FROM gradebooks WHERE student_id = $1 AND class_id = $2 AND subject_id = $3"#
        )
        .bind(student_id)
        .bind(class_id)
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        if let Some(r) = record {
            let gb_id: Uuid = r.get("id");
            let entries = self.find_entries(gb_id).await?;
            let fs_str: Option<String> = r.get("fs");

            Ok(Some(GradeBook {
                id: gb_id,
                tenant_id: r.get("tenant_id"),
                student_id: r.get("student_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                academic_year_id: r.get("academic_year_id"),
                final_score: text_to_f64(fs_str.as_deref()),
                letter_grade: r.get("letter_grade"),
                passed: r.get("passed"),
                status: r.get("status"),
                entries,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_gradebooks_by_class(
        &self,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Vec<GradeBook>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, student_id, class_id, subject_id, academic_year_id, final_score::TEXT AS fs, letter_grade, passed, status, created_at, updated_at
               FROM gradebooks WHERE class_id = $1 AND subject_id = $2
               ORDER BY student_id ASC"#
        )
        .bind(class_id)
        .bind(subject_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let mut items = Vec::new();
        for r in records {
            let gb_id: Uuid = r.get("id");
            let entries = self.find_entries(gb_id).await?;
            let fs_str: Option<String> = r.get("fs");

            items.push(GradeBook {
                id: gb_id,
                tenant_id: r.get("tenant_id"),
                student_id: r.get("student_id"),
                class_id: r.get("class_id"),
                subject_id: r.get("subject_id"),
                academic_year_id: r.get("academic_year_id"),
                final_score: text_to_f64(fs_str.as_deref()),
                letter_grade: r.get("letter_grade"),
                passed: r.get("passed"),
                status: r.get("status"),
                entries,
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                domain_events: Vec::new(),
                version: 1,
            });
        }

        Ok(items)
    }

    async fn save_entry(&self, entry: &GradeEntry) -> Result<(), InfrastructureError> {
        sqlx::query(
            r#"
            INSERT INTO gradebook_entries (id, tenant_id, student_id, class_id, subject_id, gradebook_id, component_name, source_type, raw_score, max_raw_score, weighted_score, weight_percentage, source_id, calculated_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::TEXT::NUMERIC, $10::TEXT::NUMERIC, $11::TEXT::NUMERIC, $12::TEXT::NUMERIC, $13, $14, $15)
            ON CONFLICT (id) DO UPDATE
            SET raw_score = $9::TEXT::NUMERIC,
                max_raw_score = $10::TEXT::NUMERIC,
                weighted_score = $11::TEXT::NUMERIC,
                weight_percentage = $12::TEXT::NUMERIC,
                calculated_at = $14
            "#
        )
        .bind(entry.id)
        .bind(Uuid::nil())
        .bind(Uuid::nil())
        .bind(Uuid::nil())
        .bind(Uuid::nil())
        .bind(entry.gradebook_id)
        .bind(&entry.component_name)
        .bind(&entry.source_type)
        .bind(entry.raw_score.to_string())
        .bind(entry.max_raw_score.to_string())
        .bind(entry.weighted_score.to_string())
        .bind(entry.weight_percentage.to_string())
        .bind(entry.source_id)
        .bind(entry.recorded_at)
        .bind(entry.recorded_at)
        .execute(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;
        Ok(())
    }

    async fn find_entries(
        &self,
        gradebook_id: Uuid,
    ) -> Result<Vec<GradeEntry>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, gradebook_id, source_type, source_id, component_name,
               raw_score::TEXT AS raw_score, max_raw_score::TEXT AS max_raw_score,
               weighted_score::TEXT AS weighted_score, weight_percentage::TEXT AS weight_percentage, calculated_at
               FROM gradebook_entries WHERE gradebook_id = $1
               ORDER BY component_name ASC"#
        )
        .bind(gradebook_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| {
                let raw_str: Option<String> = r.get("raw_score");
                let max_raw_str: Option<String> = r.get("max_raw_score");
                let weighted_str: Option<String> = r.get("weighted_score");
                let wp_str: Option<String> = r.get("weight_percentage");
                GradeEntry {
                    id: r.get("id"),
                    gradebook_id: r.get("gradebook_id"),
                    source_type: r.get("source_type"),
                    source_id: r.get("source_id"),
                    component_name: r.get("component_name"),
                    raw_score: text_to_f64(raw_str.as_deref()).unwrap_or(0.0),
                    max_raw_score: text_to_f64(max_raw_str.as_deref()).unwrap_or(100.0),
                    weighted_score: text_to_f64(weighted_str.as_deref()).unwrap_or(0.0),
                    weight_percentage: text_to_f64(wp_str.as_deref()).unwrap_or(0.0),
                    recorded_at: r.get("calculated_at"),
                }
            })
            .collect();

        Ok(items)
    }

    async fn find_by_class_subject(
        &self,
        class_id: Uuid,
        subject_id: Uuid,
    ) -> Result<Vec<GradebookEntry>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, student_id, class_id, subject_id, component_id, component_name, source_type,
               raw_score::TEXT AS raw_score, max_raw_score::TEXT AS max_raw_score, weighted_score::TEXT AS weighted_score,
               weight_percentage::TEXT AS weight_percentage, source_id, calculated_at, created_at
               FROM gradebook_entries WHERE class_id = $1 AND subject_id = $2
               ORDER BY student_id, component_name"#
        )
        .bind(class_id)
        .bind(subject_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| {
                let raw_str: Option<String> = r.get("raw_score");
                let max_raw_str: Option<String> = r.get("max_raw_score");
                let weighted_str: Option<String> = r.get("weighted_score");
                let wp_str: Option<String> = r.get("weight_percentage");
                GradebookEntry {
                    id: r.get("id"),
                    tenant_id: r.get("tenant_id"),
                    student_id: r.get("student_id"),
                    class_id: r.get("class_id"),
                    subject_id: r.get("subject_id"),
                    component_id: r.get("component_id"),
                    component_name: r.get("component_name"),
                    source_type: r.get("source_type"),
                    raw_score: text_to_f64(raw_str.as_deref()),
                    max_raw_score: text_to_f64(max_raw_str.as_deref()),
                    weighted_score: text_to_f64(weighted_str.as_deref()),
                    weight_percentage: text_to_f64(wp_str.as_deref()),
                    source_id: r.get("source_id"),
                    calculated_at: r.get("calculated_at"),
                    created_at: r.get("created_at"),
                }
            })
            .collect();

        Ok(items)
    }

    async fn find_by_student(
        &self,
        student_id: Uuid,
    ) -> Result<Vec<GradebookEntry>, InfrastructureError> {
        let records = sqlx::query(
            r#"SELECT id, tenant_id, student_id, class_id, subject_id, component_id, component_name, source_type,
               raw_score::TEXT AS raw_score, max_raw_score::TEXT AS max_raw_score, weighted_score::TEXT AS weighted_score,
               weight_percentage::TEXT AS weight_percentage, source_id, calculated_at, created_at
               FROM gradebook_entries WHERE student_id = $1
               ORDER BY class_id, subject_id, component_name"#
        )
        .bind(student_id)
        .fetch_all(&self.pool)
        .await
        .map_err(InfrastructureError::Database)?;

        let items = records
            .into_iter()
            .map(|r| {
                let raw_str: Option<String> = r.get("raw_score");
                let max_raw_str: Option<String> = r.get("max_raw_score");
                let weighted_str: Option<String> = r.get("weighted_score");
                let wp_str: Option<String> = r.get("weight_percentage");
                GradebookEntry {
                    id: r.get("id"),
                    tenant_id: r.get("tenant_id"),
                    student_id: r.get("student_id"),
                    class_id: r.get("class_id"),
                    subject_id: r.get("subject_id"),
                    component_id: r.get("component_id"),
                    component_name: r.get("component_name"),
                    source_type: r.get("source_type"),
                    raw_score: text_to_f64(raw_str.as_deref()),
                    max_raw_score: text_to_f64(max_raw_str.as_deref()),
                    weighted_score: text_to_f64(weighted_str.as_deref()),
                    weight_percentage: text_to_f64(wp_str.as_deref()),
                    source_id: r.get("source_id"),
                    calculated_at: r.get("calculated_at"),
                    created_at: r.get("created_at"),
                }
            })
            .collect();

        Ok(items)
    }
}
