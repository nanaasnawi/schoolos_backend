use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyName {
    ActiveAcademicYearPolicy,
    HomeroomTeacherPolicy,
    TenantIsolationPolicy,
    SchoolActivePolicy,
}

impl PolicyName {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyName::ActiveAcademicYearPolicy => "ActiveAcademicYearPolicy",
            PolicyName::HomeroomTeacherPolicy => "HomeroomTeacherPolicy",
            PolicyName::TenantIsolationPolicy => "TenantIsolationPolicy",
            PolicyName::SchoolActivePolicy => "SchoolActivePolicy",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ActiveAcademicYearPolicy" => Some(PolicyName::ActiveAcademicYearPolicy),
            "HomeroomTeacherPolicy" => Some(PolicyName::HomeroomTeacherPolicy),
            "TenantIsolationPolicy" => Some(PolicyName::TenantIsolationPolicy),
            "SchoolActivePolicy" => Some(PolicyName::SchoolActivePolicy),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            PolicyName::ActiveAcademicYearPolicy,
            PolicyName::HomeroomTeacherPolicy,
            PolicyName::TenantIsolationPolicy,
            PolicyName::SchoolActivePolicy,
        ]
    }
}
