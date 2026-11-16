//! A typed log value (`Value` of the trait) as the column group of `activity_change`, and back (ADR 0086 item 2).
//!
//! Pattern: Adapter (one column group per value kind; no JSON, no blob).

use fs_calendar_testdata::draft::{Attendee, ObjectId, PersonRef, Result, Value as Val};
use turso::Value;

use crate::codec::{
    Row, art_of, damaged, date_to_int, get_int, get_opt_text, get_text, id_text, int_to_date,
    int_to_local, local_to_int, person_of, reference_of, text, zone_of,
};
use fs_calendar_testdata::draft::{BasicValue, Utc};

/// Number of columns of one side (old or new).
pub const SIDE: usize = 10;

/// The kind word and the ten columns of a value (`None` = the side is absent: all null).
pub fn encode(v: Option<&Val>) -> Result<(Option<&'static str>, [Value; SIDE])> {
    let mut c: [Value; SIDE] = std::array::from_fn(|_| Value::Null);
    let Some(v) = v else { return Ok((None, c)) };
    let kind = match v {
        Val::Int(i) => {
            c[0] = Value::Integer(*i);
            "int"
        }
        Val::Bool(b) => {
            c[0] = Value::Integer(i64::from(*b));
            "bool"
        }
        Val::Utc(u) => {
            c[0] = Value::Integer(u.millis());
            "utc"
        }
        Val::Local(l) => {
            c[0] = Value::Integer(local_to_int(*l)?);
            "local"
        }
        Val::Date(d) => {
            c[0] = Value::Integer(date_to_int(*d)?);
            "date"
        }
        Val::Real(f) => {
            c[1] = Value::Real(*f);
            "real"
        }
        Val::Text(t) => {
            c[2] = text(t);
            "text"
        }
        Val::Zone(z) => {
            c[2] = text(z.id());
            "zone"
        }
        Val::Object(o) => {
            c[2] = text(&id_text(*o));
            "object"
        }
        Val::Reference(r) => {
            c[2] = text(&r.canonical());
            "iri"
        }
        Val::Person(p) => {
            let [a, b, d] = crate::codec::person_values(p);
            c[3] = a;
            c[4] = b;
            c[5] = d;
            "person"
        }
        Val::Attendee(Attendee::Actor(p)) => {
            let [a, b, d] = crate::codec::person_values(p);
            c[6] = text("actor");
            c[7] = a;
            c[8] = b;
            c[9] = d;
            "attendee"
        }
        Val::Attendee(Attendee::Resource(r)) => {
            c[6] = text("resource");
            c[7] = text(&r.canonical());
            "attendee"
        }
        Val::Attendee(Attendee::Location(r)) => {
            c[6] = text("location");
            c[7] = text(&r.canonical());
            "attendee"
        }
    };
    Ok((Some(kind), c))
}

/// The value of a kind word and ten columns starting at `at` of `row`; all null = absent.
pub fn decode(kind: &str, row: &[Value], at: usize) -> Result<Option<Val>> {
    let r: Row = row[at..at + SIDE].to_vec();
    if r.iter().all(|v| matches!(v, Value::Null)) {
        return Ok(None);
    }
    Ok(Some(match kind {
        "int" => Val::Int(get_int(&r, 0)?),
        "bool" => Val::Bool(get_int(&r, 0)? != 0),
        "utc" => Val::Utc(Utc::from_millis(get_int(&r, 0)?)),
        "local" => Val::Local(int_to_local(get_int(&r, 0)?)?),
        "date" => Val::Date(int_to_date(get_int(&r, 0)?)?),
        "real" => match r.get(1) {
            Some(Value::Real(f)) => Val::Real(*f),
            _ => return Err(damaged("real")),
        },
        "text" => Val::Text(get_text(&r, 2)?),
        "zone" => Val::Zone(zone_of(&get_text(&r, 2)?)?),
        "object" => {
            Val::Object(crate::codec::parse_id(&get_text(&r, 2)?).ok_or_else(|| damaged("object"))?)
        }
        "iri" => Val::Reference(reference_of(&get_text(&r, 2)?)?),
        "person" => Val::Person(person_of(&r, 3)?),
        "attendee" => Val::Attendee(attendee(&r)?),
        _ => return Err(damaged("value kind")),
    }))
}

fn attendee(r: &Row) -> Result<Attendee> {
    let who = reference_of(&get_text(r, 7)?)?;
    match get_text(r, 6)?.as_str() {
        "actor" => {
            let abzug: Option<ObjectId> = match get_opt_text(r, 9)? {
                Some(t) => Some(crate::codec::parse_id(&t).ok_or_else(|| damaged("abzug"))?),
                None => None,
            };
            Ok(Attendee::Actor(PersonRef {
                id: who,
                art: art_of(&get_text(r, 8)?)?,
                abzug,
            }))
        }
        "resource" => Ok(Attendee::Resource(who)),
        "location" => Ok(Attendee::Location(who)),
        _ => Err(damaged("attendee kind")),
    }
}
