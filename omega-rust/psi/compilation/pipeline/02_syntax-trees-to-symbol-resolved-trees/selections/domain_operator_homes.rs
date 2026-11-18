/*
    Operator homes: a top-level operator moves into its exact domain's
    operator family before symbols are assigned.

    Selection follows module name law. A domain declared in the operator's own
    module outranks same-spelled foreign declarations, qualified paths select
    exactly, and relative spellings reach a foreign domain only through an
    exposing import.
*/

use arena::{Arena, HandleSpan, OrderedRootArena};
use diagnostics::Diagnostic;
use source::SourceSpan;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::domain::DomainDefinition;

use crate::declarations::declaration_selection::ConstantSelection;
use std::mem::take;
use symbol_resolved_trees::data::TypeParameterKind;
use symbol_resolved_trees::operator::OperatorDefinition;
use symbol_resolved_trees::types::TypeReference;
use symbols::JoinedPath;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

/*
    Operator homes are selected before final symbol assignment. Rejoin each
    domain to its already prepared declaration header once, then use that
    table's ordinary import and visibility rules for every operand and owner.
    Header handles never become symbols in the resulting program.
*/
struct DomainHeaders<'a> {
    symbols: &'a SymbolTable,
    domains: Vec<SymbolHandle>,
}

impl<'a> DomainHeaders<'a> {
    fn new(program: &SymbolResolvedTrees, symbols: &'a SymbolTable) -> Result<Self, Diagnostic> {
        let domains = program
            .domain_definitions
            .iter()
            .map(|domain| {
                let declaration = domain.name.source_span();
                symbols
                    .lookup_top_level_by_name_and_kinds_from_source_matching(
                        domain.name.as_str(),
                        &[SymbolKind::Domain],
                        declaration,
                        |symbol| symbols.symbol_source_span(symbol) == Some(declaration),
                    )
                    .unique()
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "domain `{}` lost its exact prepared declaration header",
                            domain.name,
                        ))
                        .with_source_span(declaration)
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { symbols, domains })
    }

    fn candidates(
        &self,
        authored: &str,
        reference: SourceSpan,
        accepts: impl Fn(usize) -> bool,
    ) -> Vec<usize> {
        if authored.is_empty() {
            return Vec::new();
        }
        self.domains
            .iter()
            .enumerate()
            .filter_map(|(domain_position, symbol)| {
                (self
                    .symbols
                    .source_reference_can_see_symbol(reference, *symbol)
                    && self
                        .symbols
                        .domain_name_reaches(*symbol, authored, reference)
                    && accepts(domain_position))
                .then_some(domain_position)
            })
            .collect()
    }
}

/*
    Associate retained operator signatures with their exact domain before
    assigning final symbols. An explicit owner path or the operand domains
    selects the home through the prepared namespace; carrier checking and
    ambiguity remain obligations of this association. Ordinary body-bearing
    machine token bindings are associated after their symbols are assigned.
*/
pub(crate) fn normalize_domain_operator_homes(
    program: &mut SymbolResolvedTrees,
    selection: &ConstantSelection<'_>,
) -> Result<(), Diagnostic> {
    if program.operators.is_empty() {
        return Ok(());
    }
    let scope = DomainHeaders::new(program, selection.header_symbols())?;
    let mut operators_by_domain = program
        .domain_definitions
        .iter()
        .map(|domain| program.operator_definitions(domain.operators).to_vec())
        .collect::<Vec<_>>();

    let authored_roots = take(&mut program.operators);
    let mut remaining_roots = OrderedRootArena::new();

    for operator in &authored_roots {
        let path = program.operator_path_members(operator.name).to_vec();
        let Some((leaf, owner_path)) = path.split_last() else {
            remaining_roots.push(operator.clone());
            continue;
        };
        let reference = path
            .first()
            .map(|member| member.source_span())
            .unwrap_or_default();
        let explicit_owner = owner_path.joined_path();
        let explicit_matches = scope.candidates(&explicit_owner, reference, |_| true);
        let inferred_matches = inferred_domain_homes(program, &scope, operator);
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
                if !inferred_domain_homes(program, &scope, operator).is_empty() {
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

fn inferred_domain_homes(
    program: &SymbolResolvedTrees,
    scope: &DomainHeaders<'_>,
    operator: &OperatorDefinition,
) -> Vec<usize> {
    let mut matches = Vec::new();
    for parameter in program.state_parameters(operator.parameters) {
        collect_type_domain_homes(program, scope, &parameter.type_reference, &mut matches);
    }
    matches
}

fn collect_type_domain_homes(
    program: &SymbolResolvedTrees,
    scope: &DomainHeaders<'_>,
    type_reference: &TypeReference,
    matches: &mut Vec<usize>,
) {
    use symbol_resolved_trees::types::{TypeConstraint, TypeReference};

    match type_reference {
        TypeReference::AliasUse { expansion, .. } => {
            collect_type_domain_homes(program, scope, expansion, matches)
        }
        TypeReference::Reference(reference) => collect_type_domain_homes(
            program,
            scope,
            program.child_type_reference(reference.referee),
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
                for domain_position in scope.candidates(
                    authored.name.as_str(),
                    authored.name.source_span(),
                    |domain_position| {
                        domain_accepts_carrier(
                            program,
                            scope.symbols,
                            &program.domain_definitions[domain_position],
                            carrier,
                            authored.arguments.len(),
                        )
                    },
                ) {
                    if !matches.contains(&domain_position) {
                        matches.push(domain_position);
                    }
                }
            }
            collect_type_domain_homes(program, scope, carrier, matches);
        }
        super::leaf_carrier_type_reference_pattern!() => {}
    }
}

fn domain_accepts_carrier(
    program: &SymbolResolvedTrees,
    symbols: &SymbolTable,
    domain: &DomainDefinition,
    carrier: &TypeReference,
    argument_count: usize,
) -> bool {
    let parameters = program.data_type_parameters(domain.type_parameters);
    if parameters.is_empty() {
        return argument_count == 0
            && type_references_match(program, symbols, carrier, &domain.target_type);
    }
    let Some(parameter) = parameters.first() else {
        return false;
    };
    if !matches!(parameter.kind, TypeParameterKind::Type)
        || argument_count != parameters.len().saturating_sub(1)
    {
        return false;
    }
    matches!(
        &domain.target_type,
        TypeReference::Named { name, .. }
            if name.as_str() == parameter.name.as_str()
    )
}

fn type_references_match(
    program: &SymbolResolvedTrees,
    symbols: &SymbolTable,
    left: &TypeReference,
    right: &TypeReference,
) -> bool {
    use symbol_resolved_trees::types::TypeReference;

    match (left, right) {
        (TypeReference::Constrained(left), _) => type_references_match(
            program,
            symbols,
            program.child_type_reference(left.base_type),
            right,
        ),
        (_, TypeReference::Constrained(right)) => type_references_match(
            program,
            symbols,
            left,
            program.child_type_reference(right.base_type),
        ),
        (TypeReference::Reference(left), TypeReference::Reference(right)) => {
            left.access == right.access
                && type_references_match(
                    program,
                    symbols,
                    program.child_type_reference(left.referee),
                    program.child_type_reference(right.referee),
                )
        }
        (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
            left.length == right.length
                && type_references_match(
                    program,
                    symbols,
                    program.child_type_reference(left.element_type),
                    program.child_type_reference(right.element_type),
                )
        }
        (TypeReference::Slice(left), TypeReference::Slice(right)) => type_references_match(
            program,
            symbols,
            program.child_type_reference(left.element_type),
            program.child_type_reference(right.element_type),
        ),
        (TypeReference::Generic(left), TypeReference::Generic(right)) => {
            same_carrier_name(symbols, &left.base_name, &right.base_name)
                && left.lifetime_arguments == right.lifetime_arguments
                && program
                    .child_type_references(left.arguments)
                    .iter()
                    .zip(program.child_type_references(right.arguments))
                    .all(|(left, right)| type_references_match(program, symbols, left, right))
                && left.arguments.len() == right.arguments.len()
        }
        (TypeReference::Named { name: left, .. }, TypeReference::Named { name: right, .. }) => {
            same_carrier_name(symbols, left, right)
        }
        (TypeReference::Unit, TypeReference::Unit) => true,
        _ => false,
    }
}

fn same_carrier_name(
    symbols: &SymbolTable,
    left: &symbol_resolved_trees::name::DiagnosticName,
    right: &symbol_resolved_trees::name::DiagnosticName,
) -> bool {
    let select = |name: &symbol_resolved_trees::name::DiagnosticName| {
        symbols.find_top_level_by_name_and_kinds_from_source(
            name.as_str(),
            &[
                SymbolKind::BuiltinType,
                SymbolKind::Data,
                SymbolKind::Trait,
                SymbolKind::Machine,
            ],
            name.source_span(),
        )
    };
    select(left).is_some_and(|left| select(right) == Some(left))
}

fn operator_label(program: &SymbolResolvedTrees, operator: &OperatorDefinition) -> String {
    program.operator_path_string(operator.name)
}
