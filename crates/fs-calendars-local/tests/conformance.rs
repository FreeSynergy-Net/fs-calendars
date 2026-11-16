//! The conformance rules of `appointments_v1` against `LocalAppointments` (the same rules the fake passes).
//!
//! What the local provider does not offer is skipped by the rules themselves (subscriptions, `free_busy`, invitations)
//! or by this fixture (no AI account, no second asker): those rules are listed as skipped, not as passed.

mod common;

use async_trait::async_trait;
use common::{Fx, ms};
use fs_calendars_testdata::conformance::{ConformanceFixture, run_conformance};
use fs_calendars_testdata::draft::{Appointments, ObjectId, Utc};
use fs_objects::v5::hold;
use std::sync::atomic::Ordering;

struct LocalFx(Fx);

#[async_trait]
impl ConformanceFixture for LocalFx {
    fn appointments(&self) -> &dyn Appointments {
        &self.0.p
    }
    fn now(&self) -> Option<Utc> {
        Some(Utc::from_millis(self.0.clock.load(Ordering::SeqCst)))
    }
    async fn advance_clock(&self, millis: i64) -> bool {
        self.0.clock.fetch_add(millis, Ordering::SeqCst);
        // The period run of the program: what the deadline ended is removed.
        self.0.p.sweep_expired().await.is_ok()
    }
    async fn set_trash_period(&self, millis: Option<i64>) -> bool {
        let Ok(c) = self.0.p.store().begin().await else {
            return false;
        };
        let r = match millis {
            Some(ms) => {
                let days = (ms / common::DAY).max(1);
                let up = c.exec(
                    "INSERT INTO setting (key, int_value) VALUES ('trash_days', ?) ON CONFLICT (key) DO UPDATE SET int_value = excluded.int_value",
                    vec![turso::Value::Integer(days)],
                );
                match up.await {
                    Ok(_) => hold::set_trash_auto(c.inner(), true)
                        .await
                        .map_err(fs_calendars_local::convert::adapt),
                    Err(e) => Err(e),
                }
            }
            None => hold::set_trash_auto(c.inner(), false)
                .await
                .map_err(fs_calendars_local::convert::adapt),
        };
        c.finish(r).await.is_ok()
    }
    async fn hold(&self, id: ObjectId, end: Option<Utc>) -> bool {
        let Ok(c) = self.0.p.store().begin().await else {
            return false;
        };
        let r = hold::hold(c.inner(), id, end.map(|t| t.millis()))
            .await
            .map_err(fs_calendars_local::convert::adapt);
        c.finish(r).await.is_ok()
    }
}

#[tokio::test]
async fn the_local_provider_passes_the_rules_it_offers() {
    let fx = LocalFx(common::fx().await);
    fx.0.set_now(ms(2026, 11, 16, 10, 0));
    let report = run_conformance(&fx).await;
    assert!(
        report.failures.is_empty(),
        "failed rules: {:#?}",
        report.failures
    );
    assert!(report.checked >= 70, "only {} checks ran", report.checked);
    let skipped = report.skipped.join(" | ");
    for part in ["subscriptions", "free_busy", "no AI account"] {
        assert!(
            skipped.contains(part),
            "{part} should be skipped: {skipped}"
        );
    }
}
