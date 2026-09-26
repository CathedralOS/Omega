//! Field inventories for the nested-custody substitution matrices. Each
//! family declares its substitutable axes through `custody_field_inventory!`
//! here; the sibling `installation_function_nested_custody` module drives
//! every declared inventory through `run_one_field_substitution_matrix`.
//! A one-field substitution either cannot encode canonically or still
//! encodes, recomputes a distinct installation fingerprint, and independent
//! replay against the unchanged image rejects it with
//! `InstallationError::ImageBindingMismatch`.

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_nested_call_stacks_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum NestedCallStacksFieldForTest {
        UnitCallStacksOwner,
        UnitCallStacksTarget,
        UnitCallStacksTextOffset,
        UnitCallStacksDrop,
        UnitCallStacksInsertDistinct,
        UnitCallStacksTargetUnknownMachine,
        UnitCallStacksActiveFrameBytes,
        UnitCallStacksTransientBytes,
        UnitCallStacksCallerLiveBytes,
        UnitCallStacksInsertDuplicate,
        ScalarCallStacksInsertOnUnitRow,
        ForeignCallStacksInsertUnselected,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `selected` fixture in
    /// `installation_function_nested_call_stacks_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum NestedCallStacksSelectedFieldForTest {
        ForeignCallStacksInsertAdmitted,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_foreign_call_stack_row_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ForeignCallStackRowFieldForTest {
        Owner,
        TextOffset,
        CallerLiveBytes,
        ProviderPlanReportIdentity,
        ContributionReportIdentity,
        ContributionCommitment,
        ContributionBytes,
        ContributionAlignment,
        Drop,
        ProviderPlanReportIdentityUnselected,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_scalar_call_stacks_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ScalarCallStacksFieldForTest {
        ScalarCallStacksOwner,
        ScalarCallStacksTarget,
        ScalarCallStacksTextOffset,
        ScalarCallStacksCallerLiveBytes,
        ScalarCallStacksDrop,
        ScalarCallStacksInsertDistinct,
        ScalarCallStacksTargetUnknownMachine,
        ScalarCallStacksInsertDuplicate,
        UnitCallStacksInsertOnScalarRow,
        ForeignCallStacksInsertOnScalarRow,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_parameter_and_home_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ParameterAndHomeRowsFieldForTest {
        ParameterHomesLocation,
        ParameterHomesIndirect,
        ParametersPlace,
        ParametersStructuralType,
        ParametersMultiplicity,
        ParametersAccess,
        ParametersShape,
        ParametersDrop,
        ParametersInsertDuplicate,
        ParameterHomesPlace,
        ParameterHomesDrop,
        ParameterHomesStructuralType,
        ParameterHomesMultiplicity,
        ParameterHomesAccess,
        ParameterHomesShape,
        ParameterHomesSource,
        ParameterHomesInsertDuplicate,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_scalar_structural_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ScalarStructuralRowsFieldForTest {
        ParameterHomesSource,
        ParameterHomesLocation,
        ParameterHomesIndirect,
        ParametersPlace,
        ParametersStructuralType,
        ParametersMultiplicity,
        ParametersAccess,
        ParametersShape,
        ParametersDrop,
        ParametersInsertDuplicate,
        ParameterHomesPlace,
        ParameterHomesStructuralType,
        ParameterHomesMultiplicity,
        ParameterHomesAccess,
        ParameterHomesShape,
        ParameterHomesDrop,
        ParameterHomesInsertDuplicate,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_unit_scalar_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum UnitScalarRowsFieldForTest {
        ScalarHomesDefiningOperation,
        ScalarHomesSourceValue,
        ScalarHomesByteOffset,
        ScalarHomesDrop,
        IntegerConstantsInsert,
        ScalarHomesScalarType,
        ScalarHomesShape,
        ScalarHomesInsertDuplicate,
        AffineScalarRecordsInsert,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_store_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum StoreRowsFieldForTest {
        UnitStructuralScalarField,
        UnitWriteOnlyPrimitive,
        ScalarStructuralScalarField,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `scalar_record` fixture in
    /// `installation_function_store_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum StoreRowsScalarRecordFieldForTest {
        ScalarStructuralScalarFieldStoresInsertOnScalarRow,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_structural_store_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum StructuralStoreRowsFieldForTest {
        StructuralScalarFieldStoresField,
        StructuralScalarFieldStoresPathRenamed,
        StructuralScalarFieldStoresPathEmpty,
        StructuralScalarFieldStoresPathIndexed,
        StructuralScalarFieldStoresDestinationQualifications,
        StructuralScalarFieldStoresDestinationProjectedQualifications,
        StructuralScalarFieldStoresDrop,
        StructuralScalarFieldStoresPsiOperation,
        StructuralScalarFieldStoresDestinationPlace,
        StructuralScalarFieldStoresDestinationPosition,
        StructuralScalarFieldStoresDestinationIsSelf,
        StructuralScalarFieldStoresDestinationStructuralType,
        StructuralScalarFieldStoresDestinationMultiplicity,
        StructuralScalarFieldStoresDestinationAccess,
        StructuralScalarFieldStoresPathReferent,
        StructuralScalarFieldStoresPathEmptyField,
        StructuralScalarFieldStoresPathDoubleIndex,
        StructuralScalarFieldStoresDestinationPlacement,
        StructuralScalarFieldStoresFieldByteOffsetWithinBounds,
        StructuralScalarFieldStoresFieldByteOffsetOutOfBounds,
        StructuralScalarFieldStoresSourceDefiningOperation,
        StructuralScalarFieldStoresSourceSourceValue,
        StructuralScalarFieldStoresSourceScalarType,
        StructuralScalarFieldStoresSourceValue,
        StructuralScalarFieldStoresSourceBoolean,
        StructuralScalarFieldStoresSourceParameter,
        StructuralScalarFieldStoresSourceHome,
        StructuralScalarFieldStoresParameterHomeByteOffset,
        StructuralScalarFieldStoresParameterHomeIndirect,
        StructuralScalarFieldStoresOperationOrdinal,
        StructuralScalarFieldStoresCodeOffset,
        StructuralScalarFieldStoresByteCount,
        StructuralScalarFieldStoresBytesContent,
        StructuralScalarFieldStoresBytesTruncate,
        StructuralScalarFieldStoresSwap,
        StructuralScalarFieldStoresInsertDuplicate,
        StructuralScalarFieldStoresInsertFabricated,
        ParametersPlace,
        ParametersStructuralType,
        ParametersMultiplicity,
        ParametersAccess,
        ParametersShape,
        ParametersDrop,
        ParameterHomesPlace,
        ParameterHomesStructuralType,
        ParameterHomesMultiplicity,
        ParameterHomesAccess,
        ParameterHomesShape,
        ParameterHomesSource,
        ParameterHomesLocation,
        ParameterHomesIndirect,
        ParameterHomesDrop,
        ParameterHomesInsertDuplicate,
        IntegerConstantsDefiningOperation,
        IntegerConstantsSourceValue,
        IntegerConstantsScalarType,
        IntegerConstantsValue,
        IntegerConstantsOperationOrdinal,
        IntegerConstantsDrop,
        IntegerConstantsInsertDuplicate,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_write_only_store_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum WriteOnlyStoreRowsFieldForTest {
        WriteOnlyPrimitiveStoresDrop,
        WriteOnlyPrimitiveStoresDropAll,
        AffineCleanupStructuralTypesInsertDistinct,
        WriteOnlyPrimitiveStoresPsiOperation,
        WriteOnlyPrimitiveStoresDestinationPlace,
        WriteOnlyPrimitiveStoresDestinationPosition,
        WriteOnlyPrimitiveStoresDestinationIsSelf,
        WriteOnlyPrimitiveStoresDestinationStructuralType,
        WriteOnlyPrimitiveStoresDestinationMultiplicity,
        WriteOnlyPrimitiveStoresDestinationAccess,
        WriteOnlyPrimitiveStoresDestinationQualifications,
        WriteOnlyPrimitiveStoresDestinationProjectedQualifications,
        WriteOnlyPrimitiveStoresDestinationTypeId,
        WriteOnlyPrimitiveStoresDestinationTypeIdentity,
        WriteOnlyPrimitiveStoresDestinationTypeIdentityEmpty,
        WriteOnlyPrimitiveStoresDestinationTypeShape,
        WriteOnlyPrimitiveStoresDestinationPlacement,
        WriteOnlyPrimitiveStoresSourceDefiningOperation,
        WriteOnlyPrimitiveStoresSourceSourceValue,
        WriteOnlyPrimitiveStoresSourceScalarType,
        WriteOnlyPrimitiveStoresSourceValue,
        WriteOnlyPrimitiveStoresSourceParameter,
        WriteOnlyPrimitiveStoresSourceBoolean,
        WriteOnlyPrimitiveStoresSourceIeeeFloat,
        WriteOnlyPrimitiveStoresSourceHome,
        WriteOnlyPrimitiveStoresParameterHomeByteOffset,
        WriteOnlyPrimitiveStoresParameterHomeIndirect,
        WriteOnlyPrimitiveStoresOperationOrdinal,
        WriteOnlyPrimitiveStoresCodeOffset,
        WriteOnlyPrimitiveStoresByteCount,
        WriteOnlyPrimitiveStoresBytesContent,
        WriteOnlyPrimitiveStoresBytesTruncate,
        WriteOnlyPrimitiveStoresSwap,
        WriteOnlyPrimitiveStoresInsertDuplicate,
        WriteOnlyPrimitiveStoresInsertFabricated,
        ParametersPlace,
        ParametersStructuralType,
        ParametersMultiplicity,
        ParametersAccess,
        ParametersShape,
        ParametersDrop,
        ParameterHomesPlace,
        ParameterHomesStructuralType,
        ParameterHomesMultiplicity,
        ParameterHomesAccess,
        ParameterHomesShape,
        ParameterHomesSource,
        ParameterHomesLocation,
        ParameterHomesIndirect,
        ParameterHomesDrop,
        ParameterHomesInsertDuplicate,
        IntegerConstantsDefiningOperation,
        IntegerConstantsSourceValue,
        IntegerConstantsScalarType,
        IntegerConstantsValue,
        IntegerConstantsOperationOrdinal,
        IntegerConstantsDrop,
        IntegerConstantsInsertDuplicate,
        AffineCleanupStructuralTypesDrop,
        AffineCleanupStructuralTypesInsertDuplicate,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_scalar_store_rows_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ScalarStoreRowsFieldForTest {
        PsiOperation,
        DestinationPlace,
        DestinationPosition,
        DestinationIsSelf,
        DestinationStructuralType,
        DestinationMultiplicity,
        DestinationAccess,
        DestinationQualifications,
        DestinationProjectedQualifications,
        DestinationProjectedQualificationsReferent,
        PathReferent,
        PathRenamed,
        PathEmpty,
        PathIndexed,
        Field,
        DestinationPlacement,
        FieldByteOffset,
        DefiningOperation,
        SourceValue,
        ImmediateBoolean,
        ImmediateScalarType,
        ImmediateValue,
        ReturnOperation,
        ReturnSourceValue,
        ReturnField,
        ReturnFieldByteOffset,
        ReturnScalarType,
        OperationOrdinal,
        CodeOffset,
        ByteCount,
        BytesContent,
        BytesTruncate,
        Swap,
        Drop,
        InsertDuplicate,
        InsertFabricated,
        PathEmptyField,
        ImmediateScalarTypeAddress,
        InsertBeyondBound,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_affine_scalar_records_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum AffineScalarRecordsFieldForTest {
        UnitAffineScalarRecordsPsiOperation,
        UnitAffineScalarRecordsResultPlace,
        UnitAffineScalarRecordsResultStructuralType,
        UnitAffineScalarRecordsField,
        UnitAffineScalarRecordsValue,
        UnitAffineScalarRecordsOperationOrdinal,
        UnitAffineScalarRecordsDrop,
        UnitAffineScalarRecordsInsert,
        UnitAffineScalarRecordsInsertDuplicate,
        UnitAffineScalarRecordsSwap,
        UnitParameterHomesIndirect,
        UnitAffineScalarRecordsResultMultiplicity,
        UnitAffineScalarRecordsResultQualifications,
        UnitAffineScalarRecordsResultProjectedQualifications,
        UnitAffineScalarRecordsResultClaims,
        UnitAffineScalarRecordsValueUnsigned,
        UnitAffineScalarRecordsValueOverflow,
        UnitAffineScalarRecordsShape,
        UnitAffineScalarRecordsInsertNoncanonical,
        UnitParametersPlace,
        UnitParametersStructuralType,
        UnitParametersMultiplicity,
        UnitParametersAccess,
        UnitParametersShape,
        UnitParametersDrop,
        UnitParametersInsertDuplicate,
        UnitParametersSwap,
        UnitParameterHomesPlace,
        UnitParameterHomesStructuralType,
        UnitParameterHomesMultiplicity,
        UnitParameterHomesAccess,
        UnitParameterHomesShape,
        UnitParameterHomesSource,
        UnitParameterHomesLocation,
        UnitParameterHomesDrop,
        UnitParameterHomesInsertDuplicate,
        UnitParameterHomesSwap,
        InternalUnitCallsArgumentsAccess,
        InternalUnitCallsArgumentsCallStackBytes,
        InternalUnitCallsArgumentsCodeOffset,
        InternalUnitCallsArgumentsBytes,
        InternalUnitCallsCustodyClaimTransfers,
        InternalUnitCallsCustodySource,
        InternalUnitCallsCustodyOwner,
        InternalUnitCallsCustodyTarget,
        InternalUnitCallsCustodyResult,
        InternalUnitCallsCustodySemanticResult,
        InternalUnitCallsCustodyStructuralResult,
        InternalUnitCallsCustodyOperationOrdinal,
        InternalUnitCallsCustodyCodeOffset,
        InternalUnitCallsCustodyByteCount,
        InternalUnitCallsCustodyScalarArguments,
        InternalUnitCallsCustodyArgumentsPlace,
        InternalUnitCallsCustodyArgumentsPath,
        InternalUnitCallsCustodyArgumentsRootStructuralType,
        InternalUnitCallsCustodyArgumentsStructuralType,
        InternalUnitCallsCustodyArgumentsShape,
        InternalUnitCallsCustodyArgumentsSourcePlacement,
        InternalUnitCallsCustodyArgumentsSourceEstablished,
        InternalUnitCallsCustodyArgumentsSourceLocation,
        InternalUnitCallsCustodyArgumentsSourceByteOffset,
        InternalUnitCallsCustodyArgumentsDestination,
        InternalUnitCallsCustodyArgumentsByteCount,
        InternalUnitCallsCustodyArgumentsFixedArrayLength,
        InternalUnitCallsCustodyArgumentsElementStride,
        InternalUnitCallsCustodyArgumentsDrop,
        InternalUnitCallsCustodyArgumentsInsertDuplicate,
        InternalUnitCallsMachine,
        InternalUnitCallsTextOffset,
        InternalUnitCallsDrop,
        InternalUnitCallsSwap,
        InternalUnitCallsInsertDuplicate,
        InternalUnitCallsInsertDistinct,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_affine_cleanup_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum AffineCleanupFieldForTest {
        UnitAffineCleanupPsiEdge,
        UnitAffineCleanupLocalsInsert,
        UnitAffineCleanupActionsDrop,
        UnitAffineCleanupActionsInsert,
        UnitAffineCleanupActionsCleanupMachine,
        UnitAffineCleanupActionsPlace,
        UnitAffineCleanupActionsVariant,
        UnitAffineCleanupCodeOffset,
        UnitAffineCleanupByteCount,
        UnitAffineCleanupDrop,
        ScalarAffineCleanupInsertOnUnitRow,
        UnitContinuationsInsert,
        UnitAffineCleanupStructuralTypes,
        UnitContinuationsInsertOnUnitCaller,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `scalar_record` fixture in
    /// `installation_function_affine_cleanup_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum AffineCleanupScalarRecordFieldForTest {
        ScalarAffineCleanupPsiEdge,
        ScalarAffineCleanupLocalsInsert,
        ScalarAffineCleanupActionsDrop,
        ScalarAffineCleanupCodeOffset,
        ScalarAffineCleanupByteCount,
        ScalarAffineCleanupDrop,
        UnitAffineCleanupInsertOnScalarRow,
        ScalarAffineCleanupStructuralTypes,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_scalar_control_cleanups_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ScalarControlCleanupsFieldForTest {
        PsiEdge,
        StructuralTypes,
        LocalsInsert,
        ActionsDrop,
        ActionsInsert,
        CodeOffset,
        ByteCount,
        Reorder,
        Drop,
        InsertDuplicate,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_scalar_transport_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ScalarTransportFieldForTest {
        ScalarAbiInsertCanonical,
        ScalarAbiInsertCanonicalAltValue,
        ParameterAbiInsertCanonical,
        ScalarAbiInsertNoncanonical,
        MixedStructuralScalarAbiInsertOnUnitRow,
        ParameterAbiInsertWithSpills,
        StructuralCallScalarReturnInsert,
        StructuralCallScalarReturnInsertOnUnitCaller,
        ParameterAbiInsertOnCallee,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `scalar_record` fixture in
    /// `installation_function_scalar_transport_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ScalarTransportScalarRecordFieldForTest {
        ScalarAbiInsertCanonicalOnScalarRow,
        MixedStructuralScalarAbiInsertMatching,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_unit_continuations_reject_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum UnitContinuationsFieldForTest {
        SourceBlock,
        TargetBlock,
        BindingsParameter,
        BindingsArgument,
        BindingsInsert,
        BindingsDrop,
        OperationOrdinal,
        SuccessorOperationOrdinal,
        TargetBlockRevisit,
        CleanupPsiEdge,
        CleanupPsiEdgeReturnedEdge,
        CleanupCodeOffset,
        CleanupByteCount,
        CleanupLocalsInsert,
        CleanupStructuralTypes,
        CleanupActionsInsertResidual,
        CleanupActionsInsertRoot,
        BindingsParameterLiveValue,
        BindingsArgumentUnknownValue,
        BindingsScalarType,
        BindingsInsertDuplicate,
        Drop,
        InsertDuplicate,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_parameter_abi_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum ParameterAbiFieldForTest {
        Drop,
        CallPlanPolicy,
        CallPlanParametersDrop,
        CallPlanParametersInsert,
        CallPlanParameters,
        CallPlanParameters1,
        CallPlanParametersSwap,
        CallPlanParametersInsertDuplicate,
        CallPlanResult,
        CallPlanCallbackMaterializationsInsert,
        CallPlanOrdinaryClobbers,
        CallPlanStackAlignment,
        CallPlanShadowBytes,
        CallPlanEntryControl,
        ParametersValue,
        ParametersValueOtherParameter,
        ParametersScalarTypeU32,
        ParametersScalarTypeI64,
        ParametersScalarTypeBoolean,
        ParametersPlacement,
        Parameters1Value,
        Parameters1ValueOtherParameter,
        Parameters1ScalarTypeI64,
        Parameters1ScalarTypeBoolean,
        Parameters1Placement,
        ParametersInsert,
        ParametersDrop,
        ParametersSwap,
        ParametersInsertDuplicate,
        EntryRegisterSpillsSourceValue,
        EntryRegisterSpillsParameterIndex,
        EntryRegisterSpillsRegister,
        EntryRegisterSpillsByteOffset,
        EntryRegisterSpillsCodeOffset,
        EntryRegisterSpillsByteCount,
        EntryRegisterSpills1SourceValue,
        EntryRegisterSpills1ParameterIndex,
        EntryRegisterSpills1Register,
        EntryRegisterSpills1ByteOffset,
        EntryRegisterSpills1CodeOffset,
        EntryRegisterSpills1ByteCount,
        EntryRegisterSpillsInsert,
        EntryRegisterSpillsDrop,
        EntryRegisterSpillsSwap,
        EntryRegisterSpillsInsertDuplicate,
        Parameters1ScalarTypeU32,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_mixed_abi_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum MixedAbiFieldForTest {
        Drop,
        ScalarParametersValue,
        ScalarParametersScalarTypeU32,
        StructuralParametersProjectedQualificationsInsert,
        ResultValue,
        ResultScalarTypeU32,
        CallPlanPolicy,
        CallPlanParametersDrop,
        CallPlanParametersInsert,
        CallPlanParameters,
        CallPlanParameters1,
        CallPlanParametersSwap,
        CallPlanParametersInsertDuplicate,
        CallPlanResult,
        CallPlanCallbackMaterializationsInsert,
        CallPlanOrdinaryClobbers,
        CallPlanStackAlignment,
        CallPlanShadowBytes,
        CallPlanEntryControl,
        ScalarParametersValueResultCollision,
        ScalarParametersScalarTypeI64,
        ScalarParametersScalarTypeBoolean,
        ScalarParametersPlacement,
        ScalarParametersInsert,
        ScalarParametersDrop,
        ScalarParametersInsertDuplicate,
        StructuralParametersPlace,
        StructuralParametersStructuralType,
        StructuralParametersMultiplicity,
        StructuralParametersAccess,
        StructuralParametersShape,
        StructuralParametersPlacement,
        StructuralParametersInsert,
        StructuralParametersDrop,
        StructuralParametersInsertDuplicate,
        ResultScalarTypeI64,
        ResultScalarTypeBoolean,
        ResultPlacement,
        ResultValueParameterCollision,
    }
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of the `record` fixture in
    /// `installation_function_structural_call_scalar_return_rejects_every_one_field_substitution`. Each variant spells the
    /// authored leg label it replaced, CamelCased, with a leading `[0]` and
    /// the words every leg of the family shares elided.
    pub enum StructuralCallScalarReturnFieldForTest {
        PsiEdge,
        PsiOperation,
        SourceValue,
        ScalarTypeU32,
        ScalarTypeBoolean,
        CalleeUnknown,
        CalleeCaller,
        Drop,
    }
}
