-- Create assessment_rules table (defines weighted components per class+subject)
CREATE TABLE assessment_rules (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    academic_term_id UUID REFERENCES terms(id),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID,
    UNIQUE(tenant_id, class_id, subject_id)
);

CREATE INDEX idx_assessment_rules_tenant_id ON assessment_rules(tenant_id);
CREATE INDEX idx_assessment_rules_class_subject ON assessment_rules(class_id, subject_id);

-- Create assessment_rule_components table
CREATE TABLE assessment_rule_components (
    id UUID PRIMARY KEY,
    rule_id UUID NOT NULL REFERENCES assessment_rules(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    source_type VARCHAR(30) NOT NULL,
    weight_percentage NUMERIC(5,2) NOT NULL,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_rule_components_rule_id ON assessment_rule_components(rule_id);

-- Create gradebook_entries table
CREATE TABLE gradebook_entries (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    component_id UUID REFERENCES assessment_rule_components(id),
    component_name VARCHAR(100) NOT NULL,
    source_type VARCHAR(30) NOT NULL,
    raw_score NUMERIC(10,2),
    max_raw_score NUMERIC(10,2),
    weighted_score NUMERIC(10,2),
    weight_percentage NUMERIC(5,2),
    source_id UUID,
    calculated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(student_id, class_id, subject_id, component_id)
);

CREATE INDEX idx_gradebook_entries_student ON gradebook_entries(student_id);
CREATE INDEX idx_gradebook_entries_class_subject ON gradebook_entries(class_id, subject_id);

CREATE TRIGGER update_assessment_rules_updated_at
    BEFORE UPDATE ON assessment_rules
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
