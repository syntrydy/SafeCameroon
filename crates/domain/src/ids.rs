use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! strong_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[allow(clippy::new_without_default)] // IDs must be generated deliberately.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

strong_id!(ReportId);
strong_id!(CaseId);
strong_id!(AuditEventId);
strong_id!(OutboxEventId);
