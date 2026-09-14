//! A generic per-source rate-limit port (docs/SECURITY_PRIVACY.md section 8:
//! "edge rate limits; per-source throttles"). docs/DEPLOYMENT.md puts coarse
//! IP-based edge rate limiting at Cloudflare, outside this repo — this is
//! defense-in-depth for whenever the app runs without that edge in front
//! (local dev, staging, a pilot before edge rules exist), and the only place
//! that can enforce a per-*account* limit at all (Cloudflare only ever sees
//! source IPs, never which reviewer email a login attempt targets).

use core::fmt;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

/// The closed set of scopes a limit is enforced under — never a free-form
/// string, so a typo can't silently create an unenforced new bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitScope {
    AnonymousReportSubmission,
    ReviewerLoginAttempt,
}

impl RateLimitScope {
    pub fn as_database_value(self) -> &'static str {
        match self {
            Self::AnonymousReportSubmission => "ANONYMOUS_REPORT_SUBMISSION",
            Self::ReviewerLoginAttempt => "REVIEWER_LOGIN_ATTEMPT",
        }
    }

    /// How many requests a single source may make per [`Self::window`]
    /// (docs/SECURITY_PRIVACY.md section 8). Deliberately generous,
    /// hardcoded defaults — tuning these against real traffic is an
    /// operational decision, not a design one, matching AGENTS.md "do not
    /// overengineer the first release".
    pub fn limit(self) -> u32 {
        match self {
            Self::AnonymousReportSubmission => 10,
            Self::ReviewerLoginAttempt => 5,
        }
    }

    pub fn window(self) -> Duration {
        match self {
            Self::AnonymousReportSubmission => Duration::from_secs(60 * 60),
            Self::ReviewerLoginAttempt => Duration::from_secs(15 * 60),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitError {
    pub reason: String,
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for RateLimitError {}

/// Async because a durable implementation needs to persist counts (an
/// in-process limiter alone would forget on restart, and would not be
/// shared across multiple API instances — mirrors [`crate::webhook::WebhookReplayGuard`]'s
/// same reasoning).
#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// Records one attempt from `source_key` under `scope` and returns
    /// whether it is still within the limit (`true`) or the source must be
    /// refused (`false`).
    async fn record_and_check(
        &self,
        scope: RateLimitScope,
        source_key: &str,
    ) -> Result<bool, RateLimitError>;
}

/// A process-local, non-durable [`RateLimiter`] for tests only — mirrors
/// [`crate::webhook::InMemoryReplayGuard`].
#[derive(Default)]
pub struct InMemoryRateLimiter {
    counts: Mutex<HashMap<(RateLimitScope, String), u32>>,
}

#[async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn record_and_check(
        &self,
        scope: RateLimitScope,
        source_key: &str,
    ) -> Result<bool, RateLimitError> {
        let mut counts = self
            .counts
            .lock()
            .expect("in-memory rate limiter mutex must not be poisoned");
        let count = counts.entry((scope, source_key.to_owned())).or_insert(0);
        *count += 1;
        Ok(*count <= scope.limit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_requests_up_to_the_scopes_limit_then_blocks() {
        let limiter = InMemoryRateLimiter::default();
        let scope = RateLimitScope::ReviewerLoginAttempt;
        assert_eq!(scope.limit(), 5);

        for attempt in 1..=5 {
            assert!(
                limiter
                    .record_and_check(scope, "someone@example.test")
                    .await
                    .unwrap(),
                "attempt {attempt} should still be within the limit"
            );
        }
        assert!(
            !limiter
                .record_and_check(scope, "someone@example.test")
                .await
                .unwrap(),
            "the 6th attempt must be refused"
        );
    }

    #[tokio::test]
    async fn distinct_sources_are_tracked_independently() {
        let limiter = InMemoryRateLimiter::default();
        let scope = RateLimitScope::ReviewerLoginAttempt;
        for _ in 0..scope.limit() {
            limiter
                .record_and_check(scope, "a@example.test")
                .await
                .unwrap();
        }
        assert!(
            !limiter
                .record_and_check(scope, "a@example.test")
                .await
                .unwrap(),
            "the exhausted source must now be refused"
        );
        assert!(
            limiter
                .record_and_check(scope, "b@example.test")
                .await
                .unwrap(),
            "a different source must have its own budget"
        );
    }

    #[tokio::test]
    async fn distinct_scopes_are_tracked_independently_for_the_same_source() {
        let limiter = InMemoryRateLimiter::default();
        for _ in 0..RateLimitScope::ReviewerLoginAttempt.limit() {
            limiter
                .record_and_check(RateLimitScope::ReviewerLoginAttempt, "shared-key")
                .await
                .unwrap();
        }
        assert!(
            !limiter
                .record_and_check(RateLimitScope::ReviewerLoginAttempt, "shared-key")
                .await
                .unwrap()
        );
        assert!(
            limiter
                .record_and_check(RateLimitScope::AnonymousReportSubmission, "shared-key")
                .await
                .unwrap(),
            "a different scope must have its own budget even for the same source key"
        );
    }
}
