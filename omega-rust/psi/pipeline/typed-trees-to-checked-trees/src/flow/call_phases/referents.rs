//! Declared field facts handed back by a call's readable `&mut` referents.
//!
//! A callee's normal return is a default-domain consumption point for every
//! readable `&mut` referent it received, `self` included
//! (checks/contracts/exits/result_domains.rs), so after the call each such
//! referent is known to satisfy the field facts the callee assumed on entry.
//! Call invalidation retires the actual's facts along the callee's write
//! frame; this phase re-seeds exactly those paths on the actual's storage
//! place. It seeds no other place: an argument whose storage origin is not
//! one exact place, and the untouched remainder of a conservative frame, keep
//! whatever coverage survived invalidation.

use crate::flow::CallFlowContexts;
use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::common;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{BorrowCallFact, FlowConstraintKind, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint,
    QualificationEvidence,
};
use symbols::SymbolHandle;

/// Re-seed the declared field facts of every readable `&mut` referent the
/// call hands back, on the referent's exact storage place.
pub(in crate::flow) fn append_call_referent_field_domain_facts(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    pre_contexts: HandleSpan<FlowSemanticContextRef>,
    exit: &mut CallFlowContexts,
) {
    let Some(site) = crate::semantic_calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };
    let Some(parameters) =
        crate::semantic_calls::call_target_parameters(program, borrow_call.target_symbol)
    else {
        return;
    };
    let target_machine = program.machines().iter().find(|candidate| {
        candidate.symbol == borrow_call.target_symbol
            || program
                .machine_states(candidate)
                .iter()
                .any(|target_state| target_state.symbol == borrow_call.target_symbol)
    });
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, &site);
    let mut rows: Vec<(crate::flow::CanonicalPlace, Vec<PlaceSegment>, SymbolHandle)> = Vec::new();
    let mut argument_index = 0usize;
    for parameter in parameters {
        let argument = if parameter.is_self {
            None
        } else {
            let argument = arguments.get(argument_index).copied();
            argument_index += 1;
            argument
        };
        if !crate::checks::contracts::is_readable_mutable_reference(
            program,
            parameter.type_reference,
        ) {
            continue;
        }
        let paths: Vec<(Vec<PlaceSegment>, SymbolHandle)> = if parameter.is_self {
            // The callee re-proves its seeded machine field facts at every
            // return; the receiver gets exactly those rows back.
            let Some(target_machine) = target_machine else {
                continue;
            };
            machine_field_domain_rows(semantic, target_machine.symbol)
        } else {
            // The callee re-proves its parameter's declared field facts,
            // which mirror the referent type's declared paths exactly.
            crate::facts::field_domain::declared_result_field_domain_paths(
                program,
                crate::checks::contracts::result_domain_type(program, parameter.type_reference),
            )
        };
        let paths = paths
            .into_iter()
            .filter(|(_, domain_symbol)| {
                crate::checks::contracts::value_provable_domain(program, *domain_symbol)
            })
            .collect::<Vec<_>>();
        if paths.is_empty() {
            continue;
        }
        let actual = if parameter.is_self {
            crate::flow::canonical_receiver_place_for_call_site(
                program,
                machine.symbol,
                state.symbol,
                &site,
            )
        } else {
            argument.and_then(|argument| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    borrow_call.statement_index,
                    argument,
                )
            })
        };
        let Some(actual) = actual else {
            continue;
        };
        // Facts live on storage places: an argument spelled through a local
        // reference re-seeds its exact origin. A reference bound from a
        // checked reference result names one of finitely many candidates;
        // the callee guarantees the rows only on the candidate it received,
        // and the others were merely retired conservatively, so a candidate
        // gets a row back exactly when that row was live before the call.
        // Any other ambiguity seeds nothing.
        let self_symbol = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol);
        let exact = crate::flow::rebase_exact_local_place(
            program,
            state.symbol,
            borrow_call.statement_index,
            actual.clone(),
            ctx.call_frames,
        );
        let targets: Vec<(crate::flow::CanonicalPlace, bool)> = match exact {
            Some(storage) => vec![(storage, false)],
            None => {
                let PlaceRoot::Symbol(root) = actual.root else {
                    continue;
                };
                let Some(candidates) = crate::flow::reference_result_candidates_before_statement(
                    program,
                    state.symbol,
                    borrow_call.statement_index,
                    root,
                    ctx.call_frames,
                ) else {
                    continue;
                };
                candidates
                    .into_iter()
                    .map(|mut candidate| {
                        candidate.segments.extend_from_slice(&actual.segments);
                        (candidate, true)
                    })
                    .collect()
            }
        };
        for (mut storage, requires_prior_row) in targets {
            // Machine storage canonicalizes to the machine-symbol root; the
            // seeded field facts and their consumers spell it as `self`.
            if storage.root == PlaceRoot::Symbol(machine.symbol)
                && let Some(self_symbol) = self_symbol
            {
                storage.root = PlaceRoot::Symbol(self_symbol);
            }
            for (path, domain_symbol) in &paths {
                if requires_prior_row
                    && !row_was_live(
                        program,
                        semantic,
                        ctx,
                        pre_contexts,
                        &storage,
                        path,
                        *domain_symbol,
                    )
                {
                    continue;
                }
                rows.push((storage.clone(), path.clone(), *domain_symbol));
            }
        }
    }
    if rows.is_empty() {
        return;
    }
    let point = ProgramPoint::CallEnsures {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
    };
    let evidence = QualificationEvidence::from_origin(
        language_semantics::QualificationEvidenceOrigin::Propagated,
        state.symbol,
    );
    // One context per exact place, so a later write to one field retires
    // only that field's row (flow/transfers keeps the same granularity).
    let mut contexts: Vec<(crate::flow::CanonicalPlace, HandleSpan<facts::FactRef>)> = Vec::new();
    for (storage, path, domain_symbol) in rows {
        let mut place = storage.clone();
        place.extend_segments(&path);
        let place_handle = crate::semantic_places::append_place_with_segments(
            semantic,
            place.root,
            &place.segments,
        );
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place_handle),
            point,
            origin: FactOrigin::CallEnsures,
            evidence,
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol,
                semantic_domain: language_semantics::SemanticDomainId::NULL,
            },
        });
        if let Some((_, refs)) = contexts
            .iter_mut()
            .find(|(candidate, _)| *candidate == place)
        {
            semantic.append_ref(refs, fact);
        } else {
            let mut refs = HandleSpan::empty();
            semantic.append_ref(&mut refs, fact);
            contexts.push((place, refs));
        }
    }
    for (_, refs) in contexts {
        let context = semantic.append_context(point, refs);
        common::append_flow_reference(
            &mut ctx.contexts.semantic_context_refs,
            &mut exit.contexts,
            FlowSemanticContextRef { context },
        );
        append_constraint_ref(
            &mut ctx.contexts.constraint_refs,
            &mut exit.constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }
}

/// Whether `storage + path in domain` was live in the contexts active before
/// the call.
fn row_was_live(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &FlowBuildContext,
    pre_contexts: HandleSpan<FlowSemanticContextRef>,
    storage: &crate::flow::CanonicalPlace,
    path: &[PlaceSegment],
    domain_symbol: SymbolHandle,
) -> bool {
    let mut place = storage.clone();
    place.extend_segments(path);
    let place =
        crate::semantic_places::append_place_with_segments(semantic, place.root, &place.segments);
    ctx.contexts
        .semantic_context_refs
        .span_or_empty(pre_contexts)
        .iter()
        .any(|reference| {
            semantic
                .context_view(semantic.contexts.get(reference.context))
                .proves_place_domain_membership_in_program(program, place, domain_symbol)
        })
}

/// The `(field path, domain)` rows of a machine's seeded `MachineFieldDomain`
/// facts, relative to its `self` root.
fn machine_field_domain_rows(
    semantic: &FactPlan,
    machine_symbol: SymbolHandle,
) -> Vec<(Vec<PlaceSegment>, SymbolHandle)> {
    let mut rows = Vec::new();
    for context in semantic.contexts_at_point(ProgramPoint::Machine { machine_symbol }) {
        for fact in context.facts() {
            let FactOrigin::MachineFieldDomain {
                machine_symbol: origin,
            } = fact.origin
            else {
                continue;
            };
            if origin != machine_symbol {
                continue;
            }
            let (FactPlace::Place(place), FactPayload::DomainMembership { domain_symbol, .. }) =
                (fact.place, fact.payload)
            else {
                continue;
            };
            let place = semantic.places.get(place);
            if !matches!(place.root, PlaceRoot::Symbol(_)) {
                continue;
            }
            let segments = semantic
                .place_segments
                .span_or_empty(place.segments)
                .to_vec();
            if !rows.iter().any(|(candidate, candidate_domain)| {
                *candidate == segments && *candidate_domain == domain_symbol
            }) {
                rows.push((segments, domain_symbol));
            }
        }
    }
    rows
}
