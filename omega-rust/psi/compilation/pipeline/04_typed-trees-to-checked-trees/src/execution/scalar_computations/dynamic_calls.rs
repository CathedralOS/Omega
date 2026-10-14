//! Selector calls (`Trait::member(selector, data...)`) as ordered operands.
//!
//! Checking retargets a selector call to its instantiated slot, whose roster
//! is the selector followed by the requirement's instantiated formals. The
//! selector evaluates first, exactly where it was authored, and becomes the
//! callee's dispatch operand rather than one of the data operands; the data
//! operands then complete left to right through the ordinary call operand
//! evaluator against the slot's formals. Every selectable implementation is
//! a checked body, so the data operands keep checked-body custody.
//!
//! The requirement's contract needs no evidence on the route: checking
//! discharged its `requires` against these operands and every selectable
//! implementation refines it where its conformance is checked. A suspending
//! requirement declines here; the route carries no suspension frontier.
use super::call_arguments::ComputedCallOperands;
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    PrimitiveType,
};

impl Builder<'_, '_> {
    pub(super) fn dynamic_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let program = self.program;
        let slot = program.dynamic_slot_by_symbol(call.target_symbol)?;
        if !call.carries_only_positional_arguments()
            || program.primitive_type_reference(slot.return_type)? != expected_type
        {
            return None;
        }
        let requirement = program.dynamic_slot_requirement(slot)?;
        if requirement.suspends {
            return None;
        }
        let (source_call, call_ordinal) = self.call_ordinal(expression, call.target_symbol)?;
        let arguments = program.expression_table.expression_handles(call.arguments);
        let (&selector_argument, data_arguments) = arguments.split_first()?;
        let selector = self.expression(selector_argument, PrimitiveType::Selector)?;
        self.plans.nodes.get_mut(selector).authored_root = selector_argument;
        let ComputedCallOperands {
            arguments,
            structural_arguments,
            temporary_cleanups,
            claim_transfers,
        } = self.call_operands(source_call, data_arguments, slot.data_parameters())?;
        Some(self.insert(
            expected_type,
            CheckedScalarComputationKind::Call {
                source_call,
                callee: checked_trees::CheckedScalarCallee::Dynamic {
                    selector,
                    slot: slot.symbol,
                    requirement: slot.requirement,
                    interface_identity: slot.interface_identity,
                },
                call_ordinal,
                arguments,
                structural_arguments,
                temporary_cleanups,
                claim_transfers,
            },
        ))
    }
}
