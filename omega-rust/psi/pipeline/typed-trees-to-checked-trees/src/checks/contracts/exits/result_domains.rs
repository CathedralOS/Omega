//! Declared field obligations at a machine's normal return. An `ensures`
//! domain fact authored over the reserved `result` name is discharged against
//! the exact returned expression's live place; a nominal return type
//! additionally makes every declared field predicate an obligation at each
//! value-returning exit, and a reference return owes the same predicates on
//! the returned place. The return is also a default-domain consumption point
//! for every readable `&mut` referent the machine received, `self`
//! included: the declared field facts assumed on entry must hold again when
//! the referent is handed back. All of these consume only live evidence: a
//! write that retired a field's fact fails the proof, so a nominal
//! annotation alone restores nothing.

use checked_trees::{CheckFacts, FlowExitFact};
use diagnostics::Diagnostic;
use facts::{FactContextHandle, FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint};
use typed_trees::types::TypeReferenceNode;

use super::super::return_values::exit_return_expression;
use crate::flow::canonical_place_from_expression_in_state;

/// Rebase an ensures fact rooted at the reserved `result` occurrence onto the
/// returned expression's canonical place, keeping the fact's own declared
/// field coordinates. `result.bytes` on a `rows[0]` return is exactly
/// `rows[0].bytes` at this statement -- the same place mutation invalidation
/// retires, so corrupted evidence cannot satisfy it.
pub(super) fn proves_result_domain(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    contexts: &[FactContextHandle],
    requirement: &facts::Fact,
) -> bool {
    let domain_symbol = match requirement.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
        _ => return false,
    };
    let FactPlace::Place(place) = requirement.place else {
        return false;
    };
    let place = facts.semantic.places.get(place);
    let PlaceRoot::Expression(root) = place.root else {
        return false;
    };
    // Only the reserved `result` occurrence owned by THIS machine's ensures
    // rebases onto the returned expression. Other expression roots keep their
    // identity (an authored `result` parameter or a foreign occurrence is not
    // the contract result).
    if validation::reserved_result_owner(program, root)
        .is_none_or(|(owner, _)| owner != exit.machine_symbol)
    {
        return false;
    }
    let returned = exit_return_expression(program, exit);
    if !returned.is_valid() {
        return false;
    }
    let Some(mut subject) = canonical_place_from_expression_in_state(
        program,
        exit.state_symbol,
        exit.statement_index,
        returned,
    ) else {
        return false;
    };
    subject.extend_segments(facts.semantic.place_segments.span_or_empty(place.segments));
    super::super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        contexts,
        &subject,
        domain_symbol,
    )
}

/// Every declared field predicate of an owned nominal (or fixed-array nominal
/// element) return type is an obligation on the returned value at each
/// value-returning exit -- the return-position dual of the declared-field
/// requirements a call's nominal input imposes on its actuals
/// (checks/contracts/nominal_inputs.rs). A readable reference return owns no
/// result storage but hands its caller the referent, so the same predicates
/// are obligations on the returned place: the caller reads the referent's
/// declared fields through the reference, and a source write that retired
/// them must have been repaired before the return.
pub(in crate::checks::contracts) fn check_result_field_domains(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == exit.machine_symbol)
    else {
        return;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    let paths = crate::facts::field_domain::declared_result_field_domain_paths(
        program,
        result_domain_type(program, entry.return_type),
    )
    .into_iter()
    .filter(|(_, domain_symbol)| value_provable_domain(program, *domain_symbol))
    .collect::<Vec<_>>();
    if paths.is_empty() {
        return;
    }
    let returned = exit_return_expression(program, exit);
    if !returned.is_valid() {
        return;
    }
    let entry_contexts: Vec<_> = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    let base = canonical_place_from_expression_in_state(
        program,
        exit.state_symbol,
        exit.statement_index,
        returned,
    );
    for (path, domain_symbol) in paths {
        // The same two shapes as call actuals (checks/contracts/nominal_inputs):
        // a returned constructor projects each requirement onto the exact field
        // or element expression it was built from; anything else proves at the
        // returned place directly.
        let satisfied = crate::flow::literal_value_projections(
            program,
            returned,
            entry.return_type,
            &path,
            false,
        )
        .is_some_and(|projections| {
            !projections.is_empty()
                && projections.iter().all(|projection| {
                    canonical_place_from_expression_in_state(
                        program,
                        exit.state_symbol,
                        exit.statement_index,
                        projection.expression,
                    )
                    .is_some_and(|mut subject| {
                        subject.extend_segments(&projection.remaining);
                        super::super::prover::prove_domain_at_place(
                            program,
                            &facts.semantic,
                            &entry_contexts,
                            &subject,
                            domain_symbol,
                        )
                    })
                })
        }) || base.as_ref().is_some_and(|base| {
            let mut subject = base.clone();
            subject.extend_segments(&path);
            super::super::prover::prove_domain_at_place(
                program,
                &facts.semantic,
                &entry_contexts,
                &subject,
                domain_symbol,
            )
        });
        if !satisfied {
            let (root, mut segments) = base
                .as_ref()
                .map(|base| (base.root, base.segments.clone()))
                .unwrap_or((PlaceRoot::Expression(returned), Vec::new()));
            segments.extend_from_slice(&path);
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
                crate::labels::machine_name(program, exit.machine_symbol),
                exit.statement_index,
                crate::labels::canonical_place_label_from_parts(program, root, &segments),
                crate::labels::symbol_name(program, domain_symbol),
            )));
        }
    }
}

/// The type whose declared fields a return owes: the owned result type
/// itself, or the referent behind a readable reference return. A write-only
/// reference exposes no readable storage and owes nothing.
pub(crate) fn result_domain_type(
    program: &typed_trees::TypedTrees,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> typed_trees::types::TypeReferenceHandle {
    let mut reference = return_type;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => return *referee,
            _ => return reference,
        }
    }
    return_type
}

/// Whether a declared field domain is re-provable from the field's value.
/// An establishment-gated domain (`established by ..`) mints membership only
/// through its listed routes, so its rows are custody evidence transported
/// by the carry machinery rather than an invariant window a return can
/// close by proof; they are neither owed at a return nor handed back.
pub(crate) fn value_provable_domain(
    program: &typed_trees::TypedTrees,
    domain_symbol: symbols::SymbolHandle,
) -> bool {
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
        .is_none_or(|domain| domain.establishment_routes.is_empty())
}

/// Whether a declared parameter type is a readable `&mut` reference, whose
/// referent the machine may corrupt and hands back at its return. Owned
/// parameters die with the machine and write-only views expose no readable
/// referent, so neither owes anything here.
pub(crate) fn is_readable_mutable_reference(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let mut reference = type_reference;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => {
                return *access == language_semantics::ReferenceAccess::Mutable;
            }
            _ => return false,
        }
    }
    false
}

/// The default-domain field facts a state assumes on entry for its readable
/// `&mut` referents: each such parameter's `StateParameterDomain` field facts
/// and, for `&mut self`, the machine's `MachineFieldDomain` facts. These are
/// exactly the rows the self-transition arrival check re-proves
/// (checks/contracts/arrivals.rs); no second obligation vocabulary exists.
pub(crate) fn mutable_referent_field_requirements(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
) -> Vec<facts::Fact> {
    let Some(state) =
        crate::semantic_calls::find_state_in_machine(program, machine_symbol, state_symbol)
    else {
        return Vec::new();
    };
    let mut referents = Vec::new();
    let mut self_is_mutable = false;
    for parameter in program.state_parameters(state) {
        if !is_readable_mutable_reference(program, parameter.type_reference) {
            continue;
        }
        if parameter.is_self {
            self_is_mutable = true;
        } else {
            referents.push(parameter.symbol);
        }
    }
    let mut requirements = Vec::new();
    if !referents.is_empty() {
        requirements.extend(
            semantic
                .contexts_at_point(ProgramPoint::State {
                    machine_symbol,
                    state_symbol,
                })
                .flat_map(|context| context.facts())
                .filter(|fact| {
                    matches!(
                        fact.origin,
                        FactOrigin::StateParameterDomain {
                            machine_symbol: origin_machine,
                            state_symbol: origin_state,
                        } if origin_machine == machine_symbol && origin_state == state_symbol
                    ) && matches!(fact.payload, FactPayload::DomainMembership { domain_symbol, .. }
                        if value_provable_domain(program, domain_symbol))
                        && matches!(fact.place, FactPlace::Place(place)
                            if matches!(semantic.places.get(place).root, PlaceRoot::Symbol(root)
                                if referents.contains(&root)))
                })
                .cloned(),
        );
    }
    if self_is_mutable {
        requirements.extend(
            semantic
                .contexts_at_point(ProgramPoint::Machine { machine_symbol })
                .flat_map(|context| context.facts())
                .filter(|fact| {
                    matches!(
                        fact.origin,
                        FactOrigin::MachineFieldDomain {
                            machine_symbol: origin_machine,
                        } if origin_machine == machine_symbol
                    ) && matches!(fact.payload, FactPayload::DomainMembership { domain_symbol, .. }
                        if value_provable_domain(program, domain_symbol))
                })
                .cloned(),
        );
    }
    requirements
}

/// A machine's normal return is a default-domain consumption point for every
/// readable `&mut` referent it received, `self` included: the field facts
/// assumed on entry must be provable again from the live evidence at the
/// exit. A write through a bare alias (`&mut [u8; 4]` onto a `Utf8` field)
/// retires the field's fact without establishing anything, so returning
/// without repairing it rejects here with the exact place, the same way a
/// call or transition would refuse the corrupted referent.
pub(in crate::checks::contracts) fn check_mutable_referent_field_domains(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let requirements = mutable_referent_field_requirements(
        program,
        &facts.semantic,
        exit.machine_symbol,
        exit.state_symbol,
    );
    if requirements.is_empty() {
        return;
    }
    let entry_contexts: Vec<_> = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    for requirement in &requirements {
        if super::super::prover::semantic_contexts_prove_contract_fact(
            program,
            &facts.semantic,
            &entry_contexts,
            requirement,
        ) {
            continue;
        }
        let (FactPlace::Place(place), FactPayload::DomainMembership { domain_symbol, .. }) =
            (requirement.place, requirement.payload)
        else {
            continue;
        };
        let place = facts.semantic.places.get(place);
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
            crate::labels::machine_name(program, exit.machine_symbol),
            exit.statement_index,
            crate::labels::canonical_place_label_from_parts(
                program,
                place.root,
                facts.semantic.place_segments.span_or_empty(place.segments),
            ),
            crate::labels::symbol_name(program, domain_symbol),
        )));
    }
}
