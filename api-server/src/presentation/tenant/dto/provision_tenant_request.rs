use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProvisionTenantRequest {
    #[schema(example = "SDN 1 Siliasih")]
    pub tenant_name: String,

    #[schema(example = "SDN 1 Siliasih")]
    pub school_name: String,
}
