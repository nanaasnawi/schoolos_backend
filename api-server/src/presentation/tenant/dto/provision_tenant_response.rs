use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
pub struct ProvisionTenantResponse {
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub tenant_id: Uuid,

    #[schema(example = "Provisioning started successfully")]
    pub message: String,
}
