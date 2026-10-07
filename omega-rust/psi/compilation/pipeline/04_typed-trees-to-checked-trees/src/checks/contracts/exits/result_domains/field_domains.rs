//! field domains slice of result_domains.

use super::super::super::return_values::exit_return_expression;
use super::scalar_exits::*;
use crate::flow::{
    CanonicalPlace, canonical_place_from_expression_in_state, canonical_place_from_semantic_place,
};
use checked_trees::{CheckFacts, FlowExitFact};
use diagnostics::Diagnostic;
use facts::{FactOrigin, FactPayload, FactPlace, PlaceRoot, ProgramPoint};
use typed_trees::types::TypeReferenceNode;

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
    call_frames: Option<&validation::CallFrameResolver<'_>>,
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
    let mut paths = crate::facts::field_domain::declared_result_field_domain_paths(
        program,
        result_domain_type(program, entry.return_type),
    )
    .into_iter()
    .filter(|(_, domain_symbol, _)| value_provable_domain(program, *domain_symbol))
    .collect::<Vec<_>>();
    paths.extend(
        crate::facts::field_domain::declared_owned_field_domain_identities(
            program,
            result_domain_type(program, entry.return_type),
        ),
    );
    if program
        .primitive_type_reference(entry.return_type)
        .is_none()
    {
        paths.extend(
            crate::facts::field_domain::domain_constraint_identities(program, entry.return_type)
                .into_iter()
                .filter(|(domain, _)| {
                    crate::facts::field_domain::domain_requires_provenance(program, *domain)
                })
                .map(|(domain, identity)| (Vec::new(), domain, identity)),
        );
    }
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
    let cases = super::super::cases::CaseObservation::for_exit(
        program,
        facts,
        exit,
        &entry_contexts,
        call_frames,
    );
    for (path, domain_symbol, semantic_domain) in paths {
        if cases
            .as_ref()
            .is_some_and(|cases| cases.result_path_is_inactive(&path))
        {
            continue;
        }
        let proves = |subject: &CanonicalPlace| {
            if exact_scalar_membership(
                program,
                facts,
                &entry_contexts,
                subject,
                domain_symbol,
                semantic_domain,
            ) {
                true
            } else if !crate::facts::field_domain::domain_requires_provenance(
                program,
                domain_symbol,
            ) {
                super::super::super::prover::prove_domain_at_place(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    subject,
                    domain_symbol,
                )
            } else {
                false
            }
        };
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
                        if !crate::flow::place_cases_are_selected(
                            program,
                            &facts.semantic,
                            &entry_contexts,
                            exit.machine_symbol,
                            exit.state_symbol,
                            exit.statement_index,
                            &subject,
                        ) {
                            return false;
                        }
                        subject.extend_segments(&projection.remaining);
                        proves(&subject)
                    })
                })
        }) || base.as_ref().is_some_and(|base| {
            if !crate::flow::place_cases_are_selected(
                program,
                &facts.semantic,
                &entry_contexts,
                exit.machine_symbol,
                exit.state_symbol,
                exit.statement_index,
                base,
            ) {
                return false;
            }
            let mut subject = base.clone();
            subject.extend_segments(&path);
            proves(&subject)
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
                facts::canonical_place_label_from_parts(program, root, &segments),
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
/// close from predicates alone. Routed return obligations instead consume
/// exact live membership; predicate-only call reseeding must exclude them.
pub(crate) fn value_provable_domain(
    program: &typed_trees::TypedTrees,
    domain_symbol: symbols::SymbolHandle,
) -> bool {
    !crate::facts::field_domain::domain_requires_provenance(program, domain_symbol)
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
/// `&mut` referents: each such parameter's `StateParameterDomain` field facts.
/// These are exactly the rows the state-transition arrival check re-proves
/// (checks/contracts/arrivals.rs); no second obligation vocabulary exists.
pub(crate) fn mutable_referent_field_requirements(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
) -> Vec<facts::Fact> {
    let Some(state) =
        crate::semantic::calls::find_state_in_machine(program, machine_symbol, state_symbol)
    else {
        return Vec::new();
    };
    let mut referents = Vec::new();
    for parameter in program.state_parameters(state) {
        if !is_readable_mutable_reference(program, parameter.type_reference) {
            continue;
        }
        referents.push(parameter.symbol);
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
                    ) && matches!(fact.payload, FactPayload::DomainMembership { .. })
                        && matches!(fact.place, FactPlace::Place(place)
                            if matches!(semantic.places.get(place).root, PlaceRoot::Symbol(root)
                                if referents.contains(&root)))
                })
                .cloned(),
        );
    }
    requirements
}

/// A machine's normal return is a default-domain consumption point for every
/// readable `&mut` referent it received: the root and field facts
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
    let entry_contexts: Vec<_> = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    // Mutable roots use implicit signature preconditions rather than seeded
    // StateParameterDomain facts. Their declaration still creates a return
    // obligation, even when a write retired every copy of the entry evidence.
    if let Some(state) = crate::semantic::calls::find_state_in_machine(
        program,
        exit.machine_symbol,
        exit.state_symbol,
    ) {
        for parameter in program
            .state_parameters(state)
            .iter()
            .filter(|parameter| is_readable_mutable_reference(program, parameter.type_reference))
        {
            for (domain_symbol, semantic_domain) in
                crate::facts::field_domain::domain_constraint_identities(
                    program,
                    parameter.type_reference,
                )
            {
                let subject = CanonicalPlace {
                    root: PlaceRoot::Symbol(parameter.symbol),
                    segments: Vec::new(),
                };
                let established = exact_scalar_membership(
                    program,
                    facts,
                    &entry_contexts,
                    &subject,
                    domain_symbol,
                    semantic_domain,
                );
                // Predicate-only qualifications may be repaired inside the
                // invariant window. At return they need live evidence just
                // as routed qualifications do; their declaration is not a
                // postcondition we may assume. Keep indexed applications on
                // exact instance matching until their substituted predicates
                // are available to the place prover.
                let repaired = !established
                    && value_provable_domain(program, domain_symbol)
                    && program.domain_definitions().iter().any(|domain| {
                        domain.symbol == domain_symbol
                            && typed_trees::domain::index_parameters(program, domain).is_empty()
                    })
                    && super::super::super::prover::prove_domain_at_place(
                        program,
                        &facts.semantic,
                        &entry_contexts,
                        &subject,
                        domain_symbol,
                    );
                if !established && !repaired {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
                        crate::labels::machine_name(program, exit.machine_symbol), exit.statement_index,
                        parameter.name, crate::labels::symbol_name(program, domain_symbol),
                    )));
                }
            }
        }
    }
    for requirement in &requirements {
        let (
            FactPlace::Place(place),
            FactPayload::DomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            },
        ) = (requirement.place, requirement.payload)
        else {
            continue;
        };
        let place = facts.semantic.places.get(place);
        // Returning the whole referent restores only its active payloads.
        // The case-qualified row is an implication, not a claim that this case
        // is selected or that its inactive bytes contain an established value.
        if canonical_place_from_semantic_place(program, &facts.semantic, place).is_some_and(
            |subject| {
                crate::flow::place_case_path_is_inactive(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    exit.machine_symbol,
                    exit.state_symbol,
                    exit.statement_index,
                    &subject,
                )
            },
        ) {
            continue;
        }
        let satisfied =
            if crate::facts::field_domain::domain_requires_provenance(program, domain_symbol) {
                canonical_place_from_semantic_place(program, &facts.semantic, place).is_some_and(
                    |subject| {
                        exact_scalar_membership(
                            program,
                            facts,
                            &entry_contexts,
                            &subject,
                            domain_symbol,
                            semantic_domain,
                        )
                    },
                )
            } else {
                super::super::super::prover::semantic_contexts_prove_contract_fact(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    requirement,
                )
            };
        if satisfied {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove default-domain field requirement for return from {} at statement {}: {} requires {}",
            crate::labels::machine_name(program, exit.machine_symbol),
            exit.statement_index,
            facts::canonical_place_label_from_parts(
                program,
                place.root,
                facts.semantic.place_segments.span_or_empty(place.segments),
            ),
            crate::labels::symbol_name(program, domain_symbol),
        )));
    }
}
