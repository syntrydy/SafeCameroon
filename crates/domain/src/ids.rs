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

            /// Rebuilds an id from a stored value. Infrastructure adapters use this
            /// when reconstituting an aggregate; application code should otherwise
            /// only ever see ids produced by `new()`.
            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }
        }
    };
}

strong_id!(ReportId);
strong_id!(CaseId);
strong_id!(CaseEventId);
strong_id!(AlertId);
strong_id!(AlertEventId);
strong_id!(ConsumerId);
strong_id!(SubscriptionId);
strong_id!(AuditEventId);
strong_id!(OutboxEventId);
strong_id!(DeliveryId);
strong_id!(DeliveryEventId);
strong_id!(DeliveryAttemptId);
strong_id!(AttachmentId);
