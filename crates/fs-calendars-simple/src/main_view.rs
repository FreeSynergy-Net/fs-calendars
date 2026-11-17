//! The main view: one screen per calendar of the navigation on the left (all calendars first), in the look of its models (GNOME
//! Calendar, Thunderbird, Nextcloud Calendar): a large title with the shown range, "Previous / Today / Next" and the choice of the view,
//! then the month, the week, the day or the agenda.
//!
//! Everything on the screen is a block of fs-components: the text view for the title, `segmented`, `calendar-view`, the `alert` (the
//! marker "Preview", exactly once) and the `empty-state`. While the server offers no capability `appointments`, the items are the
//! example of `preview.rs` and the view says so at the top; after that the view shows what the server delivers (the client cannot read
//! appointments yet, so the empty state says that plainly).
//!
//! Pattern: Adapter (our items in, blocks of the shell out) + State (`state.rs`: range, day and example switch live outside the
//! element and are shared by all screens, because a surface is built again at every change).

use std::sync::{Arc, OnceLock};

use fs_components::blinc::layout::div::div;
use fs_components_app::commands::{KeyNames, Platform, Resolved, Target, TextKey};
use fs_components_app::{AppWidget, ModelTexts, Surface, context_trigger, key_names};
use fs_components_mf2::Mf2Texts;
use fs_components_widgets::states::{Alert, EmptyState, Severity};
use fs_components_widgets::{
    CalendarItem, CalendarNames, CalendarView, Day, Element, Glyph, Moment, Picked, ViewRange,
    styles,
};
use fs_components_words::Locale;

use crate::bar::{bar, legend, redraw};
use crate::context::{KIND, items as context_items, say, words as context_words};
use crate::filter::{Notice, notice, visible};
use crate::labels::Labels;
use crate::names::{german, names, title};
use crate::preview::{Item, Mode, items, mode};
use crate::state::{Shared, State};

/// Padding of the page in pixels (Blinc counts padding in units of 4 px).
const PAD: f32 = 16.0;
/// What the title line needs; the notice and the calendar get the rest.
const BAR_HEIGHT: f32 = 64.0 + 28.0;
/// What the notice needs, with its gap.
const NOTICE_HEIGHT: f32 = 104.0;

/// One screen of the main view: the calendar `calendar` (`None`: all calendars).
pub struct Main {
    pub(crate) labels: Labels,
    pub(crate) names: Arc<CalendarNames>,
    pub(crate) items: Arc<Vec<Item>>,
    pub(crate) now: Moment,
    pub(crate) mode: Mode,
    pub(crate) calendar: Option<&'static str>,
    pub(crate) shared: Shared,
    pub(crate) dot: bool,
    pub(crate) keys: Arc<KeyNames>,
    pub(crate) shell: Arc<OnceLock<AppWidget>>,
    pub(crate) menu: Arc<Vec<Resolved>>,
    pub(crate) menu_words: Arc<std::collections::HashMap<String, String>>,
}

impl Main {
    /// The screen "all calendars" for a server that offers `capabilities`: `today` is marked, `now` is the line on the time axis,
    /// `example` says whether the example is shown at first, `range` which view opens (the example only matters in preview mode).
    #[must_use]
    pub fn new(
        texts: &Mf2Texts,
        capabilities: &[String],
        clock: (Day, Moment),
        example: bool,
        range: ViewRange,
    ) -> Self {
        let mut state = State::new(clock.0, example);
        state.range = range;
        Self {
            labels: Labels::read(texts),
            names: Arc::new(names(texts)),
            items: Arc::new(items(texts, clock.0)),
            now: clock.1,
            mode: mode(capabilities),
            calendar: None,
            shared: Shared::new(state),
            dot: german(texts),
            keys: Arc::new(key_names(&locale_of(texts))),
            shell: Arc::new(OnceLock::new()),
            menu: Arc::new(context_items()),
            menu_words: context_words(texts),
        }
    }

    /// Another screen of the same window: the same items, the same state, another calendar.
    #[must_use]
    pub fn sibling(&self, calendar: Option<&'static str>) -> Self {
        Self {
            labels: self.labels.clone(),
            names: Arc::clone(&self.names),
            items: Arc::clone(&self.items),
            now: self.now,
            mode: self.mode,
            calendar,
            shared: self.shared.clone(),
            dot: self.dot,
            keys: Arc::clone(&self.keys),
            shell: Arc::clone(&self.shell),
            menu: Arc::clone(&self.menu),
            menu_words: Arc::clone(&self.menu_words),
        }
    }

    /// Example or real data.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The cell the window puts its shell into, so the context menu can open on it.
    #[must_use]
    pub fn shell(&self) -> &Arc<OnceLock<AppWidget>> {
        &self.shell
    }

    /// The shared state of the window.
    #[must_use]
    pub fn shared(&self) -> &Shared {
        &self.shared
    }

    /// The marker shown now.
    #[must_use]
    pub fn notice(&self) -> Option<Notice> {
        notice(self.mode, self.shared.lock().example)
    }

    /// The items shown now.
    #[must_use]
    pub fn items(&self) -> Vec<CalendarItem> {
        visible(
            self.mode,
            self.shared.lock().example,
            &self.items,
            self.calendar,
        )
    }

    /// The title line text now.
    #[must_use]
    pub fn title(&self) -> String {
        let s = self.shared.snapshot();
        if s.range == ViewRange::Agenda {
            // The agenda runs on for thirty days: "from 18 November 2026" says it shorter than the whole span.
            let point = if self.dot { "." } else { "" };
            let (day, year) = (s.anchor.day(), s.anchor.year());
            let month = self.names.month_of(s.anchor);
            return format!("{} {day}{point} {month} {year}", self.labels.agenda_from);
        }
        title(s.range, s.anchor, 0, &self.names, self.dot)
    }

    fn marker(&self, kind: Notice) -> impl Element + use<> {
        let shared = self.shared.clone();
        let (text, label, next) = match kind {
            Notice::Shown => (&self.labels.preview_text, &self.labels.hide, false),
            Notice::Hidden => (&self.labels.preview_hidden, &self.labels.show, true),
        };
        Alert::new(Severity::Info, text)
            .id("preview")
            .title(&self.labels.preview_title)
            .action(label, move || {
                shared.lock().example = next;
                redraw();
            })
            .build()
    }

    fn empty(&self, size: (f32, f32)) -> impl Element + use<> {
        let empty = EmptyState::new(&self.labels.empty_title)
            .id("empty")
            .glyph(Glyph::Calendar)
            .text(&self.labels.empty_text);
        div().w(size.0).h(size.1).child(empty.build())
    }

    fn calendar(&self, items: Vec<CalendarItem>, size: (f32, f32)) -> impl Element + use<> {
        let s = self.shared.snapshot();
        let handle = self.shared.clone();
        let on_pick: Arc<dyn Fn(Picked) + Send + Sync> = Arc::new(move |picked| {
            match picked {
                Picked::Day(day) | Picked::More(day) => handle.lock().open_day(day),
                Picked::Item(key) => handle.lock().chosen = Some(key),
                _ => return,
            }
            redraw();
        });
        let keys = Arc::clone(&self.keys);
        let (shell, menu, menu_words) = (
            Arc::clone(&self.shell),
            Arc::clone(&self.menu),
            Arc::clone(&self.menu_words),
        );
        let chosen: Vec<String> = s.chosen.iter().cloned().collect();
        let view = CalendarView::new(s.range, s.anchor, Arc::clone(&self.names), Arc::new(items))
            .now(self.now)
            .today(s.today)
            .selected_item(s.chosen)
            .named("cal")
            .position(self.shared.position())
            .start_hour(7)
            .on_pick(on_pick)
            .build(size.0, size.1);
        // Right click, the menu key and Shift+F10 open the menu of the calendar; the target is the chosen appointment, if any.
        context_trigger(div().w(size.0).h(size.1).child(view), move |trigger| {
            let Some(app) = shell.get() else { return };
            let target = Target {
                kind: KIND.to_owned(),
                ids: chosen.clone(),
            };
            let (names, words) = (keys.as_ref().clone(), Arc::clone(&menu_words));
            let say = move |key: &TextKey| say(&words, key);
            let texts = ModelTexts {
                say: &say,
                platform: Platform::current(),
                names: &names,
            };
            app.open_context_menu(&trigger, target, &menu, &texts);
        })
    }
}

impl Surface for Main {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        let marker = self.notice();
        let items = self.items();
        let inner = (width - 2.0 * PAD).max(0.0);
        let used = 2.0 * PAD + BAR_HEIGHT + marker.map_or(0.0, |_| NOTICE_HEIGHT);
        let body = (inner, (height - used).max(160.0));
        let mut page = div()
            .flex_col()
            .gap_px(12.0)
            .p(PAD / 4.0)
            .w(width)
            .h(height);
        page = page
            .child(bar(&self.shared, &self.labels, &self.title()))
            .child(legend(&self.labels));
        if let Some(kind) = marker {
            page = page.child(self.marker(kind));
        }
        Box::new(if items.is_empty() {
            page.child(self.empty(body))
        } else {
            page.child(self.calendar(items, body))
        })
    }

    fn styles(&self) -> String {
        format!(
            "{}{}{}",
            styles(),
            fs_components_fields::controls::STYLES,
            crate::preview::DOT_STYLES
        )
    }
}

/// The language of the texts as a locale (the modifier keys of the menu are named in it).
fn locale_of(texts: &Mf2Texts) -> Locale {
    if german(texts) {
        Locale::parse("de").unwrap_or_else(|_| Locale::english())
    } else {
        Locale::english()
    }
}
