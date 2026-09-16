-- AI extraction suggestions (docs/AI.md, prompts 15/16, CLAUDE.md "AI
-- integration": "an untrusted recommendation service ... require
-- structured output, validation, provenance, model metadata, and an
-- explicit application-layer decision before changing case state"). A
-- reviewer explicitly requests extraction for a specific report; the result
-- is never applied automatically -- it is only ever a suggestion the
-- reviewer reads before manually creating/verifying a case, so there is
-- nothing here to "accept" or "reject": the reviewer just acts (or doesn't)
-- using their own judgment, same as reading the raw report text itself.
--
-- `fields` stores the validated ExtractedReportFields (crates/domain) as a
-- JSON object of nullable string fields, mirroring how subscriptions.rules
-- keeps the closed Rust shape as the single source of truth rather than a
-- normalized column per field -- this schema is expected to grow as more
-- incident types are supported (docs/ROADMAP.md Stage 4).

CREATE TABLE report_extractions (
    id UUID PRIMARY KEY,
    report_id UUID NOT NULL REFERENCES reports (id) ON DELETE RESTRICT,
    requested_by UUID NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    fields JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT report_extractions_provider_is_not_blank CHECK (length(btrim(provider)) > 0),
    CONSTRAINT report_extractions_model_is_not_blank CHECK (length(btrim(model)) > 0),
    CONSTRAINT report_extractions_prompt_version_is_not_blank CHECK (length(btrim(prompt_version)) > 0),
    CONSTRAINT report_extractions_fields_is_an_object CHECK (jsonb_typeof(fields) = 'object')
);

CREATE INDEX report_extractions_report_id_idx ON report_extractions (report_id, created_at DESC);
