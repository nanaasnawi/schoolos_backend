use crate::common::domain::clock::Clock;
use crate::common::error::DomainError;
use crate::learning::domain::assessment_rule::AssessmentRule;
use crate::learning::domain::gradebook::GradeBook;

pub struct FinalGradeCalculator;

impl FinalGradeCalculator {
    pub fn calculate(
        rule: &AssessmentRule,
        gradebook: &mut GradeBook,
        clock: &dyn Clock,
    ) -> Result<(f64, String, bool), DomainError> {
        if rule.status != "active" && !rule.is_active {
            return Err(DomainError::Validation(
                "Cannot calculate gradebook using an inactive AssessmentRule".to_string(),
            ));
        }

        let mut total_weighted_score = 0.0;

        for component in &rule.components {
            let matching_entry = gradebook.entries.iter().find(|e| {
                e.component_name.eq_ignore_ascii_case(&component.name)
                    || e.source_type
                        .eq_ignore_ascii_case(&component.component_type)
            });

            if let Some(entry) = matching_entry {
                total_weighted_score += entry.weighted_score;
            } else if component.is_required {
                // If required component missing, 0 points added for component
                total_weighted_score += 0.0;
            }
        }

        let final_score = (total_weighted_score * 100.0).round() / 100.0;
        let letter_grade = match final_score {
            s if s >= 90.0 => "A".to_string(),
            s if s >= 80.0 => "B".to_string(),
            s if s >= 70.0 => "C".to_string(),
            s if s >= 60.0 => "D".to_string(),
            _ => "F".to_string(),
        };

        let passed = final_score >= rule.minimum_passing_grade;

        gradebook.set_calculated_grade(final_score, letter_grade.clone(), passed, clock);

        Ok((final_score, letter_grade, passed))
    }
}
