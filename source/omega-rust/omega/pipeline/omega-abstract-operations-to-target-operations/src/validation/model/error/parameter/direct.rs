macro_rules! direct_parameter_error {
    ($name:ident) => {
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

direct_parameter_error!(StraightLineIntegerParameterTranslationError);
direct_parameter_error!(StraightLineBooleanParameterTranslationError);
