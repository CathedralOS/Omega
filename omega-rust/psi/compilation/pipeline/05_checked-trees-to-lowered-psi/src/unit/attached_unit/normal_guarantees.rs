//! Normal-return guarantees shared by ordinary and composed callable bodies.
//! Unit has no result value, but its post-state can establish caller facts.
//! Publication creates proof obligations; it does not assume the clauses.
//! Exit reconstruction joins the distinct contract result to the returned
//! value and proves guarantees from facts shared by every normal exit.
//! Crashes and incomplete calls establish neither a result nor these facts.

use crate::lowering_error::LoweringError;
use crate::scalar_graph::scalar_contracts::{self, ContractViewNamespace};
use crate::unit::ContractClause;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use terminal_psi::ValueDeclaration;

pub(super) fn lower(
    checked: &CheckedTrees,
    state: SymbolHandle,
    parameters: &[ValueDeclaration],
    result: Option<ValueDeclaration>,
    erased: &[ValueDeclaration],
    views: &ContractViewNamespace<'_>,
    next_obligation: &mut u64,
) -> Result<Vec<ContractClause>, LoweringError> {
    let (source, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if result.is_some() {
        scalar_contracts::validate_guarantees(checked, source, source_state)?;
    } else {
        scalar_contracts::validate_post_state_guarantees(checked, source, source_state)?;
    }
    let contract = checked
        .facts
        .contract_plans
        .for_machine(source.symbol)
        .ok_or(LoweringError::Unsupported(
            "normal return has no checked contract",
        ))?;
    let refined;
    let plan = if result.is_some() {
        refined = scalar_contracts::with_result_range(
            checked,
            state,
            parameters.len(),
            &contract.closed_scalar_values,
        )?;
        &refined
    } else {
        &contract.closed_scalar_values
    };
    let mut namespace = parameters.to_vec();
    namespace.extend(result);
    scalar_contracts::clauses(plan.ensures(), &namespace, erased, views)?
        .into_iter()
        .map(|proposition| {
            Ok(ContractClause {
                obligation: crate::terminal_identities::obligation_id(
                    crate::terminal_identities::allocate_dense(next_obligation)?,
                ),
                proposition,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{TerminalMachineSelection, lower_machine};
    use proof_admission::AdmissionProfile;
    use semantic_vocabulary::IntegerValue;
    use terminal_psi::OperationKind;

    #[test]
    fn unit_call_guarantees_rejoin_the_written_post_state() {
        verify_post_state("self.length = 0;");
    }

    #[test]
    fn composed_unit_call_guarantees_rejoin_the_written_post_state() {
        verify_post_state(
            "transition { _ -> reset() }
             state reset(&mut self) { self.length = 0; }",
        );
    }

    fn verify_post_state(clear_body: &str) {
        let checked = crate::front_end::checked_program(&format!(
            "data Buffer {{ items: [i32; 4]; length: u64 in Wrapping; }}
             machine Buffer::clear(&mut self)
             ensures self.length < self.items.len {{ {clear_body} }}
             machine Buffer::accept(&mut self) requires self.length < self.items.len {{ self.length = 1; }}
             machine Buffer::run(&mut self) {{ self.clear(); self.accept(); }}",
        ));
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Buffer::run"))
            .expect("a Unit call exports its independently proved post-state guarantee");
        assert!(
            lowered
                .semantic_module
                .machines
                .iter()
                .any(|machine| !machine.contract.ensures.is_empty())
        );
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .unwrap();
        let mut substituted = lowered.semantic_module.clone();
        let mut changed = 0;
        for operation in substituted
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
        {
            if matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0)
                }
            ) {
                operation.kind = OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(9),
                };
                changed += 1;
            }
        }
        assert_eq!(changed, 1, "change only the clear store's payload");
        assert!(
            terminal_verifier::verify_module(
                &substituted,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err(),
            "the authored guarantee cannot justify a different write"
        );
    }
}
