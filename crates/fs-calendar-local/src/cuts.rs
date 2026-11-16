//! Cuts: what a partial delete remembers so that `restore(id, scope)` can undo it (`calendar_cut`, `calendar_cut_rule`).
//!
//! Pattern: Transaction script (rows of one cut: the deadline, the revision after it, the rule ends it took away).

use fs_calendar_testdata::draft::{LocalDateTime, ObjectId, Result, RuleEnd};

use crate::calendar_tables::{CUT, CUT_RULE};
use crate::codec::{
    boolean, get_int, get_opt_int, id_value, int, int_to_local, local_to_int, opt_int,
};
use crate::store::Conn;

/// What a cut remembers.
pub struct Cut {
    pub until: i64,
    pub rev_after: i64,
    pub ends: Vec<RuleEnd>,
}

/// Remember a cut: the deadline, the revision after it and the rule ends it took away.
///
/// # Errors
/// `Invalid` or the engine's error.
pub async fn save_cut(
    c: &Conn<'_>,
    master: ObjectId,
    rid: LocalDateTime,
    following: bool,
    until: i64,
    rev_after: i64,
    ends: &[RuleEnd],
) -> Result<()> {
    let at = local_to_int(rid)?;
    c.exec(
        "DELETE FROM calendar_cut WHERE appointment = ? AND recurrence_id = ? AND following = ?",
        vec![id_value(master), int(at), boolean(following)],
    )
    .await?;
    c.insert(
        &CUT,
        vec![
            id_value(master),
            int(at),
            boolean(following),
            int(until),
            int(rev_after),
        ],
    )
    .await?;
    for (seq, e) in ends.iter().enumerate() {
        let (n, u) = match e {
            RuleEnd::Never => (None, None),
            RuleEnd::Count(n) => (Some(i64::from(*n)), None),
            RuleEnd::Until(l) => (None, Some(local_to_int(*l)?)),
        };
        c.insert(
            &CUT_RULE,
            vec![
                id_value(master),
                int(at),
                boolean(following),
                int(i64::try_from(seq).unwrap_or(0)),
                opt_int(n),
                opt_int(u),
            ],
        )
        .await?;
    }
    Ok(())
}

/// Take a cut back out of the table (`None` = nothing was deleted there).
///
/// # Errors
/// `Unavailable` for a row that does not read back.
pub async fn take_cut(
    c: &Conn<'_>,
    master: ObjectId,
    rid: LocalDateTime,
    following: bool,
) -> Result<Option<Cut>> {
    let at = local_to_int(rid)?;
    let key = vec![id_value(master), int(at), boolean(following)];
    let rows = c.query("SELECT until, rev_after FROM calendar_cut WHERE appointment = ? AND recurrence_id = ? AND following = ?", key.clone()).await?;
    let Some(r) = rows.first() else {
        return Ok(None);
    };
    let (until, rev_after) = (get_int(r, 0)?, get_int(r, 1)?);
    let ends = c.query("SELECT end_count, end_until FROM calendar_cut_rule WHERE appointment = ? AND recurrence_id = ? AND following = ? ORDER BY seq", key.clone()).await?;
    let ends = ends
        .iter()
        .map(|e| {
            Ok(match (get_opt_int(e, 0)?, get_opt_int(e, 1)?) {
                (Some(n), _) => {
                    RuleEnd::Count(u32::try_from(n).map_err(|_| crate::codec::damaged("count"))?)
                }
                (None, Some(u)) => RuleEnd::Until(int_to_local(u)?),
                _ => RuleEnd::Never,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    c.exec(
        "DELETE FROM calendar_cut WHERE appointment = ? AND recurrence_id = ? AND following = ?",
        key,
    )
    .await?;
    Ok(Some(Cut {
        until,
        rev_after,
        ends,
    }))
}
