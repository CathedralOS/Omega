//! Check integer arguments using their values when they were evaluated.
//! Copies keep the bounds known at that time; references use the storage
//! observations valid at the call. Every possible value contributes to the range.

use checked_trees::{FlowCallFact, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactPlan, IntegerRange};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::PrimitiveType;

pub(crate) struct CapturedCallIntervals<'a> {
    program: &'a dyn validation::CheckingInput,
    operators: &'a checked_trees::CheckedOperatorFacts,
    semantic: &'a FactPlan,
    flow: &'a FlowFacts,
    state: &'a FlowStateFact,
    call: &'a FlowCallFact,
    parameters: &'a [StateParameter],
    arguments: &'a [FlowOperandFact],
    epoch: &'a FlowOperandFact,
}

impl<'a> CapturedCallIntervals<'a> {
    pub(crate) fn new(
        program: &'a dyn validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        semantic: &'a FactPlan,
        flow: &'a FlowFacts,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
        parameters: &'a [StateParameter],
    ) -> Option<Self> {
        if !call.authored_source_custody_valid {
            return None;
        }
        let (operands, receiver_offset) =
            crate::flow::captured_call_operands(program, flow, state, call)?;
        let arguments = crate::shared::call_formals::arguments_aligned_to_formals(
            program,
            call.target_symbol,
            parameters.len(),
            operands.get(receiver_offset..)?,
        );
        if arguments.len() != parameters.len() {
            return None;
        }
        Some(Self {
            program,
            operators,
            semantic,
            flow,
            state,
            call,
            parameters,
            arguments,
            epoch: operands.last()?,
        })
    }

    fn parameter_bounds(
        &self,
        ordinal: usize,
        path: &[facts::PlaceSegment],
        primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        let parameter = self.parameters.get(ordinal)?;
        let operand = self.arguments.get(ordinal)?;
        super::CapturedIntegerIntervals::new(
            self.program,
            self.operators,
            self.semantic,
            self.flow,
            self.state,
        )
        .bounds(
            crate::flow::CapturedOperand::unrecorded(operand),
            parameter.type_reference,
            path,
            self.call.entry_constraints,
            self.epoch,
            primitive,
        )
    }
}

impl validation::SelectedScalarIntervalSource for CapturedCallIntervals<'_> {
    fn binding(&mut self, position: usize, primitive: PrimitiveType) -> Option<IntegerRange> {
        let (ordinal, parameter) = self
            .parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| super::occupies_scalar_position(self.program, parameter))
            .nth(position)?;
        if self
            .program
            .primitive_type_reference(parameter.type_reference)
            != Some(primitive)
        {
            return None;
        }
        self.parameter_bounds(ordinal, &[], primitive)
    }

    fn storage(
        &mut self,
        _symbol: SymbolHandle,
        _primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        None
    }

    fn structural_field(
        &mut self,
        position: u32,
        path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    ) -> Option<IntegerRange> {
        let (_, segments, reference, _) = super::resolve_structural_parameter_path(
            self.program,
            self.parameters,
            position,
            path,
        )?;
        let primitive = self.program.primitive_type_reference(reference)?;
        self.parameter_bounds(position as usize, &segments, primitive)
    }

    fn indexed_read(
        &mut self,
        _root: checked_trees::CheckedStorageRoot,
        _path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        _index: &checked_trees::CheckedScalarSelector,
        _element_path: &[checked_trees::CheckedScalarReadProjection],
        _primitive: PrimitiveType,
    ) -> Option<IntegerRange> {
        None
    }
}
