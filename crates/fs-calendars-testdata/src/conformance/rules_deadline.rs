//! Rule: a retention goes before the deadline, also for the calendar around the kept appointment.
//!
//! Pattern: Template method (plain checks collected in the report).

use super::cx::Cx;
use crate::draft::{Restorable, SeriesScope, Trash, Utc};

const DAY: i64 = 86_400_000;

/// A kept appointment holds its trashed calendar in the trash beyond the period; when the retention ends, both go.
pub(super) async fn kept_calendar(cx: &mut Cx<'_>) {
    let dir = cx.dir;
    let Some(now) = cx.fx.now() else {
        cx.skip("deadline: the fixture cannot tell the time");
        return;
    };
    if !cx.fx.set_trash_period(Some(30 * DAY)).await {
        cx.skip("deadline: the fixture cannot set the period");
        return;
    }
    let Some(k) = cx.calendar("deadline-kept").await else {
        return;
    };
    let (Some(kept), Some(other)) = (
        cx.appointment(k.id, "kept", 6).await,
        cx.appointment(k.id, "other", 7).await,
    ) else {
        return;
    };
    if dir.delete_calendar(k.id, k.revision).await.is_err() {
        cx.ensure(false, "delete_calendar: answers");
        return;
    }
    let end = Utc::from_millis(now.millis() + 60 * DAY);
    if !cx.fx.hold(kept.id, Some(end)).await {
        cx.skip("deadline: the fixture cannot keep an object");
        return;
    }
    if !cx.fx.advance_clock(31 * DAY).await {
        cx.skip("deadline: the fixture cannot move the clock");
        return;
    }
    cx.ensure(
        cx.listed(k.id).await.is_some_and(|c| c.trash == Some(Restorable::Held(Some(end)))),
        "deadline: a kept appointment holds its trashed calendar beyond the period (Held(Some(end)))",
    );
    let left = cx.all(&[k.id], Trash::Only).await;
    cx.ensure(
        left.iter().any(|s| s.id == kept.id),
        "deadline: the kept appointment is still in the trash after the period",
    );
    cx.ensure(
        left.iter().any(|s| s.id == other.id),
        "deadline: the other appointments of the kept calendar stay with it",
    );
    if cx.fx.advance_clock(30 * DAY).await {
        cx.ensure(
            cx.listed(k.id).await.is_none(),
            "deadline: when the retention has ended the calendar goes",
        );
        cx.ensure(
            cx.all(&[k.id], Trash::Include).await.is_empty(),
            "deadline: and its appointments with it",
        );
        let r = dir.restore(kept.id, SeriesScope::Whole).await;
        cx.ensure(r.is_err(), "deadline: the kept appointment is gone then");
    }
}
