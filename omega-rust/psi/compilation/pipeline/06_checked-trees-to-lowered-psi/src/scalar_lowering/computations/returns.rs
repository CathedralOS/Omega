//! Normal-result formation is not an ordinary operand coercion. The source
//! checker has already proved the exact exit's predicates and provenance.
//! Complete its ordinary operand schedule first; only the fresh completion
//! parameter receives the declared result qualification. Other retained-value
//! entrypoints keep exact type equality and cannot borrow this admission.

use super::{
    Argument, CheckedScalarExpressionRole, Expansion, LoweringError, QualifiedScalarType, Site,
    storage, unsupported,
};
use crate::shared::checked_facts::value_facts;

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn normal_return(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        exit: &checked_trees::FlowClaimJoinExit,
        copied_payloads: arena::HandleSpan<checked_trees::FlowOperandFact>,
        role: CheckedScalarExpressionRole,
        bindings: &storage::ScalarBindings,
        source_types: &[QualifiedScalarType],
        result_type: QualifiedScalarType,
        target: usize,
    ) -> Result<usize, LoweringError> {
        let statement = u32::try_from(exit.statement_index)
            .map_err(|_| LoweringError::Unsupported("scalar return statement exceeds u32"))?;
        let mut roots = value_facts(self.checked)
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
        let copies = self.selected_payload_values(identities, exit, copied_payloads)?;
        let target = if copies.is_empty() {
            target
        } else {
            let mut completed_types = source_types.to_vec();
            completed_types.push(source);
            self.push(super::LoweredScalarBranchState::jumping(
                target, Vec::new(), completed_types.clone(), &completed_types,
                vec![crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect::CompleteSnapshots {
                    snapshots: copies.iter().map(|copy| {
                        crate::scalar_lowering::branch_states::prepared::CompletedSnapshot::Produced(copy.value)
                    }).collect(),
                }],
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
        if copies.is_empty() {
            return Ok(target);
        }
        // This state is reached only after selection. Every snapshot completes
        // before the return's operands; only these private homes die afterward.
        Ok(self.push(super::LoweredScalarBranchState::jumping(
            target, Vec::new(), source_types.to_vec(), source_types,
            copies.into_iter().map(|copy| {
                crate::scalar_lowering::branch_states::prepared::LoweredScalarEffect::ProduceStructuralValue {
                    value: copy.value, structural_type: copy.structural_type,
                }
            }).collect(),
        )))
    }
    fn selected_payload_values(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        exit: &checked_trees::FlowClaimJoinExit,
        copied_payloads: arena::HandleSpan<checked_trees::FlowOperandFact>,
    ) -> Result<Vec<super::super::branch_states::structural_values::ValueRequest>, LoweringError>
    {
        let mut values = Vec::new();
        for copy in crate::shared::checked_facts::flow_facts(self.checked)
            .control
            .operands
            .span(copied_payloads)
            .ok_or(LoweringError::Unsupported(
                "selected scalar payload roster is stale",
            ))?
        {
            if copy.state_symbol != exit.state_symbol
                || copy.statement_index != exit.statement_index
            {
                return unsupported("selected scalar payload has another source coordinate");
            }
            if self
                .checked
                .primitive_type_reference(copy.type_reference)
                .is_some()
                && validation::reference_shell(&self.checked.typed, copy.type_reference).is_none()
            {
                continue;
            }
            let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                copy.completion
            else {
                return unsupported("selected scalar payload has no completed copy identity");
            };
            let root = value_facts(self.checked)
                .structural_values
                .root_for_captured_copy(
                    exit.state_symbol,
                    u32::try_from(exit.statement_index).map_err(|_| {
                        LoweringError::Unsupported("scalar return statement exceeds u32")
                    })?,
                    identity,
                )
                .ok_or(LoweringError::Unsupported(
                    "selected scalar payload has no checked value producer",
                ))?;
            if !validation::type_satisfies_declared_property(
                &self.checked.typed,
                &[],
                root.type_reference,
                validation::DeclaredPropertyRequirement::Copy,
            ) || !validation::has_contents(
                &self.checked.typed,
                root.type_reference,
                &[],
                validation::ContentsRequirement::DescriptorCleanup,
            ) {
                return unsupported("selected scalar payload lost its copy and cleanup capability");
            }
            values.push(
                super::super::branch_states::structural_values::ValueRequest::prepare(
                    identities,
                    self.checked,
                    root.root,
                    self.structural_types,
                )?,
            );
        }
        Ok(values)
    }
}
