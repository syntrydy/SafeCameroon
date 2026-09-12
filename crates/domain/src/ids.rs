use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! strong_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
    };
}

strong_id!(ReportId);
strong_id!(CaseId);
