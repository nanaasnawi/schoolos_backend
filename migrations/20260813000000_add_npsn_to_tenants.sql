ALTER TABLE tenants
ADD COLUMN npsn VARCHAR(20) UNIQUE;

-- Create an index for faster lookup during Dapodik sync
CREATE INDEX idx_tenants_npsn ON tenants(npsn);
