# FreeSynergy Calendar — be somewhere at some time

**Your time belongs to you, and it should stay correct when the clocks change.**

FreeSynergy Calendar keeps your events, including the ones that repeat, with the right time zone, and shows them by month, week, day or as an agenda. You can invite people and answer invitations. Your tasks show up beside your events.

What is different here: events follow the open JSCalendar standard (RFC 8984), and this program owns it. Events import and export as iCalendar and as JSCalendar JSON, and the original file text is kept for every event. An event in New York stays at the same wall-clock time across daylight saving. If a file holds a time zone we cannot read, we refuse that event rather than silently store it at the wrong time. Tasks are not copied in; the calendar asks the Tasks program for them.

## What it can do today

Latest tag: `v0.0.5` brings the window back (see below).

- Create, edit, delete and list events; find what happens in a time range.
- Repeating events (daily to yearly, "second Tuesday", count or until), with single occurrences changed, removed or added.
- Time zones from the newest tz database available (2026e on the machine used for testing).
- Invitations: invite, accept, tentatively accept, decline; the event's sequence number moves when it should.
- Import and export iCalendar (events, repeat rules, exceptions, attendees, alarms) and JSCalendar JSON.
- An agenda that mixes events and tasks (tasks come through the Tasks interface).
- Tell other programs when an event changes.
- Events marked sensitive are left out of exports unless released by name.
- A small server for all of this over a local socket.
- A window (`fs-calendars`, the frontend `fs-calendars-simple`) with your calendars on the left, a large title with the shown range, Previous / Today / Next, and the month, the week, the day or the agenda. It shows **example data marked as a preview**: no server delivers appointments to it yet, and every menu entry says "coming soon". `fs-calendars-simple --picture FILE` draws it to a picture.
- A type description of `Event` that the server hands out (`Events.types`), so any generated frontend can use it.

## What is not there yet

- The window shows no real appointment yet (preview data only), cannot create or change anything, has no tick boxes for calendars, no mini calendar, no keys. Series dialog, editor, import and export in the window, search: planned (see DESIGN, "Window `fs-calendars-simple`").
- The calendar server is not yet connected to a task server, so tasks found in an imported file are reported as not imported.
- No CalDAV, no sending invitations by e-mail (iTIP), no free/busy calculation.
- Importing is slow for large files (about 45 seconds for 4,000 events).
- Some late corrections to the core have been tested but not yet reviewed by a second reader. The DESIGN file says which.
- The interface is a draft and has no consumer yet.

## Auf Deutsch

FreeSynergy Calendars hält deine Termine, auch die, die sich wiederholen, mit der richtigen Zeitzone und zeigt sie als Monat, Woche, Tag oder Terminübersicht. Heute gibt es: Kern und Server (Termine, Serien, Einladungen, Import und Export iCalendar und JSCalendar) und ein **Fenster** (`fs-calendars`, das Frontend `fs-calendars-simple`) mit deinen Kalendern links, großem Titel mit dem gezeigten Zeitraum, Zurück / Heute / Vor und den Ansichten. Das Fenster zeigt noch **Beispieldaten, als Vorschau markiert**: kein Server liefert ihm Termine, und jeder Menüpunkt sagt „kommt noch“. Noch nicht da: echte Termine im Fenster, Anlegen und Ändern, Serien-Dialog, Häkchen je Kalender, Minikalender, Tasten, CalDAV.

## Part of FreeSynergy

Calendar sits in the groupware suite next to people, wiki, maps and messenger, and it shows tasks from FreeSynergy Tasks. FreeSynergy is a family of programs that are free for everyone, forever, and that keep your data yours. The documentation, the manifesto and the overall plan live in [fs-documentation](https://github.com/FreeSynergy/fs-documentation); the reading version is at <https://freesynergy.github.io/fs-documentation/>. Start with the [manifesto](https://freesynergy.github.io/fs-documentation/manifesto/) if you want to know why we do this.


## For developers

- One program, one command (ADR 0063): `fs-calendars` opens the window, `fs-calendars serve` serves the contract over a Unix socket, `fs-calendars list|show|add|delete|export|import` is the command line.
- Workspace crates: `fs-calendars` (the binary and the starter), `fs-calendars-simple` (the window, the frontend `simple`, a process of its own), `fs-calendars-core`, `fs-calendars-local` (the first provider of the appointments contract draft, in process), `fs-calendars-server`, `fs-calendars-cli` (libraries behind the subcommands), `fs-calendars-tasks` and `fs-calendars-testdata`. The older admin window `fs-calendars-app` is outside the workspace and history, see DESIGN.md.
- The contract is `events.capnp` in [fs-interfaces](https://github.com/FreeSynergy/fs-interfaces) (draft). Calendar owns it. Shared JSCalendar parts: [fs-jscalendar](https://github.com/FreeSynergy/fs-jscalendar).
- Build and test: `cargo build --workspace` and `cargo test --workspace`. Try the core: `cargo run -p fs-calendars-core --example demo`.
- Design and the full state table: [DESIGN.md](DESIGN.md).
- Licence: MIT OR Apache-2.0.
