//! `check-portable-services`: compare how each target family of `std`
//! implements every portable call, and report differences nobody has reviewed.
//!
//! [Three layers](wiki/spec/build/platform_services.md#three-layers) says a
//! portable service "is identical on every target". `std` keeps one POSIX
//! adapter family (Linux and macOS) and one Windows family, so a refusal or a
//! limit that only one family applies makes the same call mean different
//! things on different targets. This check finds such differences from the
//! source text alone, in well under a second and without building anything.
//!
//! For each `Build::select_provider<Service, Provider>(builder, target)` row in
//! `std/build.omg` that covers all four hosted targets, it reads each target's
//! provider block, follows the top-level machines each member calls inside
//! that target's files, and collects four kinds of fact:
//! - `refuse K`: an `ErrorKind::K }` the adapter returns itself;
//! - `limit OP N`: an integer literal or `x.len` compared in a `transition`,
//!   `requires` or `ensures` clause (`< 255`, `<= buffer.len`);
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
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const HOSTED_TARGETS: [&str; 4] = [
    "linux_x86_64",
    "linux_arm64",
    "macos_arm64",
    "windows_x86_64",
];
const STD_ROOT: &str = "source/library/std";
const BASELINE: &str = "tools/ratchets/portable_services.txt";
const CONCERNS: &str = "OWNER_CODE_CONCERNS.md";
/// How many call levels a member's closure follows. Eight reaches every raw
/// host call in the current adapters; deeper calls are helpers.
const CLOSURE_DEPTH: usize = 8;

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
    let rows = match collect_rows(&root.join(STD_ROOT)) {
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

Compare the refusals, limits and constants each target family of std applies
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
        let key = fields[..3].join(" | ");
        if rows.insert(key.clone(), (reason, number)).is_some() {
            errors.push(format!("line {number}: duplicate row `{key}`"));
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

/// Reads `std` and returns every row, sorted.
pub fn collect_rows(std_root: &Path) -> Result<Vec<Row>, String> {
    let build = read(&std_root.join("build.omg"))?;
    let selections = provider_selections(&strip_comments(&build));
    let mut families = Vec::with_capacity(HOSTED_TARGETS.len());
    for target in HOSTED_TARGETS {
        families.push(Family::load(&family_files(std_root, target)?)?);
    }
    let mut rows = Vec::new();
    for (service, providers) in &selections {
        let Some(providers) = HOSTED_TARGETS
            .iter()
            .map(|target| providers.get(*target))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        // member -> facts per hosted target.
        let mut table: BTreeMap<String, Vec<BTreeSet<String>>> = BTreeMap::new();
        for (index, provider) in providers.iter().enumerate() {
            let family = &families[index];
            let block = family.providers.get(provider.as_str()).ok_or_else(|| {
                format!(
                    "{service}: provider `{provider}` selected for {} has no `{provider} satisfies {service} {{` block in that target's files",
                    HOSTED_TARGETS[index]
                )
            })?;
            for (member, body) in provider_members(block) {
                let facts = family.member_facts(body);
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

/// `Build::select_provider<Service, Provider>(builder, target)` rows, as
/// service -> target -> provider.
fn provider_selections(build: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    const CALL: &str = "Build::select_provider<";
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
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
            out.entry(service.to_string())
                .or_default()
                .insert(target.to_string(), provider.to_string());
        }
    }
    out
}

/// The `.omg` files one target compiles from `std`: the shared root files,
/// minus root files another family owns by prefix, plus the target directory.
fn family_files(std_root: &Path, target: &str) -> Result<Vec<PathBuf>, String> {
    let posix = target != "windows_x86_64";
    let mut files = Vec::new();
    let entries = fs::read_dir(std_root)
        .map_err(|error| format!("cannot list {}: {error}", std_root.display()))?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".omg") || name == "build.omg" {
            continue;
        }
        let owned_elsewhere = (name.starts_with("posix_") && !posix)
            || (name.starts_with("windows_") && target != "windows_x86_64")
            || ((name.starts_with("darwin_") || name.starts_with("macos_"))
                && target != "macos_arm64");
        if !owned_elsewhere {
            files.push(path);
        }
    }
    files.sort();
    let target_dir = std_root.join("targets").join(target);
    let mut nested = Vec::new();
    collect_omg(&target_dir, &mut nested)?;
    if nested.is_empty() {
        return Err(format!("{} holds no .omg files", target_dir.display()));
    }
    nested.sort();
    files.extend(nested);
    Ok(files)
}

fn collect_omg(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot list {}: {error}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            collect_omg(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "omg") {
            out.push(path);
        }
    }
    Ok(())
}

/// One target's top-level machines and provider blocks, comments stripped.
#[derive(Default)]
struct Family {
    bodies: Vec<String>,
    /// Machine name, or `Trait::member` for a machine that satisfies one,
    /// -> indices into `bodies`. A name defined twice is followed to both.
    machines: HashMap<String, Vec<usize>>,
    providers: HashMap<String, String>,
}

impl Family {
    fn load(files: &[PathBuf]) -> Result<Self, String> {
        let mut family = Family::default();
        for path in files {
            let text = strip_comments(&read(path)?);
            for (kind, body) in split_toplevel(&text) {
                match kind {
                    TopLevel::Machine(name) => {
                        let index = family.bodies.len();
                        family
                            .machines
                            .entry(name.to_string())
                            .or_default()
                            .push(index);
                        if let Some(qualified) = satisfied_member(body) {
                            family
                                .machines
                                .entry(qualified.to_string())
                                .or_default()
                                .push(index);
                        }
                        family.bodies.push(body.to_string());
                    }
                    TopLevel::Provider(name) => {
                        family.providers.insert(name.to_string(), body.to_string());
                    }
                    TopLevel::Other => {}
                }
            }
        }
        Ok(family)
    }

    /// Facts of a provider member and every machine it reaches within
    /// `CLOSURE_DEPTH` calls, skipping error-translation machines.
    fn member_facts(&self, member_body: &str) -> BTreeSet<String> {
        let mut facts = facts(member_body);
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut frontier: Vec<&str> = vec![member_body];
        for _ in 0..CLOSURE_DEPTH {
            let mut next = Vec::new();
            for body in frontier {
                for name in called_names(body) {
                    let Some((key, indices)) = self.machines.get_key_value(name) else {
                        continue;
                    };
                    if is_translation(name) || !seen.insert(key.as_str()) {
                        continue;
                    }
                    for &index in indices {
                        let callee = self.bodies[index].as_str();
                        facts.extend(self::facts(callee));
                        next.push(callee);
                    }
                }
            }
            frontier = next;
        }
        facts
    }
}

enum TopLevel<'a> {
    Machine(&'a str),
    Provider(&'a str),
    Other,
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

/// Classifies a column-0 line: `[public] machine NAME(`/`<`,
/// `[public] NAME satisfies TRAIT {`, or another top-level declaration.
fn classify(line: &str) -> Option<TopLevel<'_>> {
    let line = line
        .strip_prefix("public ")
        .map(str::trim_start)
        .unwrap_or(line);
    let first = ident(line)?;
    let rest = line[first.len()..].trim_start();
    if first == "machine" {
        let name = ident(rest)?;
        let after = rest[name.len()..].trim_start();
        return (after.starts_with('(') || after.starts_with('<'))
            .then_some(TopLevel::Machine(name));
    }
    if let Some(after) = rest.strip_prefix("satisfies")
        && after.starts_with(|c: char| c.is_whitespace())
    {
        let trait_name = ident(after.trim_start());
        let tail = trait_name.map(|t| after.trim_start()[t.len()..].trim_start());
        if tail.is_some_and(|tail| tail.starts_with('{')) {
            return Some(TopLevel::Provider(first));
        }
        if first.starts_with(|c: char| c.is_ascii_uppercase()) {
            return Some(TopLevel::Other);
        }
    }
    const OTHER: [&str; 8] = [
        "data", "use", "boundary", "domain", "trait", "enum", "const", "type",
    ];
    OTHER.contains(&first).then_some(TopLevel::Other)
}

/// Splits a file into its top-level declarations, each running to the next.
fn split_toplevel(text: &str) -> Vec<(TopLevel<'_>, &str)> {
    let mut starts: Vec<(usize, TopLevel<'_>)> = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if let Some(kind) = classify(line) {
            starts.push((offset, kind));
        }
        offset += line.len();
    }
    let ends: Vec<usize> = starts
        .iter()
        .skip(1)
        .map(|(at, _)| *at)
        .chain([text.len()])
        .collect();
    starts
        .into_iter()
        .zip(ends)
        .map(|((start, kind), end)| (kind, &text[start..end]))
        .collect()
}

/// `Trait::member` from a top-level machine declared as
/// `machine name(...) ... satisfies Trait::member`, on its first line or
/// starting the next non-blank line.
fn satisfied_member(body: &str) -> Option<&str> {
    let (first, rest) = body.split_once('\n').unwrap_or((body, ""));
    let tail = match first.find("satisfies ") {
        Some(at) => &first[at..],
        None => rest.trim_start(),
    };
    let tail = tail.strip_prefix("satisfies")?.trim_start();
    let owner = ident(tail)?;
    if !owner.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    let member = ident(tail[owner.len()..].strip_prefix("::")?)?;
    Some(&tail[..owner.len() + 2 + member.len()])
}

/// Splits a provider block into its members, each from its four-space
/// `machine` line to the next.
fn provider_members(block: &str) -> Vec<(&str, &str)> {
    let mut starts: Vec<(usize, &str)> = Vec::new();
    let mut offset = 0;
    for (index, line) in block.split_inclusive('\n').enumerate() {
        if index > 0
            && let Some(rest) = line.strip_prefix("    machine ")
            && let Some(name) = ident(rest.trim_start())
            && rest.trim_start()[name.len()..]
                .trim_start()
                .starts_with('(')
        {
            starts.push((offset, name));
        }
        offset += line.len();
    }
    let ends: Vec<usize> = starts
        .iter()
        .skip(1)
        .map(|(at, _)| *at)
        .chain([block.len()])
        .collect();
    starts
        .into_iter()
        .zip(ends)
        .map(|((start, name), end)| (name, &block[start..end]))
        .collect()
}

fn is_translation(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if ["error", "errno", "kind_of", "reconstruct"]
        .iter()
        .any(|word| lower.contains(word))
    {
        return true;
    }
    lower.match_indices("_kind").any(|(at, word)| {
        lower
            .as_bytes()
            .get(at + word.len())
            .is_none_or(|b| !is_word(*b))
    })
}

/// Every call in `body`: `name(`, `name<..>(`, and `Owner::name(` (which
/// yields both `Owner::name` and `name`).
fn called_names(body: &str) -> Vec<&str> {
    let mut names = Vec::new();
    for (start, word) in words(body) {
        let after = &body[start + word.len()..];
        if word.starts_with(|c: char| c.is_ascii_uppercase())
            && let Some(rest) = after.strip_prefix("::")
            && let Some(member) = ident(rest)
            && member.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
            && opens_call(&rest[member.len()..])
        {
            names.push(&body[start..start + word.len() + 2 + member.len()]);
        }
        if is_lower_ident(word) && opens_call(after) {
            names.push(word);
        }
    }
    names
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
/// 0, 1 and -1) or an `x.len` in a guard or contract clause.
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

/// A signed integer literal or `name.len` that ends at a word boundary.
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
        if name == 0 || !text[name..].starts_with(".len") {
            return None;
        }
        name + ".len".len()
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
    fn facts_read_refusals_limits_constants_and_clamps() {
        let body = "machine f(path: &[u8], count: u64) -> R\n\
            requires count <= buffer.len\n{\n\
            transition count > 4294967295 && count != 0 {\n\
            true -> R::Err { kind: ErrorKind::InvalidInput }\n\
            _ -> raw_write(path, 438, min_u64(count, 7))\n}\n}\n";
        let got: Vec<String> = facts(body).into_iter().collect();
        assert_eq!(
            got,
            [
                "clamp min_u64",
                "const raw_write(..438..)",
                "limit <= buffer.len",
                "limit > 4294967295",
                "refuse InvalidInput",
            ]
        );
    }

    #[test]
    fn limits_skip_trivial_bounds_and_glued_suffixes() {
        let mut out = BTreeSet::new();
        limits(
            "a < 1 && b >= -1 && c < 255u8 && d == -4 && e < self.buffer.len",
            &mut out,
        );
        assert_eq!(out.into_iter().collect::<Vec<_>>(), ["limit == -4"]);
    }

    #[test]
    fn closure_follows_calls_but_not_error_translation() {
        let text = "machine helper(x: u64) -> u64 {\n    transition x < 4096 { _ -> x }\n}\n\
            machine errno_kind(x: i64) -> K { K { kind: ErrorKind::Other } }\n\
            machine posix_permission_bits(x: u32) -> u32\n    satisfies Target::bits {\n    x\n}\n\
            P satisfies S {\n    machine go(x: u64) -> u64 {\n        helper(x) + errno_kind(1) + Target::bits(2)\n    }\n}\n";
        let mut family = Family::default();
        for (kind, body) in split_toplevel(text) {
            match kind {
                TopLevel::Machine(name) => {
                    let index = family.bodies.len();
                    family
                        .machines
                        .entry(name.to_string())
                        .or_default()
                        .push(index);
                    if let Some(q) = satisfied_member(body) {
                        family
                            .machines
                            .entry(q.to_string())
                            .or_default()
                            .push(index);
                    }
                    family.bodies.push(body.to_string());
                }
                TopLevel::Provider(name) => {
                    family.providers.insert(name.to_string(), body.to_string());
                }
                TopLevel::Other => {}
            }
        }
        assert!(family.machines.contains_key("Target::bits"));
        let members = provider_members(&family.providers["P"]);
        assert_eq!(members.len(), 1);
        let got: Vec<String> = family.member_facts(members[0].1).into_iter().collect();
        assert_eq!(got, ["limit < 4096"]);
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
        let build = "    Build::select_provider<Clock, WinClock>(builder, windows_x86_64);\n\
                     Build::select_provider<Clock, LinClock>(builder, linux_x86_64);\n";
        let selections = provider_selections(build);
        assert_eq!(selections["Clock"]["windows_x86_64"], "WinClock");
        assert_eq!(selections["Clock"]["linux_x86_64"], "LinClock");
    }
}
