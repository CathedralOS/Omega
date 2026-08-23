//! Build-time evaluation of machine-backed integer-domain facts used by
//! concrete const-generic instances.
//!
//! Generic instance synthesis runs before symbol resolution. It can discharge
//! closed arithmetic facts there, but a fact such as `is_buffer_size(self);`
//! must wait until its callee has a typed symbol and a normalized build-time
//! contract summary. This pass runs immediately after the
//! other typed const-evaluation pass and replaces a proven concrete membership
//! with the ordinary `true` fact consumed by checking.

use psi_arena::Handle;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::{ProofFact, ProofMembershipFact};
use psi_typed_trees::expression::ExpressionNode;

use crate::BuildTimeAdmissionPlan;

mod fact_expression;
mod membership;

use membership::evaluate_membership;

struct PendingMembership {
    fact: Handle<ProofFact>,
    data_symbol: psi_symbols::SymbolHandle,
    instance_name: String,
    membership: ProofMembershipFact,
}

/// Evaluate direct `machine(self)` facts for literal memberships copied
/// into synthesized const-generic data definitions.
pub fn evaluate_const_domain_facts(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut pending = Vec::new();
    for data in typed.data_definitions() {
        // The unspecialized template has no angle-bracket spelling and must
        // retain its symbolic fact for ordinary generic validation.
        if !data.name.as_str().contains('<') {
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

    if pending.is_empty() {
        return Ok(());
    }

    let admission = BuildTimeAdmissionPlan::infer(typed);
    let mut replacements = Vec::new();
    let mut affected_data = Vec::new();
    let mut diagnostics = Vec::new();

    for pending in pending {
        match evaluate_membership(typed, &admission, &pending) {
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
    let ungated: Vec<_> = typed
        .data_definitions()
        .iter()
        .filter(|data| affected_data.contains(&data.symbol))
        .filter(|data| {
            typed
                .proof_facts
                .span_or_empty(data.where_facts)
                .iter()
                .all(|fact| match fact {
                    ProofFact::Expression(expression) => matches!(
                        typed.expression_table.expression(*expression),
                        ExpressionNode::Boolean(true)
                    ),
                    ProofFact::Membership(_) => false,
                    ProofFact::Proposition(_) => false,
                })
        })
        .map(|data| data.symbol)
        .collect();
    typed
        .data_definitions
        .for_each_mut(|_, data| data.zero_gated &= !ungated.contains(&data.symbol));
    Ok(())
}
