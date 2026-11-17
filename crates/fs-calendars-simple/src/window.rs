//! The window: name, the real icon (embedded PNG), the calendars on the left, the start page and "what comes".
//!
//! Pattern: Adapter: our blocks in, a screen of the fs-components shell out; no widget is painted by hand.

use std::sync::{Arc, Mutex};

use fs_components::blinc::layout::div::div;
use fs_components::blinc::layout::image::image;
use fs_components::blinc::layout::widgets::scroll::{Scroll, ScrollPhysics};
use fs_components_app::{AppError, AppWidget, Element, Menu, MenuEntry, Screen, Status, Surface};
use fs_components_mf2::Mf2Texts;
use fs_components_widgets::{Block, Day, Inline, Moment, TextView, ViewRange, styles};

use crate::content::{coming, start};
use crate::main_view::Main;
use crate::preview::{CALENDARS, Mode};
use crate::state::Step;

/// The icon of the program, embedded in the binary.
const ICON: &[u8] = include_bytes!("../assets/fs-calendars.png");

fn icon_uri() -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(ICON)
    )
}

/// Side of the icon in the window, in pixels.
const ICON_SIZE: f32 = 64.0;

/// The start and the "what comes" screen.
struct Home {
    name: String,
    tagline: String,
    body: Vec<Block>,
    /// The start page carries the header with the icon; the page "what comes" is the list alone.
    header: bool,
}

impl Surface for Home {
    fn build(&self, width: f32, height: f32) -> Box<dyn Element> {
        let icon = image(icon_uri()).size(ICON_SIZE, ICON_SIZE).no_fade();
        let title = div().w(340.0).ml(-13.0).child(
            TextView::new(vec![Block::Heading {
                level: 1,
                text: Inline::plain(self.name.as_str()),
            }])
            .build(),
        );
        let header = div()
            .flex_row()
            .items_center()
            .gap(16.0)
            .child(icon)
            .child(title);
        let goal =
            TextView::new(vec![Block::Paragraph(Inline::plain(self.tagline.as_str()))]).build();
        let body = TextView::new(self.body.clone()).build();
        let page = div()
            .flex_col()
            .gap(4.0)
            .p(24.0)
            .w((width - 40.0).max(200.0));
        let column = if self.header {
            page.child(header).child(goal).child(body)
        } else {
            page.child(body)
        };
        let physics = Arc::new(Mutex::new(ScrollPhysics::default()));
        Box::new(
            div().w(width).h(height).child(
                Scroll::with_physics(physics)
                    .w(width)
                    .h(height)
                    .content(column),
            ),
        )
    }

    fn styles(&self) -> String {
        styles()
    }
}

/// The window for a server that offers `capabilities`. `clock` is today and now in the shown zone; `example` says whether the example
/// is shown at first; `range` is the view that opens.
///
/// # Errors
/// The shell could not be made (a name that is empty, for example).
pub fn app(
    texts: &Mf2Texts,
    capabilities: &[String],
    clock: (Day, Moment),
    example: bool,
    range: ViewRange,
) -> Result<AppWidget, AppError> {
    let main = Main::new(texts, capabilities, clock, example, range);
    let say = |id: &str| texts.plain(id);
    let page = |header: bool, body: Vec<Block>| {
        Arc::new(Home {
            name: say("app-name"),
            tagline: say("app-tagline"),
            body,
            header,
        })
    };
    let title = if main.mode() == Mode::Preview {
        format!("{} {}", say("app-name"), say("title-preview"))
    } else {
        say("app-name")
    };
    // The calendars on the left, a symbol each (the screen list takes text only); "all" first, then the start page and "what comes".
    let mut screens = vec![Screen::new(
        "all",
        format!("\u{25A6}  {}", say("screen-all")),
        Arc::new(main.sibling(None)),
    )];
    for c in CALENDARS {
        screens.push(Screen::new(
            c.id,
            format!("   {}", say(c.word)),
            Arc::new(main.sibling(Some(c.id))),
        ));
    }
    screens.push(Screen::new(
        "home",
        say("screen-start"),
        page(true, start(texts)),
    ));
    screens.push(Screen::new(
        "coming",
        say("screen-coming"),
        page(false, coming(texts)),
    ));
    let widget = AppWidget::new(title, screens)?;
    // The context menu opens on the shell; "Today" is the one command that works.
    let _ = main.shell().set(widget.clone());
    let shared = main.shared().clone();
    let soon = say("soon");
    let locked = |id: &str| {
        MenuEntry::new(id, format!("{} ({soon})", say(&format!("menu-{id}")))).disabled()
    };
    widget.set_menus(
        vec![Menu::new(
            say("menu-program"),
            vec![locked("new"), locked("search"), locked("import")],
        )],
        move |id| {
            if id == "today" {
                shared.lock().step(Step::Today);
                crate::bar::redraw();
            }
        },
    )?;
    widget.set_status(Some(Status::Info(say("status-ready"))));
    Ok(widget)
}
