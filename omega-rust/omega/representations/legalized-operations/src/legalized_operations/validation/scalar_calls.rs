use crate::LegalizedScalarCall;
use calling_conventions::{EntryControl, ValueLocation, ValuePlacement, ValueShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarCallShapeError {
    ArgumentCount,
    ArgumentPlacement { argument: usize },
    Result,
    CallControl,
}
impl LegalizedScalarCall {
    /// Raw ABI consistency only; source, types and canonical target ABI require replay.
    pub fn validate_shape(&self) -> Result<(), LegalizedScalarCallShapeError> {
        use LegalizedScalarCallShapeError as Error;
        if self.call_plan.entry_control != EntryControl::CallReturn
            || !self.call_plan.callback_materializations.is_empty()
        {
            return Err(Error::CallControl);
        }
        if self.arguments.len() != self.call_plan.parameters.len() {
            return Err(Error::ArgumentCount);
        }
        for (index, (argument, placement)) in self
            .arguments
            .iter()
            .zip(&self.call_plan.parameters)
            .enumerate()
        {
            if argument.placement() != placement
                || matches!(argument, crate::LegalizedScalarArgument::Scalar { .. })
                    && !direct_u64_register(placement)
            {
                return Err(Error::ArgumentPlacement { argument: index });
            }
        }
        if self.result_placement != self.call_plan.result
            || self
                .result_placement
                .as_ref()
                .is_some_and(|placement| !direct_u64_register(placement))
        {
            return Err(Error::Result);
        }
        Ok(())
    }
}
fn direct_u64_register(placement: &ValuePlacement) -> bool {
    placement.shape == ValueShape::integer(8, 8)
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size: 8,
                ..
            }]
        )
}
