//! Collect declarations, admit file-local imports, then bind occurrences against one settled stage.
mod declarations;
mod lookup;
mod machine_headers;
mod occurrences;

use crate::syntax::File;
use crate::{AssemblyVocabulary, Dependency, StageSource};
use declarations::index_file;
use omega_resolved::{
    CarrierVisibility, CompiledInterface, DeclarationId, DeclarationName, DomainCandidate, DomainCarrier, InterfaceExport, NameContext, NameError, NameErrorKind,
    NameRole, NameUse, OperatorDeclaration, ResolutionIndex, ResolvedStage, Symbol, SymbolResolvedTrees, TreeBindings, is_builtin_type_name as builtin,
};
use omega_syntax::NodeKind;
use omega_tokens::SourceSpan;
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn qualify(
    owner: &str,
    name: &str,
) -> String {
    if owner.is_empty() { name.into() } else { format!("{owner}::{name}") }
}
fn module_child(
    module: &str,
    name: &str,
) -> String {
    if module.is_empty() { name.into() } else { format!("{module}\\{name}") }
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

fn valid_domain_attachment(export: &InterfaceExport, local_path: &str) -> bool {
    let applied = !export.id.domain_carrier_arguments.is_empty();
    let Some((written, _)) = local_path.rsplit_once("::").filter(|_| export.id.kind == NodeKind::Domain) else {
        return !applied && export.domain_carrier.is_none();
    };
    if builtin(written) {
        return matches!(&export.domain_carrier, Some(DomainCarrier::Builtin(name)) if name == written);
    }
    if applied {
        return false; // Only builtin carrier applications contribute arguments to the identity.
    }
    let bare = !written.contains(['\\', ':']);
    let owner = match &export.domain_carrier {
        Some(DomainCarrier::Generic) => return bare,
        Some(DomainCarrier::Bound { alternative, .. }) if bare => alternative,
        Some(DomainCarrier::Named(owner)) => owner,
        _ => return false,
    };
    // Definition-site lookup can change the package and containing path, but
    // imports expose declarations under their final segment, never a new leaf.
    matches!(owner.kind, NodeKind::Data | NodeKind::Domain)
        && owner.path.rsplit(['\\', ':']).next() == written.rsplit(['\\', ':']).next()
}

fn in_subject_predicate(
    file: &File<'_>,
    node: usize,
) -> bool {
    use NodeKind::*;
    for ancestor in file.ancestors(node) {
        let syntax = &file.nodes[ancestor];
        match syntax.kind {
            Requires => {
                return syntax
                    .parent
                    .is_some_and(|owner| matches!(file.nodes[owner].kind, Data | Domain));
            }
            Data | Domain | Machine | AsmMachine | MathLet | MachineType | FunctionType | Hypothesis => return false,
            _ => {}
        }
    }
    false
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
    shadowed_generics: Vec<NameUse>,
    domain_indices: HashMap<usize, Symbol>,
    signature_names: Vec<(usize, usize)>,
    patterns: BTreeMap<usize, SourceSpan>,
    imports: Vec<Import>,
    by_alias: BTreeMap<String, Vec<usize>>,
    open_modules: BTreeSet<(String, String)>,
    domains: HashMap<String, BTreeMap<usize, Option<SourceSpan>>>,
}

/// Resolve names against the settled source files and direct package aliases for one stage.
pub fn resolve_stage<'text>(
    sources: &[StageSource<'text, '_>],
    dependencies: &[Dependency<'_>],
) -> Result<SymbolResolvedTrees<'text>, NameError> {
    resolve_stage_for_target(sources, dependencies, &AssemblyVocabulary::default())
}

pub fn resolve_stage_for_target<'text>(
    sources: &[StageSource<'text, '_>],
    dependencies: &[Dependency<'_>],
    assembly: &AssemblyVocabulary<'_>,
) -> Result<SymbolResolvedTrees<'text>, NameError> {
    resolve_stage_with_exports(sources, dependencies, assembly, &CompiledInterface::default())
}

pub fn resolve_stage_with_exports<'text>(
    sources: &[StageSource<'text, '_>],
    dependencies: &[Dependency<'_>],
    assembly: &AssemblyVocabulary<'_>,
    interface: &CompiledInterface,
) -> Result<SymbolResolvedTrees<'text>, NameError> {
    let mut ordered_sources: Vec<_> = sources.iter().collect();
    ordered_sources.sort_by_key(|source| (source.package, source.source.logical_path));
    let files: Vec<_> = ordered_sources
        .into_iter()
        .enumerate()
        .map(|(ordinal, input)| super::syntax::index_source(ordinal, input))
        .collect::<Result<_, _>>()?;
    let mut entries = Vec::new();
    let mut index = Index::new();
    let mut contexts = Vec::new();
    let mut modules = BTreeSet::new();
    for (i, file) in files.iter().enumerate() {
        if i > 0 && (file.package, file.path) == (files[i - 1].package, files[i - 1].path) {
            return Err(file.error(NameErrorKind::DuplicateLogicalFile, SourceSpan::new(0, 0), "duplicate logical file"));
        }
        let mut parts: Vec<_> = file.path.split('/').collect();
        if parts
            .pop()
            .is_none_or(|p| p.is_empty() || matches!(p, "." | "..") || p.contains(['\\', ':']))
            || parts.iter().any(|p| !identifier(p) || builtin(p))
        {
            return Err(file.error(
                NameErrorKind::InvalidLogicalPath,
                SourceSpan::new(0, 0),
                format!("invalid package-relative logical path {}", file.path),
            ));
        }
        for depth in 0..=parts.len() {
            modules.insert((file.package.to_owned(), parts[..depth].join("\\")));
        }
        contexts.push(index_file(file, &mut entries, &mut index)?);
    }
    let source_count = entries.len();
    let mut exports: Vec<_> = interface.declarations.iter().collect();
    exports.sort_by_key(|export| (&export.id, &export.module));
    for export in &exports {
        let id = &export.id;
        let error = |kind, message: &str| NameError::new(kind, &id.package, &id.path, SourceSpan::new(0, 0), message);
        if files
            .binary_search_by(|f| f.package.cmp(id.package.as_str()))
            .is_ok()
        {
            return Err(error(NameErrorKind::CompiledPackageCollision, "source package collides with a compiled interface"));
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
            return Err(error(NameErrorKind::UnsupportedCompiledDeclaration, "unsupported declaration in compiled interface"));
        }
        if id.enclosing_machine.is_some() {
            return Err(error(NameErrorKind::NestedCompiledExport, "compiled export names a nested machine or local declaration"));
        }
        if !interface
            .modules
            .contains(&(id.package.clone(), export.module.clone()))
        {
            return Err(error(NameErrorKind::CompiledExportWithoutModule, "compiled export has no defining module"));
        }
        let local_path = if export.module.is_empty() {
            id.path.as_str()
        } else {
            id.path
                .strip_prefix(&export.module)
                .and_then(|rest| rest.strip_prefix('\\'))
                .ok_or_else(|| error(NameErrorKind::CompiledExportOutsideModule, "compiled export is outside its defining module"))?
        };
        if !valid_domain_attachment(export, local_path) {
            return Err(error(
                NameErrorKind::CompiledDomainAttachmentMismatch,
                "compiled domain attachment does not match its declaration",
            ));
        }
        let slots = index.entry((id.package.clone(), id.path.clone())).or_default();
        if slots.iter().any(|&slot| entries[slot].id == *id) {
            return Err(error(NameErrorKind::DuplicateCompiledDeclaration, "duplicate declaration in compiled interface"));
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
                NameErrorKind::CompiledPackageCollision,
                package,
                path,
                SourceSpan::new(0, 0),
                "source package collides with a compiled interface",
            ));
        }
        if !path.is_empty() && path.split('\\').any(|part| !identifier(part) || builtin(part)) {
            return Err(NameError::new(
                NameErrorKind::InvalidCompiledModulePath,
                package,
                path,
                SourceSpan::new(0, 0),
                "invalid compiled module path",
            ));
        }
        let mut module = path.as_str();
        loop {
            modules.insert((package.clone(), module.to_owned()));
            if module.is_empty() {
                break;
            }
            module = module.rsplit_once('\\').map_or("", |(parent, _)| parent);
        }
    }
    let mut edges = BTreeMap::new();
    for edge in dependencies {
        if edges
            .insert((edge.requester, edge.alias), edge.package)
            .is_some_and(|p| p != edge.package)
        {
            return Err(NameError::new(
                NameErrorKind::ConflictingDependencyAlias,
                edge.requester,
                "",
                SourceSpan::new(0, 0),
                format!("conflicting packages for dependency alias {}", edge.alias),
            ));
        }
    }
    for (package, path) in &interface.declaration_queries {
        let error = |kind, message| NameError::new(kind, package, path, SourceSpan::new(0, 0), message);
        if files.binary_search_by(|f| f.package.cmp(package)).is_ok() {
            return Err(error(
                NameErrorKind::CompiledPackageCollision,
                "source package collides with a compiled interface",
            ));
        }
        let module = path.rsplit_once('\\').map_or("", |(owner, _)| owner);
        if !interface.modules.contains(&(package.clone(), module.into())) {
            return Err(error(NameErrorKind::CompiledExportWithoutModule, "compiled export has no defining module"));
        }
        if index.contains_key(&(package.clone(), path.clone())) {
            return Err(error(
                NameErrorKind::DuplicateCompiledDeclaration,
                "duplicate declaration in compiled interface",
            ));
        }
    }
    let mut catalog = Catalog {
        declaration_queries: &interface.declaration_queries,
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
    }
    let mut bindings: Vec<_> = files.iter().map(|_| TreeBindings::default()).collect();
    let mut operator_declarations = Vec::new();
    let mut carrier_visibility = Vec::new();
    let mut nested_carriers = Vec::new();
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
                return Err(file.error(NameErrorKind::AmbiguousCarrier, file.span(head), format!("ambiguous carrier {carrier}")));
            }
            if let Some(owner) = first
                && (entries[owner].lexical_owner.is_some() || entries[owner].id.kind == NodeKind::Domain)
            {
                nested_carriers.push((slot, owner));
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
                (None, None) => return Err(context.unresolved_name(file, file.span(head), carrier)),
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
        catalog.index_visible_domains(file, ctx);
    }
    // Resolve binder occurrences while the catalog still addresses declarations by slot.
    // Imports and body references receive the completed overload identities below.
    let mut signatures = Vec::new();
    let mut signature_error = None;
    for (slot, entry) in catalog.entries[..source_count].iter().enumerate() {
        let (file_number, node) = entry.source.expect("source declaration");
        let file = &files[file_number];
        let (operands, result) = if callable(entry.id.kind) {
            let params = file.parameters(node).map(|(_, ty)| ty).collect::<Vec<_>>();
            let result = file
                .result_annotation(node)
                .filter(|&ty| file.nodes[ty].kind == NodeKind::QualifiedType && file.child(node, NodeKind::OperatorToken).is_none());
            (params, result)
        } else if !entry.id.domain_carrier_arguments.is_empty() {
            let head = file
                .child(node, NodeKind::PathType)
                .expect("attached domain head");
            let sections = declarations::builtin_carrier_sections(file, head).collect();
            (sections, None)
        } else {
            continue;
        };
        match catalog.resolve_signature(file, &contexts[file_number], node, &operands, result) {
            Ok((operands, signature)) => signatures.push((slot, operands, signature)),
            Err(error) if entry.id.kind == NodeKind::Domain => return Err(error),
            Err(error) => {
                signature_error.get_or_insert(error);
            }
        }
    }
    let entries = &mut catalog.entries;
    for (slot, operands, signature) in signatures {
        let id = &mut entries[slot].id;
        let name = std::sync::Arc::make_mut(&mut id.name);
        if id.kind == NodeKind::Domain {
            name.domain_carrier_arguments = signature.operands.into_boxed_slice();
        } else {
            name.operands = operands;
            name.resolved_signature = Some(signature);
        }
    }
    for i in 0..source_count {
        if let Some(owner) = entries[i].lexical_owner {
            let (file_number, _) = entries[i].source.expect("source declaration");
            let enclosing = entries[contexts[file_number].entries[&owner]].id.clone();
            std::sync::Arc::make_mut(&mut entries[i].id.name).enclosing_machine = Some(enclosing);
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
            if !entry.id.domain_carrier_arguments.is_empty() {
                let head = file
                    .child(node, NodeKind::PathType)
                    .expect("attached domain head");
                let member = file.nodes[head]
                    .words()
                    .next_back()
                    .expect("domain member")
                    .text;
                return Err(file.error(
                    NameErrorKind::DuplicateDeclaration,
                    file.span(head),
                    format!("domain {member} is declared twice on this carrier"),
                ));
            }
            let name = file.child(node, NodeKind::Path).unwrap_or(node);
            return Err(file.error(
                NameErrorKind::DuplicateDeclaration,
                file.span(name),
                format!("duplicate declaration {}", entry.id.path),
            ));
        }
    }
    // Attachments retain their owner's final identity after binding its enclosing
    // machine and any arguments on its own domain carrier.
    for (domain, owner) in nested_carriers {
        match catalog
            .carriers
            .get_mut(&domain)
            .expect("indexed domain carrier")
        {
            DomainCarrier::Named(id) | DomainCarrier::Bound { alternative: id, .. } => *id = entries[owner].id.clone(),
            _ => unreachable!("only named carriers refer to a declaration slot"),
        }
    }
    catalog.check_member_names(&exports, source_count)?;
    for (file, ctx) in files.iter().zip(&contexts) {
        catalog.bind_imports(file, ctx, &mut bindings[file.ordinal()].uses, &mut carrier_visibility)?;
    }
    for entry in catalog.entries[..source_count]
        .iter()
        .filter(|e| callable(e.id.kind))
    {
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
    for ((file, mut context), tree) in files.iter().zip(contexts).zip(&mut bindings) {
        tree.uses.append(&mut context.shadowed_generics);
        catalog.bind_nodes(file, &context, 0..file.nodes.len(), tree, &mut carrier_visibility)?;
        tree.definitions = context.entries;
        tree.patterns = context.patterns;
    }
    // Whole-file binding reports an invalid program in source order. Never
    // publish a provisional signature if its independent name lookup failed.
    if let Some(error) = signature_error {
        return Err(error);
    }
    let trait_parents = catalog.trait_parents;
    let mut direct_dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for (&(requester, _), &package) in &edges {
        direct_dependencies
            .entry(requester.into())
            .or_default()
            .insert(package.into());
    }
    let declarations = catalog
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
        .collect();
    let index = ResolutionIndex {
        direct_dependencies,
        trait_parents,
        direct_trait_parents,
        domain_carriers: catalog.carriers,
    };
    let resolution = ResolvedStage::new(index, declarations, carrier_visibility, operator_declarations);
    Ok(SymbolResolvedTrees::new(files.into_iter().zip(bindings).collect(), resolution))
}
struct Catalog<'a> {
    declaration_queries: &'a BTreeSet<(String, String)>,
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
