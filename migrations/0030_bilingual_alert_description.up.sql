-- Bilingual (EN/FR) alert descriptions (docs/AI.md section 2
-- "Translation"). The old single-language SAFE_DESCRIPTION stays a valid
-- alert_field value so alerts created before this shipped keep reading and
-- delivering unchanged -- the application layer stops offering it on new
-- alert creation (crates/domain/src/alert.rs's community policy allowlist),
-- it is not removed here.

ALTER TYPE alert_field ADD VALUE 'SAFE_DESCRIPTION_EN';
ALTER TYPE alert_field ADD VALUE 'SAFE_DESCRIPTION_FR';

-- Audit trail for LLM-generated description suggestions, mirroring
-- report_extractions (migrations/0023) -- provenance per CLAUDE.md "AI
-- integration" / docs/AI.md section 4. Tied to case_id, not alert_id: a
-- suggestion is requested before the alert exists.
CREATE TABLE alert_description_generations (
    id UUID PRIMARY KEY,
    case_id UUID NOT NULL REFERENCES cases (id) ON DELETE RESTRICT,
    requested_by UUID NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    source_text TEXT NOT NULL,
    description_en TEXT NOT NULL,
    description_fr TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT adg_provider_is_not_blank CHECK (length(btrim(provider)) > 0),
    CONSTRAINT adg_model_is_not_blank CHECK (length(btrim(model)) > 0),
    CONSTRAINT adg_prompt_version_is_not_blank CHECK (length(btrim(prompt_version)) > 0),
    CONSTRAINT adg_source_text_is_not_blank CHECK (length(btrim(source_text)) > 0),
    CONSTRAINT adg_description_en_is_not_blank CHECK (length(btrim(description_en)) > 0),
    CONSTRAINT adg_description_fr_is_not_blank CHECK (length(btrim(description_fr)) > 0)
);

CREATE INDEX alert_description_generations_case_id_idx
    ON alert_description_generations (case_id, created_at DESC);
