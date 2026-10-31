//! The one-shot invocation observes its rank before entering a cyclic header.
//! Its actual machine formals stay separate from the header's private phis.
//! Internal arrivals use the same retained observation over completed actuals;
//! no pre-observation block is assigned a value defined by a later completion.

use super::{
    Block, LoweringError, StateGraphEmission, Terminator, ValueDeclaration, ranking, unsupported,
};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use crate::unit::attached_unit::composed_control::emission::{
    FrameValues, OperationSequence, StateFrame,
};

impl StateGraphEmission<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn initialize_invocation_rank(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        root: &checked_trees::CheckedScalarComputationRoot,
        template: &Evaluation,
        receiving: ValueDeclaration,
        body_operations: &mut OperationBuffer,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
    ) -> Result<(), LoweringError> {
        let entry = self.invocation_entry.ok_or(LoweringError::Unsupported(
            "ranked invocation has no one-shot observation entry",
        ))?;
        let checked = self.checked;
        let state = &self.plan.states[0];
        let owner = self.admitted.authored[0].0.symbol;
        let mut evaluation = template.branch(entry, 0);
        evaluation.entry = entry;
        evaluation.parameter_storage.clear();
        evaluation.previous_parameter_homes.clear();
        evaluation.erased_scalar_formals = self.invocation_erased.clone();
        let parameters = state
            .structural_parameters
            .iter()
            .zip(&self.parameters)
            .map(|(source, parameter)| {
                let mut parameter = parameter.clone();
                parameter.position = source.position;
                parameter
            })
            .collect::<Vec<_>>();
        evaluation.structural_parameters = state
            .structural_parameters
            .iter()
            .zip(&parameters)
            .map(|(source, parameter)| (source.position, parameter.clone()))
            .collect();
        evaluation.current_claims = Default::default();
        evaluation.current_claims.ingress(
            parameters
                .iter()
                .filter(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned)
                .map(|parameter| parameter.place),
            &self.claims.entry_claims,
        )?;
        evaluation.primitive_storage =
            crate::unit::attached_unit::parameters::primitive_reference_storage(
                checked,
                state.state,
                &state.structural_parameters,
                &evaluation.structural_parameters,
            )?;
        let mut values = self.scalar_parameters.clone();
        evaluation.scalar_bindings = crate::expression_preparation::bindings::ScalarBindings::entry(
            values.iter().map(|value| value.scalar_type),
            &evaluation.primitive_storage,
            &evaluation.structural_parameters,
            &self.catalogs.structural_types,
            &evaluation.place_observations,
        );
        evaluation.structural_fields =
            crate::expression_preparation::bindings::StructuralScalarFieldBinding::collect(
                &evaluation.structural_parameters,
                &self.catalogs.structural_types,
            );
        evaluation.structural_cases = crate::expression_preparation::bindings::structural_cases::StructuralCaseBinding::collect(
            &evaluation.structural_parameters, &self.catalogs.structural_types,
        );
        evaluation.element_views = crate::expression_preparation::bindings::element_views(
            &evaluation.structural_parameters,
            &self.catalogs.structural_types,
        );
        let scalar_slots = values
            .iter()
            .enumerate()
            .map(|(position, value)| (position, value.scalar_type))
            .collect::<Vec<_>>();
        let structural_actuals = parameters
            .iter()
            .map(|parameter| {
                (
                    terminal_psi::StructuralArgument {
                        place: parameter.place,
                        path: Vec::new(),
                        access: parameter.access,
                    },
                    parameter.structural_type,
                )
            })
            .collect::<Vec<_>>();
        let bindings = ranking::formal_bindings(
            checked,
            state,
            &evaluation.scalar_bindings,
            &scalar_slots,
            &structural_actuals,
        )?;
        let mut operations = OperationBuffer::new(
            body_operations.next_identity - 1,
            std::sync::Arc::clone(&self.dynamic_dispatch),
        );
        let mut sequence = OperationSequence::new(&evaluation);
        let observed = StateFrame::new(checked, owner, state, &self.invocation_erased).evaluate(
            self.catalogs,
            &parameters,
            &mut sequence,
            FrameValues {
                source_value_count: values.len(),
                evaluation: &mut evaluation,
                values: &mut values,
                next_value,
                next_block,
                next_edge,
                operations: &mut operations,
            },
            |frame| {
                let (mut environment, emission) = frame.split();
                emission.evaluation.retained_observation(
                    identities,
                    checked,
                    root,
                    &bindings,
                    emission.values,
                    emission.next_value,
                    emission.next_block,
                    emission.next_edge,
                    emission.operations,
                    emission.calls,
                    &mut environment,
                )
            },
        )?;
        sequence.require_closed()?;
        if observed.value_type() != receiving.value_type()
            || operations.invocation_completion.is_some()
        {
            return unsupported("invocation rank lost its complete normal value");
        }
        let bridge = self
            .blocks
            .iter()
            .position(|block| block.id == entry)
            .ok_or(LoweringError::Unsupported(
                "invocation observation lost its original bridge",
            ))?;
        let mut bridge = self.blocks.remove(bridge);
        let Terminator::Jump { arguments, .. } = &mut bridge.terminator else {
            return unsupported("invocation observation bridge is not a completed arrival");
        };
        arguments.push(observed.id);
        evaluation.blocks.push(Block {
            id: evaluation.current,
            parameters: std::mem::take(&mut evaluation.parameters),
            structural_parameters: std::mem::take(&mut evaluation.block_structural_parameters),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: bridge.erased_proof_formals,
            operations: operations[evaluation.operation_start..].to_vec(),
            terminator: bridge.terminator,
        });
        self.blocks.extend(evaluation.blocks);
        self.qualification_coercions
            .append(&mut evaluation.qualification_coercions);
        self.scalar_block_invariants
            .append(&mut operations.scalar_block_invariants);
        self.loan_completions
            .append(&mut operations.loan_completions);
        self.block_entry_claims
            .append(&mut operations.block_entry_claims);
        body_operations.next_identity = operations.next_identity;
        self.occurrences.retain(operations);
        Ok(())
    }
}
