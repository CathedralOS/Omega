//! Operator homes: a top-level operator moves into its exact domain's
//! operator family before symbols are assigned.
//!
//! Selection follows module name law. A domain declared in the operator's own
//! module outranks same-spelled foreign declarations, qualified paths select
//! exactly, and relative spellings reach a foreign domain only through an
//! exposing import.

use std::collections::HashMap;

use arena::{Arena, HandleSpan, OrderedRootArena};
use diagnostics::Diagnostic;
use source::SourceId;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::domain::DomainDefinition;

use crate::selection::signature_free_requirements::same_semantic_name;
use crate::symbols::NamespaceDeclarations;

/// Logical module/import custody available before the final symbol table is
/// assigned. Fresh sources answer from the collected namespace declarations;
/// retained base sources (seeded extension) answer through the base table
/// already installed on the program.
struct NamespaceScope {
    source_modules: HashMap<SourceId, String>,
    source_imports: HashMap<SourceId, Vec<String>>,
}

impl NamespaceScope {
    fn collect(namespaces: &NamespaceDeclarations) -> Self {
        let mut source_modules = HashMap::new();
        for path in &namespaces.modules {
            let Some(first) = path.first() else {
                continue;
            };
            source_modules.insert(
                first.source_span().source_id,
                path.iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            );
        }
        let mut source_imports: HashMap<SourceId, Vec<String>> = HashMap::new();
        for path in &namespaces.imports {
            let Some(first) = path.first() else {
                continue;
            };
            source_imports
                .entry(first.source_span().source_id)
                .or_default()
                .push(
                    path.iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::"),
                );
        }
        Self {
            source_modules,
            source_imports,
        }
    }

    /// The declaring module's logical path for one source, or "" for
    /// unmoduled scope. New sources come from the collected declarations;
    /// base sources resolve through the retained table.
    fn module_of_source(&self, program: &SymbolResolvedTrees, source: SourceId) -> String {
        if let Some(module) = self.source_modules.get(&source) {
            return module.clone();
        }
        let module = program.symbols.source_module(source);
        if module.is_valid() {
            program.symbols.display_path(module, "::")
        } else {
            String::new()
        }
    }

    fn imports_of_source(&self, program: &SymbolResolvedTrees, source: SourceId) -> Vec<String> {
        let mut imports = self
            .source_imports
            .get(&source)
            .cloned()
            .unwrap_or_default();
        imports.extend(
            program
                .symbols
                .source_module_import_paths(source)
                .map(str::to_owned),
        );
        imports
    }

    fn domain_module(&self, program: &SymbolResolvedTrees, domain: &DomainDefinition) -> String {
        if domain.symbol.is_valid() {
            let module = program.symbols.symbol_module(domain.symbol);
            return if module.is_valid() {
                program.symbols.display_path(module, "::")
            } else {
                String::new()
            };
        }
        self.module_of_source(program, domain.name.source_span().source_id)
    }

    /// The domain's complete logical path: module prefix plus its declared
    /// carrier-qualified name. Retained symbols answer exactly; unsymbolled
    /// declarations compose the path from their owning module.
    fn domain_path(&self, program: &SymbolResolvedTrees, domain: &DomainDefinition) -> String {
        if domain.symbol.is_valid() {
            return program.symbols.display_path(domain.symbol, "::");
        }
        let module = self.domain_module(program, domain);
        let local = domain.name.as_str();
        if module.is_empty() {
            local.to_owned()
        } else {
            format!("{module}::{local}")
        }
    }

    /// A relative spelling (the domain's declared name or its leaf) reaches a
    /// module-owned domain only inside its own module or through a narrow
    /// import of the exact declaration, which exposes the leaf spelling just
    /// like ordinary name resolution. Fully qualified spellings always
    /// select exactly; unmoduled domains keep root scope.
    fn domain_exposed_to(
        &self,
        program: &SymbolResolvedTrees,
        domain_module: &str,
        domain_path: &str,
        authored: &str,
        reference_source: SourceId,
    ) -> bool {
        if authored == domain_path || domain_module.is_empty() {
            return true;
        }
        let reference_module = self.module_of_source(program, reference_source);
        if !reference_module.is_empty() && reference_module == domain_module {
            return true;
        }
        !authored.contains("::")
            && self
                .imports_of_source(program, reference_source)
                .iter()
                .any(|import| import == domain_path)
    }
}

/// Move an ordinary top-level operator into its exact domain's semantic
/// operator family before symbols are assigned. The home is supplied either
/// by a qualified declaration name (`operator Quantity::Additive::add ...`)
/// or by one unique declared-domain constraint across the operand tuple
/// (`operator add(left: i32::Degrees, ...)`).
///
/// Selection follows module name law rather than declared spelling alone:
/// a domain declared in the operator's own module outranks same-spelled
/// foreign declarations, fully qualified paths select exactly, and relative
/// spellings reach a foreign module's domain only through an exposing import.
///
/// The declaration remains an ordinary root item in source. This one lowering
/// point owns the semantic association; domain bodies are reserved for exact
/// establishment requirements and no checked consumer reconstructs ownership
/// from an operator name later.
pub(crate) fn normalize_domain_operator_homes(
    program: &mut SymbolResolvedTrees,
    namespaces: &NamespaceDeclarations,
) -> Result<(), Diagnostic> {
    let scope = NamespaceScope::collect(namespaces);
    let domain_names = program
        .domain_definitions
        .iter()
        .map(|domain| domain.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let domain_modules = program
        .domain_definitions
        .iter()
        .map(|domain| scope.domain_module(program, domain))
        .collect::<Vec<_>>();
    let domain_paths = program
        .domain_definitions
        .iter()
        .map(|domain| scope.domain_path(program, domain))
        .collect::<Vec<_>>();
    let mut operators_by_domain = program
        .domain_definitions
        .iter()
        .map(|domain| program.operator_definitions(domain.operators).to_vec())
        .collect::<Vec<_>>();

    let authored_roots = std::mem::take(&mut program.operators);
    let mut remaining_roots = OrderedRootArena::new();

    for operator in &authored_roots {
        let path = program.operator_path_members(operator.name).to_vec();
        let Some((leaf, owner_path)) = path.split_last() else {
            remaining_roots.push(operator.clone());
            continue;
        };
        let reference_source = path
            .first()
            .map(|member| member.source_span().source_id)
            .unwrap_or_default();
        let reference_module = scope.module_of_source(program, reference_source);
        let explicit_owner = owner_path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let explicit_matches = explicit_domain_homes(
            program,
            &scope,
            &explicit_owner,
            &domain_names,
            &domain_modules,
            &domain_paths,
            reference_source,
            &reference_module,
        );
        let inferred_matches = inferred_domain_homes(
            program,
            &scope,
            operator,
            &domain_names,
            &domain_modules,
            &domain_paths,
        );
        let matches = if explicit_matches.is_empty() {
            inferred_matches
        } else if inferred_matches.is_empty() {
            explicit_matches
        } else {
            explicit_matches
                .into_iter()
                .filter(|candidate| inferred_matches.contains(candidate))
                .collect()
        };

        let [domain_index] = matches.as_slice() else {
            if matches.is_empty() {
                if !inferred_domain_homes(
                    program,
                    &scope,
                    operator,
                    &domain_names,
                    &domain_modules,
                    &domain_paths,
                )
                .is_empty()
                {
                    return Err(Diagnostic::error(format!(
                        "operator `{}` names a domain home that conflicts with its operand domains",
                        operator_label(program, operator)
                    )));
                }
                remaining_roots.push(operator.clone());
                continue;
            }
            return Err(Diagnostic::error(format!(
                "operator `{}` has more than one possible domain home; use an exact \
                 `operator Type::Domain::operation ...` declaration",
                operator_label(program, operator)
            )));
        };

        let leaf = program
            .tables
            .declarations
            .operator_path_members
            .append(leaf.clone());
        let mut operator = operator.clone();
        operator.name = HandleSpan::from_parts(leaf, 1);
        operators_by_domain[*domain_index].push(operator);
    }

    let mut rebuilt = Arena::new();
    let mut domain_index = 0usize;
    program.domain_definitions.for_each_mut(|domain| {
        let mut operators = HandleSpan::empty();
        for operator in operators_by_domain[domain_index].drain(..) {
            rebuilt.append_to_span(&mut operators, operator);
        }
        domain.operators = operators;
        if !operators.is_empty() {
            domain.semantic_roles.denotation_dimension = Some(domain.semantic_id);
        }
        domain_index += 1;
    });

    program.tables.declarations.operator_definitions = rebuilt;
    program.operators = remaining_roots;
    Ok(())
}

/// The explicit `Owner::` prefix of an operator name selects its home.
/// Module-local declarations bind tighter than imported or absolute matches;
/// each narrower set must still be unique.
fn explicit_domain_homes(
    program: &SymbolResolvedTrees,
    scope: &NamespaceScope,
    explicit_owner: &str,
    domain_names: &[String],
    domain_modules: &[String],
    domain_paths: &[String],
    reference_source: SourceId,
    reference_module: &str,
) -> Vec<usize> {
    if explicit_owner.is_empty() {
        return Vec::new();
    }
    // Same-module relative spellings bind tighter than any foreign or
    // unmoduled candidate.
    let local = (0..domain_names.len())
        .filter(|index| {
            !reference_module.is_empty()
                && domain_modules[*index] == reference_module
                && domain_names[*index].as_str() == explicit_owner
        })
        .collect::<Vec<_>>();
    if !local.is_empty() {
        return local;
    }
    let imports = scope.imports_of_source(program, reference_source);
    // A narrow import of the exact declaration exposes its leaf spelling,
    // matching ordinary name resolution.
    let imported = (0..domain_names.len())
        .filter(|index| {
            !explicit_owner.contains("::")
                && domain_names[*index].as_str() == explicit_owner
                && imports.iter().any(|import| import == &domain_paths[*index])
        })
        .collect::<Vec<_>>();
    if !imported.is_empty() {
        return imported;
    }
    (0..domain_names.len())
        .filter(|index| domain_paths[*index].as_str() == explicit_owner)
        .collect()
}

fn inferred_domain_homes(
    program: &SymbolResolvedTrees,
    scope: &NamespaceScope,
    operator: &symbol_resolved_trees::operator::OperatorDefinition,
    domain_names: &[String],
    domain_modules: &[String],
    domain_paths: &[String],
) -> Vec<usize> {
    let mut matches = Vec::new();
    for parameter in program.state_parameters(operator.parameters) {
        collect_type_domain_homes(
            program,
            scope,
            &parameter.type_reference,
            domain_names,
            domain_modules,
            domain_paths,
            &mut matches,
        );
    }
    matches
}

fn collect_type_domain_homes(
    program: &SymbolResolvedTrees,
    scope: &NamespaceScope,
    type_reference: &symbol_resolved_trees::types::TypeReference,
    domain_names: &[String],
    domain_modules: &[String],
    domain_paths: &[String],
    matches: &mut Vec<usize>,
) {
    use symbol_resolved_trees::types::{TypeConstraint, TypeReference};

    match type_reference {
        TypeReference::Reference(reference) => collect_type_domain_homes(
            program,
            scope,
            program.child_type_reference(reference.referee),
            domain_names,
            domain_modules,
            domain_paths,
            matches,
        ),
        TypeReference::Constrained(constrained) => {
            let carrier = program.child_type_reference(constrained.base_type);
            for constraint in program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
            {
                let TypeConstraint::Domain(authored) = constraint else {
                    continue;
                };
                let reference_source = authored.name.source_span().source_id;
                let reference_module = scope.module_of_source(program, reference_source);
                let mut constraint_matches = Vec::new();
                for (index, domain) in program.domain_definitions.iter().enumerate() {
                    let local = &domain_names[index];
                    let qualified = &domain_paths[index];
                    let name_matches = qualified.as_str() == authored.name.as_str()
                        || same_semantic_name(local.as_str(), authored.name.as_str());
                    if name_matches
                        && scope.domain_exposed_to(
                            program,
                            &domain_modules[index],
                            qualified,
                            authored.name.as_str(),
                            reference_source,
                        )
                        && domain_accepts_carrier(
                            program,
                            domain,
                            carrier,
                            authored.arguments.len(),
                        )
                    {
                        constraint_matches.push(index);
                    }
                }
                // A domain declared in the constraint's own module outranks
                // same-spelled foreign candidates, just like a local binding.
                let local_matches = constraint_matches
                    .iter()
                    .copied()
                    .filter(|index| {
                        !reference_module.is_empty() && domain_modules[*index] == reference_module
                    })
                    .collect::<Vec<_>>();
                for index in if local_matches.is_empty() {
                    constraint_matches
                } else {
                    local_matches
                } {
                    if !matches.contains(&index) {
                        matches.push(index);
                    }
                }
            }
            collect_type_domain_homes(
                program,
                scope,
                carrier,
                domain_names,
                domain_modules,
                domain_paths,
                matches,
            );
        }
        TypeReference::FixedArray(_)
        | TypeReference::Slice(_)
        | TypeReference::Generic(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::Named { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => {}
    }
}

fn domain_accepts_carrier(
    program: &SymbolResolvedTrees,
    domain: &symbol_resolved_trees::domain::DomainDefinition,
    carrier: &symbol_resolved_trees::types::TypeReference,
    argument_count: usize,
) -> bool {
    let parameters = program.data_type_parameters(domain.type_parameters);
    if parameters.is_empty() {
        return argument_count == 0 && type_references_match(program, carrier, &domain.target_type);
    }
    let Some(parameter) = parameters.first() else {
        return false;
    };
    if !matches!(
        parameter.kind,
        symbol_resolved_trees::data::TypeParameterKind::Type
    ) || argument_count != parameters.len().saturating_sub(1)
    {
        return false;
    }
    matches!(
        &domain.target_type,
        symbol_resolved_trees::types::TypeReference::Named { name, .. }
            if name.as_str() == parameter.name.as_str()
    )
}

fn type_references_match(
    program: &SymbolResolvedTrees,
    left: &symbol_resolved_trees::types::TypeReference,
    right: &symbol_resolved_trees::types::TypeReference,
) -> bool {
    use symbol_resolved_trees::types::TypeReference;

    match (left, right) {
        (TypeReference::Constrained(left), _) => {
            type_references_match(program, program.child_type_reference(left.base_type), right)
        }
        (_, TypeReference::Constrained(right)) => {
            type_references_match(program, left, program.child_type_reference(right.base_type))
        }
        (TypeReference::Reference(left), TypeReference::Reference(right)) => {
            left.access == right.access
                && type_references_match(
                    program,
                    program.child_type_reference(left.referee),
                    program.child_type_reference(right.referee),
                )
        }
        (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
            left.length == right.length
                && type_references_match(
                    program,
                    program.child_type_reference(left.element_type),
                    program.child_type_reference(right.element_type),
                )
        }
        (TypeReference::Slice(left), TypeReference::Slice(right)) => type_references_match(
            program,
            program.child_type_reference(left.element_type),
            program.child_type_reference(right.element_type),
        ),
        (TypeReference::Generic(left), TypeReference::Generic(right)) => {
            left.base_name == right.base_name
                && left.lifetime_arguments == right.lifetime_arguments
                && program
                    .child_type_references(left.arguments)
                    .iter()
                    .zip(program.child_type_references(right.arguments))
                    .all(|(left, right)| type_references_match(program, left, right))
                && left.arguments.len() == right.arguments.len()
        }
        (
            TypeReference::DynamicTrait { name: left, .. },
            TypeReference::DynamicTrait { name: right, .. },
        )
        | (TypeReference::Named { name: left, .. }, TypeReference::Named { name: right, .. }) => {
            left == right
        }
        (TypeReference::SelfType { symbol: left }, TypeReference::SelfType { symbol: right }) => {
            left == right
        }
        (TypeReference::Unit, TypeReference::Unit) => true,
        _ => false,
    }
}

fn operator_label(
    program: &SymbolResolvedTrees,
    operator: &symbol_resolved_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
