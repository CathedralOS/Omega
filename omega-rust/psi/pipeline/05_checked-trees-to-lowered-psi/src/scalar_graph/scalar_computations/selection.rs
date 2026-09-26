//! Carry completed condition computations into the selected branch.
//!
//! Checked computations form a DAG: an operand used by both a comparison and
//! its selected result is one value, not two evaluations. Expanding it twice
//! loses the relationship between the tested value and the returned value.
//! Retain the condition's strict evaluation prefix in ordinary block parameters.
//! Nested selections retain their own lazy branches; no branch-only computation
//! is moved into this prefix. These bindings end at this selection's result.

use super::{
    Argument, CheckedScalarComputationKind, Computation, Expansion, LoweredScalarBranchState,
    LoweredScalarBranchTerminator, LoweringError, QualifiedScalarType, ScalarType, Site, parameter,
    parameters, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn selection(
        &mut self,
        condition: Computation,
        when_true: Computation,
        when_false: Computation,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        if self.argument_type(&Argument::Computation(condition), site, input_types)?
            != ScalarType::Boolean.into()
            || self.argument_type(&Argument::Computation(when_true), site, input_types)?
                != result_type
            || self.argument_type(&Argument::Computation(when_false), site, input_types)?
                != result_type
        {
            return unsupported("scalar computation selection carriers disagree");
        }
        let order = self.condition_order(condition)?;
        let saved = self.completed.len();
        let result = (|| {
            let mut prefixes = vec![input_types.to_vec()];
            for computation in &order {
                let mut next = prefixes.last().expect("selection input prefix").clone();
                self.completed.push((*computation, next.len()));
                next.push(self.argument_type(
                    &Argument::Computation(*computation),
                    site,
                    input_types,
                )?);
                prefixes.push(next);
            }
            let carried = prefixes.last().expect("completed condition prefix");
            let condition_position = self
                .completed
                .iter()
                .find_map(|(node, position)| (*node == condition).then_some(*position))
                .ok_or(LoweringError::Unsupported(
                    "selection condition has no completed value",
                ))?;
            let mut result_types = carried.clone();
            result_types.push(result_type);
            let mut arguments = parameters(input_types);
            arguments.push(parameter(carried.len(), result_type));
            let completion = self.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                structural_effects: Vec::new(),
                parameter_types: result_types,
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Jump {
                    trivial_affine_discards: Vec::new(),
                    target,
                    arguments,
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                },
            });
            let when_true_target = self.argument(
                &Argument::Computation(when_true),
                carried,
                completion,
                site,
                active,
            )?;
            let when_false_target = self.argument(
                &Argument::Computation(when_false),
                carried,
                completion,
                site,
                active,
            )?;
            let mut continuation = self.push(LoweredScalarBranchState {
                structural_parameters: Vec::new(),
                structural_effects: Vec::new(),
                parameter_types: carried.clone(),
                erased_formal_types: Vec::new(),
                erased_proof_formals: Vec::new(),
                bindings: Vec::new(),
                terminator: LoweredScalarBranchTerminator::Conditional {
                    condition: LoweredBooleanReturnExpression::Parameter {
                        position: condition_position,
                    },
                    when_true_target,
                    when_true_arguments: parameters(carried),
                    when_true_erased_arguments: Vec::new(),
                    when_true_erased_proof_arguments: Vec::new(),
                    when_false_target,
                    when_false_arguments: parameters(carried),
                    when_false_erased_arguments: Vec::new(),
                    when_false_erased_proof_arguments: Vec::new(),
                },
            });
            for (position, computation) in order.iter().enumerate().rev() {
                // Construction runs backwards, execution forwards: only earlier
                // completed values are available while emitting this node.
                self.completed.truncate(saved + position);
                continuation = self.argument(
                    &Argument::Computation(*computation),
                    &prefixes[position],
                    continuation,
                    site,
                    active,
                )?;
            }
            Ok(continuation)
        })();
        self.completed.truncate(saved);
        result
    }

    /// Postorder of strict condition dependencies. Effectful calls and structural
    /// operands stay indivisible under their existing sequencing owners. A nested
    /// selection likewise contributes its result, not its private branch values.
    fn condition_order(&self, condition: Computation) -> Result<Vec<Computation>, LoweringError> {
        let plans = &self.checked.facts.values.scalar_computations;
        let mut pending = vec![(condition, false)];
        let mut active = Vec::new();
        let mut order = Vec::new();
        while let Some((node, completed)) = pending.pop() {
            if completed {
                active.pop();
                order.push(node);
                continue;
            }
            if self.completed.iter().any(|(value, _)| *value == node) || order.contains(&node) {
                continue;
            }
            if !plans.nodes.is_valid(node) || active.contains(&node) {
                return unsupported("selection condition contains a stale or cyclic computation");
            }
            active.push(node);
            pending.push((node, true));
            match &plans.nodes.get(node).kind {
                CheckedScalarComputationKind::Apply { operands, .. } => {
                    let operands =
                        plans
                            .operands
                            .span(*operands)
                            .ok_or(LoweringError::Unsupported(
                                "selection condition has an invalid operand span",
                            ))?;
                    pending.extend(operands.iter().rev().map(|operand| (*operand, false)));
                }
                CheckedScalarComputationKind::SelectedComparison { left, right, .. } => {
                    pending.extend([(*right, false), (*left, false)]);
                }
                CheckedScalarComputationKind::Qualification { operand, .. }
                | CheckedScalarComputationKind::BooleanToInteger { operand, .. } => {
                    pending.push((*operand, false))
                }
                CheckedScalarComputationKind::Value(_)
                | CheckedScalarComputationKind::StructuralField { .. }
                | CheckedScalarComputationKind::CaseMembership { .. }
                | CheckedScalarComputationKind::Select { .. }
                | CheckedScalarComputationKind::Dispatch { .. }
                | CheckedScalarComputationKind::Call { .. } => {}
            }
        }
        Ok(order)
    }
}
