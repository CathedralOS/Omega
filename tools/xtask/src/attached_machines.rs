/*
Rewrite retired data-attached machine declarations as free machines
(`cargo xtask retire-attached-machines [--write] <path>...`).

`machine Counter::increment(counter: &mut Counter)` declares a machine under a
data type. The `no-data-code-attachment` decision retired that form
(wiki/spec/language/machines.md, "Static declarations and explicit state"): a
data declaration is not a machine namespace, and the operation is an ordinary
machine at module scope, `machine increment(counter: &mut Counter)`. This
command performs that rename one package at a time and changes every reference
in the same package with the declaration, so a package is never half-moved.

It works on tokens, not lines, and only rewrites what it can show keeps the
program's meaning. Everything else is left exactly as it was and reported with
the reason. It declines when:
- the qualifier is not a data type declared in the package (a trait, a named
  implementation, a module, or a name with no declaration);
- the declaration carries `satisfies`, is `boundary`, or has no body: those are
  conformance edges and compiler-supplied or foreign operations, whose
  replacement is a named implementation or a catalog change, not a rename;
- the carrier is generic (`machine Pair<T>::first`), where the binder has to
  move onto the machine;
- the machine is a layout or calling policy's `plan`, which the compiler finds
  through the policy type and whose replacement is a named implementation;
- a reference is spelled through an import the command cannot resolve.

The new name is the member name. Name resolution prefers local bindings and
parameters to declarations (wiki/spec/language/modules.md, "Resolution"), and
a file's own declarations to a sibling's, so the member name is not safe when
the package already declares or calls something with that spelling, or when a
rewritten call sits inside a machine that binds it. Then the command uses
`<data>_<member>` in snake case and says so.

A package is the nearest directory holding `build.omg`; sources without one
are grouped by directory. The report is line-oriented text on stdout: one
`rename`, `skip`, `text` or `summary` record per line, tab-separated. Running
the command again after `--write` reports no further renames.
*/

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const SKIPPED_DIRECTORIES: [&str; 4] = ["target", ".git", ".agents", "node_modules"];

/*
`Access<Policy, Schema>` and calling policies find their `plan` machine by the
policy's name today. The ratified replacement is a named implementation
(wiki/spec/language/domains.md, wiki/spec/build/calling_plans.md), so renaming
such a machine would detach it from the type that selects it.
*/
const CARRIER_SELECTED_RESULTS: [&str; 2] = ["AccessPlan", "BoundaryPlanResult"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Identifier,
    String,
    Other,
}

#[derive(Clone, Copy)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
}

/// A module's declared path, or `None` for the package's module-less files.
type Module = Option<String>;
/// The declaring module, the data qualifier and the member of one machine family.
type Key = (Module, String, String);

struct Declaration {
    qualifier: usize,
    member: usize,
    line: usize,
    reason: Option<String>,
}

struct Source {
    path: PathBuf,
    text: String,
    tokens: Vec<Token>,
    comments: Vec<(usize, usize)>,
    module: Module,
    data: BTreeSet<String>,
    other_owners: BTreeSet<String>,
    taken: BTreeSet<String>,
    declarations: Vec<Declaration>,
    item_of: Vec<Option<usize>>,
    item_spans: Vec<(usize, usize)>,
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn is_plain_name(text: &str) -> bool {
    let mut characters = text.chars();
    characters.next().is_some_and(is_identifier_start) && characters.all(is_identifier_continue)
}

/// Split source into code tokens and comment spans. Block comments nest, as in
/// the compiler's lexer; comments are kept apart so code matching never sees them.
fn lex(text: &str) -> (Vec<Token>, Vec<(usize, usize)>) {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut comments = Vec::new();
    let mut position = 0;
    while position < bytes.len() {
        let rest = &text[position..];
        let character = rest
            .chars()
            .next()
            .expect("position is a character boundary");
        if character.is_whitespace() {
            position += character.len_utf8();
        } else if rest.starts_with("//") {
            let end = rest
                .find('\n')
                .map_or(bytes.len(), |offset| position + offset);
            comments.push((position, end));
            position = end;
        } else if rest.starts_with("/*") {
            let mut depth = 1;
            let mut cursor = position + 2;
            while depth > 0 && cursor < bytes.len() {
                if text[cursor..].starts_with("/*") {
                    depth += 1;
                    cursor += 2;
                } else if text[cursor..].starts_with("*/") {
                    depth -= 1;
                    cursor += 2;
                } else {
                    cursor += text[cursor..].chars().next().map_or(1, char::len_utf8);
                }
            }
            comments.push((position, cursor));
            position = cursor;
        } else if character == '"' {
            let mut cursor = position + 1;
            while cursor < bytes.len() && bytes[cursor] != b'"' && bytes[cursor] != b'\n' {
                cursor += if bytes[cursor] == b'\\' { 2 } else { 1 };
            }
            let end = (cursor + 1).min(bytes.len());
            let closed = cursor < bytes.len() && bytes[cursor] == b'"';
            let kind = if closed { Kind::String } else { Kind::Other };
            let end = if closed { end } else { position + 1 };
            tokens.push(Token {
                kind,
                start: position,
                end,
            });
            position = end;
        } else if is_identifier_start(character) {
            let length: usize = rest
                .chars()
                .take_while(|&next| is_identifier_continue(next))
                .map(char::len_utf8)
                .sum();
            tokens.push(Token {
                kind: Kind::Identifier,
                start: position,
                end: position + length,
            });
            position += length;
        } else if character.is_ascii_digit() {
            let length: usize = rest
                .chars()
                .take_while(|&next| is_identifier_continue(next) || next == '.')
                .map(char::len_utf8)
                .sum();
            tokens.push(Token {
                kind: Kind::Other,
                start: position,
                end: position + length,
            });
            position += length;
        } else {
            let length = if rest.starts_with("::") {
                2
            } else {
                character.len_utf8()
            };
            tokens.push(Token {
                kind: Kind::Other,
                start: position,
                end: position + length,
            });
            position += length;
        }
    }
    (tokens, comments)
}

fn snake(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (index, &character) in characters.iter().enumerate() {
        if character.is_uppercase() && index > 0 {
            let previous = characters[index - 1];
            let next_is_lower = characters
                .get(index + 1)
                .is_some_and(|next| next.is_lowercase());
            if previous.is_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_uppercase() && next_is_lower)
            {
                out.push('_');
            }
        }
        out.extend(character.to_lowercase());
    }
    out
}

impl Source {
    fn read(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let (tokens, comments) = lex(&text);
        let mut source = Self {
            path: path.to_path_buf(),
            text,
            tokens,
            comments,
            module: None,
            data: BTreeSet::new(),
            other_owners: BTreeSet::new(),
            taken: BTreeSet::new(),
            declarations: Vec::new(),
            item_of: Vec::new(),
            item_spans: Vec::new(),
        };
        source.scan();
        Ok(source)
    }

    fn text_of(&self, index: usize) -> &str {
        let token = self.tokens[index];
        &self.text[token.start..token.end]
    }

    fn is_identifier(&self, index: usize) -> bool {
        self.tokens
            .get(index)
            .is_some_and(|token| token.kind == Kind::Identifier)
    }

    fn line(&self, index: usize) -> usize {
        self.text[..self.tokens[index].start].matches('\n').count() + 1
    }

    /// One pass over the tokens: top-level item spans, declared owners, every
    /// name the package already uses for a machine, a call or a constant, and
    /// the attached declarations themselves.
    fn scan(&mut self) {
        let count = self.tokens.len();
        let mut depth = 0usize;
        let mut item: Option<usize> = None;
        let mut item_start: Option<usize> = None;
        self.item_of = vec![None; count];
        for index in 0..count {
            let text = self.text_of(index).to_string();
            let previous = if index > 0 {
                self.text_of(index - 1).to_string()
            } else {
                String::new()
            };
            if depth > 0 || text == "}" {
                self.item_of[index] = item;
            }
            match text.as_str() {
                "{" => {
                    if depth == 0 {
                        let first = item_start.unwrap_or(index);
                        item = Some(self.item_spans.len());
                        self.item_spans.push((first, index));
                        self.item_of[first..=index].fill(item);
                    }
                    depth += 1;
                }
                "}" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        if let Some(current) = item {
                            self.item_spans[current].1 = index;
                        }
                        item_start = None;
                    }
                }
                ";" if depth == 0 => item_start = None,
                _ if depth == 0 && item_start.is_none() => item_start = Some(index),
                _ => {}
            }
            if !self.is_identifier(index) {
                continue;
            }
            let next_is_name = self.is_identifier(index + 1);
            let next_text = if index + 1 < count {
                self.text_of(index + 1).to_string()
            } else {
                String::new()
            };
            match text.as_str() {
                "module" if depth == 0 && matches!(previous.as_str(), "" | ";" | "}") => {
                    let mut path = String::new();
                    let mut cursor = index + 1;
                    while cursor < count && self.text_of(cursor) != ";" {
                        path.push_str(self.text_of(cursor));
                        cursor += 1;
                    }
                    self.module = Some(path);
                }
                "data" if depth == 0 && next_is_name => {
                    self.data.insert(next_text);
                }
                "trait" | "domain" | "layout" | "union" if next_is_name => {
                    self.other_owners.insert(next_text);
                }
                "satisfies" if depth == 0 && index > 0 && self.is_identifier(index - 1) => {
                    let before = if index >= 2 {
                        self.text_of(index - 2)
                    } else {
                        ""
                    };
                    if index < 2 || matches!(before, "public" | ";" | "}") {
                        self.other_owners.insert(previous);
                    }
                }
                "const" if depth == 0 && next_is_name => {
                    self.taken.insert(next_text);
                }
                "machine" if !matches!(previous.as_str(), "." | "::") => {
                    self.scan_machine(index, depth)
                }
                _ if next_text == "(" && !matches!(previous.as_str(), "." | "::" | "machine") => {
                    self.taken.insert(text);
                }
                _ => {}
            }
        }
    }

    fn scan_machine(&mut self, index: usize, depth: usize) {
        let count = self.tokens.len();
        let mut name = index + 1;
        while name < count && !self.is_identifier(name) && name - index <= 4 {
            name += 1;
        }
        if !self.is_identifier(name) {
            return;
        }
        let after = name + 1;
        let line = self.line(index);
        if after + 1 < count && self.text_of(after) == "::" {
            let reason = if depth != 0 {
                Some("declared inside another declaration".to_string())
            } else if !self.is_identifier(after + 1) {
                Some("member is not a plain name".to_string())
            } else if after + 2 < count && self.text_of(after + 2) == "::" {
                Some("path has more than two segments".to_string())
            } else {
                self.header_reason(index, after + 2)
            };
            self.declarations.push(Declaration {
                qualifier: name,
                member: after + 1,
                line,
                reason,
            });
        } else if after < count && self.text_of(after) == "<" && self.generic_carrier(after) {
            let reason = Some("generic carrier: the binder must move onto the machine".to_string());
            self.declarations.push(Declaration {
                qualifier: name,
                member: name,
                line,
                reason,
            });
        } else {
            let text = self.text_of(name).to_string();
            self.taken.insert(text);
        }
    }

    /// Whether `Name<...>` after `machine` is followed by `::member`.
    fn generic_carrier(&self, opening: usize) -> bool {
        let mut nesting = 0usize;
        for cursor in opening..self.tokens.len().min(opening + 64) {
            match self.text_of(cursor) {
                "<" => nesting += 1,
                ">" => {
                    nesting = nesting.saturating_sub(1);
                    if nesting == 0 {
                        return cursor + 1 < self.tokens.len() && self.text_of(cursor + 1) == "::";
                    }
                }
                "(" | "{" | ";" => return false,
                _ => {}
            }
        }
        false
    }

    /// Why a two-segment header cannot be renamed, read up to its body.
    fn header_reason(&self, machine: usize, mut cursor: usize) -> Option<String> {
        let mut back = machine;
        while back > 0 && matches!(self.text_of(back - 1), "public" | "boundary") {
            if self.text_of(back - 1) == "boundary" {
                return Some("boundary declaration".to_string());
            }
            back -= 1;
        }
        let mut nesting = 0usize;
        while cursor < self.tokens.len() {
            let text = self.text_of(cursor);
            match text {
                "(" | "[" => nesting += 1,
                ")" | "]" => nesting = nesting.saturating_sub(1),
                _ if nesting == 0 => {
                    if text == "{" {
                        return None;
                    }
                    if text == ";" {
                        return Some("bodyless declaration".to_string());
                    }
                    if text == "satisfies" {
                        return Some("declared conformance edge (`satisfies`)".to_string());
                    }
                    if CARRIER_SELECTED_RESULTS.contains(&text) {
                        return Some(format!(
                            "`{text}` policy machine: the compiler selects it through its carrier"
                        ));
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
        Some("unterminated header".to_string())
    }
}

/// Where a reference sits: a `Qualifier::member` token run, or a string token.
#[derive(Clone, Copy)]
enum Place {
    Path(usize),
    String(usize),
}

struct Package {
    root: PathBuf,
    sources: Vec<Source>,
    groups: BTreeMap<Key, Vec<(usize, usize)>>,
    blocked: BTreeMap<Key, String>,
    renames: BTreeMap<Key, String>,
    edits: BTreeMap<PathBuf, BTreeSet<(usize, usize, String)>>,
    records: Vec<String>,
}

impl Package {
    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn name_taken(&self, name: &str) -> bool {
        self.sources
            .iter()
            .any(|source| source.taken.contains(name))
    }

    fn name_used(&self, name: &str) -> bool {
        self.sources.iter().any(|source| {
            (0..source.tokens.len())
                .any(|index| source.is_identifier(index) && source.text_of(index) == name)
        })
    }

    fn module_keys(&self, last_segment: &str) -> BTreeSet<Module> {
        self.sources
            .iter()
            .filter(|source| {
                source
                    .module
                    .as_deref()
                    .is_some_and(|module| module.rsplit("::").next() == Some(last_segment))
            })
            .map(|source| source.module.clone())
            .collect()
    }

    /// Decide the new name of every attached machine that can move.
    fn plan(&mut self) {
        for (source_index, source) in self.sources.iter().enumerate() {
            for (declaration_index, declaration) in source.declarations.iter().enumerate() {
                let qualifier = source.text_of(declaration.qualifier).to_string();
                let member = source.text_of(declaration.member).to_string();
                let key = (source.module.clone(), qualifier.clone(), member);
                let owns_data = self
                    .sources
                    .iter()
                    .any(|other| other.module == source.module && other.data.contains(&qualifier));
                let ambiguous = self
                    .sources
                    .iter()
                    .any(|other| other.other_owners.contains(&qualifier));
                let reason = declaration.reason.clone().or_else(|| {
                    if !owns_data {
                        Some("qualifier is not a data type declared in this package".to_string())
                    } else if ambiguous {
                        Some(
                            "qualifier also names a trait, implementation, domain or layout"
                                .to_string(),
                        )
                    } else {
                        None
                    }
                });
                if let Some(reason) = reason {
                    self.blocked.entry(key.clone()).or_insert(reason);
                }
                self.groups
                    .entry(key)
                    .or_default()
                    .push((source_index, declaration_index));
            }
        }
        let mut leaf_owners: BTreeMap<&str, BTreeSet<(&Module, &str)>> = BTreeMap::new();
        for (module, qualifier, member) in self.groups.keys() {
            leaf_owners
                .entry(member)
                .or_default()
                .insert((module, qualifier));
        }
        let mut renames = BTreeMap::new();
        let mut blocked = Vec::new();
        for key in self.groups.keys() {
            if self.blocked.contains_key(key) {
                continue;
            }
            let (_, qualifier, member) = key;
            if self.name_taken(member) || leaf_owners[member.as_str()].len() > 1 {
                let fallback = format!("{}_{member}", snake(qualifier));
                if self.name_taken(&fallback) {
                    let reason =
                        format!("both `{member}` and `{fallback}` are already used in the package");
                    blocked.push((key.clone(), reason));
                } else {
                    renames.insert(key.clone(), fallback);
                }
            } else {
                renames.insert(key.clone(), member.clone());
            }
        }
        self.renames = renames;
        self.blocked.extend(blocked);
    }

    fn block_spelling(&mut self, qualifier: &str, member: &str, reason: String) {
        let keys: Vec<Key> = self
            .groups
            .keys()
            .filter(|key| key.1 == qualifier && key.2 == member)
            .cloned()
            .collect();
        for key in keys {
            self.renames.remove(&key);
            self.blocked.entry(key).or_insert_with(|| reason.clone());
        }
    }

    /// The family a `Qualifier::member` path at `index` selects, or why it cannot be told.
    fn resolve(&self, source: &Source, index: usize) -> Result<Option<Key>, String> {
        let qualifier = source.text_of(index);
        let member = source.text_of(index + 2);
        let candidates: Vec<&Key> = self
            .groups
            .keys()
            .filter(|key| key.1 == qualifier && key.2 == member)
            .collect();
        if candidates.is_empty() {
            return Ok(None);
        }
        if index > 0 && source.text_of(index - 1) == "::" {
            if index < 2 || !source.is_identifier(index - 2) {
                return Err("reference through an unsupported path".to_string());
            }
            let prefix = source.text_of(index - 2);
            let modules = self.module_keys(prefix);
            let matches: Vec<&&Key> = candidates
                .iter()
                .filter(|key| modules.contains(&key.0))
                .collect();
            return match matches.as_slice() {
                [only] => Ok(Some((**only).clone())),
                _ => Err(format!(
                    "path prefix `{prefix}` does not name one declaring module"
                )),
            };
        }
        let own = (
            source.module.clone(),
            qualifier.to_string(),
            member.to_string(),
        );
        if self.groups.contains_key(&own) {
            return Ok(Some(own));
        }
        if source.data.contains(qualifier) {
            return Ok(None);
        }
        Err("bare reference from another module".to_string())
    }

    /// Find every reference, blocking families whose references cannot move with them.
    fn references(&mut self) -> Vec<(usize, Place, Key)> {
        let mut found = Vec::new();
        let mut blocks = Vec::new();
        for (source_index, source) in self.sources.iter().enumerate() {
            let declared: BTreeSet<usize> = source
                .declarations
                .iter()
                .map(|declaration| declaration.qualifier)
                .collect();
            let count = source.tokens.len();
            for index in 0..count.saturating_sub(2) {
                if !source.is_identifier(index) || source.text_of(index + 1) != "::" {
                    continue;
                }
                if !source.is_identifier(index + 2) || declared.contains(&index) {
                    continue;
                }
                if index + 3 < count && source.text_of(index + 3) == "::" {
                    continue;
                }
                match self.resolve(source, index) {
                    Ok(Some(key)) => found.push((source_index, Place::Path(index), key)),
                    Ok(None) => {}
                    Err(detail) => blocks.push((
                        source.text_of(index).to_string(),
                        source.text_of(index + 2).to_string(),
                        format!(
                            "{detail} at {}:{}",
                            self.relative(&source.path),
                            source.line(index)
                        ),
                    )),
                }
            }
            for index in 0..count {
                if source.tokens[index].kind != Kind::String {
                    continue;
                }
                let literal = source.text_of(index);
                let parts: Vec<&str> = literal[1..literal.len() - 1].split("::").collect();
                if parts.len() < 2 || !parts.iter().all(|part| is_plain_name(part)) {
                    continue;
                }
                let (qualifier, member) = (parts[parts.len() - 2], parts[parts.len() - 1]);
                let named = |key: &&Key| key.1 == qualifier && key.2 == member;
                if !self.groups.keys().any(|key| named(&key)) {
                    continue;
                }
                let keys: Vec<&Key> = if parts.len() == 2 {
                    self.groups
                        .keys()
                        .filter(named)
                        .filter(|key| key.0 == source.module)
                        .collect()
                } else {
                    let modules = self.module_keys(parts[parts.len() - 3]);
                    self.groups
                        .keys()
                        .filter(named)
                        .filter(|key| modules.contains(&key.0))
                        .collect()
                };
                match keys.as_slice() {
                    [only] => found.push((source_index, Place::String(index), (*only).clone())),
                    _ => blocks.push((
                        qualifier.to_string(),
                        member.to_string(),
                        format!(
                            "string names it at {}:{}",
                            self.relative(&source.path),
                            source.line(index)
                        ),
                    )),
                }
            }
        }
        for (qualifier, member, reason) in blocks {
            self.block_spelling(&qualifier, &member, reason);
        }
        found
    }

    /// Switch to the fallback name where a binding would capture a rewritten call.
    fn shadowed(&mut self, references: &[(usize, Place, Key)]) -> bool {
        let mut changed = false;
        for (source_index, place, key) in references {
            let (Place::Path(place), Some(name)) = (*place, self.renames.get(key).cloned()) else {
                continue;
            };
            let source = &self.sources[*source_index];
            let Some(item) = source.item_of[place] else {
                continue;
            };
            let (first, last) = source.item_spans[item];
            let bound = (first..=last).any(|index| {
                source.is_identifier(index)
                    && source.text_of(index) == name
                    && !(index > 0 && matches!(source.text_of(index - 1), "." | "::"))
                    && !(index + 1 < source.tokens.len() && source.text_of(index + 1) == "::")
            });
            if !bound {
                continue;
            }
            let fallback = format!("{}_{}", snake(&key.1), key.2);
            if name == fallback || self.name_used(&fallback) {
                let reason = format!(
                    "`{name}` is bound where it is called at {}:{}",
                    self.relative(&source.path),
                    source.line(place)
                );
                self.renames.remove(key);
                self.blocked.insert(key.clone(), reason);
            } else {
                self.renames.insert(key.clone(), fallback);
            }
            changed = true;
        }
        changed
    }

    fn edit(&mut self, source: usize, start: usize, end: usize, replacement: String) {
        let path = self.sources[source].path.clone();
        self.edits
            .entry(path)
            .or_default()
            .insert((start, end, replacement));
    }

    fn run(&mut self) -> Result<(), String> {
        self.plan();
        let references = self.references();
        while self.shadowed(&references) {}
        let groups = self.groups.clone();
        for (key, declarations) in &groups {
            let spelling = format!("{}::{}", key.1, key.2);
            let Some(name) = self.renames.get(key).cloned() else {
                for &(source_index, declaration_index) in declarations {
                    let source = &self.sources[source_index];
                    let declaration = &source.declarations[declaration_index];
                    let reason = declaration
                        .reason
                        .clone()
                        .or_else(|| self.blocked.get(key).cloned())
                        .unwrap_or_else(|| "blocked".to_string());
                    let record = format!(
                        "skip\t{}:{}\t{spelling}\t{reason}",
                        self.relative(&source.path),
                        declaration.line
                    );
                    self.records.push(record);
                }
                continue;
            };
            for &(source_index, declaration_index) in declarations {
                let source = &self.sources[source_index];
                let declaration = &source.declarations[declaration_index];
                let (start, end) = (
                    source.tokens[declaration.qualifier].start,
                    source.tokens[declaration.member].end,
                );
                self.edit(source_index, start, end, name.clone());
            }
            let mut uses = 0;
            for (source_index, place, reference_key) in &references {
                if reference_key != key {
                    continue;
                }
                uses += 1;
                let source = &self.sources[*source_index];
                match *place {
                    Place::Path(index) => {
                        let (start, end) =
                            (source.tokens[index].start, source.tokens[index + 2].end);
                        self.edit(*source_index, start, end, name.clone());
                    }
                    Place::String(index) => {
                        let literal = source.text_of(index);
                        let mut parts: Vec<&str> =
                            literal[1..literal.len() - 1].split("::").collect();
                        parts.truncate(parts.len() - 2);
                        parts.push(&name);
                        let replacement = format!("\"{}\"", parts.join("::"));
                        let token = source.tokens[index];
                        self.edit(*source_index, token.start, token.end, replacement);
                    }
                }
            }
            let (source_index, declaration_index) = declarations[0];
            let source = &self.sources[source_index];
            let record = format!(
                "rename\t{}:{}\t{spelling}\t{name}\tdeclarations={}\treferences={uses}",
                self.relative(&source.path),
                source.declarations[declaration_index].line,
                declarations.len()
            );
            self.records.push(record);
        }
        self.comments_and_sidecars()
    }

    /// Prose follows the code: comments, README and `expected*.txt` files that
    /// quote a spelling naming exactly one renamed machine in the package.
    fn comments_and_sidecars(&mut self) -> Result<(), String> {
        let mut by_spelling: BTreeMap<String, Vec<&Key>> = BTreeMap::new();
        for key in self.groups.keys() {
            by_spelling
                .entry(format!("{}::{}", key.1, key.2))
                .or_default()
                .push(key);
        }
        let spellings: Vec<(String, String)> = by_spelling
            .into_iter()
            .filter_map(|(spelling, keys)| match keys.as_slice() {
                [only] => self.renames.get(*only).map(|name| (spelling, name.clone())),
                _ => None,
            })
            .collect();
        if spellings.is_empty() {
            return Ok(());
        }
        for source_index in 0..self.sources.len() {
            let source = &self.sources[source_index];
            let mut edits = Vec::new();
            for &(start, end) in &source.comments {
                edits.extend(occurrences(&source.text, start, end, &spellings));
            }
            for (start, end, replacement) in edits {
                self.edit(source_index, start, end, replacement);
            }
        }
        let directories: BTreeSet<PathBuf> = self
            .sources
            .iter()
            .filter_map(|source| source.path.parent().map(Path::to_path_buf))
            .collect();
        for directory in directories {
            let mut names: Vec<PathBuf> = fs::read_dir(&directory)
                .map_err(|error| format!("{}: {error}", directory.display()))?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect();
            names.sort();
            for path in names {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                let sidecar =
                    name == "README.md" || (name.starts_with("expected") && name.ends_with(".txt"));
                if !sidecar {
                    continue;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let edits = occurrences(&text, 0, text.len(), &spellings);
                if !edits.is_empty() {
                    let record = format!(
                        "text\t{}\toccurrences={}",
                        self.relative(&path),
                        edits.len()
                    );
                    self.records.push(record);
                    self.edits.entry(path).or_default().extend(edits);
                }
            }
        }
        Ok(())
    }

    fn write(&self) -> Result<(), String> {
        for (path, edits) in &self.edits {
            let mut text =
                fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
            for (start, end, replacement) in edits.iter().rev() {
                text.replace_range(*start..*end, replacement);
            }
            fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))?;
        }
        Ok(())
    }
}

/// Whole-path occurrences of the renamed spellings in `text[start..end]`: not
/// part of a longer name or a longer `::` path on either side.
fn occurrences(
    text: &str,
    start: usize,
    end: usize,
    spellings: &[(String, String)],
) -> Vec<(usize, usize, String)> {
    let boundary =
        |character: Option<char>| !character.is_some_and(|c| is_identifier_continue(c) || c == ':');
    let mut found = Vec::new();
    for (spelling, name) in spellings {
        let mut cursor = start;
        while let Some(offset) = text[cursor..end].find(spelling.as_str()) {
            let at = cursor + offset;
            let after = at + spelling.len();
            if boundary(text[..at].chars().next_back()) && boundary(text[after..].chars().next()) {
                found.push((at, after, name.clone()));
            }
            cursor = after;
        }
    }
    found
}

/// Group `.omg` files by the nearest directory, at or below `root`, holding `build.omg`.
fn discover(
    root: &Path,
    directory: &Path,
    packages: &mut BTreeMap<PathBuf, Vec<PathBuf>>,
) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    entries.sort();
    let mut owner = directory.to_path_buf();
    let mut probe = directory;
    loop {
        if probe.join("build.omg").is_file() {
            owner = probe.to_path_buf();
            break;
        }
        match probe.parent() {
            Some(parent) if probe != root && probe.starts_with(root) => probe = parent,
            _ => break,
        }
    }
    for path in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if !SKIPPED_DIRECTORIES.contains(&name) {
                discover(root, &path, packages)?;
            }
        } else if name.ends_with(".omg") {
            packages.entry(owner.clone()).or_default().push(path);
        }
    }
    Ok(())
}

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    let write = args.iter().any(|argument| argument == "--write");
    let paths: Vec<&String> = args
        .iter()
        .filter(|argument| !argument.starts_with("--"))
        .collect();
    if paths.is_empty()
        || args
            .iter()
            .any(|argument| argument.starts_with("--") && argument != "--write")
    {
        eprintln!("usage: cargo xtask retire-attached-machines [--write] <path>...");
        return Err(2);
    }
    let base = std::env::current_dir().map_err(|error| {
        eprintln!("retire-attached-machines: {error}");
        1
    })?;
    let mut packages = BTreeMap::new();
    for path in paths {
        let root = fs::canonicalize(path).map_err(|error| {
            eprintln!("retire-attached-machines: {path}: {error}");
            1
        })?;
        discover(&root, &root, &mut packages).map_err(|error| {
            eprintln!("retire-attached-machines: {error}");
            1
        })?;
    }
    let base = fs::canonicalize(&base).unwrap_or(base);
    let (mut renamed, mut skipped, mut touched) = (0, 0, 0);
    for (root, files) in packages {
        let label = root
            .strip_prefix(&base)
            .unwrap_or(&root)
            .to_string_lossy()
            .replace('\\', "/");
        let sources: Result<Vec<Source>, String> =
            files.iter().map(|file| Source::read(file)).collect();
        let sources = match sources {
            Ok(sources) => sources,
            Err(error) => {
                println!("skip\t{label}\t-\t-\tunreadable source: {error}");
                skipped += 1;
                continue;
            }
        };
        if sources.iter().all(|source| source.declarations.is_empty()) {
            continue;
        }
        let mut package = Package {
            root,
            sources,
            groups: BTreeMap::new(),
            blocked: BTreeMap::new(),
            renames: BTreeMap::new(),
            edits: BTreeMap::new(),
            records: Vec::new(),
        };
        let outcome = package
            .run()
            .and_then(|()| if write { package.write() } else { Ok(()) });
        if let Err(error) = outcome {
            eprintln!("retire-attached-machines: {error}");
            return Err(1);
        }
        for record in &package.records {
            let (kind, rest) = record.split_once('\t').unwrap_or((record, ""));
            println!("{kind}\t{label}\t{rest}");
            renamed += usize::from(kind == "rename");
            skipped += usize::from(kind == "skip");
        }
        touched += package.edits.len();
    }
    let mode = if write { "written" } else { "dry-run" };
    println!("summary\t{mode}\trenames={renamed}\tskipped={skipped}\tfiles={touched}");
    Ok(())
}
