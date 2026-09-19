//! The ordered prefix completes before its authored scalar return or dispatch.
//! Returns use the existing selective evaluator and rejoin with one result;
//! structural places keep their dominating producers across guarded arms.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, Evaluation, LoweringError, ValueDeclaration,
    prepare_shared_qualifications, terminal_scalar_type, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

impl Evaluation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scalar_control_result(
        &mut self,
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        control: &checked_trees::CheckedUnitScalarControlPlan,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        operations: &mut OperationBuffer,
        calls: &mut CallEmissionContext<'_>,
    ) -> Result<ValueDeclaration, LoweringError> {
        let bindings = self
            .scalar_bindings
            .clone()
            .unwrap_or_else(|| {
                crate::expression_preparation::bindings::ScalarBindings::new(values.len())
            })
            .with_primitive_storage(&self.primitive_storage)
            .with_local_cases(&self.local_cases)
            .with_structural_locals(&self.structural_locals)
            .with_structural_parameters(&self.structural_parameters)
            .with_resolved_structural_observations(&self.structural_fields, &self.structural_cases);
        let qualifications = prepare_shared_qualifications(checked, machine, values)?;
        let source_types = values
            .iter()
            .map(|value| value.value_type())
            .collect::<Vec<_>>();
        let result_type = terminal_scalar_type(control.primitive_type)?;
        let mut expansion = crate::scalar_graph::scalar_computations::Expansion::new(
            checked,
            &qualifications,
            machine,
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
                    root.state == state
                        && root.statement_ordinal == *statement_ordinal
                        && root.role == role
                })
            {
                expansion.retained_value(
                    state,
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
                    state,
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
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                let target = arm(&CheckedScalarBranchDestination::Return {
                    statement_ordinal: *statement_ordinal,
                    is_continuation: false,
                })?;
                LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    structural_arguments: Vec::new(),
                    target,
                    arguments: crate::scalar_graph::scalar_computations::parameters(&source_types),
                    erased_arguments: Vec::new(),
                }
            }
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                let true_target = arm(when_true)?;
                let false_target = arm(when_false)?;
                crate::scalar_graph::scalar_graph_lowering::guards::lower(
                    checked,
                    state,
                    *guard_statement_ordinal,
                    &bindings,
                    &source_types,
                    (
                        true_target,
                        crate::scalar_graph::scalar_computations::parameters(&source_types),
                        Vec::new(),
                    ),
                    (
                        false_target,
                        crate::scalar_graph::scalar_computations::parameters(&source_types),
                        Vec::new(),
                    ),
                    when_false,
                    &mut expansion,
                )?
            }
            CheckedScalarStateTerminator::Guarded { arms, fallback } => {
                crate::expression_preparation::source_custody::guarded_exits::validate(
                    checked,
                    state,
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
                    let terminator = crate::scalar_graph::scalar_graph_lowering::guards::evaluate(
                        checked,
                        state,
                        guard.guard_statement_ordinal,
                        &bindings,
                        &source_types,
                        (
                            target,
                            crate::scalar_graph::scalar_computations::parameters(&source_types),
                            Vec::new(),
                        ),
                        (
                            next,
                            crate::scalar_graph::scalar_computations::parameters(&source_types),
                            Vec::new(),
                        ),
                        &mut expansion,
                    )?;
                    next = expansion.push(LoweredScalarBranchState {
                        structural_parameters: Vec::new(),
                        structural_effects: Vec::new(),
                        parameter_types: source_types.clone(),
                        erased_formal_types: Vec::new(),
                        bindings: Vec::new(),
                        terminator,
                    });
                }
                LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    structural_arguments: Vec::new(),
                    target: next,
                    arguments: crate::scalar_graph::scalar_computations::parameters(&source_types),
                    erased_arguments: Vec::new(),
                }
            }
            _ => return unsupported("ordered scalar completion requires a returning tail"),
        };
        let entry = expansion.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: source_types,
            erased_formal_types: Vec::new(),
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
