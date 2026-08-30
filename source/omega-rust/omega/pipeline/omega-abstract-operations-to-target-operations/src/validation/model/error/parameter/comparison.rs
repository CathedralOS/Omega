macro_rules! comparison_parameter_error {
    ($name:ident, $result_roster:ident) => {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StraightLineBooleanEqualParametersTranslationError {
    SourceParameters,
    SourceStructuralParameters,
    SourceResult,
    SourceEntryClaims,
    SourcePublishedServices,
    SourceBlockRoster,
    SourceOperationRoster,
    SourceParameterRoster,
    SourceParameterShape,
    SourceEqualResultRoster,
    SourceLeftOperandLink,
    SourceRightOperandLink,
    SourceReturnLink,
    SourceCleanup,
    AbiPlan,
    AbiParameterCount,
    AbiParameterPlacement,
    TargetProvenance,
    TargetOperation,
}

comparison_parameter_error!(
    StraightLineIntegerEqualParametersTranslationError,
    SourceEqualResultRoster
);
comparison_parameter_error!(
    StraightLineIntegerLessThanParametersTranslationError,
    SourceLessThanResultRoster
);
comparison_parameter_error!(
    StraightLineIntegerLessOrEqualParametersTranslationError,
    SourceLessOrEqualResultRoster
);
