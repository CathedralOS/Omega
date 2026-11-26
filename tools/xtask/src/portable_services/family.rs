//! Index the adapter files and selected provider members for one hosted target.
use super::{
    facts, file_aliases, ident, is_lower_ident, is_word, opens_call, read, strip_comments, words,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Both Windows targets compile the Windows adapters kept in this directory;
/// a target's own directory adds only what its ABI changes.
const SHARED_WINDOWS_ADAPTERS: &str = "windows_x86_64";
/// The `.omg` files one target compiles from `system`: the shared root files,
/// minus root files another family owns by prefix, plus the target directory
/// and, for Windows, the shared Windows adapter directory.
fn family_files(system_root: &Path, target: &str) -> Result<Vec<PathBuf>, String> {
    let windows = target.starts_with("windows_");
    let posix = !windows;
    let mut files = Vec::new();
    let entries = fs::read_dir(system_root)
        .map_err(|error| format!("cannot list {}: {error}", system_root.display()))?;
    for entry in entries {
        let path = entry.map_err(|error| error.to_string())?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".omg") || name == "build.omg" {
            continue;
        }
        let owned_elsewhere = (name.starts_with("posix_") && !posix)
            || (name.starts_with("windows_") && !windows)
            || ((name.starts_with("darwin_") || name.starts_with("macos_"))
                && target != "macos_arm64");
        if !owned_elsewhere {
            files.push(path);
        }
    }
    files.sort();
    let target_dir = system_root.join("targets").join(target);
    let mut nested = Vec::new();
    collect_omg(&target_dir, &mut nested)?;
    if nested.is_empty() {
        return Err(format!("{} holds no .omg files", target_dir.display()));
    }
    if windows && target != SHARED_WINDOWS_ADAPTERS {
        collect_omg(
            &system_root.join("targets").join(SHARED_WINDOWS_ADAPTERS),
            &mut nested,
        )?;
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
pub(super) struct Family {
    bodies: Vec<MachineBody>,
    /// Machine name, or `Trait::member` for a machine that satisfies one,
    /// -> indices into `bodies`. A name defined twice is followed to both.
    machines: HashMap<String, Vec<usize>>,
    files: HashMap<PathBuf, AdapterFile>,
    pub(super) selected: BTreeMap<String, BTreeMap<String, usize>>,
}

struct MachineBody {
    text: String,
    file: PathBuf,
}

struct AdapterFile {
    providers: HashMap<String, BTreeMap<String, Member>>,
    aliases: HashMap<String, PathBuf>,
    machines: HashMap<String, usize>,
}

enum Member {
    Inline(usize),
    Named { file: PathBuf, machine: String },
}

impl Family {
    pub(super) fn load(system_root: &Path, target: &str) -> Result<Self, String> {
        let mut family = Family::default();
        for path in family_files(system_root, target)? {
            let text = strip_comments(&read(&path)?);
            let relative = path.strip_prefix(system_root).map_err(|_| {
                format!(
                    "adapter source {} lies outside {}",
                    path.display(),
                    system_root.display()
                )
            })?;
            family.add_source(relative, &text)?;
        }
        Ok(family)
    }

    fn add_source(&mut self, file: &Path, text: &str) -> Result<(), String> {
        let aliases = file_aliases(text);
        let mut machines = HashMap::new();
        let mut providers = HashMap::new();
        for (kind, body) in split_toplevel(text) {
            match kind {
                TopLevel::Machine(name) => {
                    let index = self.bodies.len();
                    self.machines
                        .entry(name.to_string())
                        .or_default()
                        .push(index);
                    if let Some(qualified) = satisfied_member(body) {
                        self.machines
                            .entry(qualified.to_string())
                            .or_default()
                            .push(index);
                    }
                    machines.insert(name.to_owned(), index);
                    self.bodies.push(MachineBody {
                        text: body.to_owned(),
                        file: file.to_owned(),
                    });
                }
                TopLevel::Provider(name) => {
                    let members = read_provider_members(file, body, &aliases, &mut self.bodies)?;
                    if providers.insert(name.to_string(), members).is_some() {
                        return Err(format!("duplicate provider `{name}` in {}", file.display()));
                    }
                }
                TopLevel::Other => {}
            }
        }
        let aliases = aliases
            .into_iter()
            .map(|(name, path)| (name.to_owned(), PathBuf::from(path)))
            .collect();
        self.files.insert(
            file.to_owned(),
            AdapterFile {
                aliases,
                machines,
                providers,
            },
        );
        Ok(())
    }

    /// Look up the provider in the exact file selected by the Build.
    pub(super) fn select(
        &mut self,
        service: &str,
        file: &Path,
        provider: &str,
    ) -> Result<(), String> {
        let members = self
            .files
            .get(file)
            .and_then(|source| source.providers.get(provider))
            .ok_or_else(|| {
                format!(
                    "{service}: cannot find provider `{provider}` in {}",
                    file.display()
                )
            })?;
        let mut selected = BTreeMap::new();
        for (name, body) in members {
            let index = match body {
                Member::Inline(index) => *index,
                Member::Named { file, machine } => *self
                    .files
                    .get(file)
                    .and_then(|source| source.machines.get(machine))
                    .ok_or_else(|| {
                        format!(
                            "{service}::{name}: cannot find `{machine}` in {}",
                            file.display()
                        )
                    })?,
            };
            selected.insert(name.to_owned(), index);
        }
        self.selected.insert(service.to_owned(), selected);
        Ok(())
    }

    /// A selected service call reaches exactly its chosen implementation.
    pub(super) fn member_facts(&self, root: usize) -> Result<BTreeSet<String>, String> {
        let mut seen = BTreeSet::from([root]);
        let mut pending = vec![root];
        let mut result = BTreeSet::new();
        while let Some(index) = pending.pop() {
            let body = &self.bodies[index];
            result.extend(facts(&body.text));
            let file = &self.files[&body.file];
            for name in called_names(&body.text) {
                if is_translation(name) {
                    continue;
                }
                let targets = if let Some((alias, machine)) = name
                    .strip_prefix('|')
                    .and_then(|name| name.split_once("|::"))
                {
                    file.aliases
                        .get(alias)
                        .and_then(|path| self.files.get(path))
                        .and_then(|source| source.machines.get(machine))
                        .map(std::slice::from_ref)
                } else if let Some((service, member)) = name.split_once("::")
                    && let Some(selected) = self.selected.get(service)
                {
                    Some(std::slice::from_ref(selected.get(member).ok_or_else(
                        || {
                            format!(
                                "{} calls `{name}` but its selected provider has no such member",
                                body.file.display()
                            )
                        },
                    )?))
                } else if let Some(local) = file.machines.get(name) {
                    Some(std::slice::from_ref(local))
                } else {
                    self.machines.get(name).map(Vec::as_slice)
                };
                for &callee in targets.into_iter().flatten() {
                    if seen.insert(callee) {
                        pending.push(callee);
                    }
                }
            }
        }
        Ok(result)
    }
}

enum TopLevel<'a> {
    Machine(&'a str),
    Provider(&'a str),
    Other,
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
        // The trait may be a bare name or a path such as `|alias|::Trait`.
        let after = after.trim_start();
        let path_end = after
            .find(|c: char| c.is_whitespace() || c == '{')
            .unwrap_or(after.len());
        if path_end > 0 && after[path_end..].trim_start().starts_with('{') {
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

/// An inline machine and a named satisfier both define one member of the
/// provider. Delimit each inline body at the next member, including mappings.
fn read_provider_members(
    file: &Path,
    block: &str,
    aliases: &HashMap<&str, &str>,
    bodies: &mut Vec<MachineBody>,
) -> Result<BTreeMap<String, Member>, String> {
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in block.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let Some(line) = line.trim_end().strip_prefix("    ") else {
            continue;
        };
        if let Some(signature) = line.strip_prefix("machine ") {
            if let Some(name) = ident(signature)
                && opens_call(&signature[name.len()..])
            {
                starts.push((start, name, None));
            }
        } else if let Some((requirement, machine)) = line
            .strip_suffix(';')
            .and_then(|line| line.split_once(" = "))
            && let Some((owner, member)) = requirement.rsplit_once("::")
            && (owner.starts_with('|') || ident(owner) == Some(owner))
            && ident(member) == Some(member)
        {
            let (source, machine) =
                match machine.strip_prefix('|').and_then(|s| s.split_once("|::")) {
                    Some((alias, name)) => (
                        Path::new(aliases.get(alias).ok_or_else(|| {
                            format!("unknown machine alias `{alias}` in {}", file.display())
                        })?),
                        name,
                    ),
                    None => (file, machine),
                };
            if ident(machine) != Some(machine) {
                return Err(format!(
                    "unreadable member mapping `{line}` in {}",
                    file.display()
                ));
            }
            starts.push((
                start,
                member,
                Some(Member::Named {
                    file: source.to_owned(),
                    machine: machine.to_owned(),
                }),
            ));
        }
    }
    let mut members = BTreeMap::new();
    let mut starts = starts.into_iter().peekable();
    while let Some((start, name, mapping)) = starts.next() {
        let member = mapping.unwrap_or_else(|| {
            let end = starts.peek().map_or(block.len(), |(offset, _, _)| *offset);
            let index = bodies.len();
            bodies.push(MachineBody {
                text: block[start..end].to_owned(),
                file: file.to_owned(),
            });
            Member::Inline(index)
        });
        if members.insert(name.to_owned(), member).is_some() {
            return Err(format!("duplicate member `{name}` in {}", file.display()));
        }
    }
    Ok(members)
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
/// yields just the selected member, even when a free helper has the same name).
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
            let prefix = &body[..start];
            if prefix.strip_suffix("::").is_some_and(|owner| {
                owner.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            }) || prefix.split_whitespace().next_back() == Some("machine")
            {
                continue;
            }
            if let Some((_, alias)) = prefix
                .strip_suffix("|::")
                .and_then(|before| before.rsplit_once('|'))
                && ident(alias) == Some(alias)
            {
                names.push(&body[start - alias.len() - 4..start + word.len()]);
            } else {
                names.push(word);
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn providers_name_bare_and_aliased_traits() {
        assert!(matches!(
            classify("FilesystemPosixProvider satisfies |filesystem|::FilesystemService {"),
            Some(TopLevel::Provider("FilesystemPosixProvider"))
        ));
        assert!(matches!(
            classify("public WindowsConsole satisfies Console {"),
            Some(TopLevel::Provider("WindowsConsole"))
        ));
        assert!(matches!(
            classify("Policy satisfies |abi|::CallingPolicy"),
            Some(TopLevel::Other)
        ));
    }

    #[test]
    fn closure_follows_calls_but_not_error_translation() {
        let text = "machine helper(x: u64) -> u64 {\n    transition x < 4096 { _ -> x }\n}\n\
            machine errno_kind(x: i64) -> K { K { kind: ErrorKind::Other } }\n\
            machine posix_permission_bits(x: u32) -> u32\n    satisfies Target::bits {\n    x\n}\n\
            P satisfies S {\n    machine go(x: u64) -> u64 {\n        helper(x) + errno_kind(1) + Target::bits(2)\n    }\n}\n";
        let mut family = Family::default();
        family.add_source(Path::new("helpers.omg"), text).unwrap();
        assert!(family.machines.contains_key("Target::bits"));
        family.select("S", Path::new("helpers.omg"), "P").unwrap();
        let got: Vec<String> = family
            .member_facts(family.selected["S"]["go"])
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(got, ["limit < 4096"]);
    }

    #[test]
    fn same_named_providers_in_sibling_files_stay_distinct() {
        let mut family = Family::default();
        for (file, result) in [
            ("adapters/left.omg", "Left"),
            ("adapters/right.omg", "Right"),
        ] {
            let source = format!("Adapter satisfies S {{\n    machine go() {{ {result} }}\n}}\n");
            family.add_source(Path::new(file), &source).unwrap();
        }
        for (file, result) in [
            ("adapters/left.omg", "Left"),
            ("adapters/right.omg", "Right"),
        ] {
            family.select("S", Path::new(file), "Adapter").unwrap();
            let body = &family.bodies[family.selected["S"]["go"]].text;
            assert!(body.contains(result), "{file}: {body}");
        }
        assert!(
            family
                .select("S", Path::new("missing.omg"), "Adapter")
                .is_err()
        );
    }

    #[test]
    fn only_the_selected_raw_member_and_its_named_file_reach_the_portable_call() {
        let mut family = Family::default();
        family
            .add_source(
                Path::new("portable.omg"),
                r#"use raw.omg as raw;
PortableAdapter satisfies Portable {
    machine go() { |raw|::Raw::read() }
}
machine read() { R::Err { kind: ErrorKind::WrongFreeMachine } }
"#,
            )
            .unwrap();
        family
            .add_source(
                Path::new("selection.omg"),
                r#"use raw.omg as raw;
use target/actual.omg as actual;
Chosen satisfies |raw|::Raw {
    |raw|::Raw::read = |actual|::fetch;
}
Other satisfies |raw|::Raw {
    machine read() { R::Err { kind: ErrorKind::OtherProvider } }
}
Missing satisfies |raw|::Raw {
    machine write() {}
}
"#,
            )
            .unwrap();
        let mut source = String::from("machine fetch() { step0() }\n");
        for index in 0..10 {
            source.push_str(&format!(
                "machine step{index}() {{ step{}() }}\n",
                index + 1
            ));
        }
        source.push_str("machine step10() { fetch(); R::Err { kind: ErrorKind::Actual } }\n");
        family
            .add_source(Path::new("target/actual.omg"), &source)
            .unwrap();
        family
            .add_source(
                Path::new("unused/actual.omg"),
                "machine fetch() { R::Err { kind: ErrorKind::WrongFile } }\n",
            )
            .unwrap();
        family
            .select("Portable", Path::new("portable.omg"), "PortableAdapter")
            .unwrap();
        let root = family.selected["Portable"]["go"];
        family
            .select("Raw", Path::new("selection.omg"), "Chosen")
            .unwrap();
        assert_eq!(
            family.member_facts(root).unwrap(),
            BTreeSet::from(["refuse Actual".into()])
        );
        family
            .select("Raw", Path::new("selection.omg"), "Other")
            .unwrap();
        assert_eq!(
            family.member_facts(root).unwrap(),
            BTreeSet::from(["refuse OtherProvider".into()])
        );
        family
            .select("Raw", Path::new("selection.omg"), "Missing")
            .unwrap();
        let error = family.member_facts(root).unwrap_err();
        assert!(error.contains("calls `Raw::read` but its selected provider has no such member"));
    }
}
