use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum FeatureFlagState {
    Enabled,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct FeatureFlagContext {
    pub tenant_id: Option<Uuid>,
    pub environment: Option<String>,
    pub role: Option<String>,
    pub region: Option<String>,
}

#[derive(Clone, Debug)]
pub struct FeatureFlag {
    pub key: String,
    pub description: String,
    pub global_state: FeatureFlagState,
    pub tenant_overrides: HashMap<Uuid, FeatureFlagState>,
    pub env_overrides: HashMap<String, FeatureFlagState>,
    pub role_overrides: HashMap<String, FeatureFlagState>,
    pub percentage_rollout: Option<u8>,
}

impl FeatureFlag {
    pub fn new(
        key: impl Into<String>,
        description: impl Into<String>,
        global_state: FeatureFlagState,
    ) -> Self {
        Self {
            key: key.into(),
            description: description.into(),
            global_state,
            tenant_overrides: HashMap::new(),
            env_overrides: HashMap::new(),
            role_overrides: HashMap::new(),
            percentage_rollout: None,
        }
    }

    pub fn set_override(&mut self, tenant_id: Uuid, state: FeatureFlagState) {
        self.tenant_overrides.insert(tenant_id, state);
    }

    pub fn is_enabled(&self, context: &FeatureFlagContext) -> bool {
        // Priority 1: Tenant Override
        if let Some(tenant_id) = context.tenant_id {
            if let Some(state) = self.tenant_overrides.get(&tenant_id) {
                return matches!(state, FeatureFlagState::Enabled);
            }
        }

        // Priority 2: Role Override
        if let Some(role) = &context.role {
            if let Some(state) = self.role_overrides.get(role) {
                return matches!(state, FeatureFlagState::Enabled);
            }
        }

        // Priority 3: Environment Override
        if let Some(env) = &context.environment {
            if let Some(state) = self.env_overrides.get(env) {
                return matches!(state, FeatureFlagState::Enabled);
            }
        }

        // Fallback to Global
        matches!(self.global_state, FeatureFlagState::Enabled)
    }
}

pub struct FeatureFlagService {
    flags: HashMap<String, FeatureFlag>,
}

impl Default for FeatureFlagService {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureFlagService {
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
        }
    }

    pub fn register(&mut self, flag: FeatureFlag) {
        self.flags.insert(flag.key.clone(), flag);
    }

    pub fn is_enabled(&self, key: &str, context: &FeatureFlagContext) -> bool {
        self.flags
            .get(key)
            .map(|f| f.is_enabled(context))
            .unwrap_or(false)
    }
}
