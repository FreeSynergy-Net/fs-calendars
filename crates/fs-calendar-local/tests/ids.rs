//! K2: object ids are UUIDv7 with randomness, unique across processes, strictly increasing within one.

use std::collections::HashSet;
use std::process::Command;

use fs_calendar_local::clock::new_id;
use fs_calendar_local::codec::id_text;

const MS: i64 = 1_794_823_200_000;
const N: usize = 20_000;

fn ids(n: usize) -> Vec<String> {
    (0..n).map(|_| id_text(new_id(MS))).collect()
}

#[test]
fn child_ids() {
    if std::env::var("FS_IDS_CHILD").is_ok() {
        println!("IDS:{}", ids(N).join(","));
    }
}

#[test]
fn ids_are_uuid_v7_shaped_and_increase_within_a_process() {
    let v = ids(5000);
    for w in v.windows(2) {
        assert!(w[0] < w[1], "{} < {}", w[0], w[1]);
    }
    assert!(
        v.iter()
            .all(|i| i.as_bytes()[14] == b'7'
                && matches!(i.as_bytes()[19], b'8' | b'9' | b'a' | b'b'))
    );
    let late = id_text(new_id(MS + 5000));
    assert!(late > *v.last().unwrap(), "a later time sorts later");
    let back = id_text(new_id(MS - 5000));
    assert!(
        back > late,
        "a clock that jumps back does not make ids go back"
    );
}

#[test]
fn two_threads_and_a_second_process_never_make_the_same_id() {
    let threads: Vec<_> = (0..2).map(|_| std::thread::spawn(|| ids(N))).collect();
    let mut all: Vec<String> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_ids", "--nocapture", "--test-threads=1"])
        .env("FS_IDS_CHILD", "1")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let line = text
        .lines()
        .find_map(|l| l.split_once("IDS:").map(|x| x.1))
        .expect("the child printed its ids");
    all.extend(line.split(',').map(str::to_owned));
    assert_eq!(all.len(), 3 * N);
    let set: HashSet<&String> = all.iter().collect();
    assert_eq!(
        set.len(),
        all.len(),
        "no duplicate among {} ids of three counters",
        all.len()
    );
}
