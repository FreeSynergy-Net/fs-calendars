//! `run_conformance`: all rules, in an order that keeps clock moves for last.
//!
//! Pattern: Template method.

use super::cx::{Cx, Report};
use super::fixture::ConformanceFixture;
use super::{rules_ai, rules_calendars, rules_freebusy, rules_purge, rules_subscribe};

/// Run every rule against the provider of the fixture. The report names the failed rules; `skipped` names those a
/// fixture could not set up.
pub async fn run_conformance(fx: &dyn ConformanceFixture) -> Report {
    let mut cx = Cx::new(fx);
    rules_calendars::manage(&mut cx).await;
    rules_calendars::cascade(&mut cx).await;
    rules_subscribe::address(&mut cx).await;
    rules_subscribe::feed(&mut cx).await;
    rules_freebusy::free_busy(&mut cx).await;
    rules_ai::ai(&mut cx).await;
    rules_purge::purge(&mut cx).await;
    rules_purge::retention(&mut cx).await;
    // The clock moves last: later rules would see a different "now".
    rules_purge::period(&mut cx).await;
    cx.report
}
