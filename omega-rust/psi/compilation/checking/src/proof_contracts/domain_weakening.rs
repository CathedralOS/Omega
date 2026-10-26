//! Per-domain weakening at ordinary value-flow boundaries.
//!
//! Predicate-only knowledge may be forgotten implicitly. Static semantic
//! meaning, routed provenance, and arithmetic policy may not: an author must
//! spell an `as` whose result type omits the atom. The mirror rule holds for
//! the atoms no checker can discharge after the fact — a declared domain with
//! no predicate body and no establishment route is assumed, not proved, so a
//! store may not introduce one that the value does not already carry. This
//! checker operates on binding qualifications, not flow-proved membership, so
//! stronger prover knowledge never changes expression meaning.
//! Integer value delivery already checks carrier and arithmetic policy together
//! in `check_narrowing_assignment`. Leave that atom to its single conversion
//! diagnostic when both surfaces are integer values. References and other
//! carriers still need this check; declared semantic and provenance atoms are
//! independent obligations even at an integer delivery.

use diagnostics::Diagnostic;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone)]
enum DomainAtom {
    Declared {
        family: SymbolHandle,
        instance: language_semantics::SemanticDomainId,
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_argument_domain_weakening(
        program,
        machine,
        state,
        value,
        target_type,
        owner,
        false,
        None,
        diagnostics,
    );
}

/// A sealed primitive Float operation may retain its evaluated operand policy
/// through its adapter. All other calls obey their declared parameter policy;
/// no semantic or provenance atom is exempted by primitive adaptation.
///
/// `callee_parameter` names the declared parameter an ARGUMENT fills, so the
/// callee's own `requires` premises about it join the parameter's declared type
/// as the atom surface the argument lands on. Ordinary stores and field
/// initializers have no callee contract and pass `None`; returns use their
/// own result delivery obligations.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_argument_domain_weakening(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    owner: &str,
    retains_float_result_policy: bool,
    callee_parameter: Option<&StateParameter>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let obligations = callee_parameter
        .map(|parameter| parameter_premise_atoms(program, parameter))
        .unwrap_or_default();
    validate_domain_delivery(
        program,
        machine,
        state,
        value,
        target_type,
        owner,
        retains_float_result_policy,
        &obligations,
        diagnostics,
    );
}

/// An explicit result guarantee is a delivery obligation, just like an input
/// premise at a call. Preserving that atom is not implicit forgetting. The
/// exit checker must still prove it from the actual returned value.
pub(crate) fn validate_return_domain_weakening(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    value: ExpressionHandle,
    owner: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut obligations = Vec::new();
    append_callable_ensured_domain_atoms(program, state.symbol, &mut obligations);
    validate_domain_delivery(
        program,
        machine,
        Some(state),
        value,
        state.return_type,
        owner,
        false,
        &obligations,
        diagnostics,
    );
}

#[allow(clippy::too_many_arguments)]
fn validate_domain_delivery(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    owner: &str,
    retains_float_result_policy: bool,
    obligations: &[DomainAtom],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid()
        || !target_type.is_valid()
        || crate::proof_contracts::arithmetic_domains::expression_reads_unresolved_local_initializer(
            program, machine, state, value,
        )
    {
        return;
    }
    if let Some(state) = state {
        crate::value_custody::cleanup::validate_cleanup_delivery(
            program,
            machine,
            state,
            value,
            target_type,
            owner,
            diagnostics,
        );
    }
    let source = expression_atoms(program, machine, state, value);
    if source.is_empty() {
        return;
    }
    let mut target = Vec::new();
    append_type_atoms(program, target_type, &mut target, &mut Vec::new());
    let integer_delivery = program
        .primitive_type_reference(target_type)
        .and_then(crate::proof_contracts::arithmetic_domains::integer_bit_width)
        .is_some()
        && state
            .and_then(|state| {
                crate::value_custody::expression_types::expression_result_type_reference(
                    program, machine, state, value,
                )
            })
            .and_then(|reference| program.primitive_type_reference(reference))
            .and_then(crate::proof_contracts::arithmetic_domains::integer_bit_width)
            .is_some();
    let dropped = source
        .iter()
        .filter(|atom| !(integer_delivery && matches!(atom, DomainAtom::Arithmetic(_))))
        .filter(|atom| !(retains_float_result_policy && matches!(atom, DomainAtom::Arithmetic(_))))
        .filter(|atom| atom_requires_explicit_removal(program, atom))
        // Exact input premises and result guarantees consume membership at
        // this delivery; they do not erase it. The corresponding call/exit
        // checker proves those obligations independently. Other atoms still
        // require an explicit `as`.
        .filter(|atom| {
            !obligations
                .iter()
                .any(|premise| atoms_equivalent(program, atom, premise))
        })
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
        .map(atom_label)
        .collect::<Vec<_>>();

    // The mirror direction: a target atom no checker can discharge (no
    // predicate body, no establishment route) must be carried by the value,
    // not minted by the store. The carrier set adds a call's own `ensures`
    // result domains — the declared return type alone does not name them.
    let mut carried = source.clone();
    append_call_ensured_domain_atoms(program, value, &mut carried);
    let assumed = target
        .iter()
        .filter(|atom| atom_requires_carried_evidence(program, atom))
        .filter(|atom| {
            !carried
                .iter()
                .any(|candidate| atoms_equivalent(program, atom, candidate))
        })
        .filter(|atom| {
            !carried
                .iter()
                .any(|candidate| is_deferred_index_compatibility(atom, candidate))
        })
        .map(atom_label)
        .collect::<Vec<_>>();

    if !dropped.is_empty() {
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
    if !assumed.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "implicit domain strengthening in {owner} assumes {}; a qualification with no \
             predicate or establishment route may be introduced only by an explicit `as` to \
             the intended target",
            assumed
                .iter()
                .map(|label| format!("`{label}`"))
                .collect::<Vec<_>>()
                .join(", "),
        )));
    }
}

/// The domains a callee's `requires` rows assert about one declared parameter.
/// `requires guard in InterruptMaskGuard::Active` states the entry contract
/// hypothetically: the parameter's declared type stays the plain carrier, and
/// the caller establishes the membership for its actual argument. Only an exact
/// `<parameter> in <Domain>` conjunct of a `requires` clause contributes —
/// authored `&` conjunctions lower to one membership fact each, and a
/// projection, another binding, or an `ensures` row names no premise here. The
/// declared-atom expansion is the same one a declared qualification gets, so an
/// alias premise contributes its constituents and a refinement its parents.
fn parameter_premise_atoms(
    program: &dyn crate::CheckingInput,
    parameter: &StateParameter,
) -> Vec<DomainAtom> {
    if !parameter.symbol.is_valid() {
        return Vec::new();
    }
    let owner = program.symbols.get(parameter.symbol).parent;
    let Some(contracts) = callable_signature_contracts(program, owner) else {
        return Vec::new();
    };
    let mut atoms = Vec::new();
    for contract in contracts {
        if !matches!(contract.kind, SignatureContractKind::Requires) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            if !program
                .expression_table
                .names_parameter(membership.value, parameter)
            {
                continue;
            }
            append_declared_atom(
                program,
                membership.domain_symbol,
                membership.semantic_domain,
                !membership.domain_arguments.is_empty(),
                None,
                &mut atoms,
                &mut Vec::new(),
            );
        }
    }
    atoms
}

/// A declared atom with no predicate body and no establishment route is an
/// assumption, not a provable obligation: no checker downstream can discharge
/// it from the store itself, so the value must already carry it. Indexed
/// atoms are still eligible — their carried source atom may differ only by
/// instance, which `is_deferred_index_compatibility` leaves to checked
/// lowering. Arithmetic policy is an operand property, never a target's
/// assumption, so only declared atoms participate.
fn atom_requires_carried_evidence(program: &dyn crate::CheckingInput, atom: &DomainAtom) -> bool {
    let DomainAtom::Declared { family: symbol, .. } = atom else {
        return false;
    };
    program.domain_definition(*symbol).is_some_and(|domain| {
        !domain.predicate_body.is_present() && domain.establishment_routes.is_empty()
    })
}

/// A value-position call's result is what its signature and contract row say
/// it is: an `ensures result in D` row carries `D` the way a declared return
/// type would, even though `call_return_type` surfaces only the declared
/// type. Reference- and atomic-wrapped calls unwrap to the same call; a cast
/// stays a fresh explicit result surface and never consults the callee's
/// ensures.
fn append_call_ensured_domain_atoms(
    program: &dyn crate::CheckingInput,
    value: ExpressionHandle,
    atoms: &mut Vec<DomainAtom>,
) {
    let mut inner = value;
    let call = loop {
        match program.expression_table.expression(inner) {
            ExpressionNode::Borrow(borrow) => inner = borrow.target,
            ExpressionNode::Atomic(atomic) => inner = atomic.value,
            ExpressionNode::Call(call) => break call,
            _ => return,
        }
    };
    append_callable_ensured_domain_atoms(program, call.target_symbol, atoms);
}

fn append_callable_ensured_domain_atoms(
    program: &dyn crate::CheckingInput,
    callable: SymbolHandle,
    atoms: &mut Vec<DomainAtom>,
) {
    let Some(contracts) = callable_signature_contracts(program, callable) else {
        return;
    };
    for contract in contracts {
        if !matches!(contract.kind, SignatureContractKind::Ensures) {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            // Only the whole reserved `result` names the call's result; a
            // projection such as `result.storage` belongs to that field's
            // declared type.
            let Some((owner, _)) = crate::proof_contracts::contract_results::reserved_result_owner(
                program,
                membership.value,
            ) else {
                continue;
            };
            if owner != callable
                && !crate::machine_calls::calls::machine_state_by_symbol(program, callable)
                    .is_some_and(|(machine, _)| owner == machine.symbol)
            {
                continue;
            }
            append_declared_atom(
                program,
                membership.domain_symbol,
                membership.semantic_domain,
                !membership.domain_arguments.is_empty(),
                None,
                atoms,
                &mut Vec::new(),
            );
        }
    }
}

/// The contract rows of the callable one state symbol names, mirroring
/// `resolved_call_result_type`'s resolution: a machine body's states and their
/// machine-level rows, or the closed requirement signature a boundary call
/// targets. A value-position call passes its target symbol; an argument passes
/// the parameter's owning callable.
fn callable_signature_contracts<'program>(
    program: &'program dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<Vec<&'program typed_trees::signature::SignatureContract>> {
    if let Some((machine, state)) =
        crate::machine_calls::calls::machine_state_by_symbol(program, target)
    {
        return Some(
            program
                .state_contracts(state)
                .iter()
                .chain(program.machine_contracts(machine).iter())
                .collect(),
        );
    }
    if !target.is_valid() {
        return None;
    }
    let parent = program.symbols.get(target).parent;
    let definition = program.trait_definition_by_symbol(parent)?;
    let signature = program.trait_machine_signature_by_symbol(definition, target)?;
    Some(
        program
            .state_signature_contracts(signature)
            .iter()
            .collect(),
    )
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    value: ExpressionHandle,
) -> Vec<DomainAtom> {
    let mut atoms = Vec::new();
    match program.expression_table.expression(value) {
        ExpressionNode::Borrow(inner) => {
            return expression_atoms(program, machine, state, inner.target);
        }
        ExpressionNode::Atomic(typed_trees::expression::TableAtomicExpression {
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
                crate::proof_contracts::arithmetic_domains::call_return_type(program, call)
            {
                append_type_atoms(program, return_type, &mut atoms, &mut Vec::new());
            }
            return atoms;
        }
        _ => {}
    }

    if let Some(source_type) =
        crate::value_custody::places::declared_place_type_raw(program, machine, state, value)
    {
        append_type_atoms(program, source_type, &mut atoms, &mut Vec::new());
    }
    atoms
}

fn append_type_atoms(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
    instance: language_semantics::SemanticDomainId,
    indexed: bool,
    label: Option<String>,
    atoms: &mut Vec<DomainAtom>,
    stack: &mut Vec<SymbolHandle>,
) {
    if !symbol.is_valid() || stack.contains(&symbol) {
        return;
    }
    let Some(domain) = program.domain_definition(symbol) else {
        return;
    };
    if let Some(alias) = domain.alias.as_ref() {
        stack.push(symbol);
        for constituent in &alias.constituents {
            append_declared_atom(
                program,
                constituent.domain_symbol,
                language_semantics::SemanticDomainId::NULL,
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
        // A `self in Parent` requirement — the proposition a refinement-chain
        // declaration implies, and any authored self-membership — makes the
        // parent an atom the member carries, the way an alias's constituents
        // are.
        stack.push(symbol);
        for membership in typed_trees::domain::subject_membership_facts(program, domain) {
            append_declared_atom(
                program,
                membership.domain_symbol,
                membership.semantic_domain,
                !membership.domain_arguments.is_empty(),
                None,
                atoms,
                stack,
            );
        }
        stack.pop();
    }
}

fn push_unique(atoms: &mut Vec<DomainAtom>, atom: DomainAtom) {
    if !atoms.contains(&atom) {
        atoms.push(atom);
    }
}

fn atom_requires_explicit_removal(program: &dyn crate::CheckingInput, atom: &DomainAtom) -> bool {
    match atom {
        DomainAtom::Arithmetic(_) => true,
        DomainAtom::Declared {
            family: symbol,
            indexed,
            ..
        } => {
            *indexed
                || program.domain_definition(*symbol).is_some_and(|domain| {
                    !domain.semantic_roles.is_empty() || !domain.establishment_routes.is_empty()
                })
        }
    }
}

fn atoms_equivalent(
    program: &dyn crate::CheckingInput,
    left: &DomainAtom,
    right: &DomainAtom,
) -> bool {
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
                    .domain_definition(symbol)
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

/// A semantic-domain argument renders as its canonical const value display
/// when it names one, otherwise as the type reference itself.
fn domain_argument_display(
    program: &dyn crate::CheckingInput,
    argument: TypeReferenceHandle,
) -> String {
    match program.type_reference_table.type_reference(argument) {
        TypeReferenceNode::Named { name, .. } => {
            language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                .map_or_else(|| name.to_string(), |value| value.display)
        }
        _ => program.display_type_reference(argument),
    }
}

fn domain_constraint_label(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::types::DomainConstraint,
) -> String {
    if domain.arguments.is_empty() {
        return domain.name.to_string();
    }

    let arguments = domain
        .arguments
        .iter()
        .map(|argument| domain_argument_display(program, *argument))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}<{arguments}>", domain.name)
}

fn qualification_cast_label(
    program: &dyn crate::CheckingInput,
    cast: &typed_trees::expression::TableCastExpression,
) -> String {
    let name = program
        .expression_table
        .name_path_string(cast.semantic_domain);
    if cast.semantic_domain_arguments.is_empty() {
        return name;
    }
    let arguments = program
        .type_reference_table
        .type_reference_handles(cast.semantic_domain_arguments)
        .iter()
        .map(|argument| domain_argument_display(program, *argument))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}
