//! What the window remembers between two drawings: the range (month, week, day, agenda), the day, the chosen item, the example switch.
//!
//! Pattern: State (pure transitions, no Blinc) behind one shared handle: a surface is built again at every change, so the choice of the
//! reader lives outside the elements and is shared by every screen of the navigation (all calendars see the same month).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fs_components_widgets::calendar_view::{AGENDA_DAYS, step_from};
use fs_components_widgets::{CalendarPosition, Day, ViewRange};

/// A step of the title line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// One range back.
    Previous,
    /// One range on.
    Next,
    /// Back to today.
    Today,
}

/// The state of the window.
#[derive(Clone, Debug)]
pub struct State {
    /// The shown range.
    pub range: ViewRange,
    /// A day inside the shown range.
    pub anchor: Day,
    /// Today (what "Today" returns to and what is marked).
    pub today: Day,
    /// The key of the chosen item.
    pub chosen: Option<String>,
    /// Whether the example is shown (preview mode only).
    pub example: bool,
}

impl State {
    /// The month around `today`, nothing chosen.
    #[must_use]
    pub fn new(today: Day, example: bool) -> Self {
        Self {
            range: ViewRange::Month,
            anchor: today,
            today,
            chosen: None,
            example,
        }
    }

    /// Apply a step of the title line.
    pub fn step(&mut self, step: Step) {
        self.anchor = match step {
            Step::Previous => step_from(self.range, self.anchor, false, AGENDA_DAYS),
            Step::Next => step_from(self.range, self.anchor, true, AGENDA_DAYS),
            Step::Today => self.today,
        };
        self.chosen = None;
    }

    /// The reader clicked a day: that day opens in the day view (the model of Google Calendar and GNOME Calendar).
    pub fn open_day(&mut self, day: Day) {
        self.anchor = day;
        self.range = ViewRange::Day;
        self.chosen = None;
    }
}

/// The key of a range in the segmented control and in `--view`.
#[must_use]
pub const fn range_key(range: ViewRange) -> &'static str {
    match range {
        ViewRange::Week => "week",
        ViewRange::Day => "day",
        ViewRange::Agenda => "agenda",
        _ => "month",
    }
}

/// The range of a key; `None` for a word nobody knows.
#[must_use]
pub fn range_of(key: &str) -> Option<ViewRange> {
    Some(match key {
        "month" => ViewRange::Month,
        "week" => ViewRange::Week,
        "day" => ViewRange::Day,
        "agenda" => ViewRange::Agenda,
        _ => return None,
    })
}

/// The ranges in the order of the control.
pub const RANGES: [ViewRange; 4] = [
    ViewRange::Month,
    ViewRange::Week,
    ViewRange::Day,
    ViewRange::Agenda,
];

/// The shared handle: all screens and all callbacks hold a clone.
#[derive(Clone)]
pub struct Shared {
    state: Arc<Mutex<State>>,
    position: CalendarPosition,
}

impl Shared {
    /// A handle over `state`.
    #[must_use]
    pub fn new(state: State) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
            position: CalendarPosition::new(),
        }
    }

    /// The state, locked (a poisoned lock is taken over: a panic in another callback must not close the window).
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A copy of the state.
    #[must_use]
    pub fn snapshot(&self) -> State {
        self.lock().clone()
    }

    /// The scroll position of the time axis and the agenda, kept across drawings.
    #[must_use]
    pub fn position(&self) -> &CalendarPosition {
        &self.position
    }
}

#[cfg(test)]
mod tests {
    use super::{RANGES, State, Step, range_key, range_of};
    use fs_components_widgets::{Day, ViewRange};

    fn day(y: i32, m: u8, d: u8) -> Day {
        Day::new(y, m, d).unwrap()
    }

    #[test]
    fn previous_and_next_move_by_the_shown_range() {
        let mut s = State::new(day(2026, 11, 18), true);
        s.step(Step::Next);
        assert_eq!(s.anchor, day(2026, 12, 18));
        s.range = ViewRange::Week;
        s.step(Step::Previous);
        assert_eq!(s.anchor, day(2026, 12, 11));
        s.range = ViewRange::Day;
        s.step(Step::Next);
        assert_eq!(s.anchor, day(2026, 12, 12));
        s.step(Step::Today);
        assert_eq!(s.anchor, day(2026, 11, 18));
    }

    #[test]
    fn a_step_drops_the_chosen_item() {
        let mut s = State::new(day(2026, 11, 18), true);
        s.chosen = Some("k1".into());
        s.step(Step::Next);
        assert_eq!(s.chosen, None);
    }

    #[test]
    fn a_clicked_day_opens_the_day_view() {
        let mut s = State::new(day(2026, 11, 18), true);
        s.open_day(day(2026, 11, 3));
        assert_eq!((s.range, s.anchor), (ViewRange::Day, day(2026, 11, 3)));
        assert_eq!(s.today, day(2026, 11, 18));
    }

    #[test]
    fn the_keys_of_the_ranges_round_trip() {
        for range in RANGES {
            assert_eq!(range_of(range_key(range)), Some(range));
        }
        assert_eq!(range_of("year"), None);
    }
}
