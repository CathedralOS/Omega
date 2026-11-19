//! Collect declarations, admit file-local imports, then bind occurrences against one settled stage.
mod declarations;
mod lookup;
mod model;
mod occurrences;
mod operators;

use crate::syntax::File;
use declarations::index_file;
pub use model::*;
use omega_lexer::SourceSpan;
use omega_parser::NodeKind;
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn qualify(
    owner: &str,
    name: &str,
) -> String {
    if owner.is_empty() { name.into() } else { format!("{owner}::{name}") }
}
fn builtin(name: &str) -> bool {
    matches!(
        name,
        "core::Level"
            | "core::Type"
            | "core::Strict"
            | "core::Squash"
            | "bool"
            | "addr"
            | "f32"
            | "f64"
            | "Slice"
            | "SyscallResult"
            | "Terminal"
            | "UInt"
            | "Int"
            | "AtomicBool"
            | "AtomicU32"
            | "AtomicU64"
    ) || name
        .strip_prefix(['i', 'u'])
        .is_some_and(|s| s.bytes().all(|b| b.is_ascii_digit()))
}
fn identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
fn callable(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::Machine | NodeKind::AsmMachine | NodeKind::MathLet)
}

struct Entry {
    id: DeclarationId,
    file: usize,
    public: bool,
    lexical_owner: Option<usize>,
    node: usize,
    generic_roles: Vec<NodeKind>,
}
type Index = BTreeMap<(String, String), Vec<usize>>;
struct Local {
    kind: NodeKind,
    span: SourceSpan,
    owner: usize,
    start: usize,
    end: usize,
    static_binding: bool,
    callable_binding: bool,
    annotation: Option<usize>,
}
#[derive(Default)]
struct Context<'a> {
    heads: BTreeSet<usize>,
    entries: HashMap<usize, usize>,
    locals: HashMap<&'a str, Vec<Local>>,
    imports: BTreeMap<String, Vec<usize>>,
    modules: BTreeMap<String, Vec<(String, String)>>,
    exposed_domains: BTreeMap<usize, SourceSpan>,
}

/// Resolve names against the settled source files and direct package aliases for one stage.
pub fn resolve_stage(
    sources: &[StageSource<'_>],
    dependencies: &[Dependency<'_>],
) -> Result<ResolvedStage, NameError> {
    resolve_stage_for_target(sources, dependencies, &AssemblyVocabulary::default())
}

pub fn resolve_stage_for_target(
    sources: &[StageSource<'_>],
    dependencies: &[Dependency<'_>],
    assembly: &AssemblyVocabulary<'_>,
) -> Result<ResolvedStage, NameError> {
    let mut files: Vec<_> = sources.iter().map(File::read).collect::<Result<_, _>>()?;
    files.sort_by_key(|f| (f.package, f.path));
    let mut entries = Vec::new();
    let mut index = Index::new();
    let mut contexts = Vec::new();
    let mut modules = BTreeSet::new();
    for (i, file) in files.iter().enumerate() {
        if i > 0 && (file.package, file.path) == (files[i - 1].package, files[i - 1].path) {
            return Err(file.error(SourceSpan::new(0, 0), "duplicate logical file"));
        }
        let mut parts: Vec<_> = file.path.split('/').collect();
        if parts
            .pop()
            .is_none_or(|p| p.is_empty() || matches!(p, "." | "..") || p.contains(['\\', ':']))
            || parts.iter().any(|p| !identifier(p) || builtin(p))
        {
            return Err(file.error(SourceSpan::new(0, 0), format!("invalid package-relative logical path {}", file.path)));
        }
        for depth in 0..=parts.len() {
            modules.insert((file.package.to_owned(), parts[..depth].join("::")));
        }
        contexts.push(index_file(file, i, &mut entries, &mut index)?);
    }
    let mut edges = BTreeMap::new();
    for edge in dependencies {
        if edges
            .insert((edge.requester, edge.alias), edge.package)
            .is_some_and(|p| p != edge.package)
        {
            return Err(NameError::new(
                edge.requester,
                "",
                SourceSpan::new(0, 0),
                format!("conflicting packages for dependency alias {}", edge.alias),
            ));
        }
    }
    let mut provisional = Catalog {
        entries: &entries,
        index: &index,
        modules: &modules,
        edges: &edges,
        files: &files,
        carriers: HashMap::new(),
        assembly,
        trait_parents: HashMap::new(),
    };
    // Resolve authored signature names in their declaring files before detecting
    // duplicates. Different direct imports may both introduce a type named Point.
    for (i, file) in files.iter().enumerate() {
        provisional.imports(file, &mut contexts[i], &mut vec![], true)?;
    }
    provisional.index_trait_parents(&contexts)?;
    let owners: Vec<_> = entries
        .iter()
        .map(|entry| {
            let file = &files[entry.file];
            let ctx = &contexts[entry.file];
            let mut owners = vec![];
            for &section in file.nodes[entry.node]
                .children
                .iter()
                .filter(|&&i| matches!(file.nodes[i].kind, NodeKind::Params | NodeKind::Result))
            {
                for n in section + 1..file.nodes[section].end {
                    let node = &file.nodes[n];
                    if node.kind != NodeKind::PathType {
                        continue;
                    }
                    if file.nodes[section].kind == NodeKind::Result
                        && !file.child(section, NodeKind::QualifiedType).is_some_and(|q| {
                            file.nodes[q]
                                .children
                                .iter()
                                .skip(1)
                                .any(|&d| d <= n && n < file.nodes[d].end)
                        })
                    {
                        continue;
                    }
                    let name = node.path();
                    if builtin(&name)
                        || ctx
                            .locals
                            .get(name.as_str())
                            .into_iter()
                            .flatten()
                            .any(|l| l.static_binding && l.start <= n && n < l.end)
                    {
                        continue;
                    }
                    let mut slots = provisional.lookup(file, entry.file, ctx, n, &name);
                    slots.sort_unstable();
                    slots.dedup();
                    for i in slots {
                        let id = &entries[i].id;
                        if matches!(id.kind, NodeKind::Data | NodeKind::Trait | NodeKind::Domain) {
                            owners.push((id.package.clone(), id.path.clone()));
                        }
                    }
                }
            }
            owners
        })
        .collect();
    let trait_parents = provisional.trait_parents;
    for (i, signature_owners) in owners.into_iter().enumerate() {
        entries[i].id.signature_owners = signature_owners;
        if let Some(owner) = entries[i]
            .lexical_owner
            .and_then(|n| contexts[entries[i].file].entries.get(&n))
        {
            let parent = &entries[*owner].id;
            let mut ancestry = parent.enclosing_machines.clone();
            ancestry.push(EnclosingMachine {
                path: parent.path.clone(),
                operands: parent.operands.clone(),
                result_domains: parent.result_domains.clone(),
                signature_owners: parent.signature_owners.clone(),
            });
            entries[i].id.enclosing_machines = ancestry;
        }
        let entry = &entries[i];
        let key = (entry.id.package.clone(), entry.id.path.clone());
        if index[&key]
            .iter()
            .copied()
            .take_while(|&other| other < i)
            .any(|other| entries[other].id == entry.id)
        {
            let file = &files[entry.file];
            let name = file.child(entry.node, NodeKind::Path).unwrap_or(entry.node);
            return Err(file.error(file.span(name), format!("duplicate declaration {}", entry.id.path)));
        }
    }
    let mut catalog = Catalog {
        entries: &entries,
        index: &index,
        edges: &edges,
        files: &files,
        modules: &modules,
        assembly,
        carriers: HashMap::new(),
        trait_parents,
    };
    let mut uses = Vec::new();
    let mut import_ranges = Vec::with_capacity(files.len());
    let mut applications = Vec::new();
    let mut operators = Vec::new();
    let mut operator_declarations = Vec::new();
    for (file_number, file) in files.iter().enumerate() {
        contexts[file_number].imports.clear();
        contexts[file_number].modules.clear();
        contexts[file_number].exposed_domains.clear();
        let first = uses.len();
        catalog.imports(file, &mut contexts[file_number], &mut uses, false)?;
        import_ranges.push(first..uses.len());
    }
    for entry in entries.iter().filter(|e| callable(e.id.kind)) {
        let file = &files[entry.file];
        if let Some(head) = file.child(entry.node, NodeKind::Path)
            && let Some((owner, _)) = file.nodes[head].path().rsplit_once("::")
        {
            let owners = catalog.lookup(file, entry.file, &contexts[entry.file], head, owner);
            if owners.iter().any(|&i| entries[i].id.kind == NodeKind::Data) {
                return Err(file.error(file.span(head), "a data carrier cannot own a machine"));
            }
        }
    }
    for (slot, entry) in entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.id.kind == NodeKind::Domain)
    {
        let file = &files[entry.file];
        let context = &contexts[entry.file];
        let Some(head) = file.child(entry.node, NodeKind::PathType) else { continue };
        let name = file.nodes[head].path();
        let Some((carrier, _)) = name.rsplit_once("::") else { continue };
        let key = if builtin(carrier) {
            Carrier::Builtin(carrier.into())
        } else if context
            .locals
            .get(carrier)
            .into_iter()
            .flatten()
            .any(|l| l.owner == entry.node && l.kind == NodeKind::TypeParam)
        {
            Carrier::Generic
        } else {
            let slots = catalog.lookup(file, entry.file, context, head, carrier);
            let mut owners = slots
                .into_iter()
                .filter(|&i| matches!(entries[i].id.kind, NodeKind::Data | NodeKind::Domain));
            let Some(first) = owners.next() else {
                return Err(file.error(file.span(head), format!("unresolved name {carrier}")));
            };
            if owners.any(|i| i != first) {
                return Err(file.error(file.span(head), format!("ambiguous carrier {carrier}")));
            }
            Carrier::Named(first)
        };
        catalog.carriers.insert(slot, key);
    }
    for (file_number, file) in files.iter().enumerate() {
        let context = &contexts[file_number];
        for import in &uses[import_ranges[file_number].clone()] {
            if let Symbol::Declaration(id) = &import.target
                && id.kind == NodeKind::Domain
                && let Some(slots) = index.get(&(id.package.clone(), id.path.clone()))
            {
                for &slot in slots {
                    catalog.require_carrier(file, slot, import.span)?;
                }
            }
        }
        let first_use = uses.len();
        catalog.bind(file, file_number, context, &mut uses, &mut applications)?;
        catalog.collect_operators(file, file_number, context, &uses[first_use..], &mut operators, &mut operator_declarations)?;
    }
    Ok(ResolvedStage {
        definition_sites: entries
            .iter()
            .map(|e| DefinitionSite {
                file: files[e.file].path.into(),
                span: files[e.file].span(e.node),
            })
            .collect(),
        declarations: entries.into_iter().map(|entry| entry.id).collect(),
        uses,
        applications,
        operators,
        operator_declarations,
    })
}
struct Catalog<'a> {
    entries: &'a [Entry],
    index: &'a Index,
    modules: &'a BTreeSet<(String, String)>,
    edges: &'a BTreeMap<(&'a str, &'a str), &'a str>,
    files: &'a [File<'a>],
    carriers: HashMap<usize, Carrier>,
    trait_parents: HashMap<(String, String), BTreeSet<(String, String)>>,
    assembly: &'a AssemblyVocabulary<'a>,
}

#[derive(PartialEq, Eq)]
enum Carrier {
    Builtin(String),
    Named(usize),
    Generic,
}
fn usage(
    file: &File<'_>,
    span: SourceSpan,
    target: Symbol,
) -> NameUse {
    NameUse {
        package: file.package.into(),
        file: file.path.into(),
        span,
        target,
    }
}
