//! Declared field obligations on returned values. An `ensures` domain fact
//! authored over the reserved `result` name is discharged against the exact
//! returned expression's live place; a nominal return type additionally makes
//! every declared field predicate an obligation at each value-returning exit.
//! Both consume only live evidence: a write that retired the returned field's
//! fact fails the proof, so a nominal annotation alone restores nothing.

use checked_trees::{CheckFacts, FlowExitFact};
use diagnostics::Diagnostic;
use facts::{FactContextHandle, FactPayload, FactPlace, PlaceRoot};

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
/// (checks/contracts/nominal_inputs.rs). Reference returns own no result
/// storage and declare no obligations here; borrowed fields stay proven
/// through their source places.
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
    let paths =
        crate::facts::field_domain::declared_result_field_domain_paths(program, entry.return_type);
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
