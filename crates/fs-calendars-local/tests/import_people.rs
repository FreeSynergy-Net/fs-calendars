//! The people of an import: ONE batch of proposals for all unknown addresses, its id in the report; an AI that
//! imports a file with addresses gets `Forbidden` and nothing is written (no appointment, no batch, no proposal).

#![allow(clippy::many_single_char_names)]

mod common;

use std::sync::Arc;

use common::exchange::{Ex, ICS_TYPE, ctx, ex, import_text, media};
use fs_calendars_local::exchange::LocalExchange;
use fs_calendars_testdata::draft::{Appointments, Error, Exchange, FileRef, Trash};
use fs_calendars_testdata::sample::sample;

const TWO: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//self-written//people//EN\r\nBEGIN:VEVENT\r\nUID:two@example.org\r\nDTSTAMP:20261116T080000Z\r\nDTSTART;TZID=Europe/Berlin:20261201T090000\r\nDTEND;TZID=Europe/Berlin:20261201T100000\r\nSUMMARY:Zwei Fremde\r\nATTENDEE;CN=Neu Eins;PARTSTAT=ACCEPTED:mailto:neu.eins@example.org\r\nATTENDEE;CN=Neu Zwei;PARTSTAT=ACCEPTED:mailto:neu.zwei@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

#[tokio::test]
async fn two_unknown_addresses_open_one_batch_and_the_report_names_it() {
    let e = ex().await;
    let r = import_text(&e, TWO, ICS_TYPE).await;
    assert_eq!(r.created.len(), 1, "{r:?}");
    assert_eq!(e.people.batches().len(), 1, "exactly one batch");
    assert_eq!(e.people.proposed().len(), 2, "one proposal per address");
    assert_eq!(
        r.people_proposal,
        Some(e.people.batches()[0].0),
        "the report names the batch (K7)"
    );
}

async fn ai_import(e: &Ex) -> (Result<(), Error>, usize) {
    let ai = Arc::new(sample().directory().with_ai());
    let ex_ai = LocalExchange::new(e.local.clone(), ai.clone(), e.files.clone());
    let f: FileRef = e.files.put(TWO.as_bytes(), &media(ICS_TYPE)).unwrap();
    let r = ex_ai.import(f, &media(ICS_TYPE), &ctx(e)).await.map(|_| ());
    (r, ai.batches().len() + ai.proposed().len())
}

#[tokio::test]
async fn an_ai_importing_a_file_with_addresses_is_forbidden_and_nothing_is_written() {
    let e = ex().await;
    let (r, people) = ai_import(&e).await;
    assert!(
        matches!(&r, Err(Error::Forbidden { capability }) if capability == "person.find"),
        "{r:?}"
    );
    assert_eq!(people, 0, "no batch and no proposal");
    let q = fs_calendars_testdata::draft::AppointmentQuery {
        calendars: vec![e.fx.cal],
        trash: Trash::Include,
        ..Default::default()
    };
    let list = e.local.list(&q, None, 1000).await.unwrap();
    assert!(list.items.is_empty(), "no appointment: {:?}", list.items);
}
