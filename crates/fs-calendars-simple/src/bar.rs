//! The title line above the calendar: the shown range as a large title, "Previous / Today / Next", and the choice of the view.
//!
//! Pattern: Adapter (our state in, buttons and a segmented control of fs-components out); a click changes the shared state and asks the
//! window to draw again.

use fs_components::blinc::cn::cn;
use fs_components::blinc::cn::components::button::ButtonVariant;
use fs_components::blinc::core::BlincContextState;
use fs_components::blinc::layout::div::{Div, div};
use fs_components_fields::controls::Segmented;
use fs_components_widgets::{Block, Element, Inline, TextView};

use crate::labels::Labels;
use crate::state::{RANGES, Shared, Step, range_key, range_of};

/// Ask the window to draw the surface again (nothing to do outside a window).
pub fn redraw() {
    if BlincContextState::is_initialized() {
        BlincContextState::get().request_rebuild();
    }
}

fn button<F: Fn() + Send + Sync + 'static>(
    label: &str,
    id: &str,
    variant: ButtonVariant,
    on: F,
) -> impl Element + use<F> {
    cn::button(label.to_owned())
        .variant(variant)
        .on_click(move |_| on())
        .build_component()
        .id(id)
}

fn step_button(shared: &Shared, label: &str, id: &str, step: Step) -> impl Element + use<> {
    let shared = shared.clone();
    button(label, id, ButtonVariant::Secondary, move || {
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
