//! Field inventories and honest-recomputation hooks for the installation
//! record's canonical-encoding custody matrices. Each family declares its
//! substitutable axes through `custody_field_inventory!` here; the sibling
//! `installation_field_substitutions` module drives every declared inventory
//! through `run_one_field_substitution_matrix`. A one-field substitution
//! either cannot encode canonically or still encodes, recomputes a distinct
//! installation identity, and independent replay against the unchanged image
//! rejects it with `InstallationError::ImageBindingMismatch`.

use super::{
    WriteExitProvider, callback_private_plan, dynamic_conformance_table_plan,
    dynamic_parameter_call_plan, edge_id, forwarded_dynamic_descriptor_call_plan,
    forwarded_dynamic_parameter_call_plan, linux_write_line_exit_plan, machine_id, operation_id,
    port_effect_plan, stored_dynamic_call_plan,
};
use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    ExecutableImage, InstallationError, InstallationRecord, build_installation_record,
    build_installation_record_with_provider_executions, build_object_artifact,
    build_object_artifact_with_private_functions, decode_installation_record,
    emit_direct_executable_image, encode_installation_record, validate_installation_record,
};
use machine_code::SemanticCodeSite;
use optimization_core::MutationOutcome;
use semantic_vocabulary::{PlaceId, ProfileDecisionId, ServiceId};
use symbols::SymbolHandle;
use terminal_psi::{SemanticFingerprint, StructuralAccess};

/// The retained custody view a substitution must move. Canonical-encoding
/// families have no lazier comparable view: an encode-rejected substitution
/// has no fingerprint, so the custody view is the record itself.
pub(super) fn installation_record_custody(record: &InstallationRecord) -> InstallationRecord {
    record.clone()
}

/// The independent checker shared by installation-record families: encode the
/// substituted record, require the codec to round-trip it, then replay
/// against the unchanged image. Encoding canonicality errors surface before
/// the replay join, so both legs reduce to the checker's exact error.
pub(super) fn installation_record_check(
    image: &ExecutableImage,
) -> impl Fn(&InstallationRecord) -> Result<InstallationRecord, InstallationError> + '_ {
    move |record| {
        let bytes = encode_installation_record(record)?;
        let replayed = decode_installation_record(&bytes)?;
        assert_eq!(&replayed, record, "codec preserves the substituted record");
        validate_installation_record(&replayed, image)?;
        Ok(replayed)
    }
}

/// An authentic foreign installation record whose custody differs from every
/// family below; it grants the matrices' substitute hooks no donor values, so
/// it only has to be genuinely produced and distinct.
pub(super) fn foreign_stored_dynamic_call_record() -> InstallationRecord {
    let artifact =
        build_object_artifact(&stored_dynamic_call_plan()).expect("donor object artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("donor image");
    build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("donor installation")
}

/// A second foreign record for the family the first donor is drawn from.
pub(super) fn foreign_forwarded_parameter_call_record() -> InstallationRecord {
    let artifact = build_object_artifact(&forwarded_dynamic_parameter_call_plan())
        .expect("donor object artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("donor image");
    build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("donor installation")
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed semantic-code attribution row or
    /// of the roster itself. Semantic site identities and in-bounds byte
    /// counts have no canonical record-shape join: the substituted row still
    /// encodes and decodes, so rejection is the recomputed identity and the
    /// independent image replay. The machine join, the canonical
    /// (machine, operation_ordinal, text_offset) order, interval geometry,
    /// the boundary-joined nominal return edge and roster canonicality are
    /// rejected at encoding.
    pub enum SemanticCodeAttributionFieldForTest {
        SiteOperationIdentity,
        SiteEdgeIdentity,
        ByteCountZeroMarkerRow,
        ByteCountInBoundsRow,
        MachineUnknown,
        OperationOrdinalReordersRoster,
        CodeOffsetOperationRow,
        TextOffsetOperationRow,
        ByteCountPastFunctionEnd,
        SiteTailEdgeToOperation,
        OperationOrdinalTailEdge,
        CodeOffsetTailEdge,
        ByteCountTailEdge,
        DropOperationRow,
        DropTailEdgeRow,
        DuplicateRow,
        ReorderRows,
    }
}

pub(super) fn honest_semantic_code_attribution_record() -> InstallationRecord {
    let provider = WriteExitProvider(970);
    let artifact = build_object_artifact(&linux_write_line_exit_plan(&provider))
        .expect("attribution artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("attribution image");
    build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(97).expect("profile"),
        [&provider],
    )
    .expect("attribution installation")
}

pub(super) fn substitute_semantic_code_attribution(
    record: &mut InstallationRecord,
    field: SemanticCodeAttributionFieldForTest,
    _donor: &InstallationRecord,
) {
    use SemanticCodeAttributionFieldForTest as Field;
    let rows = record.semantic_code_attribution_mut_for_test();
    match field {
        Field::SiteOperationIdentity => {
            rows[0].attribution.site = SemanticCodeSite::Operation(operation_id(199));
        }
        Field::SiteEdgeIdentity => {
            rows[4].attribution.site = SemanticCodeSite::Edge(edge_id(199));
        }
        Field::ByteCountZeroMarkerRow => {
            rows[0].attribution.byte_count = 1;
        }
        Field::ByteCountInBoundsRow => {
            rows[1].attribution.byte_count -= 1;
        }
        Field::MachineUnknown => {
            rows[4].machine = machine_id(199);
        }
        Field::OperationOrdinalReordersRoster => {
            rows[0].attribution.operation_ordinal = 5;
        }
        Field::CodeOffsetOperationRow => {
            rows[1].attribution.code_offset += 1;
        }
        Field::TextOffsetOperationRow => {
            rows[1].text_offset += 1;
        }
        Field::ByteCountPastFunctionEnd => {
            let bound = record
                .functions()
                .iter()
                .find(|function| function.machine == machine_id(97))
                .expect("attributed function row")
                .byte_count;
            let rows = record.semantic_code_attribution_mut_for_test();
            rows[1].attribution.byte_count = bound + 1;
        }
        Field::SiteTailEdgeToOperation => {
            rows[4].attribution.site = SemanticCodeSite::Operation(operation_id(199));
        }
        Field::OperationOrdinalTailEdge => {
            rows[4].attribution.operation_ordinal = 5;
        }
        Field::CodeOffsetTailEdge => {
            rows[4].attribution.code_offset -= 1;
        }
        Field::ByteCountTailEdge => {
            rows[4].attribution.byte_count = 0;
        }
        Field::DropOperationRow => {
            rows.remove(0);
        }
        Field::DropTailEdgeRow => {
            rows.pop();
        }
        Field::DuplicateRow => {
            let row = rows[1].clone();
            rows.insert(2, row);
        }
        Field::ReorderRows => {
            rows.swap(1, 2);
        }
    }
}

pub(super) fn semantic_code_attribution_outcome(
    field: SemanticCodeAttributionFieldForTest,
) -> MutationOutcome<InstallationError> {
    use SemanticCodeAttributionFieldForTest as Field;
    let invalid_attribution =
        |site: SemanticCodeSite| InstallationError::InvalidSemanticCodeAttribution {
            machine: machine_id(97),
            site,
        };
    let exit_mismatch = || InstallationError::BoundaryRealizationMismatch {
        machine: machine_id(97),
        operation: operation_id(100),
    };
    MutationOutcome::ExactError(match field {
        Field::SiteOperationIdentity
        | Field::SiteEdgeIdentity
        | Field::ByteCountZeroMarkerRow
        | Field::ByteCountInBoundsRow
        | Field::DropOperationRow => InstallationError::ImageBindingMismatch,
        Field::MachineUnknown => {
            InstallationError::SemanticCodeAttributionMachineMissing(machine_id(199))
        }
        Field::OperationOrdinalReordersRoster | Field::DuplicateRow | Field::ReorderRows => {
            InstallationError::NonCanonicalSemanticCodeAttributionOrder
        }
        Field::CodeOffsetOperationRow
        | Field::TextOffsetOperationRow
        | Field::ByteCountPastFunctionEnd => {
            invalid_attribution(SemanticCodeSite::Operation(operation_id(98)))
        }
        Field::SiteTailEdgeToOperation
        | Field::OperationOrdinalTailEdge
        | Field::ByteCountTailEdge
        | Field::DropTailEdgeRow => exit_mismatch(),
        Field::CodeOffsetTailEdge => invalid_attribution(SemanticCodeSite::Edge(edge_id(97))),
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed compiler-private callback row or
    /// of the roster itself. Semantic identities no canonical record-shape
    /// join pins still encode and are rejected by the recomputed identity and
    /// independent image replay; identity kind, placement geometry and scalar
    /// ABI custody are canonical record-shape joins rejected at encoding.
    pub enum CompilerPrivateFunctionFieldForTest {
        IdentityContinuation,
        IdentityPlacementIndex,
        SourcePsiProgramFingerprint,
        Machine,
        ScalarAbiParameterValue,
        ScalarAbiResultValue,
        IdentitySourceKind,
        TextOffset,
        ByteCountEmpty,
        ByteCountExtended,
        ScalarAbiParameterPlacement,
        ScalarAbiResultCollision,
        DropRow,
    }
}

pub(super) fn honest_compiler_private_function_record() -> InstallationRecord {
    let artifact = build_object_artifact_with_private_functions(&callback_private_plan())
        .expect("callback private artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("callback private image");
    build_installation_record(&image, ProfileDecisionId::new(53).expect("profile"))
        .expect("callback private installation")
}

pub(super) fn substitute_compiler_private_function(
    record: &mut InstallationRecord,
    field: CompilerPrivateFunctionFieldForTest,
    _donor: &InstallationRecord,
) {
    use CompilerPrivateFunctionFieldForTest as Field;
    match field {
        Field::IdentityContinuation => {
            record.private_functions_mut_for_test()[0].identity =
                MachineFunctionIdentity::callback_thunk(
                    StateKey {
                        machine: SymbolHandle::from_parts(15, 2),
                        state: SymbolHandle::from_parts(13, 3),
                        segment_index: 0,
                    },
                    0,
                )
                .expect("substituted callback thunk identity");
        }
        Field::IdentityPlacementIndex => {
            let row = &mut record.private_functions_mut_for_test()[0];
            row.identity = MachineFunctionIdentity::callback_thunk(
                row.identity.associated_source_continuation(),
                3,
            )
            .expect("substituted placement index");
        }
        Field::SourcePsiProgramFingerprint => {
            record.private_functions_mut_for_test()[0]
                .source_psi
                .program_fingerprint = SemanticFingerprint::from_bytes([7; 32]);
        }
        Field::Machine => {
            record.private_functions_mut_for_test()[0].machine = machine_id(98);
        }
        Field::ScalarAbiParameterValue => {
            record.private_functions_mut_for_test()[0]
                .scalar_abi
                .parameters[0]
                .value =
                semantic_vocabulary::ValueId::new(41).expect("substituted parameter value");
        }
        Field::ScalarAbiResultValue => {
            record.private_functions_mut_for_test()[0]
                .scalar_abi
                .result
                .value = semantic_vocabulary::ValueId::new(43).expect("substituted result value");
        }
        Field::IdentitySourceKind => {
            let row = &mut record.private_functions_mut_for_test()[0];
            row.identity =
                MachineFunctionIdentity::source(row.identity.associated_source_continuation());
        }
        Field::TextOffset => {
            let text_offset = record.private_functions()[0].text_offset;
            record.private_functions_mut_for_test()[0].text_offset = text_offset + 1;
        }
        Field::ByteCountEmpty => {
            record.private_functions_mut_for_test()[0].byte_count = 0;
        }
        Field::ByteCountExtended => {
            let byte_count = record.private_functions()[0].byte_count;
            record.private_functions_mut_for_test()[0].byte_count = byte_count + 1;
        }
        Field::ScalarAbiParameterPlacement => {
            let row = &mut record.private_functions_mut_for_test()[0];
            row.scalar_abi.parameters[0].placement = row.scalar_abi.result.placement.clone();
        }
        Field::ScalarAbiResultCollision => {
            let row = &mut record.private_functions_mut_for_test()[0];
            row.scalar_abi.result.value = row.scalar_abi.parameters[0].value;
        }
        Field::DropRow => {
            record.private_functions_mut_for_test().pop();
        }
    }
}

pub(super) fn compiler_private_function_outcome(
    field: CompilerPrivateFunctionFieldForTest,
) -> MutationOutcome<InstallationError> {
    use CompilerPrivateFunctionFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::IdentityContinuation
        | Field::IdentityPlacementIndex
        | Field::SourcePsiProgramFingerprint
        | Field::Machine
        | Field::ScalarAbiParameterValue
        | Field::ScalarAbiResultValue => InstallationError::ImageBindingMismatch,
        Field::IdentitySourceKind
        | Field::TextOffset
        | Field::ByteCountEmpty
        | Field::ScalarAbiParameterPlacement
        | Field::ScalarAbiResultCollision => InstallationError::InvalidCompilerPrivateFunction,
        Field::ByteCountExtended | Field::DropRow => InstallationError::InvalidImageSectionLayout,
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed dynamic conformance table, its
    /// slots, its joined dynamic call row or the rosters themselves. The table
    /// report fingerprint, the unselected slot target and the call's
    /// operation, in-function text offset and byte count have no canonical
    /// record-shape join, so the substitution still encodes and is rejected
    /// by the recomputed identity and independent image replay; every other
    /// axis and the roster canonicality joins are rejected at encoding.
    pub enum DynamicConformanceTableFieldForTest {
        TableApplicationReportFingerprint,
        TableSlot1Target,
        CallOperation,
        CallTextOffset,
        CallByteCount,
        TableApplicationCommitment,
        TableApplicationReportFingerprintZero,
        TableDataOffset,
        TableByteCount,
        TableSlot0RowIndex,
        TableSlot0DataOffset,
        TableSlot0TargetUnknown,
        TableSlot0TargetRetargeted,
        TableSlot0TargetErased,
        TableSlot1RowIndex,
        TableSlot1DataOffset,
        TableSlot1TargetUnknown,
        CallMachineUnknown,
        CallMachineOtherFunction,
        CallApplicationCommitment,
        CallInitialSource,
        CallReboundSource,
        CallSelectedTableByteOffsetUnresolvedRow,
        CallSelectedTableByteOffsetMisaligned,
        CallRealization,
        CallTextOffsetBeforeFunction,
        CallByteCountEmpty,
        SwapSlots,
        DropSlot,
        DuplicateSlot,
        DropTable,
        DuplicateTable,
        DropCall,
        DuplicateCall,
    }
}

pub(super) fn honest_dynamic_conformance_table_record() -> InstallationRecord {
    let artifact = build_object_artifact(&dynamic_conformance_table_plan())
        .expect("dynamic-table object artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("dynamic-table image");
    build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("dynamic-table installation")
}

pub(super) fn substitute_dynamic_conformance_table(
    record: &mut InstallationRecord,
    field: DynamicConformanceTableFieldForTest,
    _donor: &InstallationRecord,
) {
    use DynamicConformanceTableFieldForTest as Field;
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    match field {
        Field::TableApplicationReportFingerprint => {
            record.dynamic_conformance_tables_mut_for_test()[0].application_report_fingerprint =
                u64::MAX;
        }
        Field::TableSlot1Target => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                Some(machine_id(1));
        }
        Field::CallOperation => {
            record.dynamic_calls_mut_for_test()[0].operation = operation_id(98);
        }
        Field::CallTextOffset => {
            let text_offset = record
                .functions()
                .iter()
                .find(|function| function.machine == machine_id(3))
                .expect("dynamic caller row")
                .text_offset;
            record.dynamic_calls_mut_for_test()[0].text_offset = text_offset + 1;
        }
        Field::CallByteCount => {
            record.dynamic_calls_mut_for_test()[0].byte_count += 1;
        }
        Field::TableApplicationCommitment => {
            record.dynamic_conformance_tables_mut_for_test()[0].application_commitment =
                foreign_commitment;
        }
        Field::TableApplicationReportFingerprintZero => {
            record.dynamic_conformance_tables_mut_for_test()[0].application_report_fingerprint = 0;
        }
        Field::TableDataOffset => {
            record.dynamic_conformance_tables_mut_for_test()[0].data_offset = 8;
        }
        Field::TableByteCount => {
            record.dynamic_conformance_tables_mut_for_test()[0].byte_count = 8;
        }
        Field::TableSlot0RowIndex => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[0].row_index = 1;
        }
        Field::TableSlot0DataOffset => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[0].data_offset = 8;
        }
        Field::TableSlot0TargetUnknown => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target =
                Some(machine_id(98));
        }
        Field::TableSlot0TargetRetargeted => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target =
                Some(machine_id(1));
        }
        Field::TableSlot0TargetErased => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target = None;
        }
        Field::TableSlot1RowIndex => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[1].row_index = 0;
        }
        Field::TableSlot1DataOffset => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[1].data_offset = 0;
        }
        Field::TableSlot1TargetUnknown => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                Some(machine_id(98));
        }
        Field::CallMachineUnknown => {
            record.dynamic_calls_mut_for_test()[0].machine = machine_id(98);
        }
        Field::CallMachineOtherFunction => {
            record.dynamic_calls_mut_for_test()[0].machine = machine_id(1);
        }
        Field::CallApplicationCommitment => {
            record.dynamic_calls_mut_for_test()[0].application_commitment = foreign_commitment;
        }
        Field::CallInitialSource => {
            record.dynamic_calls_mut_for_test()[0].initial_source =
                PlaceId::new(98).expect("substituted source");
        }
        Field::CallReboundSource => {
            record.dynamic_calls_mut_for_test()[0].rebound_source =
                PlaceId::new(98).expect("substituted source");
        }
        Field::CallSelectedTableByteOffsetUnresolvedRow => {
            record.dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 8;
        }
        Field::CallSelectedTableByteOffsetMisaligned => {
            record.dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 7;
        }
        Field::CallRealization => {
            record.dynamic_calls_mut_for_test()[0].realization = machine_id(1);
        }
        Field::CallTextOffsetBeforeFunction => {
            let text_offset = record
                .functions()
                .iter()
                .find(|function| function.machine == machine_id(3))
                .expect("dynamic caller row")
                .text_offset;
            record.dynamic_calls_mut_for_test()[0].text_offset = text_offset - 1;
        }
        Field::CallByteCountEmpty => {
            record.dynamic_calls_mut_for_test()[0].byte_count = 0;
        }
        Field::SwapSlots => {
            record.dynamic_conformance_tables_mut_for_test()[0]
                .slots
                .swap(0, 1);
        }
        Field::DropSlot => {
            record.dynamic_conformance_tables_mut_for_test()[0]
                .slots
                .pop();
        }
        Field::DuplicateSlot => {
            let slot = record.dynamic_conformance_tables()[0].slots[1];
            record.dynamic_conformance_tables_mut_for_test()[0]
                .slots
                .push(slot);
        }
        Field::DropTable => {
            record.dynamic_conformance_tables_mut_for_test().pop();
        }
        Field::DuplicateTable => {
            let table = record.dynamic_conformance_tables()[0].clone();
            record.dynamic_conformance_tables_mut_for_test().push(table);
        }
        Field::DropCall => {
            record.dynamic_calls_mut_for_test().pop();
        }
        Field::DuplicateCall => {
            let call = record.dynamic_calls()[0];
            record.dynamic_calls_mut_for_test().push(call);
        }
    }
}

pub(super) fn dynamic_conformance_table_outcome(
    field: DynamicConformanceTableFieldForTest,
) -> MutationOutcome<InstallationError> {
    use DynamicConformanceTableFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::TableApplicationReportFingerprint
        | Field::TableSlot1Target
        | Field::CallOperation
        | Field::CallTextOffset
        | Field::CallByteCount => InstallationError::ImageBindingMismatch,
        Field::TableApplicationCommitment
        | Field::TableSlot0TargetRetargeted
        | Field::TableSlot0TargetErased
        | Field::CallApplicationCommitment
        | Field::CallInitialSource
        | Field::CallReboundSource
        | Field::CallSelectedTableByteOffsetUnresolvedRow
        | Field::CallSelectedTableByteOffsetMisaligned
        | Field::CallRealization
        | Field::CallTextOffsetBeforeFunction
        | Field::CallByteCountEmpty => InstallationError::InvalidDynamicCall(machine_id(3)),
        Field::TableApplicationReportFingerprintZero
        | Field::TableDataOffset
        | Field::TableByteCount
        | Field::TableSlot0RowIndex
        | Field::TableSlot0DataOffset
        | Field::TableSlot0TargetUnknown
        | Field::TableSlot1RowIndex
        | Field::TableSlot1DataOffset
        | Field::TableSlot1TargetUnknown
        | Field::SwapSlots
        | Field::DropSlot
        | Field::DuplicateSlot
        | Field::DuplicateTable
        | Field::DropCall => InstallationError::InvalidDynamicConformanceTable,
        Field::CallMachineUnknown => InstallationError::InvalidDynamicCall(machine_id(98)),
        Field::CallMachineOtherFunction => InstallationError::InvalidDynamicCall(machine_id(1)),
        Field::DropTable => InstallationError::InvalidImageSectionLayout,
        Field::DuplicateCall => InstallationError::InvalidDynamicCall(machine_id(3)),
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed dynamic-parameter call row or of
    /// the roster itself. The call's operation, source value, requirement
    /// slot, in-function text offset and byte count have no canonical
    /// record-shape join, so the substitution still encodes and is rejected
    /// by the recomputed identity and independent image replay; the machine
    /// join and the call interval against the caller's text are rejected at
    /// encoding.
    pub enum DynamicParameterCallFieldForTest {
        CallOperation,
        CallSourceValue,
        CallRequirementSlot,
        CallTextOffset,
        CallByteCount,
        CallMachineUnknown,
        CallMachineOtherFunction,
        CallTextOffsetBeforeFunction,
        CallTextOffsetPastFunction,
        CallByteCountEmpty,
        CallByteCountPastFunction,
        DropCall,
        DuplicateCall,
    }
}

pub(super) fn honest_dynamic_parameter_call_record() -> InstallationRecord {
    let artifact = build_object_artifact(&dynamic_parameter_call_plan())
        .expect("dynamic-parameter object artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("dynamic-parameter image");
    build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("dynamic-parameter installation")
}

pub(super) fn substitute_dynamic_parameter_call(
    record: &mut InstallationRecord,
    field: DynamicParameterCallFieldForTest,
    _donor: &InstallationRecord,
) {
    use DynamicParameterCallFieldForTest as Field;
    match field {
        Field::CallOperation => {
            record.dynamic_parameter_calls_mut_for_test()[0].operation = operation_id(98);
        }
        Field::CallSourceValue => {
            record.dynamic_parameter_calls_mut_for_test()[0].source_value =
                semantic_vocabulary::ValueId::new(98);
        }
        Field::CallRequirementSlot => {
            record.dynamic_parameter_calls_mut_for_test()[0].requirement_slot = 1;
        }
        Field::CallTextOffset => {
            let text_offset = record
                .functions()
                .iter()
                .find(|function| function.machine == machine_id(2))
                .expect("dynamic-parameter caller row")
                .text_offset;
            record.dynamic_parameter_calls_mut_for_test()[0].text_offset = text_offset + 1;
        }
        Field::CallByteCount => {
            record.dynamic_parameter_calls_mut_for_test()[0].byte_count += 1;
        }
        Field::CallMachineUnknown => {
            record.dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(98);
        }
        Field::CallMachineOtherFunction => {
            record.dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(1);
        }
        Field::CallTextOffsetBeforeFunction => {
            let text_offset = record
                .functions()
                .iter()
                .find(|function| function.machine == machine_id(2))
                .expect("dynamic-parameter caller row")
                .text_offset;
            record.dynamic_parameter_calls_mut_for_test()[0].text_offset = text_offset - 1;
        }
        Field::CallTextOffsetPastFunction => {
            let end = {
                let caller = record
                    .functions()
                    .iter()
                    .find(|function| function.machine == machine_id(2))
                    .expect("dynamic-parameter caller row");
                caller.text_offset + caller.byte_count
            };
            record.dynamic_parameter_calls_mut_for_test()[0].text_offset = end;
        }
        Field::CallByteCountEmpty => {
            record.dynamic_parameter_calls_mut_for_test()[0].byte_count = 0;
        }
        Field::CallByteCountPastFunction => {
            record.dynamic_parameter_calls_mut_for_test()[0].byte_count += 10;
        }
        Field::DropCall => {
            record.dynamic_parameter_calls_mut_for_test().pop();
        }
        Field::DuplicateCall => {
            let call = record.dynamic_parameter_calls()[0];
            record.dynamic_parameter_calls_mut_for_test().push(call);
        }
    }
}

pub(super) fn dynamic_parameter_call_outcome(
    field: DynamicParameterCallFieldForTest,
) -> MutationOutcome<InstallationError> {
    use DynamicParameterCallFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::CallOperation
        | Field::CallSourceValue
        | Field::CallRequirementSlot
        | Field::CallTextOffset
        | Field::CallByteCount
        | Field::DropCall => InstallationError::ImageBindingMismatch,
        Field::CallMachineUnknown => InstallationError::InvalidDynamicParameterCall(machine_id(98)),
        Field::CallMachineOtherFunction => {
            InstallationError::InvalidDynamicParameterCall(machine_id(1))
        }
        Field::CallTextOffsetBeforeFunction
        | Field::CallTextOffsetPastFunction
        | Field::CallByteCountEmpty
        | Field::CallByteCountPastFunction
        | Field::DuplicateCall => InstallationError::InvalidDynamicParameterCall(machine_id(2)),
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed stored dynamic call, its joined
    /// conformance table or the rosters themselves. The table report
    /// fingerprint and unselected slot target plus the call's operation,
    /// establishment operation, descriptor/selection ordinals, descriptor
    /// home offset, establishment extent, in-function text offset and byte
    /// count still encode and are rejected by the recomputed identity and
    /// independent image replay; every other axis and the roster
    /// canonicality joins are rejected at encoding.
    pub enum StoredDynamicCallFieldForTest {
        TableApplicationReportFingerprint,
        TableSlot1Target,
        CallOperation,
        CallEstablishmentOperation,
        CallDescriptorOrdinal,
        CallSelectionOrdinal,
        CallDescriptorHomeByteOffset,
        CallEstablishmentByteCount,
        CallTextOffset,
        CallByteCount,
        CallMachineUnknown,
        CallMachineOtherFunction,
        CallSourceUnknownPlace,
        CallApplicationCommitment,
        CallSelectedTableByteOffsetUnresolvedRow,
        CallSelectedTableByteOffsetMisaligned,
        CallSelectedTableByteOffsetPastRows,
        CallRealizationNotSelected,
        CallDescriptorHomeByteOffsetMisaligned,
        CallEstablishmentTextOffsetBeforeFunction,
        CallEstablishmentTextOffsetInside,
        CallEstablishmentByteCountEmpty,
        CallEstablishmentByteCountOverlapsCall,
        CallTextOffsetInsideEstablishment,
        CallTextOffsetPastFunction,
        CallByteCountEmpty,
        CallByteCountPastFunction,
        DropCall,
        DuplicateCall,
        DropTable,
        DuplicateTable,
        SwapSlots,
        DropSlot,
        DuplicateSlot,
    }
}

pub(super) fn honest_stored_dynamic_call_record() -> InstallationRecord {
    let artifact =
        build_object_artifact(&stored_dynamic_call_plan()).expect("stored dynamic object artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("stored dynamic image");
    build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("stored dynamic installation")
}

pub(super) fn substitute_stored_dynamic_call(
    record: &mut InstallationRecord,
    field: StoredDynamicCallFieldForTest,
    _donor: &InstallationRecord,
) {
    use StoredDynamicCallFieldForTest as Field;
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    let caller_bounds = |record: &InstallationRecord| {
        let caller = record
            .functions()
            .iter()
            .find(|function| function.machine == machine_id(3))
            .expect("stored dynamic caller row");
        (caller.text_offset, caller.text_offset + caller.byte_count)
    };
    match field {
        Field::TableApplicationReportFingerprint => {
            record.dynamic_conformance_tables_mut_for_test()[0].application_report_fingerprint =
                u64::MAX;
        }
        Field::TableSlot1Target => {
            record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                Some(machine_id(1));
        }
        Field::CallOperation => {
            record.stored_dynamic_calls_mut_for_test()[0].operation = operation_id(98);
        }
        Field::CallEstablishmentOperation => {
            record.stored_dynamic_calls_mut_for_test()[0].establishment_operation =
                operation_id(98);
        }
        Field::CallDescriptorOrdinal => {
            record.stored_dynamic_calls_mut_for_test()[0].descriptor_ordinal = 1;
        }
        Field::CallSelectionOrdinal => {
            record.stored_dynamic_calls_mut_for_test()[0].selection_ordinal = 1;
        }
        Field::CallDescriptorHomeByteOffset => {
            record.stored_dynamic_calls_mut_for_test()[0].descriptor_home_byte_offset = 8;
        }
        Field::CallEstablishmentByteCount => {
            record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count -= 1;
        }
        Field::CallTextOffset => {
            let (text_offset, _) = caller_bounds(record);
            record.stored_dynamic_calls_mut_for_test()[0].text_offset = text_offset + 26;
        }
        Field::CallByteCount => {
            record.stored_dynamic_calls_mut_for_test()[0].byte_count -= 1;
        }
        Field::CallMachineUnknown => {
            record.stored_dynamic_calls_mut_for_test()[0].machine = machine_id(98);
        }
        Field::CallMachineOtherFunction => {
            record.stored_dynamic_calls_mut_for_test()[0].machine = machine_id(1);
        }
        Field::CallSourceUnknownPlace => {
            record.stored_dynamic_calls_mut_for_test()[0].source = PlaceId::new(98).expect("place");
        }
        Field::CallApplicationCommitment => {
            record.stored_dynamic_calls_mut_for_test()[0].application_commitment =
                foreign_commitment;
        }
        Field::CallSelectedTableByteOffsetUnresolvedRow => {
            record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 8;
        }
        Field::CallSelectedTableByteOffsetMisaligned => {
            record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 7;
        }
        Field::CallSelectedTableByteOffsetPastRows => {
            record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 16;
        }
        Field::CallRealizationNotSelected => {
            record.stored_dynamic_calls_mut_for_test()[0].realization = machine_id(1);
        }
        Field::CallDescriptorHomeByteOffsetMisaligned => {
            record.stored_dynamic_calls_mut_for_test()[0].descriptor_home_byte_offset = 4;
        }
        Field::CallEstablishmentTextOffsetBeforeFunction => {
            let (text_offset, _) = caller_bounds(record);
            record.stored_dynamic_calls_mut_for_test()[0].establishment_text_offset =
                text_offset - 1;
        }
        Field::CallEstablishmentTextOffsetInside => {
            let (text_offset, _) = caller_bounds(record);
            record.stored_dynamic_calls_mut_for_test()[0].establishment_text_offset =
                text_offset + 1;
        }
        Field::CallEstablishmentByteCountEmpty => {
            record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count = 0;
        }
        Field::CallEstablishmentByteCountOverlapsCall => {
            record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count += 1;
        }
        Field::CallTextOffsetInsideEstablishment => {
            let (text_offset, _) = caller_bounds(record);
            record.stored_dynamic_calls_mut_for_test()[0].text_offset = text_offset + 24;
        }
        Field::CallTextOffsetPastFunction => {
            let (_, text_end) = caller_bounds(record);
            record.stored_dynamic_calls_mut_for_test()[0].text_offset = text_end;
        }
        Field::CallByteCountEmpty => {
            record.stored_dynamic_calls_mut_for_test()[0].byte_count = 0;
        }
        Field::CallByteCountPastFunction => {
            record.stored_dynamic_calls_mut_for_test()[0].byte_count += 100;
        }
        Field::DropCall => {
            record.stored_dynamic_calls_mut_for_test().pop();
        }
        Field::DuplicateCall => {
            let call = record.stored_dynamic_calls()[0];
            record.stored_dynamic_calls_mut_for_test().push(call);
        }
        Field::DropTable => {
            record.dynamic_conformance_tables_mut_for_test().pop();
        }
        Field::DuplicateTable => {
            let table = record.dynamic_conformance_tables()[0].clone();
            record.dynamic_conformance_tables_mut_for_test().push(table);
        }
        Field::SwapSlots => {
            record.dynamic_conformance_tables_mut_for_test()[0]
                .slots
                .swap(0, 1);
        }
        Field::DropSlot => {
            record.dynamic_conformance_tables_mut_for_test()[0]
                .slots
                .pop();
        }
        Field::DuplicateSlot => {
            let slot = record.dynamic_conformance_tables()[0].slots[0];
            record.dynamic_conformance_tables_mut_for_test()[0]
                .slots
                .push(slot);
        }
    }
}

pub(super) fn stored_dynamic_call_outcome(
    field: StoredDynamicCallFieldForTest,
) -> MutationOutcome<InstallationError> {
    use StoredDynamicCallFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::TableApplicationReportFingerprint
        | Field::TableSlot1Target
        | Field::CallOperation
        | Field::CallEstablishmentOperation
        | Field::CallDescriptorOrdinal
        | Field::CallSelectionOrdinal
        | Field::CallDescriptorHomeByteOffset
        | Field::CallEstablishmentByteCount
        | Field::CallTextOffset
        | Field::CallByteCount => InstallationError::ImageBindingMismatch,
        Field::CallMachineUnknown => InstallationError::InvalidStoredDynamicCall(machine_id(98)),
        Field::CallMachineOtherFunction => {
            InstallationError::InvalidStoredDynamicCall(machine_id(1))
        }
        Field::CallSourceUnknownPlace
        | Field::CallApplicationCommitment
        | Field::CallSelectedTableByteOffsetUnresolvedRow
        | Field::CallSelectedTableByteOffsetMisaligned
        | Field::CallSelectedTableByteOffsetPastRows
        | Field::CallRealizationNotSelected
        | Field::CallDescriptorHomeByteOffsetMisaligned
        | Field::CallEstablishmentTextOffsetBeforeFunction
        | Field::CallEstablishmentTextOffsetInside
        | Field::CallEstablishmentByteCountEmpty
        | Field::CallEstablishmentByteCountOverlapsCall
        | Field::CallTextOffsetInsideEstablishment
        | Field::CallTextOffsetPastFunction
        | Field::CallByteCountEmpty
        | Field::CallByteCountPastFunction => {
            InstallationError::InvalidStoredDynamicCall(machine_id(3))
        }
        Field::DropCall
        | Field::DuplicateTable
        | Field::SwapSlots
        | Field::DropSlot
        | Field::DuplicateSlot => InstallationError::InvalidDynamicConformanceTable,
        Field::DuplicateCall => InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        Field::DropTable => InstallationError::InvalidImageSectionLayout,
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed forwarded dynamic-parameter call
    /// row or of the roster itself. The call's operation, source value,
    /// scalar type, callee identity, in-function text offset and byte count
    /// still encode and are rejected by the recomputed identity and
    /// independent image replay; the machine join, caller/callee resolution,
    /// parameter ordinals and the call interval are rejected at encoding.
    pub enum ForwardedDynamicParameterCallFieldForTest {
        CallOperation,
        CallSourceValue,
        CallScalarType,
        CallCalleeSelfFunction,
        CallTextOffset,
        CallByteCount,
        CallMachineUnknown,
        CallMachineOtherFunction,
        CallCalleeUnknown,
        CallSourceParameterOrdinal,
        CallTargetParameterOrdinal,
        CallSourceValueCleared,
        CallScalarTypeCleared,
        CallScalarTypeFloat,
        CallTextOffsetBeforeFunction,
        CallTextOffsetPastFunction,
        CallByteCountEmpty,
        DropCall,
        DuplicateCall,
    }
}

pub(super) fn honest_forwarded_dynamic_parameter_call_record() -> InstallationRecord {
    let artifact = build_object_artifact(&forwarded_dynamic_parameter_call_plan())
        .expect("forwarded dynamic-parameter object artifact");
    let image =
        emit_direct_executable_image(&artifact, 3).expect("forwarded dynamic-parameter image");
    build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("forwarded dynamic-parameter installation")
}

pub(super) fn substitute_forwarded_dynamic_parameter_call(
    record: &mut InstallationRecord,
    field: ForwardedDynamicParameterCallFieldForTest,
    _donor: &InstallationRecord,
) {
    use ForwardedDynamicParameterCallFieldForTest as Field;
    let caller_bounds = |record: &InstallationRecord| {
        let caller = record
            .functions()
            .iter()
            .find(|function| function.machine == machine_id(2))
            .expect("forwarded dynamic-parameter caller row");
        (caller.text_offset, caller.text_offset + caller.byte_count)
    };
    match field {
        Field::CallOperation => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].operation = operation_id(98);
        }
        Field::CallSourceValue => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].source_value =
                semantic_vocabulary::ValueId::new(98);
        }
        Field::CallScalarType => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type =
                Some(semantic_vocabulary::ScalarType::Boolean);
        }
        Field::CallCalleeSelfFunction => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].callee = machine_id(2);
        }
        Field::CallTextOffset => {
            let (text_offset, _) = caller_bounds(record);
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                text_offset + 1;
        }
        Field::CallByteCount => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].byte_count += 1;
        }
        Field::CallMachineUnknown => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(98);
        }
        Field::CallMachineOtherFunction => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(1);
        }
        Field::CallCalleeUnknown => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].callee = machine_id(98);
        }
        Field::CallSourceParameterOrdinal => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].source_parameter_ordinal = 1;
        }
        Field::CallTargetParameterOrdinal => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].target_parameter_ordinal = 1;
        }
        Field::CallSourceValueCleared => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].source_value = None;
        }
        Field::CallScalarTypeCleared => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type = None;
        }
        Field::CallScalarTypeFloat => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type =
                Some(semantic_vocabulary::ScalarType::IeeeFloat(
                    semantic_vocabulary::IeeeFloatFormat::Binary64,
                ));
        }
        Field::CallTextOffsetBeforeFunction => {
            let (text_offset, _) = caller_bounds(record);
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                text_offset - 1;
        }
        Field::CallTextOffsetPastFunction => {
            let (_, text_end) = caller_bounds(record);
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset = text_end;
        }
        Field::CallByteCountEmpty => {
            record.forwarded_dynamic_parameter_calls_mut_for_test()[0].byte_count = 0;
        }
        Field::DropCall => {
            record
                .forwarded_dynamic_parameter_calls_mut_for_test()
                .pop();
        }
        Field::DuplicateCall => {
            let call = record.forwarded_dynamic_parameter_calls()[0];
            record
                .forwarded_dynamic_parameter_calls_mut_for_test()
                .push(call);
        }
    }
}

pub(super) fn forwarded_dynamic_parameter_call_outcome(
    field: ForwardedDynamicParameterCallFieldForTest,
) -> MutationOutcome<InstallationError> {
    use ForwardedDynamicParameterCallFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::CallOperation
        | Field::CallSourceValue
        | Field::CallScalarType
        | Field::CallCalleeSelfFunction
        | Field::CallTextOffset
        | Field::CallByteCount
        | Field::DropCall => InstallationError::ImageBindingMismatch,
        Field::CallMachineUnknown => {
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(98))
        }
        Field::CallMachineOtherFunction => {
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(1))
        }
        Field::CallCalleeUnknown
        | Field::CallSourceParameterOrdinal
        | Field::CallTargetParameterOrdinal
        | Field::CallSourceValueCleared
        | Field::CallScalarTypeCleared
        | Field::CallScalarTypeFloat
        | Field::CallTextOffsetBeforeFunction
        | Field::CallTextOffsetPastFunction
        | Field::CallByteCountEmpty
        | Field::DuplicateCall => {
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2))
        }
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed forwarded dynamic-descriptor
    /// adapter, table, slot or call row, or of the rosters themselves. The
    /// table report fingerprint, the call's operation, alternate callee,
    /// structural source place/access, in-function text offset and byte count
    /// still encode and are rejected by the recomputed identity and
    /// independent image replay; every other axis and the roster
    /// canonicality joins are rejected at encoding.
    pub enum ForwardedDynamicDescriptorFieldForTest {
        TableApplicationReportFingerprint,
        CallOperation,
        CallCalleeOtherFunction,
        CallSourcePlace,
        CallSourceAccess,
        CallTextOffset,
        CallByteCount,
        AdapterApplicationCommitment,
        AdapterRowIndex,
        AdapterRealization,
        AdapterTextOffset,
        AdapterByteCountEmpty,
        AdapterByteCountPastTextEnd,
        TableApplicationCommitment,
        TableDataOffset,
        TableByteCount,
        TableSlot0RowIndex,
        TableSlot0RealizationUnknown,
        TableSlot0RealizationNotAdapter,
        TableSlot0AdapterTextOffset,
        TableSlot0DataOffset,
        CallMachineUnknown,
        CallMachineOtherFunction,
        CallCalleeUnknown,
        CallApplicationCommitment,
        CallSemanticResultWithoutResult,
        CallTextOffsetBeforeFunction,
        CallTextOffsetPastFunction,
        CallByteCountEmpty,
        CallByteCountPastFunction,
        DropCall,
        DuplicateCall,
        DropAdapter,
        DuplicateAdapter,
        DropTable,
        DuplicateTable,
        DropSlot,
        DuplicateSlot,
    }
}

pub(super) fn honest_forwarded_dynamic_descriptor_record() -> InstallationRecord {
    let artifact = build_object_artifact(&forwarded_dynamic_descriptor_call_plan())
        .expect("forwarded dynamic-descriptor object artifact");
    let image =
        emit_direct_executable_image(&artifact, 3).expect("forwarded dynamic-descriptor image");
    build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("forwarded dynamic-descriptor installation")
}

pub(super) fn substitute_forwarded_dynamic_descriptor(
    record: &mut InstallationRecord,
    field: ForwardedDynamicDescriptorFieldForTest,
    _donor: &InstallationRecord,
) {
    use ForwardedDynamicDescriptorFieldForTest as Field;
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    let caller_bounds = |record: &InstallationRecord| {
        let caller = record
            .functions()
            .iter()
            .find(|function| function.machine == machine_id(3))
            .expect("forwarded dynamic-descriptor caller row");
        (caller.text_offset, caller.text_offset + caller.byte_count)
    };
    match field {
        Field::TableApplicationReportFingerprint => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
                .application_report_fingerprint = u64::MAX;
        }
        Field::CallOperation => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].operation =
                operation_id(98);
        }
        Field::CallCalleeOtherFunction => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].callee = machine_id(2);
        }
        Field::CallSourcePlace => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                .source
                .place = PlaceId::new(98).expect("place");
        }
        Field::CallSourceAccess => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                .source
                .access = StructuralAccess::Owned;
        }
        Field::CallTextOffset => {
            let (text_offset, _) = caller_bounds(record);
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                text_offset + 1;
        }
        Field::CallByteCount => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count -= 1;
        }
        Field::AdapterApplicationCommitment => {
            record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].application_commitment =
                foreign_commitment;
        }
        Field::AdapterRowIndex => {
            record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].row_index = 1;
        }
        Field::AdapterRealization => {
            record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].realization =
                machine_id(1);
        }
        Field::AdapterTextOffset => {
            record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].text_offset += 1;
        }
        Field::AdapterByteCountEmpty => {
            record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].byte_count = 0;
        }
        Field::AdapterByteCountPastTextEnd => {
            record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].byte_count += 1;
        }
        Field::TableApplicationCommitment => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].application_commitment =
                foreign_commitment;
        }
        Field::TableDataOffset => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].data_offset = 8;
        }
        Field::TableByteCount => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].byte_count = 16;
        }
        Field::TableSlot0RowIndex => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].row_index = 1;
        }
        Field::TableSlot0RealizationUnknown => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].realization =
                machine_id(98);
        }
        Field::TableSlot0RealizationNotAdapter => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].realization =
                machine_id(1);
        }
        Field::TableSlot0AdapterTextOffset => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0]
                .adapter_text_offset += 1;
        }
        Field::TableSlot0DataOffset => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].data_offset = 8;
        }
        Field::CallMachineUnknown => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].machine = machine_id(98);
        }
        Field::CallMachineOtherFunction => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].machine = machine_id(1);
        }
        Field::CallCalleeUnknown => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].callee = machine_id(98);
        }
        Field::CallApplicationCommitment => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].application_commitment =
                foreign_commitment;
        }
        Field::CallSemanticResultWithoutResult => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].semantic_result =
                Some(abstract_operations::AbstractResult {
                    value: semantic_vocabulary::ValueId::new(98).expect("value"),
                    scalar_type: semantic_vocabulary::ScalarType::Boolean,
                });
        }
        Field::CallTextOffsetBeforeFunction => {
            let (text_offset, _) = caller_bounds(record);
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                text_offset - 1;
        }
        Field::CallTextOffsetPastFunction => {
            let (_, text_end) = caller_bounds(record);
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset = text_end;
        }
        Field::CallByteCountEmpty => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count = 0;
        }
        Field::CallByteCountPastFunction => {
            record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count += 100;
        }
        Field::DropCall => {
            record
                .forwarded_dynamic_descriptor_calls_mut_for_test()
                .pop();
        }
        Field::DuplicateCall => {
            let call = record.forwarded_dynamic_descriptor_calls()[0].clone();
            record
                .forwarded_dynamic_descriptor_calls_mut_for_test()
                .push(call);
        }
        Field::DropAdapter => {
            record
                .forwarded_dynamic_descriptor_adapters_mut_for_test()
                .pop();
        }
        Field::DuplicateAdapter => {
            let adapter = record.forwarded_dynamic_descriptor_adapters()[0];
            record
                .forwarded_dynamic_descriptor_adapters_mut_for_test()
                .push(adapter);
        }
        Field::DropTable => {
            record
                .forwarded_dynamic_descriptor_tables_mut_for_test()
                .pop();
        }
        Field::DuplicateTable => {
            let table = record.forwarded_dynamic_descriptor_tables()[0].clone();
            record
                .forwarded_dynamic_descriptor_tables_mut_for_test()
                .push(table);
        }
        Field::DropSlot => {
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
                .slots
                .pop();
        }
        Field::DuplicateSlot => {
            let slot = record.forwarded_dynamic_descriptor_tables()[0].slots[0];
            record.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
                .slots
                .push(slot);
        }
    }
}

pub(super) fn forwarded_dynamic_descriptor_outcome(
    field: ForwardedDynamicDescriptorFieldForTest,
) -> MutationOutcome<InstallationError> {
    use ForwardedDynamicDescriptorFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::TableApplicationReportFingerprint
        | Field::CallOperation
        | Field::CallCalleeOtherFunction
        | Field::CallSourcePlace
        | Field::CallSourceAccess
        | Field::CallTextOffset
        | Field::CallByteCount => InstallationError::ImageBindingMismatch,
        Field::AdapterApplicationCommitment
        | Field::AdapterRowIndex
        | Field::AdapterRealization
        | Field::AdapterTextOffset
        | Field::TableApplicationCommitment
        | Field::TableDataOffset
        | Field::TableByteCount
        | Field::TableSlot0RowIndex
        | Field::TableSlot0RealizationUnknown
        | Field::TableSlot0RealizationNotAdapter
        | Field::TableSlot0AdapterTextOffset
        | Field::TableSlot0DataOffset
        | Field::DropCall
        | Field::DropAdapter
        | Field::DuplicateTable
        | Field::DropSlot
        | Field::DuplicateSlot => InstallationError::InvalidForwardedDynamicDescriptorTable,
        Field::AdapterByteCountEmpty | Field::DuplicateAdapter => {
            InstallationError::InvalidForwardedDynamicDescriptorAdapter
        }
        Field::AdapterByteCountPastTextEnd | Field::DropTable => {
            InstallationError::InvalidImageSectionLayout
        }
        Field::CallMachineUnknown => {
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(98))
        }
        Field::CallMachineOtherFunction => {
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(1))
        }
        Field::CallCalleeUnknown
        | Field::CallApplicationCommitment
        | Field::CallSemanticResultWithoutResult
        | Field::CallTextOffsetBeforeFunction
        | Field::CallTextOffsetPastFunction
        | Field::CallByteCountEmpty
        | Field::CallByteCountPastFunction
        | Field::DuplicateCall => {
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3))
        }
    })
}

optimization_core::custody_field_inventory! {
    /// One substitutable axis of an installed privileged port-effect row or of
    /// the roster itself. The unbound row's semantic axes are independently
    /// representable: each substitution still encodes and is rejected by the
    /// recomputed identity and independent image replay. Physical axes are
    /// canonical projections, and every field of the settlement-consumed row
    /// is pinned by the `MetadataOnlyPort` realization's exact effect lookup;
    /// both are rejected at encoding.
    pub enum PortEffectFieldForTest {
        UnboundPsiOperation,
        UnboundService,
        UnboundPort,
        UnboundValue,
        UnboundOperationOrdinal,
        UnboundMachine,
        UnboundCodeOffset,
        UnboundByteCount,
        UnboundTextOffset,
        BoundMachine,
        BoundPsiOperation,
        BoundService,
        BoundPort,
        BoundValue,
        BoundOperationOrdinal,
        BoundCodeOffset,
        BoundByteCount,
        BoundTextOffset,
        DropUnboundRow,
        DropBoundRow,
        DuplicateBoundAsFirstOperation,
        ReorderRows,
    }
}

pub(super) fn honest_port_effect_record() -> InstallationRecord {
    let provider = WriteExitProvider(7);
    let artifact =
        build_object_artifact(&port_effect_plan(&provider)).expect("port-effect artifact");
    let image = emit_direct_executable_image(&artifact, 3).expect("port-effect image");
    build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(17).expect("profile"),
        [&provider],
    )
    .expect("port-effect installation")
}

pub(super) fn substitute_port_effect(
    record: &mut InstallationRecord,
    field: PortEffectFieldForTest,
    _donor: &InstallationRecord,
) {
    use PortEffectFieldForTest as Field;
    match field {
        Field::UnboundPsiOperation => {
            record.port_effects_mut_for_test()[0].effect.psi_operation = operation_id(96);
        }
        Field::UnboundService => {
            record.port_effects_mut_for_test()[0].effect.service =
                ServiceId::new(96).expect("drifted service");
        }
        Field::UnboundPort => {
            record.port_effects_mut_for_test()[0].effect.port = 0x64;
        }
        Field::UnboundValue => {
            record.port_effects_mut_for_test()[0].effect.value = 0x7f;
        }
        Field::UnboundOperationOrdinal => {
            record.port_effects_mut_for_test()[0]
                .effect
                .operation_ordinal = 9;
        }
        Field::UnboundMachine => {
            record.port_effects_mut_for_test()[0].machine = machine_id(96);
        }
        Field::UnboundCodeOffset => {
            record.port_effects_mut_for_test()[0].effect.code_offset += 1;
        }
        Field::UnboundByteCount => {
            record.port_effects_mut_for_test()[0].effect.byte_count -= 1;
        }
        Field::UnboundTextOffset => {
            record.port_effects_mut_for_test()[0].text_offset += 1;
        }
        Field::BoundMachine => {
            record.port_effects_mut_for_test()[1].machine = machine_id(96);
        }
        Field::BoundPsiOperation => {
            record.port_effects_mut_for_test()[1].effect.psi_operation = operation_id(96);
        }
        Field::BoundService => {
            record.port_effects_mut_for_test()[1].effect.service =
                ServiceId::new(96).expect("drifted service");
        }
        Field::BoundPort => {
            record.port_effects_mut_for_test()[1].effect.port = 0x64;
        }
        Field::BoundValue => {
            record.port_effects_mut_for_test()[1].effect.value = 0x7f;
        }
        Field::BoundOperationOrdinal => {
            record.port_effects_mut_for_test()[1]
                .effect
                .operation_ordinal = 9;
        }
        Field::BoundCodeOffset => {
            record.port_effects_mut_for_test()[1].effect.code_offset += 1;
        }
        Field::BoundByteCount => {
            record.port_effects_mut_for_test()[1].effect.byte_count -= 1;
        }
        Field::BoundTextOffset => {
            record.port_effects_mut_for_test()[1].text_offset += 1;
        }
        Field::DropUnboundRow => {
            record.port_effects_mut_for_test().remove(0);
        }
        Field::DropBoundRow => {
            record.port_effects_mut_for_test().remove(1);
        }
        Field::DuplicateBoundAsFirstOperation => {
            let mut duplicate = record.port_effects()[1].clone();
            duplicate.effect.psi_operation = operation_id(1);
            duplicate.effect.operation_ordinal = 5;
            record.port_effects_mut_for_test().push(duplicate);
        }
        Field::ReorderRows => {
            record.port_effects_mut_for_test().swap(0, 1);
        }
    }
}

pub(super) fn port_effect_outcome(
    field: PortEffectFieldForTest,
) -> MutationOutcome<InstallationError> {
    use PortEffectFieldForTest as Field;
    MutationOutcome::ExactError(match field {
        Field::UnboundPsiOperation
        | Field::UnboundService
        | Field::UnboundPort
        | Field::UnboundValue
        | Field::UnboundOperationOrdinal
        | Field::DropUnboundRow => InstallationError::ImageBindingMismatch,
        Field::UnboundMachine | Field::BoundMachine => {
            InstallationError::EffectMachineMissing(machine_id(96))
        }
        Field::UnboundCodeOffset | Field::UnboundByteCount | Field::UnboundTextOffset => {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(1),
            }
        }
        Field::BoundPsiOperation
        | Field::BoundService
        | Field::BoundPort
        | Field::BoundValue
        | Field::BoundOperationOrdinal
        | Field::DropBoundRow => InstallationError::BoundaryRealizationMismatch {
            machine: machine_id(1),
            operation: operation_id(3),
        },
        Field::BoundCodeOffset | Field::BoundByteCount | Field::BoundTextOffset => {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(2),
            }
        }
        Field::DuplicateBoundAsFirstOperation => InstallationError::DuplicatePortEffectOperation {
            machine: machine_id(1),
            operation: operation_id(1),
        },
        Field::ReorderRows => InstallationError::NonCanonicalPortEffectOrder,
    })
}
