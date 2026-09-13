//! The ordered prefix completes before its authored scalar dispatch. Both arms
//! use the existing selective evaluator and rejoin with one result; structural
//! places keep their dominating producers instead of being rebuilt in each arm.

use super::*;
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

impl Evaluation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scalar_control_result(
        &mut self,
        checked: &CheckedTrees,
        machine: &CheckedUnitEffectMachinePlan,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let control = machine
            .scalar_control
            .as_ref()
            .ok_or(LoweringError::Unsupported(
                "ordered scalar body has no control completion",
            ))?;
        let bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| crate::scalar_bindings::ScalarBindings::new(values.len()))
            .with_primitive_storage(&self.primitive_storage)
            .with_local_cases(&self.local_cases)
            .with_structural_locals(&self.structural_locals)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases);
        let qualifications = prepare_shared_qualifications(checked, machine.machine, values)?;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let result_type = terminal_scalar_type(control.primitive_type)?;
        let mut expansion = crate::scalar_computations::Expansion::new(
            checked,
            &qualifications,
            machine.machine,
            1,
        )
        .with_arrays(&self.arrays)
        .with_cases(&self.cases)
        .with_fields(&self.record_fields);
        let mut arm = |destination: &CheckedScalarBranchDestination| {
            let CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = destination
            else {
                return unsupported("ordered scalar completion cannot drop a non-returning edge");
            };
            let role = if *is_continuation {
                CheckedScalarExpressionRole::ContinuationReturn
            } else {
                CheckedScalarExpressionRole::Return
            };
            // Presence selects computation replay; ambiguity must be rejected
            // there, never interpreted as permission to use a pure fallback.
            if checked
                .facts
                .values
                .scalar_computations
                .roots
                .iter()
                .any(|(_, root)| {
                    root.state == machine.state
                        && root.statement_ordinal == *statement_ordinal
                        && root.role == role
                })
            {
                expansion.retained_value(
                    machine.state,
                    *statement_ordinal,
                    role,
                    symbols::SymbolHandle::invalid(),
                    &bindings,
                    &source_types,
                    result_type.into(),
                    0,
                )
            } else {
                expansion.retained_pure_value(
                    machine.state,
                    *statement_ordinal,
                    role,
                    &bindings,
                    &source_types,
                    result_type.into(),
                    0,
                )
            }
        };
        let terminator = match &control.terminator {
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                let true_target = arm(when_true)?;
                let false_target = arm(when_false)?;
                crate::scalar_graph_lowering::guards::lower(
                    checked,
                    machine.state,
                    *guard_statement_ordinal,
                    &bindings,
                    &source_types,
                    (
                        true_target,
                        crate::scalar_computations::parameters(&source_types),
                    ),
                    (
                        false_target,
                        crate::scalar_computations::parameters(&source_types),
                    ),
                    when_false,
                    &mut expansion,
                )?
            }
            CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                crate::scalar_source_custody::guarded_exits::validate(
                    checked,
                    machine.state,
                    *arms,
                    fallback.as_ref(),
                )?;
                let arms = checked
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span(*arms)
                    .ok_or(LoweringError::Unsupported(
                        "ordered scalar guard roster is stale",
                    ))?;
                let targets = arms
                    .iter()
                    .map(|guard| arm(&guard.destination))
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let mut next = if let Some(fallback) = fallback {
                    arm(fallback)?
                } else {
                    *targets.last().ok_or(LoweringError::Unsupported(
                        "ordered scalar guards are empty",
                    ))?
                };
                // Every guard, including the final exhaustive case, is observed
                // once. Its false continuation is unreachable by the independent
                // coverage replay above, not an invented implicit fallback.
                for (guard, target) in arms.iter().zip(targets).rev() {
                    let terminator = crate::scalar_graph_lowering::guards::evaluate(
                        checked,
                        machine.state,
                        guard.guard_statement_ordinal,
                        &bindings,
                        &source_types,
                        (
                            target,
                            crate::scalar_computations::parameters(&source_types),
                        ),
                        (next, crate::scalar_computations::parameters(&source_types)),
                        &mut expansion,
                    )?;
                    next = expansion.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        structural_effects: Vec::new(),
                        parameter_types: source_types.clone(),
                        bindings: Vec::new(),
                        terminator,
                    });
                }
                LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    structural_arguments: Vec::new(),
                    target: next,
                    arguments: crate::scalar_computations::parameters(&source_types),
                }
            }
            _ => return unsupported("ordered scalar completion requires a guarded return"),
        };
        let entry = expansion.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: source_types,
            bindings: Vec::new(),
            terminator,
        });
        let states = expansion.finish();
        let completed = self.complete_expansion(
            &states,
            entry,
            &[result_type.into()],
            values,
            next_value,
            next_block,
            next_edge,
            operations,
            calls,
        )?;
        match completed.as_slice() {
            [result] => Ok(*result),
            _ => unsupported("ordered scalar control has no unique completed value"),
        }
    }
}
