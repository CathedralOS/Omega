//! One lexical mixed prefix for scalar consumers of ordinary values.
//! Each scalar snapshot or structural producer completes in source order.
//! The consumer receives the exact resulting handles; the whole expression's
//! normal continuation retains its independently checked retirement actions.
use super::{
    Argument, Computation, EstablishedValue, Expansion, LoweringError, QualifiedScalarType, Site,
    parameter, parameters,
};
use crate::scalar_lowering::branch_states::prepared::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};
use checked_trees::CheckedScalarEvaluationOperand as Operand;

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn operand_scope(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        operands: arena::HandleSpan<Operand>,
        operand: Computation,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let operands = self
            .checked
            .facts
            .values
            .scalar_computations
            .evaluation_operands
            .span(operands)
            .ok_or(LoweringError::Unsupported(
                "scalar operand scope has an invalid mixed span",
            ))?
            .to_vec();
        let mut prefixes = vec![input_types.to_vec()];
        for operand in &operands {
            let mut prefix = prefixes.last().expect("operand input prefix").clone();
            if let Operand::Scalar(value) = operand {
                prefix.push(self.argument_type(
                    identities,
                    &Argument::Computation(*value),
                    site,
                    input_types,
                )?);
            }
            prefixes.push(prefix);
        }
        let result_type = self.argument_type(
            identities,
            &Argument::Computation(operand),
            site,
            input_types,
        )?;
        let scoped_types = prefixes.last().expect("completed operand prefix");
        let mut result_types = scoped_types.clone();
        result_types.push(result_type);
        let mut arguments = parameters(input_types);
        arguments.push(parameter(scoped_types.len(), result_type));
        let completion = self.push(LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: result_types,
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::jump(target, arguments),
        });
        let retained = self.established_values.len();
        let completed = self.completed.len();
        self.retain_scope_prefix(&operands, &prefixes);
        let result = self.argument(
            identities,
            &Argument::Computation(operand),
            scoped_types,
            completion,
            site,
            active,
        );
        self.established_values.truncate(retained);
        self.completed.truncate(completed);
        let mut continuation = result?;
        for (ordinal, value) in operands.iter().enumerate().rev() {
            self.retain_scope_prefix(&operands[..ordinal], &prefixes);
            let entry = match value {
                Operand::Scalar(value) => self.argument(
                    identities,
                    &Argument::Computation(*value),
                    &prefixes[ordinal],
                    continuation,
                    site,
                    active,
                ),
                Operand::Structural(value) => {
                    crate::scalar_lowering::branch_states::structural_values::ValueRequest::prepare(
                        identities,
                        self.checked,
                        *value,
                        self.structural_types,
                    )
                    .map(|request| request.finish(&prefixes[ordinal], continuation, self))
                }
            };
            self.established_values.truncate(retained);
            self.completed.truncate(completed);
            continuation = entry?;
        }
        Ok(continuation)
    }

    fn retain_scope_prefix(&mut self, operands: &[Operand], prefixes: &[Vec<QualifiedScalarType>]) {
        for (ordinal, operand) in operands.iter().enumerate() {
            match operand {
                Operand::Structural(root) => self
                    .established_values
                    .push(EstablishedValue::Produced { root: *root }),
                Operand::Scalar(value) => self.completed.push((*value, prefixes[ordinal].len())),
            }
        }
    }
}
