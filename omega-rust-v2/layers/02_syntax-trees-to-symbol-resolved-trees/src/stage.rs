//! Collect declarations, admit file-local imports, then bind occurrences against one settled stage.
mod declarations;
mod lookup;
mod machine_headers;
mod model;
mod occurrences;

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
pub fn builtin(name: &str) -> bool {
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
    source: Option<(usize, usize)>,
    public: bool,
    lexical_owner: Option<usize>,
}
type Index = BTreeMap<(String, String), Vec<usize>>;
struct Local {
    node: usize,
    kind: NodeKind,
    span: SourceSpan,
    owner: usize,
    start: usize,
    end: usize,
    static_binding: bool,
}
impl Local {
    fn symbol(&self) -> Symbol {
        Symbol::Binding {
            binder_node: self.node,
            span: self.span,
            kind: self.kind,
        }
    }
}
struct Import {
    node: usize,
    address: (String, String),
    slots: Vec<usize>,
}

#[derive(Default)]
struct Context<'a> {
    heads: BTreeSet<usize>,
    entries: BTreeMap<usize, usize>,
    locals: HashMap<&'a str, Vec<Local>>,
    domain_indices: HashMap<usize, Symbol>,
    signature_names: Vec<(usize, usize)>,
    imports: Vec<Import>,
    by_alias: BTreeMap<String, Vec<usize>>,
    open_modules: BTreeSet<(String, String)>,
    domains: HashMap<String, BTreeMap<usize, Option<SourceSpan>>>,
}

/// Resolve names against the settled source files and direct package aliases for one stage.
pub fn resolve_stage<'text>(
    sources: &[StageSource<'text, '_>],
    dependencies: &[Dependency<'_>],
) -> Result<crate::SymbolResolvedTrees<'text>, NameError> {
    resolve_stage_for_target(sources, dependencies, &AssemblyVocabulary::default())
}

pub fn resolve_stage_for_target<'text>(
    sources: &[StageSource<'text, '_>],
    dependencies: &[Dependency<'_>],
    assembly: &AssemblyVocabulary<'_>,
) -> Result<crate::SymbolResolvedTrees<'text>, NameError> {
    resolve_stage_with_exports(sources, dependencies, assembly, &CompiledInterface::default())
}

/// Names bound in the defining package retain their declaration identities across admission.
pub struct InterfaceExport {
    pub id: DeclarationId,
    pub module: String,
    pub trait_parents: Vec<DeclarationId>,
    pub domain_carrier: Option<DomainCarrier>,
}

/// The namespaces and the public declarations supplied by compiled source packages.
/// A directory remains importable when none of its declarations are public.
#[derive(Default)]
pub struct CompiledInterface {
    pub modules: BTreeSet<(String, String)>,
    pub declarations: Vec<InterfaceExport>,
}

pub fn resolve_stage_with_exports<'text>(
    sources: &[StageSource<'text, '_>],
    dependencies: &[Dependency<'_>],
    assembly: &AssemblyVocabulary<'_>,
    interface: &CompiledInterface,
) -> Result<crate::SymbolResolvedTrees<'text>, NameError> {
    let mut ordered_sources: Vec<_> = sources.iter().collect();
    ordered_sources.sort_by_key(|source| (source.package, source.source.logical_path));
    let mut files: Vec<_> = ordered_sources
        .into_iter()
        .map(File::read)
        .collect::<Result<_, _>>()?;
    for (ordinal, file) in files.iter_mut().enumerate() {
        file.ordinal = ordinal;
    }
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
        contexts.push(index_file(file, &mut entries, &mut index)?);
    }
    let source_count = entries.len();
    let mut exports: Vec<_> = interface.declarations.iter().collect();
    exports.sort_by_key(|export| (&export.id, &export.module));
    for export in &exports {
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
            NodeKind::Data
                | NodeKind::Machine
                | NodeKind::AsmMachine
                | NodeKind::MathLet
                | NodeKind::Const
                | NodeKind::Domain
                | NodeKind::Conformance
                | NodeKind::Case
                | NodeKind::CaseField
                | NodeKind::Field
                | NodeKind::Trait
        ) {
            return Err(error("unsupported declaration in compiled interface"));
        }
        if id.enclosing_machine.is_some() {
            return Err(error("compiled export names a nested machine or local declaration"));
        }
        if !interface
            .modules
            .contains(&(id.package.clone(), export.module.clone()))
        {
            return Err(error("compiled export has no defining module"));
        }
        let local_path = if export.module.is_empty() {
            id.path.as_str()
        } else {
            id.path
                .strip_prefix(&export.module)
                .and_then(|rest| rest.strip_prefix("::"))
                .ok_or_else(|| error("compiled export is outside its defining module"))?
        };
        if export.domain_carrier.is_some() != (id.kind == NodeKind::Domain && local_path.contains("::")) {
            return Err(error("compiled domain attachment does not match its declaration"));
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
        });
    }
    for (package, path) in &interface.modules {
        if files
            .binary_search_by(|file| file.package.cmp(package.as_str()))
            .is_ok()
        {
            return Err(NameError::new(
                package,
                path,
                SourceSpan::new(0, 0),
                "source package collides with a compiled interface",
            ));
        }
        if !path.is_empty() && path.split("::").any(|part| !identifier(part) || builtin(part)) {
            return Err(NameError::new(package, path, SourceSpan::new(0, 0), "invalid compiled module path"));
        }
        let mut module = path.as_str();
        loop {
            modules.insert((package.clone(), module.to_owned()));
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
    let mut catalog = Catalog {
        entries,
        index: &index,
        modules: &modules,
        edges: &edges,
        files: &files,
        carriers: HashMap::new(),
        domains_by_module: HashMap::new(),
        assembly,
        trait_parents: HashMap::new(),
    };
    // Resolve authored signature names in their declaring files before detecting
    // duplicates. Different direct imports may both introduce a type named Point.
    for (i, file) in files.iter().enumerate() {
        catalog.collect_imports(file, &mut contexts[i])?;
    }
    let direct_trait_parents = catalog.index_trait_parents(&contexts, &exports, source_count)?;
    // A declaration takes precedence over a same-named directory, including an inherited requirement.
    for (ctx, file) in contexts.iter_mut().zip(&files) {
        for import in &mut ctx.imports {
            catalog.inherited_members(file, &import.address, &mut import.slots);
            if !import.slots.is_empty() {
                ctx.open_modules.remove(&import.address);
            }
        }
    }
    let mut owners = vec![Vec::new(); source_count];
    for (file, ctx) in files.iter().zip(&contexts) {
        for &(declaration, node) in &ctx.signature_names {
            let name = file.nodes[node].path();
            // Only a definite type binder hides every nominal alternative before typing.
            if builtin(&name)
                || ctx
                    .carrier_binders(&name, node)
                    .any(|b| matches!(b.kind, NodeKind::TypeParam | NodeKind::SomeParam))
            {
                continue;
            }
            let mut slots = catalog.lookup(file, ctx, node, &name)?;
            slots.sort_unstable();
            slots.dedup();
            for slot in slots {
                let id = &catalog.entries[slot].id;
                if matches!(id.kind, NodeKind::Data | NodeKind::Trait | NodeKind::Domain) {
                    owners[declaration].push((id.package.clone(), id.path.clone()));
                }
            }
        }
    }
    let entries = &mut catalog.entries;
    for (i, signature_owners) in owners.into_iter().enumerate() {
        let enclosing = entries[i].lexical_owner.and_then(|node| {
            let (file, _) = entries[i].source.expect("source declaration");
            contexts[file]
                .entries
                .get(&node)
                .map(|&parent| entries[parent].id.clone())
        });
        let name = std::sync::Arc::make_mut(&mut entries[i].id.name);
        name.signature_owners = signature_owners;
        name.enclosing_machine = enclosing;
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
    let mut bindings: Vec<_> = files
        .iter()
        .map(|_| crate::resolved_trees::TreeBindings::default())
        .collect();
    let mut operator_declarations = Vec::new();
    let mut carrier_visibility = Vec::new();
    let entries = &catalog.entries;
    for (slot, entry) in entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.id.kind == NodeKind::Domain)
    {
        let Some((file_number, node)) = entry.source else {
            let export = &exports[slot - source_count];
            if let Some(carrier) = &export.domain_carrier {
                catalog.carriers.insert(slot, carrier.clone());
                catalog
                    .domains_by_module
                    .entry((entry.id.package.clone(), export.module.clone()))
                    .or_default()
                    .push(slot);
            }
            continue;
        };
        let file = &files[file_number];
        let context = &contexts[file_number];
        let Some(head) = file.child(node, NodeKind::PathType) else { continue };
        let name = file.nodes[head].path();
        let Some((carrier, _)) = name.rsplit_once("::") else { continue };
        let binder = context
            .carrier_binders(carrier, head)
            .find(|local| local.owner == node);
        let key = if builtin(carrier) {
            DomainCarrier::Builtin(carrier.into())
        } else if binder.is_some_and(|local| local.kind == NodeKind::TypeParam) {
            DomainCarrier::Generic
        } else {
            let slots = catalog.lookup(file, context, head, carrier)?;
            let mut owners = slots
                .into_iter()
                .filter(|&i| matches!(entries[i].id.kind, NodeKind::Data | NodeKind::Domain));
            let first = owners.next();
            if owners.any(|i| Some(i) != first) {
                return Err(file.error(file.span(head), format!("ambiguous carrier {carrier}")));
            }
            match (binder, first) {
                (Some(local), Some(owner)) => {
                    let params = file.nodes[local.node].parent.expect("domain parameter list");
                    let position = file.nodes[params]
                        .children
                        .iter()
                        .position(|&p| p == local.node)
                        .unwrap();
                    DomainCarrier::Bound {
                        position,
                        alternative: entries[owner].id.clone(),
                    }
                }
                (Some(_), None) => DomainCarrier::Generic,
                (None, Some(owner)) => DomainCarrier::Named(entries[owner].id.clone()),
                (None, None) => return Err(file.error(file.span(head), format!("unresolved name {carrier}"))),
            }
        };
        catalog.carriers.insert(slot, key);
        catalog
            .domains_by_module
            .entry((file.package.into(), file.namespace.clone()))
            .or_default()
            .push(slot);
    }
    for (file, ctx) in files.iter().zip(&mut contexts) {
        catalog.bind_imports(file, ctx, &mut bindings[file.ordinal].uses, &mut carrier_visibility)?;
    }
    for entry in entries[..source_count].iter().filter(|e| callable(e.id.kind)) {
        let (file_number, node) = entry.source.expect("source declaration");
        let file = &files[file_number];
        if let Some(binding) = catalog.machine_header(
            file,
            &contexts[file_number],
            entry,
            node,
            &mut bindings[file_number].uses,
            &mut carrier_visibility,
        )? {
            operator_declarations.push(binding);
        }
    }
    for ((file, context), tree) in files.iter().zip(contexts).zip(&mut bindings) {
        catalog.bind(file, &context, tree, &mut carrier_visibility)?;
        tree.definitions = context.entries;
    }
    let trait_parents = catalog.trait_parents;
    let mut direct_dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for (&(requester, _), &package) in &edges {
        direct_dependencies
            .entry(requester.into())
            .or_default()
            .insert(package.into());
    }
    let resolution = ResolvedStage {
        direct_dependencies,
        trait_parents,
        direct_trait_parents,
        domain_carriers: catalog.carriers,
        declarations: catalog
            .entries
            .into_iter()
            .enumerate()
            .map(|(slot, entry)| {
                if let Some((file, _)) = entry.source
                    && entry.public
                    && entry.lexical_owner.is_none()
                {
                    bindings[file].exports.push(slot);
                }
                entry.id
            })
            .collect(),
        operator_declarations,
        carrier_visibility,
    };
    Ok(crate::SymbolResolvedTrees::new(files, bindings, resolution))
}
struct Catalog<'a> {
    entries: Vec<Entry>,
    index: &'a Index,
    modules: &'a BTreeSet<(String, String)>,
    edges: &'a BTreeMap<(&'a str, &'a str), &'a str>,
    files: &'a [File<'a>],
    carriers: HashMap<usize, DomainCarrier>,
    domains_by_module: HashMap<(String, String), Vec<usize>>,
    trait_parents: HashMap<(String, String), BTreeSet<(String, String)>>,
    assembly: &'a AssemblyVocabulary<'a>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainCarrier {
    Builtin(String),
    Named(DeclarationId),
    /// A bound decides whether an annotated binder or its visible named alternative supplies the carrier.
    Bound {
        position: usize,
        alternative: DeclarationId,
    },
    Generic,
}
fn usage(
    file: &File<'_>,
    node: usize,
    role: NameRole,
    span: SourceSpan,
    target: Symbol,
) -> NameUse {
    NameUse {
        node: file.node_id(node),
        role,
        span,
        target,
    }
}
