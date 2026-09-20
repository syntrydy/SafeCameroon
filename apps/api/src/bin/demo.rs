//! Pilot/hackathon demo (prompts/15_PILOT_DEMO.md): a real, narrated,
//! end-to-end run of the whole platform against a real Postgres database
//! and the real mock/sandbox channel adapters (plus a real AI extraction
//! call when `OPENROUTER_API_KEY` is configured) -- every step below calls
//! the exact same application-layer functions the HTTP handlers call, just
//! without going through HTTP/Google auth (a demo operator is not a
//! browser). Run with:
//!
//!   DATABASE_URL=... [OPENROUTER_API_KEY=...] cargo run -p safe-cameroon-api --bin demo
//!
//! Not idempotent -- each run creates fresh rows (new report, case, alert,
//! consumers), same as a real reviewer using the system would. Safe to run
//! against a scratch/demo database repeatedly.

use std::collections::HashMap;

use safe_cameroon_application::ai_extraction::{ReportExtractor, request_report_extraction};
use safe_cameroon_application::alert_workflow::{create_alert_from_case, resolve_policy};
use safe_cameroon_application::case_workflow::{Actor, create_case_from_report, review_case};
use safe_cameroon_application::channel::{Channel, build_outbound_message};
use safe_cameroon_application::citizen_subscription::{
    CitizenSubscriptionRequest, prepare_citizen_subscription,
};
use safe_cameroon_application::delivery_workflow::{
    plan_deliveries, record_delivery_failure, record_delivery_success,
};
use safe_cameroon_domain::{
    AlertFieldValue, CaseStatus, ChannelEndpoint, ChannelType, Comparison, Consumer, ConsumerType,
    DeliveryPreference, DeliveryStrategy, GeoArea, IncidentType, Severity, Subscription,
    SubscriptionId, SubscriptionRule, TargetGeography, deduplicate_by_consumer,
    evaluate_subscriptions,
};
use safe_cameroon_infrastructure::ai::{DisabledExtractor, OpenRouterExtractor};
use safe_cameroon_infrastructure::channels::{EmailChannel, SmsChannel, WhatsAppChannel};
use safe_cameroon_infrastructure::postgres::{
    AuditEventFilter, PostgresAlertRepository, PostgresAuditEventRepository,
    PostgresCaseRepository, PostgresConsumerRepository, PostgresDeliveryPreferenceRepository,
    PostgresDeliveryRepository, PostgresReportExtractionRepository, PostgresReportRepository,
    PostgresSubscriptionRepository,
};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

fn header(step: &str, title: &str) {
    println!("\n== {step}. {title} ==");
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be configured");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("database connection must succeed");

    let reports = PostgresReportRepository::new(pool.clone());
    let extractions = PostgresReportExtractionRepository::new(pool.clone());
    let cases = PostgresCaseRepository::new(pool.clone());
    let alerts = PostgresAlertRepository::new(pool.clone());
    let consumers = PostgresConsumerRepository::new(pool.clone());
    let subscriptions = PostgresSubscriptionRepository::new(pool.clone());
    let delivery_preferences = PostgresDeliveryPreferenceRepository::new(pool.clone());
    let deliveries = PostgresDeliveryRepository::new(pool.clone());
    let audit_events = PostgresAuditEventRepository::new(pool.clone());

    // A synthetic reviewer identity: this demo calls application-layer
    // functions directly (as an admin CLI would), not through HTTP/Google
    // auth, so there is no real Google account or reviewers-table row
    // behind this id -- audit_events.actor_id has no foreign key, exactly
    // so identities like this remain attributable without one.
    let reviewer_id = Uuid::new_v4();
    let reviewer = Actor::Reviewer(reviewer_id);
    println!("Sentinel pilot demo -- reviewer id {reviewer_id}");

    // -- 1. Anonymous citizen submits a missing-child report -----------------
    header("1", "Anonymous citizen submits a report");
    let report_content = "My 8-year-old daughter Amina has not returned from school. She was \
        last seen near Carrefour Bonamoussadi around 3pm today, wearing a blue school uniform. \
        She has short black hair and was carrying a red backpack. A grey Toyota Corolla was seen \
        slowing near her around that time. Please call me if you have any information.";
    let submission = safe_cameroon_application::prepare_anonymous_report(
        report_content.to_owned(),
        Uuid::new_v4(),
        None,
        Some(IncidentType::MissingChild),
    )
    .expect("report content is valid");
    let report_id = submission.report.id;
    reports
        .submit_anonymous(&submission)
        .await
        .expect("report must persist");
    println!(
        "Report {} received. Reference code: {}",
        report_id.as_uuid(),
        submission.reference_code
    );

    // -- 2. AI extracts candidate information ---------------------------------
    header("2", "AI extraction");
    let extractor: Box<dyn ReportExtractor> = match std::env::var("OPENROUTER_API_KEY") {
        Ok(api_key) => {
            let model = std::env::var("OPENROUTER_MODEL")
                .unwrap_or_else(|_| "openai/gpt-4o-mini".to_owned());
            println!("Calling OpenRouter ({model})...");
            Box::new(OpenRouterExtractor::new(api_key, model))
        }
        Err(_) => {
            println!("OPENROUTER_API_KEY not set -- skipping the real call (feature is opt-in).");
            Box::new(DisabledExtractor)
        }
    };
    match request_report_extraction(extractor.as_ref(), report_id, report_content, reviewer_id)
        .await
    {
        Ok(record) => {
            extractions
                .create(&record)
                .await
                .expect("extraction must persist");
            println!("Suggested fields (unverified -- a reviewer decides what to do with these):");
            println!(
                "  person_description: {:?}",
                record.fields.person_description
            );
            println!("  age:                {:?}", record.fields.age);
            println!("  time:               {:?}", record.fields.time);
            println!("  place:              {:?}", record.fields.place);
            println!(
                "  incident_category:  {:?}",
                record.fields.incident_category
            );
            println!("  vehicle_details:    {:?}", record.fields.vehicle_details);
            println!("  contact_request:    {:?}", record.fields.contact_request);
        }
        Err(error) => println!("Extraction unavailable: {error}"),
    }

    // -- 3. A reviewer opens and verifies a case ------------------------------
    header("3", "Reviewer opens a case from the report");
    let request_id = Uuid::new_v4();
    let creation =
        create_case_from_report(IncidentType::MissingChild, report_id, reviewer, request_id);
    let case_id = creation.case.id();
    cases.create(&creation).await.expect("case must persist");
    println!(
        "Case {} opened (status: {:?}).",
        case_id.as_uuid(),
        creation.case.status()
    );

    header("3b", "Review begins, then the case is verified");
    let mut case = cases
        .find_by_id(case_id)
        .await
        .unwrap()
        .expect("case exists");
    let review = review_case(&mut case, reviewer, CaseStatus::UnderReview, Uuid::new_v4())
        .expect("automated/reviewer actor may start review");
    cases
        .apply_review(&case, &review)
        .await
        .expect("review must persist");

    let mut case = cases
        .find_by_id(case_id)
        .await
        .unwrap()
        .expect("case exists");
    let review = review_case(&mut case, reviewer, CaseStatus::Verified, Uuid::new_v4())
        .expect("an identified reviewer may verify");
    cases
        .apply_review(&case, &review)
        .await
        .expect("verification must persist");
    println!("Case verified.");

    // -- 4. Verified case generates a community-safe alert --------------------
    header("4", "A community-safe alert is issued");
    let policy = resolve_policy("MISSING_CHILD_COMMUNITY").expect("policy is registered");
    let target_geography = TargetGeography::new("Douala - Bonamoussadi").unwrap();
    let fields = vec![AlertFieldValue {
        field: safe_cameroon_domain::AlertField::IncidentCategory,
        value: "MISSING_CHILD".into(),
    }];
    let alert_creation = create_alert_from_case(
        &case,
        &policy,
        Severity::High,
        target_geography,
        fields,
        reviewer,
        Uuid::new_v4(),
        None,
        None,
    )
    .expect("alert is valid for a verified case");
    let alert_id = alert_creation.alert.id();
    alerts
        .create(&alert_creation)
        .await
        .expect("alert must persist");
    println!(
        "Alert {} issued: {:?} visibility, {:?} severity, target \"{}\".",
        alert_id.as_uuid(),
        alert_creation.alert.visibility(),
        alert_creation.alert.severity(),
        alert_creation.alert.target_geography().as_str()
    );

    // -- 5. Four differently-configured consumers -----------------------------
    header(
        "5",
        "Police, NGO, association, and a self-subscribed citizen register",
    );

    // Douala Police: wants every incident type/severity in Douala, over WhatsApp.
    let police = Consumer::new("Douala Police", ConsumerType::Organization).unwrap();
    consumers.create(&police).await.unwrap();
    let police_sub = Subscription::new(
        SubscriptionId::new(),
        police.id(),
        1,
        vec![
            SubscriptionRule::IncidentType(vec![
                IncidentType::MissingChild,
                IncidentType::OtherProtectionIncident,
            ]),
            SubscriptionRule::Severity {
                operator: Comparison::GreaterThanOrEqual,
                value: Severity::Low,
            },
            SubscriptionRule::Geography(vec![GeoArea::new("Douala").unwrap()]),
        ],
    )
    .unwrap();
    subscriptions.create(&police_sub).await.unwrap();
    delivery_preferences
        .upsert(
            police.id(),
            &DeliveryPreference::new(
                DeliveryStrategy::All,
                vec![ChannelEndpoint::new(ChannelType::WhatsApp, "+237600000001").unwrap()],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    println!("Registered: Douala Police (WhatsApp, all incident types, Douala).");

    // Bonamoussadi NGO: medium+ severity, narrower area, with an invalid
    // primary channel so its first delivery attempt fails a real
    // validation check (see step 7).
    let ngo = Consumer::new("Bonamoussadi NGO", ConsumerType::Organization).unwrap();
    consumers.create(&ngo).await.unwrap();
    let ngo_sub = Subscription::new(
        SubscriptionId::new(),
        ngo.id(),
        1,
        vec![
            SubscriptionRule::IncidentType(vec![IncidentType::MissingChild]),
            SubscriptionRule::Severity {
                operator: Comparison::GreaterThanOrEqual,
                value: Severity::Medium,
            },
            SubscriptionRule::Geography(vec![GeoArea::new("Bonamoussadi").unwrap()]),
        ],
    )
    .unwrap();
    subscriptions.create(&ngo_sub).await.unwrap();
    delivery_preferences
        .upsert(
            ngo.id(),
            &DeliveryPreference::new(
                DeliveryStrategy::PrimaryFallback,
                vec![
                    ChannelEndpoint::new(ChannelType::Sms, "not-a-real-number").unwrap(),
                    ChannelEndpoint::new(ChannelType::Email, "ngo@example.test").unwrap(),
                ],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    println!("Registered: Bonamoussadi NGO (SMS then email, medium+ severity, Bonamoussadi).");

    // Yaounde Association: same incident type, but a different city --
    // deliberately does not match this Douala alert.
    let association = Consumer::new(
        "Yaounde Child Protection Association",
        ConsumerType::Organization,
    )
    .unwrap();
    consumers.create(&association).await.unwrap();
    let association_sub = Subscription::new(
        SubscriptionId::new(),
        association.id(),
        1,
        vec![
            SubscriptionRule::IncidentType(vec![IncidentType::MissingChild]),
            SubscriptionRule::Severity {
                operator: Comparison::GreaterThanOrEqual,
                value: Severity::High,
            },
            SubscriptionRule::Geography(vec![GeoArea::new("Yaounde").unwrap()]),
        ],
    )
    .unwrap();
    subscriptions.create(&association_sub).await.unwrap();
    delivery_preferences
        .upsert(
            association.id(),
            &DeliveryPreference::new(
                DeliveryStrategy::All,
                vec![ChannelEndpoint::new(ChannelType::Email, "association@example.test").unwrap()],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    println!(
        "Registered: Yaounde Association (email, Yaounde only -- will NOT match this Douala alert)."
    );

    // A citizen, self-subscribed exactly the way apps/citizen does it:
    // Community/Public visibility only, enforced server-side.
    let citizen_prepared = prepare_citizen_subscription(CitizenSubscriptionRequest {
        incident_types: vec![IncidentType::MissingChild],
        minimum_severity: Severity::High,
        geography: vec!["Douala".into()],
        push_subscription_json: r#"{"endpoint":"https://example-push.invalid/demo","keys":{"p256dh":"demo","auth":"demo"}}"#.into(),
        locale: Some("fr".into()),
    })
    .expect("citizen request is valid");
    consumers
        .create_with_management_token(
            &citizen_prepared.consumer,
            &citizen_prepared.management_token_hash,
        )
        .await
        .unwrap();
    delivery_preferences
        .upsert(
            citizen_prepared.consumer.id(),
            &citizen_prepared.delivery_preference,
        )
        .await
        .unwrap();
    subscriptions
        .create(&citizen_prepared.subscription)
        .await
        .unwrap();
    println!(
        "Registered: a self-subscribed citizen (Web Push, Community/Public only, management token {}...).",
        &citizen_prepared.management_token[..8]
    );

    // -- 6. Subscription matching ----------------------------------------------
    header("6", "Subscription matching");
    let alert = alerts.find_by_id(alert_id).await.unwrap().unwrap();
    let all_subscriptions = subscriptions.list_all().await.unwrap();
    let decisions = evaluate_subscriptions(&all_subscriptions, &alert);
    for decision in &decisions {
        let consumer_name = [&police, &ngo, &association, &citizen_prepared.consumer]
            .iter()
            .find(|consumer| consumer.id() == decision.consumer_id)
            .map(|consumer| consumer.name())
            .unwrap_or("(unknown consumer)");
        println!(
            "- {consumer_name}: {}",
            if decision.matched {
                "MATCHED"
            } else {
                "did not match"
            }
        );
        for reason in &decision.reasons {
            println!("    {reason}");
        }
    }
    let consumer_matches = deduplicate_by_consumer(&decisions);
    println!(
        "{} consumer(s) will receive this alert.",
        consumer_matches.len()
    );

    // -- 7. Plan and dispatch deliveries, including a provider failure --------
    header("7", "Delivery planning and dispatch");
    let mut preferences = HashMap::new();
    for consumer_id in [
        police.id(),
        ngo.id(),
        association.id(),
        citizen_prepared.consumer.id(),
    ] {
        if let Some(preference) = delivery_preferences
            .find_by_consumer(consumer_id)
            .await
            .unwrap()
        {
            preferences.insert(consumer_id, preference);
        }
    }
    let planned = plan_deliveries(
        &alert,
        &consumer_matches,
        &preferences,
        safe_cameroon_domain::RetryPolicy::standard(),
        Actor::Automated,
        Uuid::new_v4(),
    );
    println!("{} delivery attempt(s) planned.", planned.len());
    deliveries.create_planned(&planned).await.unwrap();

    // Fallback tiers are gated: the NGO's tier-1 email is not claimable
    // until its tier-0 SMS has failed permanently, so this can take two
    // rounds -- round 1 claims every tier-0 delivery (plus any consumer with
    // no fallback at all), and round 2 picks up whatever fallback tiers just
    // became unblocked as a result. A fixed two rounds is enough for this
    // demo's fixture data (a single fallback tier); a real worker just loops
    // `claim_next` until it comes back empty.
    for round in 1..=2 {
        let claimed = deliveries.claim_next(10).await.unwrap();
        if claimed.is_empty() {
            println!("  (round {round}: nothing claimable)");
            continue;
        }
        println!("  -- round {round} --");
        for mut delivery in claimed {
            let message = build_outbound_message(&alert, &delivery, None);
            let outcome = match delivery.channel() {
                ChannelType::WhatsApp => WhatsAppChannel.send(message).await,
                ChannelType::Sms => SmsChannel.send(message).await,
                ChannelType::Email => EmailChannel.send(message).await,
                ChannelType::Push => Err(safe_cameroon_application::channel::ChannelError {
                    retryable: false,
                    message: "demo push endpoint is a placeholder, not a real subscription".into(),
                }),
            };
            match outcome {
                Ok(send_outcome) => {
                    let transition = record_delivery_success(
                        &mut delivery,
                        send_outcome.provider_message_id,
                        Actor::Automated,
                        Uuid::new_v4(),
                    )
                    .unwrap();
                    deliveries
                        .apply_attempt_transition(&delivery, &transition)
                        .await
                        .unwrap();
                    println!(
                        "  OK   {:?} -> {} (tier {})",
                        delivery.channel(),
                        delivery.endpoint_address(),
                        delivery.tier()
                    );
                }
                Err(error) => {
                    let transition = record_delivery_failure(
                        &mut delivery,
                        error.retryable,
                        error.message.clone(),
                        Actor::Automated,
                        Uuid::new_v4(),
                    )
                    .unwrap();
                    deliveries
                        .apply_attempt_transition(&delivery, &transition)
                        .await
                        .unwrap();
                    println!(
                        "  FAIL {:?} -> {} (tier {}): {} ({})",
                        delivery.channel(),
                        delivery.endpoint_address(),
                        delivery.tier(),
                        error.message,
                        if error.retryable {
                            "will retry"
                        } else {
                            "permanent"
                        }
                    );
                }
            }
        }
    }
    println!(
        "Note: a fallback tier (tier > 0) is now gated -- it only becomes claimable once every \
         lower tier for that alert/consumer has failed permanently. The Bonamoussadi NGO's \
         invalid SMS (tier 0) fails permanently in round 1, which is what unblocks its email \
         (tier 1) for round 2; a consumer whose primary tier succeeds never has its fallback \
         claimed at all."
    );

    // -- 8. Case resolved; a follow-up update alert is issued ------------------
    header("8", "Child found -- case resolved, follow-up alert issued");
    let mut case = cases.find_by_id(case_id).await.unwrap().unwrap();
    let review = review_case(&mut case, reviewer, CaseStatus::Active, Uuid::new_v4())
        .expect("a verified case can become actively worked");
    cases.apply_review(&case, &review).await.unwrap();

    let mut case = cases.find_by_id(case_id).await.unwrap().unwrap();
    let review = review_case(&mut case, reviewer, CaseStatus::Resolved, Uuid::new_v4()).unwrap();
    cases.apply_review(&case, &review).await.unwrap();
    println!("Case resolved.");

    let followup_fields = vec![AlertFieldValue {
        field: safe_cameroon_domain::AlertField::IncidentCategory,
        value: "MISSING_CHILD_RESOLVED".into(),
    }];
    let followup_creation = create_alert_from_case(
        &case,
        &policy,
        Severity::Low,
        TargetGeography::new("Douala - Bonamoussadi").unwrap(),
        followup_fields,
        reviewer,
        Uuid::new_v4(),
        None,
        None,
    )
    .expect("a resolved case may still carry a follow-up community update");
    alerts.create(&followup_creation).await.unwrap();
    println!(
        "Note: today a reviewer issues this follow-up alert explicitly (same endpoint as step \
         4) -- there is no automatic 'resolving a case notifies prior recipients' linkage yet \
         (docs/MVP_FLOWS.md Flow 6 describes this as a future step)."
    );

    // -- 9. Audit trail ----------------------------------------------------------
    header("9", "Audit trail");
    let case_events = audit_events
        .list(
            &AuditEventFilter {
                resource_type: Some("CASE".into()),
                resource_id: Some(case_id.as_uuid()),
                ..Default::default()
            },
            50,
            0,
        )
        .await
        .unwrap();
    for event in case_events.iter().rev() {
        println!(
            "  [{}] {} (actor: {:?})",
            event.occurred_at, event.action, event.actor_id
        );
    }

    println!(
        "\nDemo complete. Report reference: {}",
        submission.reference_code
    );
    println!("Case: {}", case_id.as_uuid());
    println!("Original alert: {}", alert_id.as_uuid());
    println!(
        "Follow-up alert: {}",
        followup_creation.alert.id().as_uuid()
    );
}
