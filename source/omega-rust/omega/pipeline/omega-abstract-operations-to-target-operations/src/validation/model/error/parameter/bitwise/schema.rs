//! Shared field schema instantiated by each exact bitwise family.

macro_rules! bitwise_parameter_error {
    ($name:ident, $result_roster:ident, $type_mismatch:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            SourceParameters,
            SourceStructuralParameters,
            SourceResult,
            SourceEntryClaims,
            SourcePublishedServices,
            SourceBlockRoster,
            SourceOperationRoster,
            SourceParameterRoster,
            SourceParameterShape,
            $result_roster,
            SourceLeftOperandLink,
            SourceRightOperandLink,
            SourceOperandTypeMismatch,
            $type_mismatch,
            SourceReturnLink,
            SourceCleanup,
            AbiPlan,
            AbiParameterCount,
            AbiParameterPlacement,
            TargetProvenance,
            TargetOperation,
        }
    };
}

pub(super) use bitwise_parameter_error;
