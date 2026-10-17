//! Returned storage owes both root qualifications and declared field domains.
//!
//! An executable predicate-only structural root is no less a promise than a
//! scalar or a routed root. Checking only its fields would let a byte-field copy
//! claim a stronger encoding on return. Value establishment and return admission
//! share the occurrence-bound qualification judgment; annotations supply obligations,
//! not membership. Field requirements still prove at their exact live paths.
//! Proof-only predicate roots remain outside this storage judgment, as before;
//! mathematical constructor entailment is not occurrence-bound storage evidence.

use validation::{CanonicalPlace, canonical_place_from_expression_in_state};

use super::super::super::return_values::exit_return_expression;
use super::scalar_exits::*;
use crate::flow::canonical_place_from_semantic_place;
use checked_trees::{CheckFacts, FlowExitFact, FlowStateFact};
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit: &FlowExitFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program.machine(exit.machine_symbol) else {
        return;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    let mut paths = validation::field_domains::declared_result_field_domain_paths(
        program,
        result_domain_type(program, entry.return_type),
    )
    .into_iter()
    .filter(|(_, domain_symbol, _)| value_provable_domain(program, *domain_symbol))
    .collect::<Vec<_>>();
    paths.extend(
        validation::field_domains::declared_owned_field_domain_identities(
            program,
            result_domain_type(program, entry.return_type),
        ),
    );
    if program
        .primitive_type_reference(entry.return_type)
        .is_none()
    {
        let proof_only = validation::proof_only_classification(program);
        paths.extend(
            validation::field_domains::domain_constraint_identities(program, entry.return_type)
                .into_iter()
                .filter(|(domain, _)| {
                    validation::field_domains::domain_requires_provenance(program, *domain)
                        || !proof_only.is_proof_machine(program, machine)
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
    // A reference return observes the completed descriptor's live referents.
    // An owned return keeps its detached value namespace below; redirecting a
    // copy to a historical source would let later writes change its evidence.
    let readable_reference = validation::reference_shell(program, entry.return_type)
        .is_some_and(|shell| shell.access.is_readable());
    let returned_storage = readable_reference
        .then(|| {
            if !facts.flow.control.operands.is_valid(exit.returned_value) {
                return None;
            }
            let operand = facts.flow.control.operands.get(exit.returned_value);
            if operand.expression != returned
                || operand.state_symbol != exit.state_symbol
                || operand.statement_index != exit.statement_index
            {
                return None;
            }
            let referents = facts
                .flow
                .control
                .operand_referents
                .span(operand.referents)?;
            if referents.is_empty() {
                return None;
            }
            referents
                .iter()
                .map(|referent| {
                    Some(CanonicalPlace {
                        root: referent.root,
                        segments: facts
                            .flow
                            .control
                            .operand_referent_segments
                            .span(referent.segments)?
                            .to_vec(),
                    })
                })
                .collect::<Option<Vec<_>>>()
        })
        .flatten();
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
        let proves_in = |contexts: &[facts::FactContextHandle], subject: &CanonicalPlace| {
            if exact_scalar_membership(
                program,
                facts,
                contexts,
                subject,
                domain_symbol,
                semantic_domain,
            ) {
                true
            } else if !validation::field_domains::domain_requires_provenance(program, domain_symbol)
            {
                super::super::super::prover::prove_domain_at_place(
                    program,
                    &facts.semantic,
                    contexts,
                    subject,
                    domain_symbol,
                )
            } else {
                false
            }
        };
        let proves = |subject: &CanonicalPlace| proves_in(&entry_contexts, subject);
        // The same two shapes as call actuals (checks/contracts/nominal_inputs):
        // a returned constructor projects each requirement onto the exact field
        // or element expression it was built from; anything else proves at the
        // returned place directly.
        // A structural result's root annotation is an obligation too, not
        // evidence available to its body. Use the same occurrence-bound
        // membership judgment as value construction: it proves literals,
        // completed calls and live places without minting routed membership.
        // A reference return with captured referents proves at that storage
        // below; one without (a returned literal has no storage) keeps this
        // value judgment, which is what grants a literal its domain.
        let established_root = path.is_empty()
            && returned_storage.is_none()
            && program.domain_definition(domain_symbol).is_some_and(|_| {
                // Only this returned root may acquire provenance from the
                // authorized machine. Its children still owe their own live
                // memberships; authorization never proves a predicate or an
                // indexed application from its declaration alone.
                validation::field_domains::domain_constraint_identities(program,entry.return_type)
                    .contains(&(domain_symbol,semantic_domain))
                    && crate::facts::qualification_evidence::machine_has_checked_domain_establishment(
                        program, machine, domain_symbol,
                    )
                    && super::super::super::writes::value_satisfies_domain_predicates_in_contexts(
                        program, facts, state_flow, exit.statement_index, returned,
                        domain_symbol, &entry_contexts, &mut Vec::new(),
                        facts.flow.control.operands.is_valid(exit.returned_value)
                            .then(|| facts.flow.control.operands.get(exit.returned_value)),
                        call_frames,
                    )
            });
        let root_satisfied = established_root
            || (returned_storage.is_none()
                && path.is_empty()
                && super::super::super::writes::value_proves_qualification_in_contexts(
                    program,
                    facts,
                    state_flow,
                    exit.statement_index,
                    returned,
                    domain_symbol,
                    semantic_domain,
                    &entry_contexts,
                ));
        let satisfied = if let Some(subjects) = returned_storage.as_ref() {
            {
                subjects.iter().all(|storage| {
                    crate::flow::place_cases_are_selected(
                        program,
                        &facts.semantic,
                        &entry_contexts,
                        exit.machine_symbol,
                        exit.state_symbol,
                        exit.statement_index,
                        storage,
                    ) && crate::flow::project_captured_view_place(storage.clone(), &path)
                        .is_some_and(|subject| {
                            crate::flow::place_case_path_is_inactive(
                                program,
                                &facts.semantic,
                                &entry_contexts,
                                exit.machine_symbol,
                                exit.state_symbol,
                                exit.statement_index,
                                &subject,
                            ) || proves(&subject)
                        })
                })
            }
        } else {
            // Consume the exact completed return tree. A child copied before a
            // later sibling mutation keeps its own facts; syntax/current storage
            // cannot replace that evaluated occurrence's evidence.
            let captured_satisfies = |selected: &[facts::PlaceSegment]| {
                facts.flow.control.operands.is_valid(exit.returned_value)
                    && crate::flow::captured_operand_values(
                        program,
                        &facts.flow,
                        &facts.semantic,
                        state_flow,
                        facts.flow.control.operands.get(exit.returned_value),
                        entry.return_type,
                        selected,
                    )
                    .is_some_and(|values| {
                        !values.is_empty()
                            && values.iter().all(|captured| {
                                let contexts = crate::flow::operand_contexts(
                                    program,
                                    &facts.flow,
                                    &facts.semantic,
                                    captured.operand,
                                    captured.type_reference,
                                    exit.entry_constraints,
                                );
                                crate::flow::captured_value_places(program, &facts.flow, captured)
                                    .is_some_and(|places| {
                                        !places.is_empty()
                                            && places.iter().all(|place| {
                                                crate::flow::place_cases_are_selected(
                                                    program,
                                                    &facts.semantic,
                                                    &contexts,
                                                    exit.machine_symbol,
                                                    exit.state_symbol,
                                                    exit.statement_index,
                                                    &place.storage,
                                                ) && (crate::flow::place_case_path_is_inactive(
                                                    program,
                                                    &facts.semantic,
                                                    &contexts,
                                                    exit.machine_symbol,
                                                    exit.state_symbol,
                                                    exit.statement_index,
                                                    &place.projected,
                                                ) || proves_in(&contexts, &place.projected))
                                            })
                                    })
                            })
                    })
            };
            // A completed owned value keeps its observations independently of
            // the source storage. Project the returned value, not the syntax of
            // a field initializer after later sibling effects. Routed and
            // indexed membership still need their exact evidence owner.
            let completed_satisfies = || {
                !validation::field_domains::domain_requires_provenance(program, domain_symbol)
                    && program
                        .domain_definition(domain_symbol)
                        .is_some_and(|domain| {
                            !program.proof_facts(domain).is_empty()
                                && program.proof_facts(domain).iter().all(|fact| {
                                    fact.as_expression().is_some_and(|predicate| {
                                        program
                                            .state_in_machine(machine, exit.state_symbol)
                                            .is_some_and(|state| {
                                                validation::completed_value_proves_domain_predicate(
                                                    program,
                                                    machine,
                                                    state,
                                                    exit.statement_index,
                                                    returned,
                                                    domain_symbol,
                                                    *predicate,
                                                    &path,
                                                )
                                            })
                                    })
                                })
                        })
            };
            root_satisfied || completed_satisfies() || captured_satisfies(&path)
        };
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
fn result_domain_type(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    domain_symbol: symbols::SymbolHandle,
) -> bool {
    !validation::field_domains::domain_requires_provenance(program, domain_symbol)
}

/// Whether a declared reference supplies readable mutable alias authority.
/// This is narrower than the normal-return validity obligation: write-only
/// references owe validity too, but cannot supply a readable alias.
pub(crate) fn is_readable_mutable_reference(
    program: &dyn ::validation::CheckingInput,
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

/// The referent whose validity an exclusive borrow owes on normal return.
/// Write-only authority changes which operations may establish that validity,
/// not the requirement to return a valid value. The retained entry memberships
/// are invariant premises; this query grants no observation or readable alias.
pub(crate) fn exclusive_referent_type(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> typed_trees::types::TypeReferenceHandle {
    let mut reference = type_reference;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference {
                referee, access, ..
            } if matches!(
                access,
                language_semantics::ReferenceAccess::Mutable
                    | language_semantics::ReferenceAccess::WriteOnly
            ) =>
            {
                return *referee;
            }
            _ => break,
        }
    }
    typed_trees::types::TypeReferenceHandle::invalid()
}

/// The default-domain field facts a state assumes on entry for its exclusive
/// referents: each such parameter's `StateParameterDomain` field facts.
/// These are exactly the rows the state-transition arrival check re-proves
/// (checks/contracts/arrivals.rs); no second obligation vocabulary exists.
pub(crate) fn exclusive_referent_field_requirements(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    machine_symbol: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
) -> Vec<facts::Fact> {
    let Some(state) = validation::find_state_in_machine(program, machine_symbol, state_symbol)
    else {
        return Vec::new();
    };
    let mut referents = Vec::new();
    for parameter in program.state_parameters(state) {
        if !exclusive_referent_type(program, parameter.type_reference).is_valid() {
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
/// mutable or write-only referent it received: the root and field facts
/// assumed on entry must be provable again from the live evidence at the
/// exit. A write through a bare alias (`&mut [u8; 4]` onto a `Utf8` field)
/// retires the field's fact without establishing anything, so returning
/// without repairing it rejects here with the exact place, the same way a
/// call or transition would refuse the corrupted referent.
pub(in crate::checks::contracts) fn check_exclusive_referent_field_domains(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let requirements = exclusive_referent_field_requirements(
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
    // Exclusive roots use implicit signature preconditions rather than seeded
    // StateParameterDomain facts. Their declaration still creates a return
    // obligation, even when a write retired every copy of the entry evidence.
    if let Some(state) = validation::find_state_for_exit(program, exit) {
        for parameter in program.state_parameters(state).iter().filter(|parameter| {
            exclusive_referent_type(program, parameter.type_reference).is_valid()
        }) {
            for (domain_symbol, semantic_domain) in
                validation::field_domains::domain_constraint_identities(
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
            if validation::field_domains::domain_requires_provenance(program, domain_symbol) {
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
                    facts,
                    facts::ProgramPoint::Statement {
                        machine_symbol: exit.machine_symbol,
                        state_symbol: exit.state_symbol,
                        statement_index: exit.statement_index,
                    },
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
