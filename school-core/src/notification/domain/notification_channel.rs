use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationChannel {
    InApp,
    Push,
    Email,
    Webhook,
}

impl NotificationChannel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::InApp => "in_app",
            Self::Push => "push",
            Self::Email => "email",
            Self::Webhook => "webhook",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "in_app" => Some(Self::InApp),
            "push" => Some(Self::Push),
            "email" => Some(Self::Email),
            "webhook" => Some(Self::Webhook),
            _ => None,
        }
    }

    pub fn default_channels() -> Vec<Self> {
        vec![Self::InApp]
    }
}

impl std::fmt::Display for NotificationChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
