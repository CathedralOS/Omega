//! Expand one nested boundary application in authored operand order.
//!
//! The operands complete left to right into the scalar prefix, then one block
//! binds the application's result through a single `BoundaryCall` emission.
//! The exact source occurrence is consumed once per expansion; a repeated
//! occurrence (for example a shared node reached twice) rejects rather than
//! invoking the provider again.
use super::{
    Argument, Computation, Expansion, Handle, LoweringError, QualifiedScalarType, Site, parameters,
    unsupported,
};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boundary_calls::LoweredBoundaryCallBinding;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use arena::HandleSpan;

impl Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn boundary_call(
        &mut self,
        source_call: Handle<checked_trees::FlowCallFact>,
        target_state: symbols::SymbolHandle,
        target_machine: symbols::SymbolHandle,
        call_ordinal: u32,
        arguments: HandleSpan<Computation>,
        result_type: QualifiedScalarType,
        input_types: &[QualifiedScalarType],
        target: usize,
        site: &Site<'_>,
        active: &mut Vec<Computation>,
    ) -> Result<usize, LoweringError> {
        let control = &self.checked.facts.flow.control;
        if !control.calls.is_valid(source_call) {
            return unsupported("nested boundary application lost its exact checked invocation");
        }
        let source = control.calls.get(source_call);
        let state = control
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.machine_symbol == self.machine && state.state_symbol == site.state)
            .ok_or(LoweringError::Unsupported(
                "nested boundary application invocation state is absent",
            ))?;
        if !control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .any(|call| std::ptr::eq(call, source))
            || source.statement_index != site.statement as usize
            || source.call_ordinal != call_ordinal as usize
            || source.target_symbol != target_state
        {
            return unsupported("nested boundary application invocation coordinate disagrees");
        }
        let coordinate = SourceCallCoordinate {
            state: site.state,
            statement_index: source.statement_index,
            call_ordinal: source.call_ordinal,
        };
        if self.calls.contains(&coordinate) {
            return unsupported("scalar computation repeats a call occurrence");
        }
        self.calls.push(coordinate);
        let operands = self
            .checked
            .facts
            .values
            .scalar_computations
            .operands
            .span(arguments)
            .ok_or(LoweringError::Unsupported(
                "nested boundary application has an invalid operand span",
            ))?
            .iter()
            .copied()
            .map(Argument::Computation)
            .collect::<Vec<_>>();
        let mut operand_types = input_types.to_vec();
        for operand in &operands {
            operand_types.push(self.argument_type(operand, site, input_types)?);
        }
        let application = self.binding(
            &operand_types,
            input_types.len(),
            target,
            LoweredScalarBinding::BoundaryCall(LoweredBoundaryCallBinding {
                source_coordinate: coordinate,
                source_expression: source.authored_expression,
                target_machine,
                result_type,
                arguments: parameters(&operand_types)
                    .into_iter()
                    .skip(input_types.len())
                    .collect(),
            }),
        );
        self.sequence(&operands, input_types, application, site, active)
    }
}
