//! The reading methods of the fake.
//!
//! Pattern: Repository queries over the in-memory state (sorted, paged by an opaque `Place`).

use crate::draft::{
    Appointment, AppointmentQuery, AppointmentSummary, CalendarInfo, Change, Damaged, Error,
    HistoryEntry, ObjectId, Page, Place, Problem, Result, Utc, Window, Zone,
};
use crate::fake::FakeAppointments;
use crate::instances::in_window;
use crate::state::{Item, State};
use crate::zone::{self, bounds};

fn page<T: Clone>(all: &[T], after: Option<&Place>, size: usize) -> Result<Page<T>> {
    if size == 0 {
        return Err(Error::Invalid(vec![Problem {
            field: "size".to_owned(),
            reason: "must be at least 1".to_owned(),
        }]));
    }
    let from = match after {
        None => 0,
        Some(p) => p.0.parse::<usize>().map_err(|_| {
            Error::Invalid(vec![Problem {
                field: "after".to_owned(),
                reason: "not a place of this provider".to_owned(),
            }])
        })?,
    };
    let items: Vec<T> = all.iter().skip(from).take(size).cloned().collect();
    let end = from + items.len();
    Ok(Page {
        items,
        next: (end < all.len()).then(|| Place(end.to_string())),
    })
}

fn summary(a: &Appointment) -> AppointmentSummary {
    AppointmentSummary {
        id: a.id,
        calendar: a.calendar,
        uid: a.uid.clone(),
        title: a.title.clone(),
        when: a.when.clone(),
        recurring: !a.recurrence.is_empty(),
        revision: a.revision,
        restorable_until: None,
    }
}

/// The zone of the places that have no viewer (sorting, `list`): UTC.
fn utc_zone() -> Zone {
    zone::iana("UTC")
}

fn first_start(s: &State, a: &Appointment) -> i64 {
    bounds(&a.when, &s.customs, &utc_zone()).map_or(i64::MAX, |(start, _)| start.millis())
}

fn contains(a: &Appointment, needle: &str) -> bool {
    let n = needle.to_lowercase();
    let hit = |t: &str| t.to_lowercase().contains(&n);
    hit(&a.title) || hit(&a.description) || a.locations.iter().any(|l| hit(&l.name))
}

fn sorted(s: &State, keep: impl Fn(&Item) -> bool) -> Vec<&Appointment> {
    let mut v: Vec<&Appointment> = s
        .items
        .values()
        .filter(|i| keep(i))
        .map(|i| &i.appt)
        .collect();
    v.sort_by_key(|a| (first_start(s, a), a.id));
    v
}

impl FakeAppointments {
    pub(crate) fn calendars_impl(&self) -> Vec<CalendarInfo> {
        self.lock().calendars.clone()
    }

    pub(crate) fn get_impl(&self, id: ObjectId) -> Option<Appointment> {
        self.lock().live(id).ok().map(|i| i.appt.clone())
    }

    pub(crate) fn get_by_uid_impl(&self, uid: &str) -> Option<Appointment> {
        self.lock()
            .items
            .values()
            .find(|i| i.deleted.is_none() && i.appt.uid == uid)
            .map(|i| i.appt.clone())
    }

    pub(crate) fn list_impl(
        &self,
        q: &AppointmentQuery,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<AppointmentSummary>> {
        let s = self.lock();
        let rows = sorted(&s, |i| {
            (q.include_deleted || i.deleted.is_none())
                && (q.calendars.is_empty() || q.calendars.contains(&i.appt.calendar))
        });
        let kept: Vec<AppointmentSummary> = rows
            .into_iter()
            .filter(|a| q.text.as_deref().is_none_or(|t| contains(a, t)))
            .filter(|a| {
                q.participant
                    .as_ref()
                    .is_none_or(|p| a.participants.iter().any(|x| x.who == *p))
            })
            .filter(|a| match (q.from, q.to) {
                (None, None) => true,
                (from, to) => {
                    let (lo, hi) = (
                        from.unwrap_or(Utc::from_millis(i64::MIN)),
                        to.unwrap_or(Utc::from_millis(i64::MAX)),
                    );
                    let (found, _) = in_window(a, &s.customs, None, &utc_zone(), lo, hi);
                    !found.is_empty()
                }
            })
            .map(summary)
            .collect();
        page(&kept, after, size)
    }

    pub(crate) fn window_impl(
        &self,
        from: Utc,
        to: Utc,
        view_zone: &Zone,
        calendars: &[ObjectId],
    ) -> Result<Window> {
        if from >= to {
            return Err(Error::Invalid(vec![Problem {
                field: "window".to_owned(),
                reason: "from must be before to".to_owned(),
            }]));
        }
        let s = self.lock();
        let viewer = s.viewer.clone();
        let mut w = Window::default();
        for item in s.items.values().filter(|i| {
            i.deleted.is_none() && (calendars.is_empty() || calendars.contains(&i.appt.calendar))
        }) {
            if let Some(reason) = s.damaged.get(&item.appt.id) {
                w.damaged.push(Damaged {
                    appointment: item.appt.id,
                    recurrence_id: None,
                    reason: reason.clone(),
                    cut: false,
                });
                continue;
            }
            let (found, bad) =
                in_window(&item.appt, &s.customs, Some(&viewer), view_zone, from, to);
            w.instances.extend(found);
            w.damaged.extend(bad);
        }
        w.instances.sort_by(|a, b| {
            (a.start, a.end, &a.title, a.appointment).cmp(&(
                b.start,
                b.end,
                &b.title,
                b.appointment,
            ))
        });
        Ok(w)
    }

    pub(crate) fn search_impl(
        &self,
        text: &str,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<AppointmentSummary>> {
        let s = self.lock();
        let hits: Vec<AppointmentSummary> = sorted(&s, |i| i.deleted.is_none())
            .into_iter()
            .filter(|a| contains(a, text))
            .map(summary)
            .collect();
        page(&hits, after, size)
    }

    pub(crate) fn history_impl(
        &self,
        id: ObjectId,
        after: Option<&Place>,
        size: usize,
    ) -> Result<Page<HistoryEntry>> {
        let s = self.lock();
        if !s.items.contains_key(&id) {
            return Err(Error::NotFound(id));
        }
        let entries: Vec<HistoryEntry> = s
            .history
            .iter()
            .filter(|(i, _)| *i == id)
            .map(|(_, e)| e.clone())
            .collect();
        page(&entries, after, size)
    }

    pub(crate) fn changes_impl(&self, since: Option<&Place>, size: usize) -> Result<Page<Change>> {
        let s = self.lock();
        let Some(since) = since else {
            return Ok(Page {
                items: Vec::new(),
                next: Some(Place(s.changes.len().to_string())),
            });
        };
        let p = page(&s.changes, Some(since), size)?;
        let next = p.next.or_else(|| Some(Place(s.changes.len().to_string())));
        Ok(Page {
            items: p.items,
            next,
        })
    }
}
