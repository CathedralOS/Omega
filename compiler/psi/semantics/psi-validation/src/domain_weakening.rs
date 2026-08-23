//! Per-domain weakening at ordinary value-flow boundaries.
//!
//! Predicate-only knowledge may be forgotten implicitly. Static semantic
//! meaning, routed provenance, and arithmetic policy may not: an author must
//! spell an `as` whose result type omits the atom. This checker operates on
//! binding qualifications, not flow-proved membership, so stronger prover
//! knowledge never changes expression meaning.

use psi_diagnostics::Diagnostic;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone)]
enum DomainAtom {
    Declared {
        family: SymbolHandle,
        instance: psi_language_semantics::SemanticDomainId,
        indexed: bool,
        label: String,
    },
    Arithmetic(ArithmeticDomain),
}

impl PartialEq for DomainAtom {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Arithmetic(left), Self::Arithmetic(right)) => left == right,
            (
                Self::Declared {
                    family: left_family,
                    instance: left_instance,
                    indexed: left_indexed,
                    ..
                },
                Self::Declared {
                    family: right_family,
                    instance: right_instance,
                    indexed: right_indexed,
                    ..
                },
            ) => {
                left_family == right_family
                    && left_instance == right_instance
                    && left_indexed == right_indexed
            }
            _ => false,
        }
    }
}

impl Eq for DomainAtom {}

/// Reject an ordinary store/call/return flow that silently removes a static
/// semantic, routed-provenance, or arithmetic-policy atom. A value cast is a
/// fresh explicit result surface: `value as T` deliberately has `T`'s atoms,
/// while `value as T in D` deliberately has `D` as well.
pub(crate) fn validate_implicit_domain_weakening(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_implicit_domain_weakening_with_policy_retention(
        program,
        machine,
        state,
        value,
        target_type,
        owner,
        false,
        diagnostics,
    );
}

/// Named float operators deliberately carry an operand arithmetic policy to
/// their float result through checked adapter evidence. Other semantic atoms
/// still obey the ordinary explicit-removal rule.
pub(crate) fn validate_implicit_domain_weakening_retaining_arithmetic_policy(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_implicit_domain_weakening_with_policy_retention(
        program,
        machine,
        state,
        value,
        target_type,
        owner,
        true,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_implicit_domain_weakening_with_policy_retention(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    owner: &str,
    retain_arithmetic_policy: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() || !target_type.is_valid() {
        return;
    }
    if matches!(
        program.expression_table.expression(value),
        ExpressionNode::Cast(cast) if cast.form.is_recast()
    ) {
        // Reference recasts have their own representation/fact weakening law.
        return;
    }

    let source = expression_atoms(program, machine, state, value);
    if source.is_empty() {
        return;
    }
    let mut target = Vec::new();
    append_type_atoms(program, target_type, &mut target, &mut Vec::new());

    let dropped = source
        .into_iter()
        .filter(|atom| !(retain_arithmetic_policy && matches!(atom, DomainAtom::Arithmetic(_))))
        .filter(|atom| atom_requires_explicit_removal(program, atom))
        // PDI3 same-family indexed mismatches are equality obligations, not
        // implicit semantic removal. Their exact closed/normalization/local-
        // fact judgment runs after semantic flow contexts exist in checked
        // lowering. Every other missing atom remains an error here.
        .filter(|atom| {
            !target
                .iter()
                .any(|candidate| is_deferred_index_compatibility(atom, candidate))
        })
        .filter(|atom| {
            !target
                .iter()
                .any(|candidate| atoms_equivalent(program, atom, candidate))
        })
        .map(|atom| atom_label(&atom))
        .collect::<Vec<_>>();
    if dropped.is_empty() {
        return;
    }

    diagnostics.push(Diagnostic::error(format!(
        "implicit domain weakening in {owner} drops {}; semantic meaning, routed provenance, \
         and arithmetic policy may be removed only by an explicit `as` to the intended target",
        dropped
            .iter()
            .map(|label| format!("`{label}`"))
            .collect::<Vec<_>>()
            .join(", "),
    )));
}

fn is_deferred_index_compatibility(left: &DomainAtom, right: &DomainAtom) -> bool {
    matches!(
        (left, right),
        (
            DomainAtom::Declared {
                family: left_family,
                instance: left_instance,
                indexed: true,
                ..
            },
            DomainAtom::Declared {
                family: right_family,
                instance: right_instance,
                indexed: true,
                ..
            }
        ) if left_family == right_family
            && left_instance.is_valid()
            && right_instance.is_valid()
            && left_instance != right_instance
    )
}

fn expression_atoms(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
) -> Vec<DomainAtom> {
    let mut atoms = Vec::new();
    match program.expression_table.expression(value) {
        ExpressionNode::Borrow(inner) => {
            return expression_atoms(program, machine, state, inner.target);
        }
        ExpressionNode::Atomic(psi_typed_trees::expression::TableAtomicExpression {
            value: inner,
            ..
        }) => {
            return expression_atoms(program, machine, state, *inner);
        }
        ExpressionNode::Cast(cast) => {
            append_type_atoms(program, cast.target_type, &mut atoms, &mut Vec::new());
            if cast.semantic_domain_symbol.is_valid() {
                append_declared_atom(
                    program,
                    cast.semantic_domain_symbol,
                    cast.semantic_domain_id,
                    !cast.semantic_domain_arguments.is_empty(),
                    Some(qualification_cast_label(program, cast)),
                    &mut atoms,
                    &mut Vec::new(),
                );
            }
            if cast.domain != ArithmeticDomain::Exact {
                push_unique(&mut atoms, DomainAtom::Arithmetic(cast.domain));
            }
            return atoms;
        }
        ExpressionNode::Call(call) => {
            if let Some(return_type) =
                crate::arithmetic_domains::call_return_type(program, machine, call)
            {
                append_type_atoms(program, return_type, &mut atoms, &mut Vec::new());
            }
            return atoms;
        }
        _ => {}
    }

    if let Some(source_type) =
        crate::places::declared_place_type_raw(program, machine, state, value)
    {
        append_type_atoms(program, source_type, &mut atoms, &mut Vec::new());
    }
    atoms
}

fn append_type_atoms(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    atoms: &mut Vec<DomainAtom>,
    stack: &mut Vec<SymbolHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            append_type_atoms(program, *referee, atoms, stack);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            append_type_atoms(program, *base_type, atoms, stack);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                        append_declared_atom(
                            program,
                            domain.symbol,
                            domain.semantic_id,
                            !domain.arguments.is_empty(),
                            Some(domain_constraint_label(program, domain)),
                            atoms,
                            stack,
                        );
                    }
                    TypeConstraintNode::ArithmeticDomain(domain)
                        if *domain != ArithmeticDomain::Exact =>
                    {
                        push_unique(atoms, DomainAtom::Arithmetic(*domain));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn append_declared_atom(
    program: &TypedTrees,
    symbol: SymbolHandle,
    instance: psi_language_semantics::SemanticDomainId,
    indexed: bool,
    label: Option<String>,
    atoms: &mut Vec<DomainAtom>,
    stack: &mut Vec<SymbolHandle>,
) {
    if !symbol.is_valid() || stack.contains(&symbol) {
        return;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
    else {
        return;
    };
    if let Some(alias) = domain.alias.as_ref() {
        stack.push(symbol);
        for constituent in &alias.constituents {
            append_declared_atom(
                program,
                constituent.domain_symbol,
                psi_language_semantics::SemanticDomainId::NULL,
                false,
                None,
                atoms,
                stack,
            );
        }
        stack.pop();
    } else {
        push_unique(
            atoms,
            DomainAtom::Declared {
                family: symbol,
                instance,
                indexed,
                label: label.unwrap_or_else(|| domain.name.to_string()),
            },
        );
    }
}

fn push_unique(atoms: &mut Vec<DomainAtom>, atom: DomainAtom) {
    if !atoms.contains(&atom) {
        atoms.push(atom);
    }
}

fn atom_requires_explicit_removal(program: &TypedTrees, atom: &DomainAtom) -> bool {
    match atom {
        DomainAtom::Arithmetic(_) => true,
        DomainAtom::Declared {
            family: symbol,
            indexed,
            ..
        } => {
            *indexed
                || program
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == *symbol)
                    .is_some_and(|domain| {
                        !domain.semantic_roles.is_empty() || !domain.establishment_routes.is_empty()
                    })
        }
    }
}

fn atoms_equivalent(program: &TypedTrees, left: &DomainAtom, right: &DomainAtom) -> bool {
    match (left, right) {
        (DomainAtom::Arithmetic(left), DomainAtom::Arithmetic(right)) => left == right,
        (
            DomainAtom::Declared {
                family: left_family,
                instance: left_instance,
                ..
            },
            DomainAtom::Declared {
                family: right_family,
                instance: right_instance,
                ..
            },
        ) => {
            if left_instance.is_valid() && right_instance.is_valid() {
                return left_instance == right_instance;
            }
            if left_family == right_family {
                return true;
            }
            let semantic_id = |symbol| {
                program
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == symbol)
                    .map(|domain| domain.semantic_id)
            };
            matches!((semantic_id(*left_family), semantic_id(*right_family)), (Some(left), Some(right)) if left.is_valid() && left == right)
        }
        _ => false,
    }
}

fn atom_label(atom: &DomainAtom) -> String {
    match atom {
        DomainAtom::Arithmetic(domain) => domain.name().to_owned(),
        DomainAtom::Declared { label, .. } => label.clone(),
    }
}

fn domain_constraint_label(
    program: &TypedTrees,
    domain: &psi_typed_trees::types::DomainConstraint,
) -> String {
    if domain.arguments.is_empty() {
        return domain.name.to_string();
    }

    let arguments = domain
        .arguments
        .iter()
        .map(
            |argument| match program.type_reference_table.type_reference(*argument) {
                TypeReferenceNode::Named { name, .. } => {
                    psi_language_semantics::const_value::CanonicalConstValue::from_atom(
                        name.as_str(),
                    )
                    .map_or_else(|| name.to_string(), |value| value.display)
                }
                _ => program.display_type_reference(*argument),
            },
        )
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}<{arguments}>", domain.name)
}

fn qualification_cast_label(
    program: &TypedTrees,
    cast: &psi_typed_trees::expression::TableCastExpression,
) -> String {
    let name = program
        .expression_table
        .name_path_members(cast.semantic_domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    if cast.semantic_domain_arguments.is_empty() {
        return name;
    }
    let arguments = program
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments)
        .iter()
        .map(
            |argument| match program.type_reference_table.type_reference(*argument) {
                TypeReferenceNode::Named { name, .. } => {
                    psi_language_semantics::const_value::CanonicalConstValue::from_atom(
                        name.as_str(),
                    )
                    .map_or_else(|| name.to_string(), |value| value.display)
                }
                _ => program.display_type_reference(*argument),
            },
        )
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}
