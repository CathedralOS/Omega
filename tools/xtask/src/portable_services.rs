//! `check-portable-services`: compare how each target family of `system`
//! implements every portable call, and report differences nobody has reviewed.
//!
//! [Three layers](wiki/spec/build/platform_services.md#three-layers) says a
//! portable service "is identical on every target". `system` keeps one POSIX
//! adapter family (Linux and macOS) and one Windows family, so a refusal or a
//! limit that only one family applies makes the same call mean different
//! things on different targets. This check finds such differences from the
//! source text alone, in well under a second and without building anything.
//!
//! For each `Build::select_provider::<Service, Provider>(builder, target)` row in
//! `system/build.omg` that covers all five hosted targets, it reads each target's
//! provider block and follows each member through local machines and the raw
//! providers selected for that target. It collects four kinds of fact:
//! - `refuse K`: an `ErrorKind::K }` the adapter returns itself;
//! - `limit OP N`: an integer literal or `x.length` compared in a `transition`,
//!   `requires` or `ensures` clause (`< 255`, `<= buffer.length`);
//! - `const f(..N..)`: an integer literal of three or more digits passed as a
//!   whole call argument (modes, budgets, flag words);
//! - `clamp f`: a call to a `min_u64`, `saturating` or `wrapping` helper.
//!
//! Errno and Win32 translation tables are not followed: their names match
//! `error`, `errno`, `kind_of`, `_kind` or `reconstruct`. Each fact that some
//! targets have and others lack becomes one row. A row is a lead, not a
//! verdict, so every row must be reviewed once and recorded in the baseline
//! (`tools/ratchets/portable_services.txt`) with a reason. The check fails on
//! a row the baseline lacks and on a baseline row that no longer appears, so
//! the baseline stays exact.
//!
//! The extraction is textual. It reads column-0 declarations, strips `//`
//! comments, and does not parse Omega; a row it misses is a gap in coverage,
//! not a pass. Keep it a fast lexical scan: the behaviour probes it stands in
//! for cannot compile yet.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::Instant;

mod family;
use family::Family;

pub const HOSTED_TARGETS: [&str; 5] = [
    "linux_x86_64",
    "linux_arm64",
    "macos_arm64",
    "windows_x86_64",
    "windows_arm64",
];
const SYSTEM_ROOT: &str = "source/library/system";
const BASELINE: &str = "tools/ratchets/portable_services.txt";
const CONCERNS: &str = "OWNER_CODE_CONCERNS.md";
pub fn run(args: &[String]) -> Result<(), u8> {
    let mut observe = false;
    let mut list = false;
    for arg in args {
        match arg.as_str() {
            "--observe" => observe = true,
            "--list" => list = true,
            "-h" | "--help" => {
                println!("{}", usage());
                return Ok(());
            }
            other => {
                eprintln!(
                    "check-portable-services: unknown argument `{other}`\n\n{}",
                    usage()
                );
                return Err(2);
            }
        }
    }
    let root = crate::repo_root()?;
    let started = Instant::now();
    let rows = match collect_rows(&root.join(SYSTEM_ROOT)) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("check-portable-services: {error}");
            return Err(1);
        }
    };
    if list {
        for row in &rows {
            println!("{row}");
        }
        return Ok(());
    }
    let baseline_text = fs::read_to_string(root.join(BASELINE)).map_err(|error| {
        eprintln!("check-portable-services: cannot read {BASELINE}: {error}");
        1u8
    })?;
    let concerns = fs::read_to_string(root.join(CONCERNS)).map_err(|error| {
        eprintln!("check-portable-services: cannot read {CONCERNS}: {error}");
        1u8
    })?;
    let baseline = match parse_baseline(&baseline_text, &concern_headings(&concerns)) {
        Ok(baseline) => baseline,
        Err(errors) => {
            for error in errors {
                eprintln!("{BASELINE}: {error}");
            }
            return Err(1);
        }
    };
    let comparison = compare(&rows, &baseline);
    let elapsed = started.elapsed();
    print!("{}", comparison.render(rows.len(), &baseline));
    println!("check-portable-services: {:.2} s", elapsed.as_secs_f64());
    if comparison.is_clean() {
        return Ok(());
    }
    if observe {
        println!(
            "check-portable-services: observe mode; the rows above would block once this check enforces"
        );
        return Ok(());
    }
    Err(1)
}

fn usage() -> &'static str {
    "\
usage: cargo xtask check-portable-services [--observe] [--list]

Compare the refusals, limits and constants each target family of system applies
to every portable call, and fail on a difference the reviewed baseline
(tools/ratchets/portable_services.txt) does not record, or on a baseline row
that no longer appears.

  --observe   report what would fail, but exit 0
  --list      print every current row, without comparing"
}

/// One fact that some hosted targets apply to a portable call and others do not.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Row {
    pub service: String,
    pub member: String,
    pub fact: String,
    /// The hosted targets that have the fact, in `HOSTED_TARGETS` order.
    pub targets: Vec<&'static str>,
}

impl Row {
    fn key(&self) -> String {
        format!(
            "{}.{} | {} | {}",
            self.service,
            self.member,
            self.fact,
            self.targets.join(",")
        )
    }
}

impl std::fmt::Display for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.key())
    }
}

// ---------------------------------------------------------------------------
// Baseline

/// Why a reviewed row may stay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    /// Layout offsets, OS flag words, access masks and helper bounds: the
    /// row names how a family talks to its OS, not what the call means.
    Noise,
    /// A difference the portable contract permits, such as `Unsupported`.
    Allowed,
    /// A real divergence queued in `OWNER_CODE_CONCERNS.md` under a heading.
    Known,
}

impl Reason {
    fn word(self) -> &'static str {
        match self {
            Reason::Noise => "noise",
            Reason::Allowed => "allowed",
            Reason::Known => "known",
        }
    }
}

#[derive(Debug)]
pub struct Baseline {
    /// Row key -> (reason, line number).
    pub rows: BTreeMap<String, (Reason, usize)>,
}

/// Reads `key | reason | note` lines. `known` rows name an
/// `OWNER_CODE_CONCERNS.md` heading as their note, and the heading must exist,
/// so removing a fixed concern also forces its rows out of the baseline.
pub fn parse_baseline(text: &str, headings: &BTreeSet<&str>) -> Result<Baseline, Vec<String>> {
    let mut rows = BTreeMap::new();
    let mut errors = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(" | ").map(str::trim).collect();
        if fields.len() != 5 {
            errors.push(format!(
                "line {number}: expected `Service.member | fact | targets | reason | note`"
            ));
            continue;
        }
        let reason = match fields[3] {
            "noise" => Reason::Noise,
            "allowed" => Reason::Allowed,
            "known" => Reason::Known,
            other => {
                errors.push(format!(
                    "line {number}: reason `{other}` is not noise, allowed or known"
                ));
                continue;
            }
        };
        let note = fields[4];
        if note.is_empty() {
            errors.push(format!(
                "line {number}: the row needs a one-line note saying why"
            ));
        }
        if reason == Reason::Known && !headings.contains(note) {
            errors.push(format!(
                "line {number}: `known` must name an {CONCERNS} heading; `{note}` is not one"
            ));
        }
        let Some((service, names)) = fields[0].split_once('.') else {
            errors.push(format!("line {number}: expected a service and member"));
            continue;
        };
        let members = names
            .strip_prefix('{')
            .and_then(|members| members.strip_suffix('}'));
        for member in members.unwrap_or(names).split(',') {
            if ident(service) != Some(service)
                || ident(member) != Some(member)
                || (names.contains(',') && members.is_none())
            {
                errors.push(format!(
                    "line {number}: invalid service member `{}`",
                    fields[0]
                ));
                break;
            }
            let key = format!("{service}.{member} | {} | {}", fields[1], fields[2]);
            if rows.insert(key.clone(), (reason, number)).is_some() {
                errors.push(format!("line {number}: duplicate row `{key}`"));
            }
        }
    }
    if errors.is_empty() {
        Ok(Baseline { rows })
    } else {
        Err(errors)
    }
}

pub fn concern_headings(text: &str) -> BTreeSet<&str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("### "))
        .map(str::trim)
        .collect()
}

#[derive(Debug, Default)]
pub struct Comparison {
    pub new_rows: Vec<String>,
    pub stale_rows: Vec<(String, usize)>,
}

impl Comparison {
    pub fn is_clean(&self) -> bool {
        self.new_rows.is_empty() && self.stale_rows.is_empty()
    }

    fn render(&self, total: usize, baseline: &Baseline) -> String {
        let mut out = String::new();
        let mut by_reason: BTreeMap<Reason, usize> = BTreeMap::new();
        for (reason, _) in baseline.rows.values() {
            *by_reason.entry(*reason).or_default() += 1;
        }
        let counts: Vec<String> = by_reason
            .iter()
            .map(|(reason, count)| format!("{count} {}", reason.word()))
            .collect();
        let _ = writeln!(
            out,
            "check-portable-services: {total} rows; baseline {} ({})",
            baseline.rows.len(),
            counts.join(", ")
        );
        if !self.new_rows.is_empty() {
            let _ = writeln!(
                out,
                "\n{} rows are not in {BASELINE}. Read each in both adapter families, then fix the \
                 divergence or add the row with a reason (noise, allowed, or known + an {CONCERNS} heading):",
                self.new_rows.len()
            );
            for key in &self.new_rows {
                let _ = writeln!(out, "  {key} | <reason> | <why>");
            }
        }
        if !self.stale_rows.is_empty() {
            let _ = writeln!(
                out,
                "\n{} baseline rows no longer appear; delete them from {BASELINE}:",
                self.stale_rows.len()
            );
            for (key, line) in &self.stale_rows {
                let _ = writeln!(out, "  line {line}: {key}");
            }
        }
        out
    }
}

pub fn compare(rows: &[Row], baseline: &Baseline) -> Comparison {
    let current: BTreeSet<String> = rows.iter().map(Row::key).collect();
    Comparison {
        new_rows: current
            .iter()
            .filter(|key| !baseline.rows.contains_key(*key))
            .cloned()
            .collect(),
        stale_rows: baseline
            .rows
            .iter()
            .filter(|(key, _)| !current.contains(*key))
            .map(|(key, (_, line))| (key.clone(), *line))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Extraction

/// Reads `system` and returns every row, sorted.
pub fn collect_rows(system_root: &Path) -> Result<Vec<Row>, String> {
    let build = read(&system_root.join("build.omg"))?;
    let selections = provider_selections(&strip_comments(&build))?;
    let mut families = Vec::with_capacity(HOSTED_TARGETS.len());
    for target in HOSTED_TARGETS {
        let mut family = Family::load(system_root, target)?;
        for (service, providers) in &selections {
            if let Some((file, provider)) = providers.get(target) {
                family
                    .select(service, Path::new(file), provider)
                    .map_err(|why| format!("{target}: {why}"))?;
            }
        }
        families.push(family);
    }
    let mut rows = Vec::new();
    for (service, providers) in &selections {
        if !HOSTED_TARGETS
            .iter()
            .all(|target| providers.contains_key(*target))
        {
            continue;
        }
        // member -> facts per hosted target.
        let mut table: BTreeMap<String, Vec<BTreeSet<String>>> = BTreeMap::new();
        for (index, family) in families.iter().enumerate() {
            for (member, &body) in &family.selected[service] {
                let facts = family.member_facts(body)?;
                table
                    .entry(member.to_string())
                    .or_insert_with(|| vec![BTreeSet::new(); HOSTED_TARGETS.len()])[index] = facts;
            }
        }
        for (member, per_target) in table {
            let union: BTreeSet<&String> = per_target.iter().flatten().collect();
            for fact in union {
                let targets: Vec<&'static str> = HOSTED_TARGETS
                    .iter()
                    .zip(&per_target)
                    .filter(|(_, facts)| facts.contains(fact))
                    .map(|(target, _)| *target)
                    .collect();
                if targets.len() != HOSTED_TARGETS.len() {
                    rows.push(Row {
                        service: service.clone(),
                        member: member.clone(),
                        fact: fact.clone(),
                        targets,
                    });
                }
            }
        }
    }
    rows.sort();
    Ok(rows)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// A local file import binds its explicit alias or the stem of the imported file.
fn file_aliases(source: &str) -> HashMap<&str, &str> {
    source
        .lines()
        .filter_map(|line| {
            let import = line.trim().strip_prefix("use ")?.strip_suffix(';')?;
            let (file, alias) = match import.split_once(" as ") {
                Some((file, alias)) => (file.trim(), alias.trim()),
                None => (import, Path::new(import).file_stem()?.to_str()?),
            };
            Some((alias, file))
        })
        .collect()
}

/// A service by its declared name, without the alias or path that reached it.
fn service_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

/// `Build::select_provider::<Service, Provider>(builder, target)` rows, as
/// service -> target -> provider.
type ProviderSelections = BTreeMap<String, BTreeMap<String, (String, String)>>;

fn provider_selections(build: &str) -> Result<ProviderSelections, String> {
    const CALL: &str = "Build::select_provider::<";
    let aliases = file_aliases(build);
    let mut out = ProviderSelections::new();
    let mut rest = build;
    while let Some(start) = rest.find(CALL) {
        rest = &rest[start + CALL.len()..];
        let Some(close) = rest.find('>') else { break };
        let generics: Vec<&str> = rest[..close].split(',').map(str::trim).collect();
        let after = rest[close + 1..].trim_start();
        let Some(args) = after.strip_prefix('(') else {
            continue;
        };
        let Some(end) = args.find(')') else { continue };
        let args: Vec<&str> = args[..end].split(',').map(str::trim).collect();
        if let ([service, provider], [_, target]) = (generics.as_slice(), args.as_slice()) {
            let (file, name) = if let Some((alias, name)) =
                provider.strip_prefix('|').and_then(|s| s.split_once("|::"))
            {
                let file = aliases
                    .get(alias)
                    .ok_or_else(|| format!("unknown provider file alias `{alias}`"))?;
                (*file, name)
            } else {
                ("build.omg", *provider)
            };
            out.entry(service_name(service).to_string())
                .or_default()
                .insert(target.to_string(), (file.to_owned(), name.to_owned()));
        }
    }
    Ok(out)
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        match line.find("//") {
            Some(at) => {
                out.push_str(&line[..at]);
                if line.ends_with('\n') {
                    out.push('\n');
                }
            }
            None => out.push_str(line),
        }
    }
    out
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The identifier at the start of `text`, if any.
fn ident(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes
        .first()
        .is_none_or(|b| !(b.is_ascii_alphabetic() || *b == b'_'))
    {
        return None;
    }
    let end = bytes
        .iter()
        .position(|b| !is_word(*b))
        .unwrap_or(bytes.len());
    Some(&text[..end])
}

/// Maximal identifier runs that start at a word boundary, with their offsets.
fn words(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let bytes = text.as_bytes();
    let mut at = 0;
    std::iter::from_fn(move || {
        while at < bytes.len() {
            if is_word(bytes[at]) && (at == 0 || !is_word(bytes[at - 1])) {
                let start = at;
                while at < bytes.len() && is_word(bytes[at]) {
                    at += 1;
                }
                if !bytes[start].is_ascii_digit() {
                    return Some((start, &text[start..at]));
                }
            } else {
                at += 1;
            }
        }
        None
    })
}

fn is_lower_ident(word: &str) -> bool {
    let bytes = word.as_bytes();
    (bytes[0].is_ascii_lowercase() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}

/// Whether `text` (what follows a name) opens a call: optional whitespace,
/// optional `<..>` without `>`, `(` or `)` inside, then `(`.
fn opens_call(text: &str) -> bool {
    call_arguments_start(text).is_some()
}

/// The offset just past the `(` that opens a call, per [`opens_call`].
fn call_arguments_start(text: &str) -> Option<usize> {
    let trimmed = text.trim_start();
    let mut at = text.len() - trimmed.len();
    if let Some(generic) = trimmed.strip_prefix('<') {
        let close = generic.find(['>', '(', ')'])?;
        if !generic[close..].starts_with('>') {
            return None;
        }
        at += 1 + close + 1;
        at += text[at..].len() - text[at..].trim_start().len();
    }
    text[at..].starts_with('(').then_some(at + 1)
}

/// The facts one machine body applies itself.
fn facts(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (at, _) in body.match_indices("ErrorKind::") {
        let rest = &body[at + "ErrorKind::".len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        if end > 0 && rest[end..].trim_start().starts_with('}') {
            out.insert(format!("refuse {}", &rest[..end]));
        }
    }
    for (start, word) in words(body) {
        let after = &body[start + word.len()..];
        if matches!(word, "transition" | "requires" | "ensures")
            && after.starts_with(|c: char| c.is_whitespace())
        {
            let clause = after.trim_start();
            let end = clause.find(['{', '\n']).unwrap_or(clause.len());
            limits(&clause[..end], &mut out);
        }
        if is_lower_ident(word)
            && !matches!(word, "transition" | "match")
            && let Some(open) = call_arguments_start(after)
        {
            if word.contains("min_u64") || word.contains("saturating") || word.contains("wrapping")
            {
                out.insert(format!("clamp {word}"));
            }
            for argument in top_level_arguments(&after[open..]) {
                if argument.len() >= 3 && argument.bytes().all(|b| b.is_ascii_digit()) {
                    out.insert(format!("const {word}(..{argument}..)"));
                }
            }
        }
    }
    out
}

/// `limit OP RHS` for each comparison against an integer literal (other than
/// 0, 1 and -1) or an `x.length` in a guard or contract clause.
fn limits(clause: &str, out: &mut BTreeSet<String>) {
    let bytes = clause.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let op = ["<=", ">=", "==", "!=", "<", ">"]
            .into_iter()
            .find(|op| clause[at..].starts_with(op));
        let Some(op) = op else {
            at += 1;
            continue;
        };
        let rhs_text = clause[at + op.len()..].trim_start();
        if let Some(rhs) = comparison_operand(rhs_text)
            && !matches!(rhs, "0" | "1" | "-1")
        {
            out.insert(format!("limit {op} {rhs}"));
        }
        at += op.len();
    }
}

/// A signed integer literal or `name.length` that ends at a word boundary.
fn comparison_operand(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let digits_from = usize::from(bytes.first() == Some(&b'-'));
    let digits = bytes[digits_from..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    let end = if digits > 0 {
        digits_from + digits
    } else {
        let name = bytes
            .iter()
            .take_while(|b| b.is_ascii_lowercase() || **b == b'_')
            .count();
        if name == 0 || !text[name..].starts_with(".length") {
            return None;
        }
        name + ".length".len()
    };
    bytes
        .get(end)
        .is_none_or(|b| !is_word(*b))
        .then_some(&text[..end])
}

/// The comma-separated arguments of a call, given the text after its `(`,
/// ignoring commas inside nested brackets.
fn top_level_arguments(text: &str) -> Vec<&str> {
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (at, c) in text.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => {
                arguments.push(text[start..at].trim());
                return arguments;
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                arguments.push(text[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_selections_resolve_default_and_explicit_file_aliases() {
        let build = "use linux_arm64_clock.omg;\n\
            use targets/linux_arm64/clock_impl.omg as wait;\n\
            machine build(builder: @mutable Build) {\n\
            Build::select_provider::<|clock|::Clock, |linux_arm64_clock|::LinuxClock>(builder, linux_arm64);\n\
            Build::select_provider::<|clock|::Clock, |wait|::Nested>(builder, macos_arm64);\n\
            }\n";
        let selections = provider_selections(build).unwrap();
        assert_eq!(
            selections["Clock"]["linux_arm64"],
            ("linux_arm64_clock.omg".into(), "LinuxClock".into())
        );
        assert_eq!(
            selections["Clock"]["macos_arm64"],
            ("targets/linux_arm64/clock_impl.omg".into(), "Nested".into())
        );
    }

    #[test]
    fn facts_read_refusals_limits_constants_and_clamps() {
        let body = "machine f(path: @[u8], count: u64) -> R\n\
            requires count <= buffer.length\n{\n\
            transition count > 4294967295 && count != 0 {\n\
            true -> R::Err { kind: ErrorKind::InvalidInput }\n\
            _ -> raw_write(path, 438, min_u64(count, 7))\n}\n}\n";
        let got: Vec<String> = facts(body).into_iter().collect();
        assert_eq!(
            got,
            [
                "clamp min_u64",
                "const raw_write(..438..)",
                "limit <= buffer.length",
                "limit > 4294967295",
                "refuse InvalidInput",
            ]
        );
    }

    #[test]
    fn limits_skip_trivial_bounds_and_glued_suffixes() {
        let mut out = BTreeSet::new();
        limits(
            "a < 1 && b >= -1 && c < 255u8 && d == -4 && e < self.buffer.length",
            &mut out,
        );
        assert_eq!(out.into_iter().collect::<Vec<_>>(), ["limit == -4"]);
    }

    #[test]
    fn baseline_needs_reasons_and_existing_concern_headings() {
        let headings: BTreeSet<&str> = ["A queued divergence"].into_iter().collect();
        let good = "# comment\n\
            S.m | limit < 4 | windows_x86_64 | known | A queued divergence\n\
            S.m | const f(..438..) | linux_x86_64 | noise | open mode word\n";
        let baseline = parse_baseline(good, &headings).expect("valid baseline");
        assert_eq!(baseline.rows.len(), 2);
        let bad = "S.m | limit < 4 | windows_x86_64 | known | A fixed divergence\n\
            S.m | limit < 5 | windows_x86_64 | maybe | ?\n\
            S.m | limit < 6 | windows_x86_64\n";
        let errors = parse_baseline(bad, &headings).expect_err("three bad rows");
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    #[test]
    fn grouped_reviews_name_every_member_and_reject_overlaps() {
        let group = "Store.{read,write} | limit < 9 | linux_arm64 | noise | shared buffer";
        let headings = BTreeSet::new();
        let baseline = parse_baseline(group, &headings).unwrap();
        assert_eq!(baseline.rows.len(), 2);
        assert!(
            baseline
                .rows
                .contains_key("Store.read | limit < 9 | linux_arm64")
        );
        assert!(
            baseline
                .rows
                .contains_key("Store.write | limit < 9 | linux_arm64")
        );
        assert!(
            parse_baseline(
                &format!("{group}\n{}", group.replace("{read,write}", "read")),
                &headings
            )
            .is_err()
        );
        assert!(parse_baseline(&group.replace("read,write", "*"), &headings).is_err());
    }

    #[test]
    fn comparison_reports_new_and_stale_rows() {
        let headings = BTreeSet::new();
        let baseline = parse_baseline(
            "S.a | refuse Other | windows_x86_64 | allowed | x\n\
                            S.b | refuse Other | windows_x86_64 | allowed | x\n",
            &headings,
        )
        .expect("valid baseline");
        let row = |member: &str, fact: &str| Row {
            service: "S".into(),
            member: member.into(),
            fact: fact.into(),
            targets: vec!["windows_x86_64"],
        };
        let comparison = compare(
            &[row("a", "refuse Other"), row("c", "limit < 9")],
            &baseline,
        );
        assert_eq!(comparison.new_rows, ["S.c | limit < 9 | windows_x86_64"]);
        assert_eq!(
            comparison.stale_rows,
            [("S.b | refuse Other | windows_x86_64".to_string(), 2)]
        );
        assert!(
            compare(
                &[row("a", "refuse Other"), row("b", "refuse Other")],
                &baseline
            )
            .is_clean()
        );
    }

    #[test]
    fn provider_selections_read_build_rows() {
        let build = "    Build::select_provider::<Clock, WinClock>(builder, windows_x86_64);\n\
                     Build::select_provider::<Clock, LinClock>(builder, linux_x86_64);\n";
        let selections = provider_selections(build).unwrap();
        assert_eq!(
            selections["Clock"]["windows_x86_64"],
            ("build.omg".into(), "WinClock".into())
        );
        assert_eq!(
            selections["Clock"]["linux_x86_64"],
            ("build.omg".into(), "LinClock".into())
        );
    }
}
