-- Attachment metadata only (docs/DATABASE.md section 7); the file itself
-- lives in the storage_provider named here, addressed by object_key. Never
-- store the bytes in Postgres.

CREATE TYPE storage_provider AS ENUM ('R2');
CREATE TYPE attachment_content_type AS ENUM (
    'IMAGE_JPEG', 'IMAGE_PNG', 'IMAGE_WEBP', 'APPLICATION_PDF',
    'AUDIO_MPEG', 'AUDIO_OGG', 'VIDEO_MP4'
);

CREATE TABLE attachments (
    id UUID PRIMARY KEY,
    report_id UUID NOT NULL REFERENCES reports (id) ON DELETE RESTRICT,
    storage_provider storage_provider NOT NULL,
    object_key TEXT NOT NULL,
    content_type attachment_content_type NOT NULL,
    size_bytes BIGINT NOT NULL,
    checksum TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT attachments_object_key_is_not_blank CHECK (length(btrim(object_key)) > 0),
    CONSTRAINT attachments_checksum_is_not_blank CHECK (length(btrim(checksum)) > 0),
    CONSTRAINT attachments_size_bytes_is_positive CHECK (size_bytes > 0),
    CONSTRAINT attachments_object_key_unique UNIQUE (object_key)
);

CREATE INDEX attachments_report_id_idx ON attachments (report_id);
