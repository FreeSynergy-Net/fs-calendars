//! The title line above the calendar: the shown range as a large title, "Previous / Today / Next", and the choice of the view.
//!
//! Pattern: Adapter (our state in, buttons and a segmented control of fs-components out); a click changes the shared state and asks the
//! window to draw again.

use fs_components::blinc::cn::cn;
use fs_components::blinc::cn::components::button::ButtonVariant;
use fs_components::blinc::core::BlincContextState;
use fs_components::blinc::layout::div::{Div, div};
use fs_components::blinc::layout::prelude::text;
use fs_components_fields::controls::Segmented;
use fs_components_widgets::{Block, Element, Inline, TextView};

use crate::labels::Labels;
use crate::preview::dot_class;
use crate::state::{RANGES, Shared, Step, range_key, range_of};

/// Ask the window to draw the surface again (nothing to do outside a window).
pub fn redraw() {
    if BlincContextState::is_initialized() {
        BlincContextState::get().request_rebuild();
    }
}

fn button<F: Fn() + Send + Sync + 'static>(
    label: &str,
    help: &str,
    id: &str,
    variant: ButtonVariant,
    on: F,
) -> impl Element + use<F> {
    let inner = cn::button(label.to_owned())
        .variant(variant)
        .on_click(move |_| on())
        .build_component()
        .id(id);
    // The word of an arrow stays as its help text (reading aloud and tooltip), also when the button shows a sign only.
    div()
        .attr("aria-label", help)
        .attr("title", help)
        .child(inner)
}

/// The text on a step button: an arrow for back and on, the word for today. The word of an arrow stays as its help text.
#[must_use]
pub const fn arrow(step: Step) -> Option<&'static str> {
    match step {
        Step::Previous => Some("\u{2039}"),
        Step::Next => Some("\u{203A}"),
        Step::Today => None,
    }
}

fn step_button(shared: &Shared, word: &str, id: &str, step: Step) -> impl Element + use<> {
    let shared = shared.clone();
    let shown = arrow(step).unwrap_or(word);
    button(shown, word, id, ButtonVariant::Outline, move || {
        shared.lock().step(step);
        redraw();
    })
}

fn views(shared: &Shared, labels: &Labels) -> Segmented {
    let handle = shared.clone();
    let mut group = Segmented::new().id("cal-view");
    for range in RANGES {
        let label = match range_key(range) {
            "week" => &labels.week,
            "day" => &labels.day,
            "agenda" => &labels.agenda,
            _ => &labels.month,
        };
        group = group.segment(range_key(range), label, false);
    }
    let group = group.on_change(move |key| {
        if let Some(range) = range_of(key) {
            handle.lock().range = range;
            redraw();
        }
    });
    group.set(range_key(shared.lock().range));
    group
}

/// The legend: a dot in the colour of each calendar, then its name.
pub fn legend(labels: &Labels) -> Div {
    let mut row = div()
        .id("cal-legend")
        .flex_row()
        .items_center()
        .gap_px(18.0)
        .w_full();
    for (category, name) in &labels.calendars {
        let dot = div()
            .class("fs-cal-dot")
            .class(dot_class(*category))
            .w(12.0)
            .h(12.0);
        row = row.child(
            div()
                .flex_row()
                .items_center()
                .gap_px(6.0)
                .child(dot)
                .child(text(name.as_str()).size(13.0)),
        );
    }
    row
}

/// The line: `title` at the left, the steps and the view choice at the right.
pub fn bar(shared: &Shared, labels: &Labels, title: &str) -> Div {
    let title = TextView::new(vec![Block::Heading {
        level: 1,
        text: Inline::plain(title),
    }])
    .build();
    let steps = div()
        .flex_row()
        .items_center()
        .gap_px(8.0)
        .child(step_button(
            shared,
            &labels.previous,
            "cal-previous",
            Step::Previous,
        ))
        .child(step_button(shared, &labels.today, "cal-today", Step::Today))
        .child(step_button(shared, &labels.next, "cal-next", Step::Next));
    div()
        .id("cal-bar")
        .flex_row()
        .items_center()
        .gap_px(16.0)
        .w_full()
        .child(div().flex_grow_value(1.0).child(title))
        .child(steps)
        .child(views(shared, labels).build())
}

#[cfg(test)]
mod tests {
    use super::arrow;
    use crate::state::Step;

    #[test]
    fn back_and_on_are_arrows_and_today_keeps_its_word() {
        assert_eq!(arrow(Step::Previous), Some("\u{2039}"));
        assert_eq!(arrow(Step::Next), Some("\u{203A}"));
        assert_eq!(arrow(Step::Today), None);
    }
}
