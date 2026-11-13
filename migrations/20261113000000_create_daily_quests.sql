-- 20261113000000_create_daily_quests.sql
-- Misi Harian Dinamis (Daily Quest Mini-Games)

CREATE TABLE IF NOT EXISTS daily_quests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    quest_date DATE NOT NULL,
    grade_band VARCHAR(32) NOT NULL DEFAULT 'PAKET_C',
    game_mode VARCHAR(40) NOT NULL,
    title VARCHAR(255) NOT NULL,
    subtitle VARCHAR(255) NOT NULL DEFAULT '',
    subject_name VARCHAR(120) NOT NULL DEFAULT 'Literasi & Numerasi',
    source_label VARCHAR(50) NOT NULL DEFAULT 'AI_CURRICULUM',
    xp_reward INT NOT NULL DEFAULT 60,
    challenges_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_daily_quests_tenant_date_band_mode UNIQUE (tenant_id, quest_date, grade_band, game_mode)
);

CREATE INDEX IF NOT EXISTS idx_daily_quests_lookup
    ON daily_quests(tenant_id, quest_date, grade_band);

CREATE TABLE IF NOT EXISTS daily_quest_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    quest_id UUID NOT NULL REFERENCES daily_quests(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    correct_count INT NOT NULL DEFAULT 0,
    total_count INT NOT NULL DEFAULT 0,
    score_percent INT NOT NULL DEFAULT 0,
    max_combo INT NOT NULL DEFAULT 0,
    duration_seconds INT NOT NULL DEFAULT 0,
    xp_earned INT NOT NULL DEFAULT 0,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_daily_quest_attempt_student UNIQUE (quest_id, student_id)
);

CREATE INDEX IF NOT EXISTS idx_daily_quest_attempts_quest
    ON daily_quest_attempts(quest_id, score_percent DESC, duration_seconds ASC);
CREATE INDEX IF NOT EXISTS idx_daily_quest_attempts_student
    ON daily_quest_attempts(student_id, completed_at DESC);

