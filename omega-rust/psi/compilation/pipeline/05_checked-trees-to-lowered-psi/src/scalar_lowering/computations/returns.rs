//! Normal-result formation is not an ordinary operand coercion. The source
//! checker has already proved the exact exit's predicates and provenance.
//! Complete its ordinary operand schedule first; only the fresh completion
//! parameter receives the declared result qualification. Other retained-value
//! entrypoints keep exact type equality and cannot borrow this admission.

use super::{
    Argument, CheckedScalarExpressionRole, Expansion, LoweringError, QualifiedScalarType, Site,
    storage, unsupported,
};
use crate::checked_facts::value_facts;
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarEffect,
};

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn normal_return(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        exit: &checked_trees::FlowClaimJoinExit,
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let statement = u32::try_from(exit.statement_index)
            .map_err(|_| LoweringError::Unsupported("scalar return statement exceeds u32"))?;
        let mut roots = value_facts(&self.checked)
            .scalar_computations
            .roots_for_statement_role(exit.state_symbol, statement, role);
        let argument = if let Some(root) = roots.next() {
            if roots.next().is_some() {
                return unsupported("scalar computation root custody is duplicated or mismatched");
            }
            self.retained_root_argument(identities, root, bindings)?
        } else {
            Argument::Value(bindings.expression_at(
                identities,
                self.checked,
                exit.state_symbol,
                statement,
                role,
            )?)
        };
        let site = Site {
            state: exit.state_symbol,
            statement,
            bindings,
        };
        let source = self.argument_type(identities, &argument, &site, source_types)?;
        let completion = self.qualifications.return_completion(
            self.checked,
            self.machine,
            exit,
            source,
            result_type,
        )?;
        let destination = completion.destination();
        let target_state = target
            .checked_sub(self.base)
            .and_then(|index| self.states.get(index))
            .ok_or(LoweringError::Unsupported(
                "scalar return completion target is absent",
            ))?;
        if target_state.parameter_types.len() != source_types.len() + 1
            || target_state.parameter_types[..source_types.len()] != *source_types
            || target_state.parameter_types.last() != Some(&destination)
        {
            return unsupported("checked scalar return completion disagrees with its destination");
        }
        let copies = if role == CheckedScalarExpressionRole::Return {
            crate::expression_preparation::selected_values::roots(
                self.checked,
                self.machine,
                exit.state_symbol,
                statement,
            )
        } else {
            Vec::new()
        };
        let target = if copies.is_empty() {
            target
        } else {
            let mut completed_types = source_types.to_vec();
            completed_types.push(destination);
            self.push(LoweredScalarBranchState::jumping(
                target, Vec::new(), completed_types.clone(), &completed_types,
                vec![LoweredScalarEffect::CompleteSnapshots {
                    snapshots: copies.iter().rev().map(|root|
                        crate::scalar_lowering::branch_states::prepared::CompletedSnapshot::Produced(root.root)
                    ).collect(),
                }],
            ))
        };
        let target = if source == destination {
            target
        } else {
            let mut completed_types = source_types.to_vec();
            completed_types.push(source);
            self.push(super::LoweredScalarBranchState::continuing(
                completed_types,
                super::LoweredScalarBranchTerminator::CompleteReturn {
                    target,
                    exit: *exit,
                },
            ))
        };
        let target = self.argument(
            identities,
            &argument,
            source_types,
            target,
            &site,
            &mut Vec::new(),
        )?;
        let mut effects = Vec::new();
        for root in copies {
            let request =
                crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(
                    identities,
                    self.checked,
                    root.root,
                    self.structural_types,
                )?;
            effects.push(LoweredScalarEffect::ProduceStructuralValue {
                value: request.value,
                structural_type: request.structural_type,
            });
        }
        Ok(if effects.is_empty() {
            target
        } else {
            self.push(LoweredScalarBranchState::jumping(
                target,
                Vec::new(),
                source_types.to_vec(),
                source_types,
                effects,
            ))
        })
    }
}
