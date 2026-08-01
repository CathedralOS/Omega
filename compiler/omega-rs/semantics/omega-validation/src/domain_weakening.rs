//! Per-domain weakening at ordinary value-flow boundaries.
//!
//! Predicate-only knowledge may be forgotten implicitly. Static semantic
//! meaning, routed provenance, and arithmetic policy may not: an author must
//! spell an `as` whose result type omits the atom. This checker operates on
//! binding qualifications, not flow-proved membership, so stronger prover
//! knowledge never changes expression meaning.

use omega_core::arithmetic::ArithmeticDomain;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DomainAtom {
    Declared(SymbolHandle),
    Arithmetic(ArithmeticDomain),
}

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
        .filter(|atom| atom_requires_explicit_removal(program, *atom))
        .filter(|atom| {
            !target
                .iter()
                .any(|candidate| atoms_equivalent(program, *atom, *candidate))
        })
        .map(|atom| atom_label(program, atom))
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

fn expression_atoms(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
) -> Vec<DomainAtom> {
    let mut atoms = Vec::new();
    match program.expression_table.expression(value) {
        ExpressionNode::Mutable(inner)
        | ExpressionNode::Atomic(omega_typed_trees::expression::TableAtomicExpression {
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
                        append_declared_atom(program, domain.symbol, atoms, stack);
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
            append_declared_atom(program, constituent.domain_symbol, atoms, stack);
        }
        stack.pop();
    } else {
        push_unique(atoms, DomainAtom::Declared(symbol));
    }
}

fn push_unique(atoms: &mut Vec<DomainAtom>, atom: DomainAtom) {
    if !atoms.contains(&atom) {
        atoms.push(atom);
    }
}

fn atom_requires_explicit_removal(program: &TypedTrees, atom: DomainAtom) -> bool {
    match atom {
        DomainAtom::Arithmetic(_) => true,
        DomainAtom::Declared(symbol) => program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .is_some_and(|domain| {
                !domain.semantic_roles.is_empty() || !domain.establishment_routes.is_empty()
            }),
    }
}

fn atoms_equivalent(program: &TypedTrees, left: DomainAtom, right: DomainAtom) -> bool {
    match (left, right) {
        (DomainAtom::Arithmetic(left), DomainAtom::Arithmetic(right)) => left == right,
        (DomainAtom::Declared(left), DomainAtom::Declared(right)) => {
            if left == right {
                return true;
            }
            let semantic_id = |symbol| {
                program
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == symbol)
                    .map(|domain| domain.semantic_id)
            };
            matches!((semantic_id(left), semantic_id(right)), (Some(left), Some(right)) if left.is_valid() && left == right)
        }
        _ => false,
    }
}

fn atom_label(program: &TypedTrees, atom: DomainAtom) -> String {
    match atom {
        DomainAtom::Arithmetic(domain) => domain.name().to_owned(),
        DomainAtom::Declared(symbol) => program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .map(|domain| domain.name.to_string())
            .unwrap_or_else(|| "unknown domain".to_owned()),
    }
}
