//! One dispatch cycle: claim ready deliveries, send each through whichever
//! [`Channel`] matches its [`ChannelType`](safe_cameroon_domain::ChannelType),
//! and record the outcome. Kept as a plain function (not a struct/trait) so
//! it is directly unit-testable against a real database without a running
//! process.

use safe_cameroon_application::case_workflow::Actor;
use safe_cameroon_application::channel::{ChannelRegistry, build_outbound_message};
use safe_cameroon_application::delivery_workflow::{
    record_delivery_failure, record_delivery_success,
};
use safe_cameroon_domain::Delivery;
use safe_cameroon_infrastructure::postgres::{PostgresAlertRepository, PostgresDeliveryRepository};
use uuid::Uuid;

/// Claims up to `batch_size` deliveries and dispatches each one. Returns how
/// many were claimed (0 means the caller should back off before polling
/// again). A single delivery failing to dispatch never aborts the batch or
/// propagates as an error here — only a database error does; per-delivery
/// outcomes are always recorded on the delivery itself.
pub async fn process_batch(
    deliveries: &PostgresDeliveryRepository,
    alerts: &PostgresAlertRepository,
    registry: &ChannelRegistry,
    batch_size: i64,
) -> Result<usize, sqlx::Error> {
    let claimed = deliveries.claim_next(batch_size).await?;
    let count = claimed.len();
    for mut delivery in claimed {
        dispatch_one(deliveries, alerts, registry, &mut delivery).await?;
    }
    Ok(count)
}

/// `delivery` must already be `Sending` (i.e. just returned by `claim_next`);
/// every `record_delivery_*` call below is expected to succeed against that
/// guarantee. One `operation_id` correlates whichever outcome this dispatch
/// produces (docs/OBSERVABILITY.md section 2) — previously `record_success`/
/// `record_failure` each minted their own disconnected id.
async fn dispatch_one(
    deliveries: &PostgresDeliveryRepository,
    alerts: &PostgresAlertRepository,
    registry: &ChannelRegistry,
    delivery: &mut Delivery,
) -> Result<(), sqlx::Error> {
    let operation_id = Uuid::new_v4();
    let delivery_id = delivery.id().as_uuid();
    let channel = delivery.channel();

    let Some(channel_adapter) = registry.get(channel) else {
        return fail(
            deliveries,
            delivery,
            operation_id,
            false,
            "no channel registered for this delivery's channel type",
        )
        .await;
    };

    let Some(alert) = alerts.find_by_id(delivery.alert_id()).await? else {
        return fail(
            deliveries,
            delivery,
            operation_id,
            false,
            "alert no longer exists",
        )
        .await;
    };

    let message = build_outbound_message(&alert, delivery);
    match channel_adapter.send(message).await {
        Ok(outcome) => {
            let transition = record_delivery_success(
                delivery,
                outcome.provider_message_id,
                Actor::Automated,
                operation_id,
            )
            .expect("a claimed delivery is always Sending");
            deliveries
                .apply_attempt_transition(delivery, &transition)
                .await?;
            tracing::info!(
                request_id = %operation_id, %delivery_id, channel = ?channel,
                "delivery sent"
            );
        }
        Err(error) => {
            fail(
                deliveries,
                delivery,
                operation_id,
                error.retryable,
                &error.message,
            )
            .await?;
        }
    }
    Ok(())
}

async fn fail(
    deliveries: &PostgresDeliveryRepository,
    delivery: &mut Delivery,
    operation_id: Uuid,
    retryable: bool,
    reason: &str,
) -> Result<(), sqlx::Error> {
    let delivery_id = delivery.id().as_uuid();
    let channel = delivery.channel();
    let transition =
        record_delivery_failure(delivery, retryable, reason, Actor::Automated, operation_id)
            .expect("a claimed delivery is always Sending");
    deliveries
        .apply_attempt_transition(delivery, &transition)
        .await?;
    tracing::warn!(
        request_id = %operation_id, %delivery_id, channel = ?channel, retryable, reason,
        "delivery failed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::env;
    use std::sync::Arc;

    use async_trait::async_trait;
    use safe_cameroon_application::alert_workflow::create_alert_from_case;
    use safe_cameroon_application::case_workflow::{create_case_from_report, review_case};
    use safe_cameroon_application::channel::{
        Channel, ChannelError, ChannelSendOutcome, EndpointValidationError, OutboundMessage,
    };
    use safe_cameroon_application::delivery_workflow::plan_deliveries;
    use safe_cameroon_application::prepare_anonymous_report;
    use safe_cameroon_domain::{
        Alert, AlertField, AlertFieldValue, AlertPolicy, CaseStatus, ChannelEndpoint, ChannelType,
        ConsumerId, ConsumerMatch, DeliveryPreference, DeliveryStatus, DeliveryStrategy,
        IncidentType, MatchedSubscription, ReportId, RetryPolicy, Severity, SubscriptionId,
        TargetGeography,
    };
    use safe_cameroon_infrastructure::postgres::{
        PostgresCaseRepository, PostgresReportRepository,
    };
    use sqlx::{PgPool, postgres::PgPoolOptions};

    static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

    async fn test_pool() -> PgPool {
        let database_url = env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must point to a dedicated PostgreSQL test database");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database must be reachable");

        let (database_name,): (String,) = sqlx::query_as("SELECT current_database()")
            .fetch_one(&pool)
            .await
            .expect("test database name must be readable");
        assert!(
            database_name.to_ascii_lowercase().contains("test"),
            "TEST_DATABASE_URL must target a database with 'test' in its name"
        );

        MIGRATOR.run(&pool).await.expect("migrations must apply");
        sqlx::query(
            "TRUNCATE attachments, webhook_replay_events, delivery_events, delivery_attempts, \
             deliveries, alert_events, alert_fields, alerts, case_events, case_reports, cases, \
             outbox_events, audit_events, reports, reporters",
        )
        .execute(&pool)
        .await
        .expect("test tables must be reset");
        pool
    }

    async fn verified_alert(pool: &PgPool) -> Alert {
        let report_repository = PostgresReportRepository::new(pool.clone());
        let submission =
            prepare_anonymous_report("A child is missing.".into(), Uuid::new_v4(), None).unwrap();
        report_repository
            .submit_anonymous(&submission)
            .await
            .unwrap();

        let case_repository = PostgresCaseRepository::new(pool.clone());
        let creation = create_case_from_report(
            IncidentType::MissingChild,
            ReportId::from_uuid(submission.report.id.as_uuid()),
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
        );
        case_repository.create(&creation).await.unwrap();

        let mut case = case_repository
            .find_by_id(creation.case.id())
            .await
            .unwrap()
            .unwrap();
        for target in [CaseStatus::UnderReview, CaseStatus::Verified] {
            let review = review_case(
                &mut case,
                Actor::Reviewer(Uuid::new_v4()),
                target,
                Uuid::new_v4(),
            )
            .unwrap();
            case_repository.apply_review(&case, &review).await.unwrap();
        }
        let case = case_repository
            .find_by_id(case.id())
            .await
            .unwrap()
            .unwrap();

        let alert_repository = PostgresAlertRepository::new(pool.clone());
        let policy = AlertPolicy::missing_child_community_v1();
        let alert_creation = create_alert_from_case(
            &case,
            &policy,
            Severity::High,
            TargetGeography::new("Douala").unwrap(),
            vec![AlertFieldValue {
                field: AlertField::IncidentCategory,
                value: "MISSING_CHILD".into(),
            }],
            Actor::Reviewer(Uuid::new_v4()),
            Uuid::new_v4(),
        )
        .unwrap();
        alert_repository.create(&alert_creation).await.unwrap();
        alert_creation.alert
    }

    async fn plan_and_persist_one(
        delivery_repository: &PostgresDeliveryRepository,
        alert: &Alert,
        channel: ChannelType,
    ) -> safe_cameroon_domain::DeliveryId {
        let consumer_id = ConsumerId::new();
        let preference = DeliveryPreference::new(
            DeliveryStrategy::All,
            vec![ChannelEndpoint::new(channel, "+237600000000").unwrap()],
        )
        .unwrap();
        let mut preferences = HashMap::new();
        preferences.insert(consumer_id, preference);
        let consumer_matches = vec![ConsumerMatch {
            consumer_id,
            matching_subscriptions: vec![MatchedSubscription {
                subscription_id: SubscriptionId::new(),
                subscription_version: 1,
            }],
        }];
        let planned = plan_deliveries(
            alert,
            &consumer_matches,
            &preferences,
            RetryPolicy::standard(),
            Actor::Automated,
            Uuid::new_v4(),
        );
        delivery_repository.create_planned(&planned).await.unwrap();
        planned[0].delivery.id()
    }

    struct AlwaysSucceeds;

    #[async_trait]
    impl Channel for AlwaysSucceeds {
        fn channel_type(&self) -> ChannelType {
            ChannelType::WhatsApp
        }

        fn validate_endpoint(
            &self,
            _endpoint_address: &str,
        ) -> Result<(), EndpointValidationError> {
            Ok(())
        }

        async fn send(
            &self,
            _message: OutboundMessage,
        ) -> Result<ChannelSendOutcome, ChannelError> {
            Ok(ChannelSendOutcome {
                provider_message_id: Some("provider-msg-1".into()),
            })
        }
    }

    struct AlwaysFailsPermanently;

    #[async_trait]
    impl Channel for AlwaysFailsPermanently {
        fn channel_type(&self) -> ChannelType {
            ChannelType::WhatsApp
        }

        fn validate_endpoint(
            &self,
            _endpoint_address: &str,
        ) -> Result<(), EndpointValidationError> {
            Ok(())
        }

        async fn send(
            &self,
            _message: OutboundMessage,
        ) -> Result<ChannelSendOutcome, ChannelError> {
            Err(ChannelError {
                retryable: false,
                message: "provider rejected the message".into(),
            })
        }
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn dispatches_a_queued_delivery_and_records_its_provider_message_id() {
        let pool = test_pool().await;
        let alert = verified_alert(&pool).await;
        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let alerts = PostgresAlertRepository::new(pool.clone());
        let delivery_id = plan_and_persist_one(&deliveries, &alert, ChannelType::WhatsApp).await;

        let mut registry = ChannelRegistry::new();
        registry.register(Arc::new(AlwaysSucceeds));

        let processed = process_batch(&deliveries, &alerts, &registry, 10)
            .await
            .unwrap();
        assert_eq!(processed, 1);

        let delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
        assert_eq!(delivery.status(), DeliveryStatus::Sent);

        let (provider_message_id,): (Option<String>,) = sqlx::query_as(
            "SELECT provider_message_id FROM delivery_attempts WHERE delivery_id = $1",
        )
        .bind(delivery_id.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(provider_message_id, Some("provider-msg-1".into()));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_provider_failure_marks_only_the_delivery_and_leaves_the_alert_and_case_untouched() {
        let pool = test_pool().await;
        let alert = verified_alert(&pool).await;
        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let alerts = PostgresAlertRepository::new(pool.clone());
        let delivery_id = plan_and_persist_one(&deliveries, &alert, ChannelType::WhatsApp).await;

        let (alert_status_before, alert_version_before): (String, i64) =
            sqlx::query_as("SELECT status::text, aggregate_version FROM alerts WHERE id = $1")
                .bind(alert.id().as_uuid())
                .fetch_one(&pool)
                .await
                .unwrap();
        let (case_status_before, case_version_before): (String, i64) =
            sqlx::query_as("SELECT status::text, aggregate_version FROM cases WHERE id = $1")
                .bind(alert.case_id().as_uuid())
                .fetch_one(&pool)
                .await
                .unwrap();

        let mut registry = ChannelRegistry::new();
        registry.register(Arc::new(AlwaysFailsPermanently));

        let processed = process_batch(&deliveries, &alerts, &registry, 10)
            .await
            .unwrap();
        assert_eq!(processed, 1);

        let delivery = deliveries.find_by_id(delivery_id).await.unwrap().unwrap();
        assert_eq!(delivery.status(), DeliveryStatus::FailedPermanently);

        // The provider failure that just permanently failed this delivery
        // must be invisible to the alert/case it was delivering, per
        // AGENTS.md: "A provider failure must not fail alert creation."
        let (alert_status_after, alert_version_after): (String, i64) =
            sqlx::query_as("SELECT status::text, aggregate_version FROM alerts WHERE id = $1")
                .bind(alert.id().as_uuid())
                .fetch_one(&pool)
                .await
                .unwrap();
        let (case_status_after, case_version_after): (String, i64) =
            sqlx::query_as("SELECT status::text, aggregate_version FROM cases WHERE id = $1")
                .bind(alert.case_id().as_uuid())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(alert_status_before, alert_status_after);
        assert_eq!(alert_version_before, alert_version_after);
        assert_eq!(case_status_before, case_status_after);
        assert_eq!(case_version_before, case_version_after);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL for a dedicated PostgreSQL test database"]
    async fn a_delivery_is_never_processed_twice_across_batches() {
        let pool = test_pool().await;
        let alert = verified_alert(&pool).await;
        let deliveries = PostgresDeliveryRepository::new(pool.clone());
        let alerts = PostgresAlertRepository::new(pool.clone());
        plan_and_persist_one(&deliveries, &alert, ChannelType::WhatsApp).await;

        let mut registry = ChannelRegistry::new();
        registry.register(Arc::new(AlwaysSucceeds));

        assert_eq!(
            process_batch(&deliveries, &alerts, &registry, 10)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            process_batch(&deliveries, &alerts, &registry, 10)
                .await
                .unwrap(),
            0,
            "an already-Sent delivery must not be reclaimed by a later batch"
        );
    }
}
