//! Build-time evaluation of machine-backed integer-domain facts used by
//! concrete const-generic instances.
//!
//! Generic instance synthesis runs before symbol resolution. It can discharge
//! closed arithmetic facts there, but a fact such as `is_buffer_size(self);`
//! must wait until its callee has a typed symbol and a normalized build-time
//! contract summary. This pass runs immediately after the
//! other typed const-evaluation pass. Proven static obligations leave the
//! concrete record's fact roster; only residual runtime invariants flow into
//! field bounds, stores and entry establishment. Retaining a `true` placeholder
//! there makes a checked capacity look like an unsupported runtime predicate.
//!
//! A fact callee whose reachable closure holds a resolved boundary-operator
//! use cannot run before provider selection: the owning pre-check continuation
//! defers the whole typed stage, then this pass evaluates under the same
//! selected rows the deferred fixed-array lengths and range endpoints use --
//! sealed primitive-float meanings plus each selected provider's ordinary
//! checked body, rebound on a private execution copy so the caller's tree
//! keeps its authored selection.

use arena::Handle;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::domain::{ProofFact, ProofMembershipFact};
use typed_trees::expression::ExpressionNode;

use crate::BuildTimeAdmissionPlan;

mod fact_expression;
mod membership;

pub(in crate::const_evaluation) use membership::evaluate_closed_membership;
use membership::evaluate_membership;

struct PendingMembership {
    fact: Handle<ProofFact>,
    data_symbol: symbols::SymbolHandle,
    instance_name: String,
    membership: ProofMembershipFact,
}

/// Concrete const-generic instance memberships whose subject already folded
/// to an integer literal. Only a retained specialization origin identifies an
/// instance; diagnostic spelling cannot supply that identity.
fn pending_memberships(typed: &TypedTrees) -> Vec<PendingMembership> {
    let mut pending = Vec::new();
    for data in typed.data_definitions() {
        if data.generic_instance.is_none() {
            continue;
        }
        for offset in 0..data.where_facts.count() {
            let fact = Handle::from_parts(
                data.where_facts.start().arena_index() + offset,
                data.where_facts.start().generation(),
            );
            let ProofFact::Membership(membership) = typed.proof_facts.get(fact) else {
                continue;
            };
            if matches!(
                typed.expression_table.expression(membership.value),
                ExpressionNode::Integer(_)
            ) {
                pending.push(PendingMembership {
                    fact,
                    data_symbol: data.symbol,
                    instance_name: data.name.as_str().to_owned(),
                    membership: *membership,
                });
            }
        }
    }
    pending
}

/// Whether any concrete membership's fact tree would invoke a machine whose
/// reachable closure holds a resolved boundary-operator use that only exact
/// selected execution can run. Mirrors the fixed-array-length and
/// range-endpoint deferral gates: the owning continuation waits for Omega's
/// settled rows rather than failing such a membership against an unselected
/// admission.
pub(crate) fn pending_memberships_need_operator_selection(
    typed: &TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<bool, Vec<Diagnostic>> {
    let pending = pending_memberships(typed);
    if pending.is_empty() {
        return Ok(false);
    }
    // Admission and the deferral scan must see the same call closure as the
    // eventual evaluation: this pass executes the unprepared working tree, so
    // the check reads it directly.
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
    let admission = BuildTimeAdmissionPlan::infer(typed, selection_authority);
    Ok(pending.iter().any(|pending| {
        membership::domain_facts_need_operator_selection(
            typed,
            &admission,
            &facts,
            pending.membership.domain_symbol,
            &mut Vec::new(),
        )
    }))
}

pub fn evaluate_const_domain_facts(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    evaluate_selected_domain_facts(
        typed,
        selection_authority,
        crate::SelectedBuildTimeOperators::default(),
    )
}

/// Finish concrete memberships under the exact selected rows Omega settled
/// for this program. `operators` supplies the sealed primitive-float
/// meanings; `provider_bodies` rebinds each retained boundary-operator
/// occurrence to its selected provider's ordinary checked body on the private
/// execution copy -- the caller's tree keeps its authored selection and
/// source-owned handles for final checking, exactly as deferred fixed-array
/// lengths and range endpoints do.
pub(crate) fn evaluate_selected_domain_facts(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    selected: crate::SelectedBuildTimeOperators<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let pending = pending_memberships(typed);
    if pending.is_empty() {
        return Ok(());
    }

    // Admission and execution must see the same selected realization. When a
    // selected use means an ordinary checked provider body, rebind its
    // occurrence to the exact provider entry state on one private copy: the
    // provider joins the ordinary call closure and runs its real body, while
    // the caller's tree and its source-owned handles stay untouched.
    let selected_program =
        crate::machine_execution::selected_operators::prepare_selected_execution(
            typed,
            selected.provider_bodies,
            selected.operators,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let execution = selected_program.as_ref().unwrap_or(typed);
    let admission = BuildTimeAdmissionPlan::infer(execution, selection_authority)
        .with_selected_operators(execution, selected.operators)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut replacements = Vec::new();
    let mut affected_data = Vec::new();
    let mut diagnostics = Vec::new();

    for pending in &pending {
        match evaluate_membership(execution, &admission, pending) {
            Ok(Some(true)) => {
                replacements.push(pending.fact);
                affected_data.push(pending.data_symbol);
            }
            Ok(Some(false)) => diagnostics.push(Diagnostic::error(format!(
                "const fact for generic instance `{}` is false",
                pending.instance_name
            ))),
            Ok(None) => {}
            Err(reason) => diagnostics.push(Diagnostic::error(format!(
                "const domain fact evaluation for generic instance `{}` failed: {reason}",
                pending.instance_name
            ))),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let proven = typed.expression_table.insert(ExpressionNode::Boolean(true));
    for fact in replacements {
        *typed.proof_facts.get_mut(fact) = ProofFact::Expression(proven);
    }
    let residual_facts = typed
        .data_definitions()
        .iter()
        .filter(|data| affected_data.contains(&data.symbol))
        .map(|data| {
            let mut pending = typed
                .proof_facts
                .span_or_empty(data.where_facts)
                .iter()
                .rev()
                .cloned()
                .collect::<Vec<_>>();
            let mut residual = Vec::new();
            while let Some(fact) = pending.pop() {
                if let ProofFact::Expression(expression) = fact {
                    // Conjunction is the same obligation whether authored in
                    // one clause or several. Preserve order and source handles
                    // while removing only independently proved static leaves.
                    if let ExpressionNode::Binary(binary) =
                        typed.expression_table.expression(expression)
                        && binary.operator == typed_trees::expression::BinaryOperator::And
                    {
                        pending.push(ProofFact::Expression(binary.right));
                        pending.push(ProofFact::Expression(binary.left));
                        continue;
                    }
                    if fact_expression::data_fact_is_static_true(typed, data, expression) {
                        continue;
                    }
                }
                residual.push(fact);
            }
            (data.symbol, residual)
        })
        .collect::<Vec<_>>();
    for (symbol, residual) in residual_facts {
        let facts = typed.proof_facts.insert_many(residual);
        typed.data_definitions.for_each_mut(|_, data| {
            if data.symbol == symbol {
                data.where_facts = facts;
            }
        });
    }
    let ungated: Vec<_> = typed
        .data_definitions()
        .iter()
        .filter(|data| affected_data.contains(&data.symbol))
        .filter(|data| {
            // A discharged capacity predicate is not a proof of the other
            // runtime invariants. Re-evaluate those at zero without replacing
            // them: construction, writes and reads still owe the authored fact.
            typed
                .proof_facts
                .span_or_empty(data.where_facts)
                .iter()
                .all(|fact| match fact {
                    ProofFact::Expression(expression) =>
                        fact_expression::data_fact_holds_at_zero(typed, data, *expression),
                    ProofFact::Membership(_) => false,
                    ProofFact::Proposition(_) => false,
                })
                // Zero storage selects the first case. Data-level membership
                // evaluation cannot discharge that case's separate predicates.
                && typed.data_members(data).iter().find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant) => Some(variant),
                    _ => None,
                }).is_none_or(|variant| variant.where_facts.is_empty())
        })
        .map(|data| data.symbol)
        .collect();
    typed
        .data_definitions
        .for_each_mut(|_, data| data.zero_gated &= !ungated.contains(&data.symbol));
    Ok(())
}

#[cfg(test)]
mod tests;
