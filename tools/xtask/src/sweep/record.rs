//! What a sweep leaves on disk, built so a long run cannot lose its work
//! without saying so.
//!
//! A sweep given `--out <file>` appends each checked program to a journal
//! (`<file>.partial`) as it finishes, and rewrites a one-line status file
//! (`<file>.status`) as it goes. The finished output is written beside the
//! final path, renamed into place and read back before the journal is
//! removed. A run that dies keeps its journal, and running the same command
//! again resumes from it rather than starting over; a journal made by a
//! different commit, target, selection or shard is refused, never mixed in.
//!
//! Every output records the commit, the `--target`, the shard and a
//! fingerprint of the whole selection, so `merge` can refuse shards from
//! different runs and say exactly which shard or program is missing.

use super::Row;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Which part of a selection one run checks: units `first..=last` of
/// `count`. Program `i` of the sorted selection belongs to unit `i % count`,
/// so every shard gets a mix of fast and slow areas rather than one folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Shard {
    pub first: usize,
    pub last: usize,
    pub count: usize,
}

impl Shard {
    pub const WHOLE: Shard = Shard {
        first: 1,
        last: 1,
        count: 1,
    };

    /// `K/N` or `A-B/N`, counted from 1.
    pub fn parse(text: &str) -> Result<Shard, String> {
        let bad = || format!("--shard takes K/N or A-B/N counted from 1, not `{text}`");
        let (units, count) = text.split_once('/').ok_or_else(bad)?;
        let count: usize = count.parse().map_err(|_| bad())?;
        let (first, last) = match units.split_once('-') {
            Some((first, last)) => (
                first.parse().map_err(|_| bad())?,
                last.parse().map_err(|_| bad())?,
            ),
            None => {
                let unit = units.parse().map_err(|_| bad())?;
                (unit, unit)
            }
        };
        if first == 0 || first > last || last > count {
            return Err(bad());
        }
        Ok(Shard { first, last, count })
    }

    pub fn holds(&self, index: usize) -> bool {
        let unit = index % self.count + 1;
        (self.first..=self.last).contains(&unit)
    }

    pub fn label(&self) -> String {
        if self.first == self.last {
            format!("{}/{}", self.first, self.count)
        } else {
            format!("{}-{}/{}", self.first, self.last, self.count)
        }
    }
}

/// A program's name as outputs record it: its folder.
pub(crate) fn short(program: &str) -> &str {
    program.trim_end_matches("/main.omg")
}

/// A fingerprint of the whole selection, before sharding: every shard of one
/// run carries the same one.
pub(crate) fn fingerprint<'a>(programs: impl IntoIterator<Item = &'a str>) -> String {
    let mut names: Vec<&str> = programs.into_iter().map(short).collect();
    names.sort_unstable();
    let mut hash = Sha256::new();
    for name in names {
        hash.update(name.as_bytes());
        hash.update(b"\n");
    }
    hash.finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// What one run is: the same header heads its journal, its status and its
/// output.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Header {
    pub commit: String,
    /// The `--target` passed to `omega --check`; empty for the host default.
    pub target: String,
    pub shard: Shard,
    /// Programs in the whole selection, before sharding.
    pub selection: usize,
    pub fingerprint: String,
    /// Programs this shard checks.
    pub total: usize,
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

fn sibling(out: &Path, suffix: &str) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub(crate) fn journal_path(out: &Path) -> PathBuf {
    sibling(out, ".partial")
}

pub(crate) fn status_path(out: &Path) -> PathBuf {
    sibling(out, ".status")
}

/// The labels a row may carry. Reading a row back maps its text onto these,
/// and a label this code does not know is refused rather than guessed.
const LABELS: &[&str] = &[
    "Checks",
    "Blocked",
    "Rejected as expected",
    "Rejected for another reason",
    "Accepted a wrong program",
    "Build program",
    "Package roots",
    "Name resolution",
    "Checker coverage",
    "Product route",
    "Parsing",
    "Typing",
    "Checker verdict",
    "Other",
];

fn label(text: &str) -> Option<&'static str> {
    LABELS.iter().copied().find(|known| *known == text)
}

pub(crate) fn row_json(row: &Row) -> serde_json::Value {
    serde_json::json!({
        "program": short(&row.program),
        "exit": row.exit,
        "ms": row.ms as u64,
        "outcome": row.outcome,
        "stage": row.stage,
        "cause": row.cause,
        "detail": row.detail,
        "expect": if row.reject { "reject" } else { "accept" },
    })
}

pub(crate) fn row_from_json(value: &serde_json::Value) -> Result<Row, String> {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("a row has no `{key}`"))
    };
    let program = format!("{}/main.omg", text("program")?);
    let known = |key: &str| {
        let found = text(key)?;
        label(found).ok_or_else(|| format!("{program}: unknown {key} `{found}`"))
    };
    Ok(Row {
        exit: value
            .get("exit")
            .and_then(serde_json::Value::as_i64)
            .ok_or("a row has no `exit`")? as i32,
        ms: value
            .get("ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u128,
        outcome: known("outcome")?,
        stage: known("stage")?,
        cause: text("cause")?.to_owned(),
        detail: text("detail")?.to_owned(),
        reject: text("expect")? == "reject",
        program,
    })
}

/// The journal of one run with `--out`: rows already checked, and the file
/// each new row is appended to.
pub(crate) struct Journal {
    path: PathBuf,
    file: File,
}

impl Journal {
    /// Opens the journal for `header`, returning the rows an earlier attempt
    /// of the same run already checked. Refuses a journal from another run.
    pub fn open(out: &Path, header: &Header) -> Result<(Journal, Vec<Row>), String> {
        let path = journal_path(out);
        if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        let mut done = Vec::new();
        if path.exists() {
            let reader = BufReader::new(
                File::open(&path).map_err(|error| format!("{}: {error}", path.display()))?,
            );
            let mut lines = reader.lines();
            let first = lines
                .next()
                .transpose()
                .map_err(|error| format!("{}: {error}", path.display()))?
                .unwrap_or_default();
            let earlier: Header = serde_json::from_str(&first).map_err(|_| {
                format!(
                    "{} does not start with a sweep header; remove it or choose another --out",
                    path.display()
                )
            })?;
            // A journal of another run that checked nothing (a refused
            // `--target`, say) holds no work to protect.
            let mut lines = lines.peekable();
            let holds_rows = lines
                .peek()
                .is_some_and(|line| line.as_ref().is_ok_and(|line| !line.trim().is_empty()));
            let same_run = earlier == *header;
            if !same_run && holds_rows {
                return Err(format!(
                    "{} belongs to another run (commit {}, target `{}`, shard {}, selection {}); \
                     remove it or choose another --out",
                    path.display(),
                    earlier.commit,
                    earlier.target,
                    earlier.shard.label(),
                    earlier.fingerprint,
                ));
            }
            for line in lines.take_while(|_| same_run) {
                let Ok(line) = line else { break };
                // A run killed mid-write leaves a torn last line; that program
                // is simply checked again.
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    break;
                };
                done.push(
                    row_from_json(&value)
                        .map_err(|error| format!("{}: {error}", path.display()))?,
                );
            }
            // Rewrite the journal without any torn tail before appending.
            let mut text = serde_json::to_string(header).map_err(|error| error.to_string())?;
            text.push('\n');
            for row in &done {
                text.push_str(&row_json(row).to_string());
                text.push('\n');
            }
            fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
        } else {
            let mut text = serde_json::to_string(header).map_err(|error| error.to_string())?;
            text.push('\n');
            fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
        }
        let file = OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Ok((Journal { path, file }, done))
    }

    pub fn append(&mut self, row: &Row) -> Result<(), String> {
        let mut line = row_json(row).to_string();
        line.push('\n');
        self.file
            .write_all(line.as_bytes())
            .and_then(|()| self.file.flush())
            .map_err(|error| format!("{}: {error}", self.path.display()))
    }

    pub fn remove(self) -> Result<(), String> {
        drop(self.file);
        fs::remove_file(&self.path).map_err(|error| format!("{}: {error}", self.path.display()))
    }
}

/// The one-line progress record beside an output, rewritten as a run goes.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Status {
    /// `running`, `finished` or `failed` from the sweep itself; a fleet job
    /// script also writes `fetching`, `building` and `refused` before it.
    pub state: String,
    #[serde(default)]
    pub done: usize,
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub elapsed: u64,
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub detail: String,
}

pub(crate) fn write_status(out: &Path, status: &Status) -> Result<(), String> {
    let path = status_path(out);
    let temporary = sibling(out, ".status.tmp");
    let mut text = serde_json::to_string(status).map_err(|error| error.to_string())?;
    text.push('\n');
    fs::write(&temporary, text)
        .and_then(|()| fs::rename(&temporary, &path))
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// Writes the finished output beside `out`, renames it into place, and reads
/// it back: the run only counts as finished once its rows are on disk.
pub(crate) fn write_output(
    out: &Path,
    header: &Header,
    rows: &[Row],
    render: impl Fn(&Path, &Header, &[Row]) -> Result<(), String>,
) -> Result<(), String> {
    // `out.tmp.json` keeps the extension that chooses the format.
    let temporary = match out.extension() {
        Some(extension) => out.with_extension(format!("tmp.{}", extension.to_string_lossy())),
        None => sibling(out, ".tmp"),
    };
    render(&temporary, header, rows)?;
    fs::rename(&temporary, out).map_err(|error| format!("{}: {error}", out.display()))?;
    let read = read_rows(out)?;
    if read.len() != rows.len() {
        return Err(format!(
            "{} holds {} rows after writing {}",
            out.display(),
            read.len(),
            rows.len()
        ));
    }
    Ok(())
}

/// The programs in a written output, in either format.
fn read_rows(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        let (_, rows) =
            parse_output(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(rows.into_iter().map(|row| row.program).collect())
    } else {
        Ok(text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.starts_with("program\t"))
            .filter_map(|line| line.split('\t').next().map(str::to_owned))
            .collect())
    }
}

/// A JSON output's header and rows.
pub(crate) fn parse_output(text: &str) -> Result<(Header, Vec<Row>), String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("not sweep JSON: {error}"))?;
    let header: Header = serde_json::from_value(
        value
            .get("run")
            .cloned()
            .ok_or("no `run` header; it was written before sweeps recorded one")?,
    )
    .map_err(|error| format!("bad `run` header: {error}"))?;
    let rows = value
        .get("programs")
        .and_then(serde_json::Value::as_array)
        .ok_or("no `programs`")?
        .iter()
        .map(row_from_json)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((header, rows))
}

/// Joins the shards of one run into the whole, refusing anything that does
/// not add up to exactly one row per selected program.
pub(crate) fn merge(parts: &[(PathBuf, Header, Vec<Row>)]) -> Result<(Header, Vec<Row>), String> {
    let Some((first_path, first, _)) = parts.first() else {
        return Err("merge needs at least one sweep output".to_owned());
    };
    for (path, header, _) in parts {
        let same = header.commit == first.commit
            && header.target == first.target
            && header.fingerprint == first.fingerprint
            && header.selection == first.selection
            && header.shard.count == first.shard.count;
        if !same {
            return Err(format!(
                "{} is from another run than {}: commit {} vs {}, target `{}` vs `{}`, \
                 selection {} vs {}, {} vs {} units",
                path.display(),
                first_path.display(),
                header.commit,
                first.commit,
                header.target,
                first.target,
                header.fingerprint,
                first.fingerprint,
                header.shard.count,
                first.shard.count,
            ));
        }
    }
    let count = first.shard.count;
    let mut owner: Vec<Option<&Path>> = vec![None; count + 1];
    for (path, header, _) in parts {
        let units = header.shard.first..=header.shard.last;
        for (unit, slot) in owner
            .iter_mut()
            .enumerate()
            .skip(*units.start())
            .take(units.count())
        {
            if let Some(other) = slot {
                return Err(format!(
                    "unit {unit}/{count} is in both {} and {}",
                    other.display(),
                    path.display()
                ));
            }
            *slot = Some(path);
        }
    }
    let missing: Vec<String> = (1..=count)
        .filter(|unit| owner[*unit].is_none())
        .map(|unit| unit.to_string())
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "no output covers unit(s) {} of {count}",
            missing.join(", ")
        ));
    }
    let mut rows = Vec::with_capacity(first.selection);
    let mut seen = BTreeSet::new();
    for (path, header, part) in parts {
        if part.len() != header.total {
            return Err(format!(
                "{} holds {} rows but its shard has {} programs",
                path.display(),
                part.len(),
                header.total
            ));
        }
        for row in part {
            if !seen.insert(row.program.clone()) {
                return Err(format!(
                    "{} is in more than one output",
                    short(&row.program)
                ));
            }
            rows.push(row.clone());
        }
    }
    let fingerprint = fingerprint(rows.iter().map(|row| row.program.as_str()));
    if rows.len() != first.selection || fingerprint != first.fingerprint {
        return Err(format!(
            "the shards hold {} programs (selection {fingerprint}) but the run selected {} \
             (selection {})",
            rows.len(),
            first.selection,
            first.fingerprint
        ));
    }
    rows.sort_by(|a, b| a.program.cmp(&b.program));
    let header = Header {
        shard: Shard::WHOLE,
        total: rows.len(),
        ..first.clone()
    };
    Ok((header, rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sweep::row;

    fn header(shard: Shard, programs: &[&str], total: usize) -> Header {
        Header {
            commit: "abc".to_owned(),
            target: "linux_x86_64".to_owned(),
            shard,
            selection: programs.len(),
            fingerprint: fingerprint(programs.iter().copied()),
            total,
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sweep-record-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn shards_split_by_unit_and_parse_both_forms() {
        assert_eq!(
            Shard::parse("2/3").unwrap(),
            Shard {
                first: 2,
                last: 2,
                count: 3
            }
        );
        let weighted = Shard::parse("2-3/5").unwrap();
        assert_eq!(weighted.label(), "2-3/5");
        let held: Vec<usize> = (0..10).filter(|index| weighted.holds(*index)).collect();
        assert_eq!(held, [1, 2, 6, 7]);
        for bad in ["0/3", "4/3", "3-2/4", "1", "a/b", "1-/2"] {
            assert!(Shard::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_journal_resumes_its_own_run_and_refuses_another() {
        let dir = scratch("journal");
        let out = dir.join("out.json");
        let programs = ["samples/a/main.omg", "samples/b/main.omg"];
        let run = header(Shard::WHOLE, &programs, 2);
        let (mut journal, done) = Journal::open(&out, &run).unwrap();
        assert!(done.is_empty());
        journal.append(&row(programs[0], 0, 3, "")).unwrap();
        drop(journal);
        // A run killed mid-write leaves a torn line.
        let mut file = OpenOptions::new()
            .append(true)
            .open(journal_path(&out))
            .unwrap();
        file.write_all(b"{\"program\":\"samp").unwrap();
        drop(file);

        let (journal, done) = Journal::open(&out, &run).unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].program, programs[0]);
        assert_eq!(done[0].outcome, "Checks");
        drop(journal);

        let other = Header {
            commit: "def".to_owned(),
            ..run.clone()
        };
        let refused = Journal::open(&out, &other).err().unwrap();
        assert!(refused.contains("another run"), "{refused}");

        // A journal of another run that checked nothing is simply replaced.
        let empty = dir.join("empty.json");
        drop(Journal::open(&empty, &other).unwrap());
        let (_, done) = Journal::open(&empty, &run).unwrap();
        assert!(done.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_takes_every_unit_once_and_names_what_is_missing() {
        let programs = [
            "samples/a/main.omg",
            "samples/b/main.omg",
            "samples/c/main.omg",
        ];
        let part = |shard: &str| {
            let shard = Shard::parse(shard).unwrap();
            let rows: Vec<Row> = programs
                .iter()
                .enumerate()
                .filter(|(index, _)| shard.holds(*index))
                .map(|(_, program)| row(program, 0, 1, ""))
                .collect();
            (
                PathBuf::from(shard.label()),
                header(shard, &programs, rows.len()),
                rows,
            )
        };
        let (whole, rows) = merge(&[part("1/2"), part("2/2")]).unwrap();
        assert_eq!(whole.shard, Shard::WHOLE);
        assert_eq!(rows.len(), 3);

        let missing = merge(&[part("1/2")]).err().unwrap();
        assert!(missing.contains("unit(s) 2 of 2"), "{missing}");
        let twice = merge(&[part("1-2/2"), part("2/2")]).err().unwrap();
        assert!(twice.contains("unit 2/2 is in both"), "{twice}");

        let mut short_part = part("2/2");
        short_part.2.clear();
        let short = merge(&[part("1/2"), short_part]).err().unwrap();
        assert!(short.contains("holds 0 rows"), "{short}");

        let mut other = part("2/2");
        other.1.commit = "zzz".to_owned();
        let mixed = merge(&[part("1/2"), other]).err().unwrap();
        assert!(mixed.contains("another run"), "{mixed}");
    }

    #[test]
    fn an_output_is_read_back_before_it_counts() {
        let dir = scratch("output");
        let out = dir.join("out.json");
        let programs = ["samples/a/main.omg"];
        let run = header(Shard::WHOLE, &programs, 1);
        let rows = [row(programs[0], 0, 1, "")];
        let lying = |path: &Path, _: &Header, _: &[Row]| {
            fs::write(path, "{\"run\":null}").map_err(|error| error.to_string())
        };
        assert!(write_output(&out, &run, &rows, lying).is_err());
        write_output(&out, &run, &rows, crate::sweep::render_rows).unwrap();
        let (read, read_rows) = parse_output(&fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(read, run);
        assert_eq!(read_rows.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
