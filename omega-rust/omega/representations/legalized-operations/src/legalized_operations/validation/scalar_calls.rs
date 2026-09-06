use crate::LegalizedScalarCallUnitCall;
use calling_conventions::{EntryControl, ValueLocation, ValuePlacement, ValueShape};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target_operations::TargetUnitScalarArgumentSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalizedScalarCallShapeError {
    ArgumentCount,
    ArgumentIndex { argument: usize },
    ArgumentPlacement { argument: usize },
    ArgumentSource { argument: usize },
    Result,
    CallControl,
}

impl LegalizedScalarCallUnitCall {
    /// Checks the current argument roster against its retained ABI. Canonical
    /// target ABI and source/effect/proof custody require independent admission.
    pub fn validate_shape(&self) -> Result<(), LegalizedScalarCallShapeError> {
        use LegalizedScalarCallShapeError as Error;
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).expect("U64");
        let scalar = ScalarType::Integer(integer);
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
            if usize::try_from(argument.parameter_index).ok() != Some(index) {
                return Err(Error::ArgumentIndex { argument: index });
            }
            if argument.placement != *placement || !direct_u64_register(placement) {
                return Err(Error::ArgumentPlacement { argument: index });
            }
            let source_valid = match argument.source {
                TargetUnitScalarArgumentSource::IntegerImmediate {
                    scalar_type, value, ..
                } => {
                    scalar_type == integer
                        && matches!(value, IntegerValue::Unsigned(value) if value <= u128::from(u64::MAX))
                }
                TargetUnitScalarArgumentSource::Home(home) => {
                    home.scalar_type == scalar && home.shape == ValueShape::integer(8, 8)
                }
                TargetUnitScalarArgumentSource::Parameter { scalar_type, .. } => {
                    scalar_type == scalar
                }
                TargetUnitScalarArgumentSource::BooleanImmediate { .. } => false,
            };
            if !source_valid {
                return Err(Error::ArgumentSource { argument: index });
            }
        }
        if self.result_home.defining_operation != self.operation
            || self.result_home.scalar_type != scalar
            || self.result_home.shape != ValueShape::integer(8, 8)
            || !self
                .call_plan
                .result
                .as_ref()
                .is_some_and(direct_u64_register)
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
