//! Collect declarations, admit file-local imports, then bind occurrences against one settled stage.
mod binder_roles;
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

#[derive(Clone, Copy)]
struct GenericBinder {
    kind: NodeKind,
    takes_type: bool,
}

impl GenericBinder {
    fn from_kind(kind: NodeKind) -> Self {
        Self {
            kind,
            takes_type: matches!(kind, NodeKind::TypeParam | NodeKind::SomeParam),
        }
    }
}

struct Entry {
    id: DeclarationId,
    source: Option<(usize, usize)>,
    public: bool,
    lexical_owner: Option<usize>,
    generic_roles: Vec<GenericBinder>,
}
type Index = BTreeMap<(String, String), Vec<usize>>;
struct Local {
    kind: NodeKind,
    span: SourceSpan,
    owner: usize,
    start: usize,
    end: usize,
    static_binding: bool,
    type_binding: bool,
    callable_binding: bool,
    annotation: Option<usize>,
}
#[derive(Default)]
struct Context<'a> {
    // Keep the syntax node and its position among locals of the same name until imports are settled.
    binder_annotations: Vec<(usize, usize)>,
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
    resolve_stage_with_exports(sources, dependencies, assembly, &[])
}

/// A public name from a compiled interface. The supported declarations have no
/// lexical owner, inherited traits, domain carriers or target-specific behavior.
pub struct InterfaceExport {
    pub id: DeclarationId,
    pub generic_roles: Vec<NodeKind>,
    pub module: &'static str,
}

pub fn resolve_stage_with_exports(
    sources: &[StageSource<'_>],
    dependencies: &[Dependency<'_>],
    assembly: &AssemblyVocabulary<'_>,
    exports: &[InterfaceExport],
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
    let source_count = entries.len();
    for export in exports {
        let id = &export.id;
        let error = |message: &str| NameError::new(&id.package, &id.path, SourceSpan::new(0, 0), message);
        if files
            .binary_search_by(|f| f.package.cmp(id.package.as_str()))
            .is_ok()
        {
            return Err(error("source package collides with a compiled interface"));
        }
        if !matches!(
            id.kind,
            NodeKind::Data | NodeKind::Machine | NodeKind::Case | NodeKind::CaseField | NodeKind::Field | NodeKind::Trait
        ) {
            return Err(error("unsupported declaration in compiled interface"));
        }
        let slots = index.entry((id.package.clone(), id.path.clone())).or_default();
        if slots.iter().any(|&slot| entries[slot].id == *id) {
            return Err(error("duplicate declaration in compiled interface"));
        }
        slots.push(entries.len());
        entries.push(Entry {
            id: id.clone(),
            source: None,
            public: true,
            lexical_owner: None,
            generic_roles: export
                .generic_roles
                .iter()
                .map(|&kind| GenericBinder::from_kind(kind))
                .collect(),
        });
        let mut module = export.module;
        loop {
            modules.insert((id.package.clone(), module.to_owned()));
            if module.is_empty() {
                break;
            }
            module = module.rsplit_once("::").map_or("", |(parent, _)| parent);
        }
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
    let owners: Vec<_> = entries[..source_count]
        .iter()
        .map(|entry| {
            let (file_number, node) = entry.source.expect("source declaration");
            let file = &files[file_number];
            let ctx = &contexts[file_number];
            let mut owners = vec![];
            for &section in file.nodes[node]
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
                    let mut slots = provisional.lookup(file, file_number, ctx, n, &name);
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
    let universe_binders = provisional.bind_universe_roles(&mut contexts)?;
    let trait_parents = provisional.trait_parents;
    for (slot, position) in universe_binders {
        entries[slot].generic_roles[position].takes_type = true;
    }
    // A use supplies the domain head's indices. The carrier binder is supplied
    // by the qualified value, and has no argument position of its own.
    for entry in entries[..source_count]
        .iter_mut()
        .filter(|entry| entry.id.kind == NodeKind::Domain)
    {
        let (file_number, node) = entry.source.expect("source domain");
        let file = &files[file_number];
        let context = &contexts[file_number];
        let head = file.child(node, NodeKind::PathType).expect("domain head");
        entry.generic_roles = file
            .child(head, NodeKind::TypeArgs)
            .into_iter()
            .flat_map(|section| &file.nodes[section].children)
            .filter_map(|&arg| {
                context
                    .locals
                    .get(file.nodes[arg].path().as_str())?
                    .iter()
                    .find(|local| local.owner == node)
                    .map(|local| GenericBinder {
                        kind: local.kind,
                        takes_type: local.type_binding,
                    })
            })
            .collect();
    }
    for (i, signature_owners) in owners.into_iter().enumerate() {
        entries[i].id.signature_owners = signature_owners;
        if let Some(owner) = entries[i].lexical_owner.and_then(|n| {
            contexts[entries[i].source.expect("source declaration").0]
                .entries
                .get(&n)
        }) {
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
            let (file_number, node) = entry.source.expect("source declaration");
            let file = &files[file_number];
            let name = file.child(node, NodeKind::Path).unwrap_or(node);
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
    for entry in entries[..source_count].iter().filter(|e| callable(e.id.kind)) {
        let (file_number, node) = entry.source.expect("source declaration");
        let file = &files[file_number];
        if let Some(head) = file.child(node, NodeKind::Path)
            && let Some((owner, _)) = file.nodes[head].path().rsplit_once("::")
        {
            let owners = catalog.lookup(file, file_number, &contexts[file_number], head, owner);
            if owners.iter().any(|&i| entries[i].id.kind == NodeKind::Data) {
                return Err(file.error(file.span(head), "a data carrier cannot own a machine"));
            }
        }
    }
    for (slot, entry) in entries[..source_count]
        .iter()
        .enumerate()
        .filter(|(_, e)| e.id.kind == NodeKind::Domain)
    {
        let (file_number, node) = entry.source.expect("source declaration");
        let file = &files[file_number];
        let context = &contexts[file_number];
        let Some(head) = file.child(node, NodeKind::PathType) else { continue };
        let name = file.nodes[head].path();
        let Some((carrier, _)) = name.rsplit_once("::") else { continue };
        let key = if builtin(carrier) {
            Carrier::Builtin(carrier.into())
        } else if context
            .locals
            .get(carrier)
            .into_iter()
            .flatten()
            .any(|l| l.owner == node && l.kind == NodeKind::TypeParam)
        {
            Carrier::Generic
        } else {
            let slots = catalog.lookup(file, file_number, context, head, carrier);
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
            .map(|e| match e.source {
                Some((file, node)) => DefinitionSite {
                    file: files[file].path.into(),
                    span: files[file].span(node),
                },
                None => DefinitionSite {
                    file: e.id.path.clone(),
                    span: SourceSpan::new(0, 0),
                },
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
