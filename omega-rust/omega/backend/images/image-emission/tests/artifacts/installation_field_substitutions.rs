use super::{
    WriteExitProvider, callback_private_plan, dynamic_conformance_table_plan,
    dynamic_parameter_call_plan, edge_id, forwarded_dynamic_descriptor_call_plan,
    forwarded_dynamic_parameter_call_plan, identity, linux_write_line_exit_plan, machine_id,
    operation_id, port_effect_plan, stored_dynamic_call_plan,
};
use function_identity::{MachineFunctionIdentity, StateKey};
use image_emission::{
    InstallationError, build_installation_record,
    build_installation_record_with_provider_executions, build_object_artifact,
    build_object_artifact_with_private_functions, decode_installation_record,
    emit_executable_image, encode_installation_record, installation_fingerprint,
    validate_installation_record,
};
use machine_code::{SemanticCodeAttribution, SemanticCodeSite};
use semantic_vocabulary::{PlaceId, ProfileDecisionId, ServiceId};
use symbols::SymbolHandle;
use terminal_psi::{SemanticFingerprint, StructuralAccess, StructuralArgument};

/// Every representable field of an installed semantic-code attribution row is
/// an authenticated custody axis: a one-field substitution either cannot
/// encode canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects it.
/// Operation and edge site identities and in-bounds byte counts join only
/// against the retained image rows; the machine join, the canonical
/// (machine, operation_ordinal, text_offset) order, interval geometry, and
/// the boundary-joined nominal return edge are canonical record-shape
/// custody rejected at encoding.
#[test]
fn installation_semantic_code_attribution_rejects_every_one_field_substitution() {
    let write_provider = WriteExitProvider(970);
    let plan = linux_write_line_exit_plan(&write_provider);
    let artifact = build_object_artifact(&plan).expect("attribution artifact");
    let image = emit_executable_image(&artifact, 3).expect("attribution image");
    let record = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(97).unwrap(),
        [&write_provider],
    )
    .expect("attribution installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let function = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(97))
        .expect("attributed function row");
    let function_byte_count = function.byte_count;
    let [literal_row, write_row, constant_row, exit_row, return_row] =
        record.semantic_code_attribution()
    else {
        panic!("write+exit fixture retains five attribution rows");
    };
    assert_eq!(literal_row.machine, machine_id(97));
    assert_eq!(
        literal_row.attribution,
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(operation_id(97)),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 0,
        }
    );
    assert_eq!(literal_row.text_offset, function.text_offset);
    assert_eq!(
        write_row.attribution.site,
        SemanticCodeSite::Operation(operation_id(98))
    );
    assert_eq!(write_row.attribution.operation_ordinal, 1);
    assert_eq!(write_row.attribution.code_offset, 0);
    assert_eq!(
        constant_row.attribution.site,
        SemanticCodeSite::Operation(operation_id(99))
    );
    assert_eq!(constant_row.attribution.operation_ordinal, 2);
    assert_eq!(constant_row.attribution.byte_count, 0);
    assert_eq!(
        exit_row.attribution.site,
        SemanticCodeSite::Operation(operation_id(100))
    );
    assert_eq!(exit_row.attribution.operation_ordinal, 3);
    assert_eq!(
        return_row.attribution.site,
        SemanticCodeSite::Edge(edge_id(97))
    );
    assert_eq!(return_row.attribution.operation_ordinal, 4);
    assert_eq!(
        return_row
            .attribution
            .code_offset
            .checked_add(return_row.attribution.byte_count),
        Some(function_byte_count)
    );

    type ReplayMutation = (
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::ObjectCodeAttribution)>,
    );
    // Semantic site identities and in-bounds byte counts have no canonical
    // record-shape join: the substituted row still encodes and decodes, so
    // rejection is the recomputed identity and the independent image replay.
    let still_encodes: Vec<ReplayMutation> = vec![
        (
            "site::operation_identity",
            0,
            Box::new(|row| {
                row.attribution.site = SemanticCodeSite::Operation(operation_id(199));
            }),
        ),
        (
            "site::edge_identity",
            4,
            Box::new(|row| {
                row.attribution.site = SemanticCodeSite::Edge(edge_id(199));
            }),
        ),
        (
            "byte_count::zero_marker_row",
            0,
            Box::new(|row| {
                row.attribution.byte_count = 1;
            }),
        ),
        (
            "byte_count::in_bounds_row",
            1,
            Box::new(|row| {
                row.attribution.byte_count -= 1;
            }),
        ),
    ];
    for (field, index, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed.semantic_code_attribution_mut_for_test()[index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted row decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted row"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    let invalid_attribution =
        |site: SemanticCodeSite| InstallationError::InvalidSemanticCodeAttribution {
            machine: machine_id(97),
            site,
        };
    let exit_mismatch = || InstallationError::BoundaryRealizationMismatch {
        machine: machine_id(97),
        operation: operation_id(100),
    };
    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let rejected: Vec<(
        &'static str,
        usize,
        Box<dyn Fn(&mut image_emission::ObjectCodeAttribution)>,
        InstallationError,
    )> = vec![
        (
            "machine::unknown",
            4,
            Box::new(|row| {
                row.machine = machine_id(199);
            }),
            InstallationError::SemanticCodeAttributionMachineMissing(machine_id(199)),
        ),
        (
            "operation_ordinal::reorders_roster",
            0,
            Box::new(|row| {
                row.attribution.operation_ordinal = 5;
            }),
            InstallationError::NonCanonicalSemanticCodeAttributionOrder,
        ),
        (
            "code_offset::operation_row",
            1,
            Box::new(|row| {
                row.attribution.code_offset += 1;
            }),
            invalid_attribution(SemanticCodeSite::Operation(operation_id(98))),
        ),
        (
            "text_offset::operation_row",
            1,
            Box::new(|row| {
                row.text_offset += 1;
            }),
            invalid_attribution(SemanticCodeSite::Operation(operation_id(98))),
        ),
        (
            "byte_count::past_function_end",
            1,
            Box::new(move |row| {
                row.attribution.byte_count = function_byte_count + 1;
            }),
            invalid_attribution(SemanticCodeSite::Operation(operation_id(98))),
        ),
        (
            "site::tail_edge_to_operation",
            4,
            Box::new(|row| {
                row.attribution.site = SemanticCodeSite::Operation(operation_id(199));
            }),
            exit_mismatch(),
        ),
        (
            "operation_ordinal::tail_edge",
            4,
            Box::new(|row| {
                row.attribution.operation_ordinal = 5;
            }),
            exit_mismatch(),
        ),
        (
            "code_offset::tail_edge",
            4,
            Box::new(|row| {
                row.attribution.code_offset -= 1;
            }),
            invalid_attribution(SemanticCodeSite::Edge(edge_id(97))),
        ),
        (
            "byte_count::tail_edge",
            4,
            Box::new(|row| {
                row.attribution.byte_count = 0;
            }),
            exit_mismatch(),
        ),
    ];
    for (field, index, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed.semantic_code_attribution_mut_for_test()[index]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping an unjoined operation row still encodes,
    // so only the image binding rejects it; dropping the nominal return-edge
    // row breaks the boundary-joined tail at encoding, while a duplicated or
    // reordered roster hits the canonical ordering rule.
    let mut dropped_row = record.clone();
    dropped_row
        .semantic_code_attribution_mut_for_test()
        .remove(0);
    let bytes = encode_installation_record(&dropped_row).expect("dropped row encodes");
    let replayed = decode_installation_record(&bytes).expect("dropped row decodes");
    assert_ne!(
        installation_fingerprint(&replayed).expect("dropped fingerprint"),
        authentic_fingerprint
    );
    assert_eq!(
        validate_installation_record(&replayed, &image),
        Err(InstallationError::ImageBindingMismatch)
    );
    let mut dropped_edge_row = record.clone();
    dropped_edge_row
        .semantic_code_attribution_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_edge_row),
        Err(exit_mismatch())
    );
    let mut duplicated_row = record.clone();
    let row = duplicated_row.semantic_code_attribution()[1].clone();
    duplicated_row
        .semantic_code_attribution_mut_for_test()
        .insert(2, row);
    assert_eq!(
        encode_installation_record(&duplicated_row),
        Err(InstallationError::NonCanonicalSemanticCodeAttributionOrder)
    );
    let mut reordered = record.clone();
    reordered
        .semantic_code_attribution_mut_for_test()
        .swap(1, 2);
    assert_eq!(
        encode_installation_record(&reordered),
        Err(InstallationError::NonCanonicalSemanticCodeAttributionOrder)
    );
}

/// Every representable field of an installed compiler-private callback row is
/// an authenticated custody axis: a one-field substitution either cannot
/// encode canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects it.
#[test]
fn installation_private_function_row_rejects_every_one_field_substitution() {
    let plan = callback_private_plan();
    let artifact =
        build_object_artifact_with_private_functions(&plan).expect("callback private artifact");
    let image = emit_executable_image(&artifact, 3).expect("callback private image");
    let record = build_installation_record(&image, ProfileDecisionId::new(53).expect("profile"))
        .expect("callback private installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let [authentic] = record.private_functions() else {
        panic!("callback fixture retains one private row");
    };
    let text_offset = authentic.text_offset;
    let byte_count = authentic.byte_count;
    assert_eq!(authentic.machine, machine_id(97));
    assert_eq!(authentic.source_psi, identity());
    assert!(
        authentic
            .identity
            .callback_thunk_placement_index()
            .is_some()
    );

    type ReplayMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstalledCompilerPrivateFunction)>,
    );
    // Semantic identities no canonical record-shape join pins: the substituted
    // row still encodes and decodes, so rejection is the recomputed identity
    // and the independent image replay.
    let still_encodes: Vec<ReplayMutation> = vec![
        (
            "identity::continuation",
            Box::new(|row| {
                row.identity = MachineFunctionIdentity::callback_thunk(
                    StateKey {
                        machine: SymbolHandle::from_parts(15, 2),
                        state: SymbolHandle::from_parts(13, 3),
                        segment_index: 0,
                    },
                    0,
                )
                .expect("substituted callback thunk identity");
            }),
        ),
        (
            "identity::placement_index",
            Box::new(|row| {
                row.identity = MachineFunctionIdentity::callback_thunk(
                    row.identity.associated_source_continuation(),
                    3,
                )
                .expect("substituted placement index");
            }),
        ),
        (
            "source_psi::program_fingerprint",
            Box::new(|row| {
                row.source_psi.program_fingerprint = SemanticFingerprint::from_bytes([7; 32]);
            }),
        ),
        (
            "machine",
            Box::new(|row| {
                row.machine = machine_id(98);
            }),
        ),
        (
            "scalar_abi::parameters::value",
            Box::new(|row| {
                row.scalar_abi.parameters[0].value =
                    semantic_vocabulary::ValueId::new(41).expect("substituted parameter value");
            }),
        ),
        (
            "scalar_abi::result::value",
            Box::new(|row| {
                row.scalar_abi.result.value =
                    semantic_vocabulary::ValueId::new(43).expect("substituted result value");
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed.private_functions_mut_for_test()[0]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted row decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted row"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstalledCompilerPrivateFunction)>,
        InstallationError,
    );
    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let rejected: Vec<RejectedMutation> = vec![
        (
            "identity::source_kind",
            Box::new(|row| {
                row.identity =
                    MachineFunctionIdentity::source(row.identity.associated_source_continuation());
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "text_offset",
            Box::new(move |row| {
                row.text_offset = text_offset + 1;
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "byte_count::empty",
            Box::new(|row| {
                row.byte_count = 0;
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "byte_count::extended",
            Box::new(move |row| {
                row.byte_count = byte_count + 1;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "scalar_abi::parameters::placement",
            Box::new(|row| {
                row.scalar_abi.parameters[0].placement = row.scalar_abi.result.placement.clone();
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
        (
            "scalar_abi::result::collision",
            Box::new(|row| {
                row.scalar_abi.result.value = row.scalar_abi.parameters[0].value;
            }),
            InstallationError::InvalidCompilerPrivateFunction,
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed.private_functions_mut_for_test()[0]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the row leaves its text bytes unaccounted,
    // so the canonical section join rejects it at encoding.
    let mut dropped_row = record.clone();
    dropped_row.private_functions_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_row),
        Err(InstallationError::InvalidImageSectionLayout)
    );
}

#[test]
fn installation_dynamic_conformance_table_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&dynamic_conformance_table_plan())
        .expect("dynamic-table object artifact");
    assert_eq!(artifact.dynamic_conformance_tables().len(), 1);
    assert_eq!(artifact.dynamic_conformance_tables()[0].slots.len(), 2);
    assert_eq!(
        artifact.dynamic_conformance_tables()[0].slots[0].target,
        Some(machine_id(2))
    );
    assert_eq!(
        artifact.dynamic_conformance_tables()[0].slots[1].target,
        None
    );
    let image = emit_executable_image(&artifact, 3).expect("dynamic-table image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("dynamic-table installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_conformance_tables().len(), 1);
    assert_eq!(record.dynamic_conformance_tables()[0].slots.len(), 2);
    assert_eq!(record.dynamic_calls().len(), 1);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let function_text_offset = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("dynamic caller row")
        .text_offset;

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "table::application_report_fingerprint",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0]
                    .application_report_fingerprint = u64::MAX;
            }),
        ),
        (
            "table::slots[1]::target",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                    Some(machine_id(1));
            }),
        ),
        (
            "call::operation",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].operation = operation_id(98);
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.dynamic_calls_mut_for_test()[0].text_offset = function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].byte_count += 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "table::application_commitment",
            Box::new(move |record| {
                record.dynamic_conformance_tables_mut_for_test()[0].application_commitment =
                    foreign_commitment;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "table::application_report_fingerprint::zero",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0]
                    .application_report_fingerprint = 0;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::data_offset",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].data_offset = 8;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::byte_count",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].byte_count = 8;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::row_index",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].row_index = 1;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::data_offset",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].data_offset = 8;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::target::unknown",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target =
                    Some(machine_id(98));
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[0]::target::retargeted",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target =
                    Some(machine_id(1));
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "table::slots[0]::target::erased",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[0].target = None;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "table::slots[1]::row_index",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].row_index = 0;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[1]::data_offset",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].data_offset = 0;
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "table::slots[1]::target::unknown",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                    Some(machine_id(98));
            }),
            InstallationError::InvalidDynamicConformanceTable,
        ),
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidDynamicCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidDynamicCall(machine_id(1)),
        ),
        (
            "call::application_commitment",
            Box::new(move |record| {
                record.dynamic_calls_mut_for_test()[0].application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::initial_source",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].initial_source =
                    PlaceId::new(98).expect("substituted source");
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::rebound_source",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].rebound_source =
                    PlaceId::new(98).expect("substituted source");
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::unresolved_row",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 8;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::misaligned",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 7;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::realization",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].realization = machine_id(1);
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.dynamic_calls_mut_for_test()[0].text_offset = function_text_offset - 1;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.dynamic_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidDynamicCall(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: reordering, dropping, or duplicating slots and
    // tables breaks canonical ordering, byte counts, or commitment closure
    // before any identity or replay could accept it.
    let mut swapped_slots = record.clone();
    swapped_slots.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .swap(0, 1);
    assert_eq!(
        encode_installation_record(&swapped_slots),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_slot = record.clone();
    dropped_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_slot = record.clone();
    let slot = duplicated_slot.dynamic_conformance_tables()[0].slots[1];
    duplicated_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .push(slot);
    assert_eq!(
        encode_installation_record(&duplicated_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_table = record.clone();
    dropped_table
        .dynamic_conformance_tables_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_table),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut duplicated_table = record.clone();
    let table = duplicated_table.dynamic_conformance_tables()[0].clone();
    duplicated_table
        .dynamic_conformance_tables_mut_for_test()
        .push(table);
    assert_eq!(
        encode_installation_record(&duplicated_table),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_call = record.clone();
    dropped_call.dynamic_calls_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_call),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.dynamic_calls()[0];
    duplicated_call.dynamic_calls_mut_for_test().push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidDynamicCall(machine_id(3)))
    );
}

#[test]
fn installation_dynamic_parameter_call_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&dynamic_parameter_call_plan())
        .expect("dynamic-parameter object artifact");
    let image = emit_executable_image(&artifact, 3).expect("dynamic-parameter image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("dynamic-parameter installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_parameter_calls().len(), 1);
    let authentic = record.dynamic_parameter_calls()[0];
    assert_eq!(authentic.machine, machine_id(2));
    assert_eq!(authentic.operation, operation_id(2));
    assert_eq!(authentic.source_value, None);
    assert_eq!(authentic.requirement_slot, 0);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(2))
        .expect("dynamic-parameter caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic.text_offset, function_text_offset);
    assert_eq!(authentic.byte_count, 8);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "call::operation",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].operation = operation_id(98);
            }),
        ),
        (
            "call::source_value",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].source_value =
                    semantic_vocabulary::ValueId::new(98);
            }),
        ),
        (
            "call::requirement_slot",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].requirement_slot = 1;
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].byte_count += 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(1)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.dynamic_parameter_calls_mut_for_test()[0].text_offset = function_text_end;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::byte_count::past_function",
            Box::new(|record| {
                record.dynamic_parameter_calls_mut_for_test()[0].byte_count += 10;
            }),
            InstallationError::InvalidDynamicParameterCall(machine_id(2)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the row still encodes, but its recomputed
    // identity diverges and independent replay rejects it; duplicating the row
    // collides on the canonical (machine, operation) call site.
    let mut dropped_call = record.clone();
    dropped_call.dynamic_parameter_calls_mut_for_test().pop();
    let dropped_bytes =
        encode_installation_record(&dropped_call).expect("dropped dynamic-parameter call encodes");
    let dropped =
        decode_installation_record(&dropped_bytes).expect("dropped dynamic-parameter call decodes");
    assert_ne!(
        installation_fingerprint(&dropped).expect("dropped fingerprint"),
        authentic_fingerprint,
        "dropped row recomputes a different installation identity"
    );
    assert_eq!(
        validate_installation_record(&dropped, &image),
        Err(InstallationError::ImageBindingMismatch),
        "independent replay rejects the dropped row"
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.dynamic_parameter_calls()[0];
    duplicated_call
        .dynamic_parameter_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidDynamicParameterCall(machine_id(
            2
        )))
    );
}

#[test]
fn installation_stored_dynamic_call_rejects_every_one_field_substitution() {
    let artifact =
        build_object_artifact(&stored_dynamic_call_plan()).expect("stored dynamic object artifact");
    assert_eq!(artifact.dynamic_conformance_tables().len(), 1);
    assert_eq!(artifact.dynamic_conformance_tables()[0].slots.len(), 2);
    let image = emit_executable_image(&artifact, 3).expect("stored dynamic image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("stored dynamic installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_conformance_tables().len(), 1);
    assert_eq!(record.stored_dynamic_calls().len(), 1);
    let authentic = record.stored_dynamic_calls()[0];
    assert_eq!(authentic.machine, machine_id(3));
    assert_eq!(authentic.establishment_operation, operation_id(4));
    assert_eq!(authentic.operation, operation_id(5));
    assert_eq!(authentic.descriptor_ordinal, 0);
    assert_eq!(authentic.selection_ordinal, 0);
    assert_eq!(authentic.source, PlaceId::new(1).expect("place"));
    assert_eq!(authentic.descriptor_home_byte_offset, 0);
    assert_eq!(authentic.selected_table_byte_offset, 0);
    assert_eq!(authentic.realization, machine_id(2));
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("stored dynamic caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic.establishment_text_offset, function_text_offset);
    assert_eq!(authentic.establishment_byte_count, 25);
    assert_eq!(authentic.text_offset, function_text_offset + 25);
    assert_eq!(authentic.byte_count, 32);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "table::application_report_fingerprint",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0]
                    .application_report_fingerprint = u64::MAX;
            }),
        ),
        (
            "table::slots[1]::target",
            Box::new(|record| {
                record.dynamic_conformance_tables_mut_for_test()[0].slots[1].target =
                    Some(machine_id(1));
            }),
        ),
        (
            "call::operation",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].operation = operation_id(98);
            }),
        ),
        (
            "call::establishment_operation",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_operation =
                    operation_id(98);
            }),
        ),
        (
            "call::descriptor_ordinal",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].descriptor_ordinal = 1;
            }),
        ),
        (
            "call::selection_ordinal",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selection_ordinal = 1;
            }),
        ),
        (
            "call::descriptor_home_byte_offset",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].descriptor_home_byte_offset = 8;
            }),
        ),
        (
            "call::establishment_byte_count",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count -= 1;
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 26;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].byte_count -= 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(1)),
        ),
        (
            "call::source::unknown_place",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].source =
                    PlaceId::new(98).expect("place");
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::application_commitment",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].application_commitment =
                    foreign_commitment;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::unresolved_row",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 8;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::misaligned",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 7;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::selected_table_byte_offset::past_rows",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].selected_table_byte_offset = 16;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::realization::not_selected",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].realization = machine_id(1);
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::descriptor_home_byte_offset::misaligned",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].descriptor_home_byte_offset = 4;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_text_offset::before_function",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_text_offset::inside",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_text_offset =
                    function_text_offset + 1;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_byte_count::empty",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count = 0;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::establishment_byte_count::overlaps_call",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].establishment_byte_count += 1;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::text_offset::inside_establishment",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 24;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.stored_dynamic_calls_mut_for_test()[0].text_offset = function_text_end;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
        (
            "call::byte_count::past_function",
            Box::new(|record| {
                record.stored_dynamic_calls_mut_for_test()[0].byte_count += 100;
            }),
            InstallationError::InvalidStoredDynamicCall(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the only stored call leaves the emitted
    // conformance table unreferenced, a duplicated call collides on the
    // canonical establishment/dispatch ordering, and table roster mutations
    // hit the data-layout and commitment canonicality joins.
    let mut dropped_call = record.clone();
    dropped_call.stored_dynamic_calls_mut_for_test().pop();
    assert_eq!(
        encode_installation_record(&dropped_call),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.stored_dynamic_calls()[0];
    duplicated_call
        .stored_dynamic_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidStoredDynamicCall(machine_id(3)))
    );
    let mut dropped_table = record.clone();
    dropped_table
        .dynamic_conformance_tables_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_table),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut duplicated_table = record.clone();
    let table = duplicated_table.dynamic_conformance_tables()[0].clone();
    duplicated_table
        .dynamic_conformance_tables_mut_for_test()
        .push(table);
    assert_eq!(
        encode_installation_record(&duplicated_table),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut swapped_slots = record.clone();
    swapped_slots.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .swap(0, 1);
    assert_eq!(
        encode_installation_record(&swapped_slots),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut dropped_slot = record.clone();
    dropped_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
    let mut duplicated_slot = record.clone();
    let slot = duplicated_slot.dynamic_conformance_tables()[0].slots[0];
    duplicated_slot.dynamic_conformance_tables_mut_for_test()[0]
        .slots
        .push(slot);
    assert_eq!(
        encode_installation_record(&duplicated_slot),
        Err(InstallationError::InvalidDynamicConformanceTable)
    );
}

#[test]
fn installation_forwarded_dynamic_parameter_call_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&forwarded_dynamic_parameter_call_plan())
        .expect("forwarded dynamic-parameter object artifact");
    let image = emit_executable_image(&artifact, 3).expect("forwarded dynamic-parameter image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("forwarded dynamic-parameter installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.dynamic_parameter_calls().len(), 1);
    assert_eq!(record.forwarded_dynamic_parameter_calls().len(), 1);
    let authentic = record.forwarded_dynamic_parameter_calls()[0];
    assert_eq!(authentic.machine, machine_id(2));
    assert_eq!(authentic.operation, operation_id(2));
    assert_eq!(authentic.callee, machine_id(1));
    assert_eq!(
        authentic.source_value,
        Some(semantic_vocabulary::ValueId::new(91).expect("forwarded result"))
    );
    assert_eq!(
        authentic.scalar_type,
        Some(semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                .expect("i32 scalar type")
        ))
    );
    assert_eq!(authentic.source_parameter_ordinal, 0);
    assert_eq!(authentic.target_parameter_ordinal, 0);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(2))
        .expect("forwarded dynamic-parameter caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic.text_offset, function_text_offset);
    assert_eq!(authentic.byte_count, 7);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "call::operation",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].operation =
                    operation_id(98);
            }),
        ),
        (
            "call::source_value",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].source_value =
                    semantic_vocabulary::ValueId::new(98);
            }),
        ),
        (
            "call::scalar_type",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type =
                    Some(semantic_vocabulary::ScalarType::Boolean);
            }),
        ),
        (
            "call::callee::self_function",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].callee = machine_id(2);
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].byte_count += 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(1)),
        ),
        (
            "call::callee::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].callee = machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::source_parameter_ordinal",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0]
                    .source_parameter_ordinal = 1;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::target_parameter_ordinal",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0]
                    .target_parameter_ordinal = 1;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::source_value::cleared",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].source_value = None;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::scalar_type::cleared",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type = None;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::scalar_type::float",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].scalar_type =
                    Some(semantic_vocabulary::ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    ));
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].text_offset =
                    function_text_end;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.forwarded_dynamic_parameter_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidForwardedDynamicParameterCall(machine_id(2)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the row still encodes, but its recomputed
    // identity diverges and independent replay rejects it; duplicating the row
    // collides on the canonical (machine, operation) call site.
    let mut dropped_call = record.clone();
    dropped_call
        .forwarded_dynamic_parameter_calls_mut_for_test()
        .pop();
    let dropped_bytes = encode_installation_record(&dropped_call)
        .expect("dropped forwarded dynamic-parameter call encodes");
    let dropped = decode_installation_record(&dropped_bytes)
        .expect("dropped forwarded dynamic-parameter call decodes");
    assert_ne!(
        installation_fingerprint(&dropped).expect("dropped fingerprint"),
        authentic_fingerprint,
        "dropped row recomputes a different installation identity"
    );
    assert_eq!(
        validate_installation_record(&dropped, &image),
        Err(InstallationError::ImageBindingMismatch),
        "independent replay rejects the dropped row"
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.forwarded_dynamic_parameter_calls()[0];
    duplicated_call
        .forwarded_dynamic_parameter_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidForwardedDynamicParameterCall(
            machine_id(2)
        ))
    );
}

#[test]
fn installation_forwarded_dynamic_descriptor_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact(&forwarded_dynamic_descriptor_call_plan())
        .expect("forwarded dynamic-descriptor object artifact");
    let image = emit_executable_image(&artifact, 3).expect("forwarded dynamic-descriptor image");
    let record = build_installation_record(&image, ProfileDecisionId::new(1).expect("profile"))
        .expect("forwarded dynamic-descriptor installation");
    validate_installation_record(&record, &image).expect("authentic binding");
    assert_eq!(record.forwarded_dynamic_descriptor_adapters().len(), 1);
    assert_eq!(record.forwarded_dynamic_descriptor_tables().len(), 1);
    assert_eq!(record.forwarded_dynamic_descriptor_calls().len(), 1);
    let authentic_adapter = record.forwarded_dynamic_descriptor_adapters()[0];
    let authentic_table = &record.forwarded_dynamic_descriptor_tables()[0];
    let authentic_call = &record.forwarded_dynamic_descriptor_calls()[0];
    assert_eq!(authentic_adapter.row_index, 0);
    assert_eq!(authentic_adapter.realization, machine_id(2));
    assert_eq!(authentic_adapter.byte_count, 17);
    assert_eq!(authentic_table.data_offset, 0);
    assert_eq!(authentic_table.byte_count, 8);
    assert_eq!(authentic_table.slots.len(), 1);
    assert_eq!(authentic_table.slots[0].row_index, 0);
    assert_eq!(authentic_table.slots[0].realization, machine_id(2));
    assert_eq!(
        authentic_table.slots[0].adapter_text_offset,
        authentic_adapter.text_offset
    );
    assert_eq!(authentic_table.slots[0].data_offset, 0);
    assert_eq!(authentic_call.machine, machine_id(3));
    assert_eq!(authentic_call.operation, operation_id(4));
    assert_eq!(authentic_call.callee, machine_id(1));
    assert_eq!(
        authentic_call.application_commitment,
        authentic_table.application_commitment
    );
    assert_eq!(
        authentic_call.source,
        StructuralArgument {
            place: PlaceId::new(1).expect("place"),
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        }
    );
    assert_eq!(authentic_call.semantic_result, None);
    assert_eq!(authentic_call.result, None);
    let authentic_fingerprint =
        installation_fingerprint(&record).expect("authentic installation fingerprint");

    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("forwarded dynamic-descriptor caller row");
    let function_text_offset = caller.text_offset;
    let function_text_end = function_text_offset + caller.byte_count;
    assert_eq!(authentic_call.text_offset, function_text_offset);
    assert_eq!(authentic_call.byte_count, 24);
    assert!(authentic_adapter.text_offset >= function_text_end);

    // These one-field substitutions remain representable: they encode and
    // decode canonically, recompute to a different installation identity, and
    // independent replay against the unchanged image rejects them.
    type StillEncodedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
    );
    let still_encodes: Vec<StillEncodedMutation> = vec![
        (
            "table::application_report_fingerprint",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
                    .application_report_fingerprint = u64::MAX;
            }),
        ),
        (
            "call::operation",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].operation =
                    operation_id(98);
            }),
        ),
        (
            "call::callee::other_function",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].callee = machine_id(2);
            }),
        ),
        (
            "call::source::place",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                    .source
                    .place = PlaceId::new(98).expect("place");
            }),
        ),
        (
            "call::source::access",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                    .source
                    .access = StructuralAccess::Owned;
            }),
        ),
        (
            "call::text_offset",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                    function_text_offset + 1;
            }),
        ),
        (
            "call::byte_count",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count -= 1;
            }),
        ),
    ];
    for (field, mutate) in still_encodes {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted record encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted record decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted record"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted record"
        );
    }

    // Canonical record-shape joins reject every other one-field substitution
    // at encoding, before any identity or replay could accept it.
    let foreign_commitment =
        terminal_psi::ClosedConformanceApplicationCommitment::from_digest([7; 32]);
    type RejectedMutation = (
        &'static str,
        Box<dyn Fn(&mut image_emission::InstallationRecord)>,
        InstallationError,
    );
    let rejected: Vec<RejectedMutation> = vec![
        (
            "adapter::application_commitment",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0]
                    .application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::row_index",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].row_index = 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::realization",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].realization =
                    machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::text_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].text_offset += 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "adapter::byte_count::empty",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorAdapter,
        ),
        (
            "adapter::byte_count::past_text_end",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_adapters_mut_for_test()[0].byte_count += 1;
            }),
            InstallationError::InvalidImageSectionLayout,
        ),
        (
            "table::application_commitment",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
                    .application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::data_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].data_offset = 8;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::byte_count",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].byte_count = 16;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::row_index",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].row_index = 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::realization::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].realization =
                    machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::realization::not_adapter",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].realization =
                    machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::adapter_text_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0]
                    .adapter_text_offset += 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "table::slots[0]::data_offset",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_tables_mut_for_test()[0].slots[0].data_offset =
                    8;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorTable,
        ),
        (
            "call::machine::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].machine =
                    machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(98)),
        ),
        (
            "call::machine::other_function",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].machine = machine_id(1);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(1)),
        ),
        (
            "call::callee::unknown",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].callee = machine_id(98);
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::application_commitment",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0]
                    .application_commitment = foreign_commitment;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::semantic_result::without_result",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].semantic_result =
                    Some(abstract_operations::AbstractResult {
                        value: semantic_vocabulary::ValueId::new(98).expect("value"),
                        scalar_type: semantic_vocabulary::ScalarType::Boolean,
                    });
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::text_offset::before_function",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                    function_text_offset - 1;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::text_offset::past_function",
            Box::new(move |record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].text_offset =
                    function_text_end;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::byte_count::empty",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count = 0;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
        (
            "call::byte_count::past_function",
            Box::new(|record| {
                record.forwarded_dynamic_descriptor_calls_mut_for_test()[0].byte_count += 100;
            }),
            InstallationError::InvalidForwardedDynamicDescriptorCall(machine_id(3)),
        ),
    ];
    for (field, mutate, expected) in rejected {
        let mut changed = record.clone();
        mutate(&mut changed);
        assert_ne!(changed, record, "{field}: substitution changes the record");
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: canonical encoding rejects the substitution"
        );
    }

    // Roster-level custody: dropping the only call leaves the forwarded table
    // unreferenced, a duplicated call collides on the canonical (machine,
    // operation) call site, and adapter/table/slot roster mutations each hit
    // their canonicality joins.
    let mut dropped_call = record.clone();
    dropped_call
        .forwarded_dynamic_descriptor_calls_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_call),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut duplicated_call = record.clone();
    let call = duplicated_call.forwarded_dynamic_descriptor_calls()[0].clone();
    duplicated_call
        .forwarded_dynamic_descriptor_calls_mut_for_test()
        .push(call);
    assert_eq!(
        encode_installation_record(&duplicated_call),
        Err(InstallationError::InvalidForwardedDynamicDescriptorCall(
            machine_id(3)
        ))
    );
    let mut dropped_adapter = record.clone();
    dropped_adapter
        .forwarded_dynamic_descriptor_adapters_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_adapter),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut duplicated_adapter = record.clone();
    let adapter = duplicated_adapter.forwarded_dynamic_descriptor_adapters()[0];
    duplicated_adapter
        .forwarded_dynamic_descriptor_adapters_mut_for_test()
        .push(adapter);
    assert_eq!(
        encode_installation_record(&duplicated_adapter),
        Err(InstallationError::InvalidForwardedDynamicDescriptorAdapter)
    );
    let mut dropped_table = record.clone();
    dropped_table
        .forwarded_dynamic_descriptor_tables_mut_for_test()
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_table),
        Err(InstallationError::InvalidImageSectionLayout)
    );
    let mut duplicated_table = record.clone();
    let table = duplicated_table.forwarded_dynamic_descriptor_tables()[0].clone();
    duplicated_table
        .forwarded_dynamic_descriptor_tables_mut_for_test()
        .push(table);
    assert_eq!(
        encode_installation_record(&duplicated_table),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut dropped_slot = record.clone();
    dropped_slot.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
        .slots
        .pop();
    assert_eq!(
        encode_installation_record(&dropped_slot),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
    let mut duplicated_slot = record.clone();
    let slot = duplicated_slot.forwarded_dynamic_descriptor_tables()[0].slots[0];
    duplicated_slot.forwarded_dynamic_descriptor_tables_mut_for_test()[0]
        .slots
        .push(slot);
    assert_eq!(
        encode_installation_record(&duplicated_slot),
        Err(InstallationError::InvalidForwardedDynamicDescriptorTable)
    );
}

/// Every representable field of an installed privileged port-effect row is an
/// authenticated custody axis: a one-field substitution either cannot encode
/// canonically or still encodes, recomputes a distinct installation
/// fingerprint, and independent replay against the unchanged image rejects
/// it. The fixture retains one unbound effect and one effect consumed by an
/// admitted-provider `MetadataOnlyPort` settlement, so both the free semantic
/// axes and the axes pinned by the settlement join are exercised.
#[test]
fn installation_port_effect_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(7);
    let plan = port_effect_plan(&provider);
    let artifact = build_object_artifact(&plan).expect("port-effect artifact");
    let image = emit_executable_image(&artifact, 3).expect("port-effect image");
    let record = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(17).expect("profile"),
        [&provider],
    )
    .expect("port-effect installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let authentic_fingerprint = installation_fingerprint(&record).expect("fingerprint");
    let [unbound, bound] = record.port_effects() else {
        panic!("port-effect fixture retains two rows");
    };
    assert_eq!(unbound.effect.psi_operation, operation_id(1));
    assert_eq!(bound.effect.psi_operation, operation_id(2));

    // The unbound row's semantic axes are independently representable: each
    // one-field substitution still encodes canonically, recomputes a distinct
    // installation fingerprint, and independent replay against the unchanged
    // image rejects it.
    let representable: [(&str, fn(&mut image_emission::ObjectPortEffect)); 5] = [
        ("psi_operation", |row| {
            row.effect.psi_operation = operation_id(96);
        }),
        ("service", |row| {
            row.effect.service = ServiceId::new(96).expect("drifted service");
        }),
        ("port", |row| {
            row.effect.port = 0x64;
        }),
        ("value", |row| {
            row.effect.value = 0x7f;
        }),
        ("operation_ordinal", |row| {
            row.effect.operation_ordinal = 9;
        }),
    ];
    for (field, mutate) in representable {
        let mut changed = record.clone();
        mutate(&mut changed.port_effects_mut_for_test()[0]);
        assert_ne!(changed, record, "{field}: substitution changes the row");
        let bytes = encode_installation_record(&changed)
            .unwrap_or_else(|error| panic!("{field}: substituted row encodes: {error:?}"));
        let replayed = decode_installation_record(&bytes)
            .unwrap_or_else(|error| panic!("{field}: substituted row decodes: {error:?}"));
        assert_eq!(
            replayed, changed,
            "{field}: codec preserves the substituted row"
        );
        assert_ne!(
            installation_fingerprint(&replayed)
                .unwrap_or_else(|error| panic!("{field}: substituted fingerprint: {error:?}")),
            authentic_fingerprint,
            "{field}: recomputed identity differs from the authentic record"
        );
        assert_eq!(
            validate_installation_record(&replayed, &image),
            Err(InstallationError::ImageBindingMismatch),
            "{field}: independent replay rejects the substituted row"
        );
    }

    // Physical axes are canonical projections: the row must name an installed
    // machine, sit at `function.text_offset + code_offset`, and span exactly
    // the emitted `out` sequence. Drifting any of them fails at encoding.
    let unbound_encode_rejected: [(
        &str,
        fn(&mut image_emission::ObjectPortEffect),
        InstallationError,
    ); 4] = [
        ("machine", |row| row.machine = machine_id(96), {
            InstallationError::EffectMachineMissing(machine_id(96))
        }),
        ("code_offset", |row| row.effect.code_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(1),
            }
        }),
        ("byte_count", |row| row.effect.byte_count -= 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(1),
            }
        }),
        ("text_offset", |row| row.text_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(1),
            }
        }),
    ];
    for (field, mutate, expected) in unbound_encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed.port_effects_mut_for_test()[0]);
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: non-canonical substitution rejected at encoding"
        );
    }

    // Every field of the settlement-consumed row is pinned: the physical axes
    // fail the same canonical projection joins, while the semantic axes fail
    // the `MetadataOnlyPort` realization's exact effect lookup.
    let bound_encode_rejected: [(
        &str,
        fn(&mut image_emission::ObjectPortEffect),
        InstallationError,
    ); 9] = [
        ("machine", |row| row.machine = machine_id(96), {
            InstallationError::EffectMachineMissing(machine_id(96))
        }),
        (
            "psi_operation",
            |row| row.effect.psi_operation = operation_id(96),
            {
                InstallationError::BoundaryRealizationMismatch {
                    machine: machine_id(1),
                    operation: operation_id(3),
                }
            },
        ),
        (
            "service",
            |row| row.effect.service = ServiceId::new(96).unwrap(),
            {
                InstallationError::BoundaryRealizationMismatch {
                    machine: machine_id(1),
                    operation: operation_id(3),
                }
            },
        ),
        ("port", |row| row.effect.port = 0x64, {
            InstallationError::BoundaryRealizationMismatch {
                machine: machine_id(1),
                operation: operation_id(3),
            }
        }),
        ("value", |row| row.effect.value = 0x7f, {
            InstallationError::BoundaryRealizationMismatch {
                machine: machine_id(1),
                operation: operation_id(3),
            }
        }),
        (
            "operation_ordinal",
            |row| row.effect.operation_ordinal = 9,
            {
                InstallationError::BoundaryRealizationMismatch {
                    machine: machine_id(1),
                    operation: operation_id(3),
                }
            },
        ),
        ("code_offset", |row| row.effect.code_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(2),
            }
        }),
        ("byte_count", |row| row.effect.byte_count -= 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(2),
            }
        }),
        ("text_offset", |row| row.text_offset += 1, {
            InstallationError::InvalidPortEffectOffset {
                machine: machine_id(1),
                operation: operation_id(2),
            }
        }),
    ];
    for (field, mutate, expected) in bound_encode_rejected {
        let mut changed = record.clone();
        mutate(&mut changed.port_effects_mut_for_test()[1]);
        assert_eq!(
            encode_installation_record(&changed),
            Err(expected),
            "{field}: non-canonical substitution rejected at encoding"
        );
    }

    // Dropping the unbound row still encodes: replay then rejects it because
    // the retained roster no longer matches the image. Dropping the consumed
    // row instead orphans the settlement join and fails at encoding, as do a
    // duplicated `(machine, psi_operation)` pair and a non-canonical order.
    let mut dropped_unbound = record.clone();
    dropped_unbound.port_effects_mut_for_test().remove(0);
    let bytes =
        encode_installation_record(&dropped_unbound).expect("dropped unbound effect still encodes");
    let replayed = decode_installation_record(&bytes).expect("dropped unbound effect decodes");
    assert_ne!(
        installation_fingerprint(&replayed).expect("substituted fingerprint"),
        authentic_fingerprint
    );
    assert_eq!(
        validate_installation_record(&replayed, &image),
        Err(InstallationError::ImageBindingMismatch),
        "dropped unbound row: independent replay rejects the roster"
    );
    let mut dropped_bound = record.clone();
    dropped_bound.port_effects_mut_for_test().remove(1);
    assert_eq!(
        encode_installation_record(&dropped_bound),
        Err(InstallationError::BoundaryRealizationMismatch {
            machine: machine_id(1),
            operation: operation_id(3),
        })
    );
    let mut duplicated_operation = record.clone();
    let mut duplicate = duplicated_operation.port_effects()[1].clone();
    duplicate.effect.psi_operation = operation_id(1);
    duplicate.effect.operation_ordinal = 5;
    duplicated_operation
        .port_effects_mut_for_test()
        .push(duplicate);
    assert_eq!(
        encode_installation_record(&duplicated_operation),
        Err(InstallationError::DuplicatePortEffectOperation {
            machine: machine_id(1),
            operation: operation_id(1),
        })
    );
    let mut reordered = record.clone();
    reordered.port_effects_mut_for_test().swap(0, 1);
    assert_eq!(
        encode_installation_record(&reordered),
        Err(InstallationError::NonCanonicalPortEffectOrder)
    );
}
