//! Canonical-encoding custody drivers for the installation record's joined
//! rows: every inventory declared in `installation_field_substitution_fields`
//! runs through the shared `run_one_field_substitution_matrix` driver.

use super::installation_field_substitution_fields::{
    CompilerPrivateFunctionFieldForTest, DynamicConformanceTableFieldForTest,
    DynamicParameterCallFieldForTest, ForwardedDynamicDescriptorFieldForTest,
    ForwardedDynamicParameterCallFieldForTest, PortEffectFieldForTest,
    SemanticCodeAttributionFieldForTest, StoredDynamicCallFieldForTest,
    compiler_private_function_outcome, dynamic_conformance_table_outcome,
    dynamic_parameter_call_outcome, foreign_forwarded_parameter_call_record,
    foreign_stored_dynamic_call_record, forwarded_dynamic_descriptor_outcome,
    forwarded_dynamic_parameter_call_outcome, honest_compiler_private_function_record,
    honest_dynamic_conformance_table_record, honest_dynamic_parameter_call_record,
    honest_forwarded_dynamic_descriptor_record, honest_forwarded_dynamic_parameter_call_record,
    honest_port_effect_record, honest_semantic_code_attribution_record,
    honest_stored_dynamic_call_record, installation_record_check, installation_record_custody,
    port_effect_outcome, semantic_code_attribution_outcome, stored_dynamic_call_outcome,
    substitute_compiler_private_function, substitute_dynamic_conformance_table,
    substitute_dynamic_parameter_call, substitute_forwarded_dynamic_descriptor,
    substitute_forwarded_dynamic_parameter_call, substitute_port_effect,
    substitute_semantic_code_attribution, substitute_stored_dynamic_call,
};
use super::{
    WriteExitProvider, callback_private_plan, dynamic_conformance_table_plan,
    dynamic_parameter_call_plan, edge_id, forwarded_dynamic_descriptor_call_plan,
    forwarded_dynamic_parameter_call_plan, identity, linux_write_line_exit_plan, machine_id,
    operation_id, port_effect_plan, stored_dynamic_call_plan,
};
use image_emission::{
    build_installation_record, build_installation_record_with_provider_executions,
    build_object_artifact, build_object_artifact_with_private_functions, emit_executable_image,
    validate_installation_record,
};
use machine_code::{SemanticCodeAttribution, SemanticCodeSite};
use optimization_core::{OneFieldSubstitutionMatrix, run_one_field_substitution_matrix};
use semantic_vocabulary::{PlaceId, ProfileDecisionId};
use terminal_psi::{StructuralAccess, StructuralArgument};

/// Every representable field of an installed semantic-code attribution row is
/// an authenticated custody axis covered by the shared matrix driver.
#[test]
fn installation_semantic_code_attribution_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(970);
    let artifact = build_object_artifact(&linux_write_line_exit_plan(&provider))
        .expect("attribution artifact");
    let image = emit_executable_image(&artifact, 3).expect("attribution image");
    let record = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(97).expect("profile"),
        [&provider],
    )
    .expect("attribution installation");
    validate_installation_record(&record, &image).expect("exact image binding");
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

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation semantic-code attribution",
        fields: SemanticCodeAttributionFieldForTest::INVENTORY,
        honest: &honest_semantic_code_attribution_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_semantic_code_attribution,
        check: &check,
        outcome: &semantic_code_attribution_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}

/// Every representable field of an installed compiler-private callback row is
/// an authenticated custody axis covered by the shared matrix driver.
#[test]
fn installation_private_function_row_rejects_every_one_field_substitution() {
    let artifact = build_object_artifact_with_private_functions(&callback_private_plan())
        .expect("callback private artifact");
    let image = emit_executable_image(&artifact, 3).expect("callback private image");
    let record = build_installation_record(&image, ProfileDecisionId::new(53).expect("profile"))
        .expect("callback private installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let [authentic] = record.private_functions() else {
        panic!("callback fixture retains one private row");
    };
    assert_eq!(authentic.machine, machine_id(97));
    assert_eq!(authentic.source_psi, identity());
    assert!(
        authentic
            .identity
            .callback_thunk_placement_index()
            .is_some()
    );

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation compiler-private function row",
        fields: CompilerPrivateFunctionFieldForTest::INVENTORY,
        honest: &honest_compiler_private_function_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_compiler_private_function,
        check: &check,
        outcome: &compiler_private_function_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}

/// Every representable field of an installed dynamic conformance table and
/// its joined call row is an authenticated custody axis covered by the shared
/// matrix driver.
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

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation dynamic conformance table",
        fields: DynamicConformanceTableFieldForTest::INVENTORY,
        honest: &honest_dynamic_conformance_table_record,
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_dynamic_conformance_table,
        check: &check,
        outcome: &dynamic_conformance_table_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}

/// Every representable field of an installed dynamic-parameter call row is an
/// authenticated custody axis covered by the shared matrix driver.
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
    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(2))
        .expect("dynamic-parameter caller row");
    assert_eq!(authentic.text_offset, caller.text_offset);
    assert_eq!(authentic.byte_count, 8);

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation dynamic-parameter call",
        fields: DynamicParameterCallFieldForTest::INVENTORY,
        honest: &honest_dynamic_parameter_call_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_dynamic_parameter_call,
        check: &check,
        outcome: &dynamic_parameter_call_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}

/// Every representable field of an installed stored dynamic call and its
/// joined conformance table is an authenticated custody axis covered by the
/// shared matrix driver.
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
    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("stored dynamic caller row");
    assert_eq!(authentic.establishment_text_offset, caller.text_offset);
    assert_eq!(authentic.establishment_byte_count, 25);
    assert_eq!(authentic.text_offset, caller.text_offset + 25);
    assert_eq!(authentic.byte_count, 32);

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation stored dynamic call",
        fields: StoredDynamicCallFieldForTest::INVENTORY,
        honest: &honest_stored_dynamic_call_record,
        donor: foreign_forwarded_parameter_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_stored_dynamic_call,
        check: &check,
        outcome: &stored_dynamic_call_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}

/// Every representable field of an installed forwarded dynamic-parameter call
/// row is an authenticated custody axis covered by the shared matrix driver.
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
    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(2))
        .expect("forwarded dynamic-parameter caller row");
    assert_eq!(authentic.text_offset, caller.text_offset);
    assert_eq!(authentic.byte_count, 7);

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation forwarded dynamic-parameter call",
        fields: ForwardedDynamicParameterCallFieldForTest::INVENTORY,
        honest: &honest_forwarded_dynamic_parameter_call_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_forwarded_dynamic_parameter_call,
        check: &check,
        outcome: &forwarded_dynamic_parameter_call_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}

/// Every representable field of an installed forwarded dynamic-descriptor
/// adapter, table, slot or call row is an authenticated custody axis covered
/// by the shared matrix driver.
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
    let caller = record
        .functions()
        .iter()
        .find(|function| function.machine == machine_id(3))
        .expect("forwarded dynamic-descriptor caller row");
    assert_eq!(authentic_call.text_offset, caller.text_offset);
    assert_eq!(authentic_call.byte_count, 24);
    assert!(authentic_adapter.text_offset >= caller.text_offset + caller.byte_count);

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation forwarded dynamic descriptor",
        fields: ForwardedDynamicDescriptorFieldForTest::INVENTORY,
        honest: &honest_forwarded_dynamic_descriptor_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_forwarded_dynamic_descriptor,
        check: &check,
        outcome: &forwarded_dynamic_descriptor_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}

/// Every representable field of an installed privileged port-effect row is an
/// authenticated custody axis covered by the shared matrix driver. The fixture
/// retains one unbound effect and one effect consumed by an admitted-provider
/// `MetadataOnlyPort` settlement, so both the free semantic axes and the axes
/// pinned by the settlement join are exercised.
#[test]
fn installation_port_effect_rejects_every_one_field_substitution() {
    let provider = WriteExitProvider(7);
    let artifact =
        build_object_artifact(&port_effect_plan(&provider)).expect("port-effect artifact");
    let image = emit_executable_image(&artifact, 3).expect("port-effect image");
    let record = build_installation_record_with_provider_executions(
        &image,
        ProfileDecisionId::new(17).expect("profile"),
        [&provider],
    )
    .expect("port-effect installation");
    validate_installation_record(&record, &image).expect("exact image binding");
    let [unbound, bound] = record.port_effects() else {
        panic!("port-effect fixture retains two rows");
    };
    assert_eq!(unbound.effect.psi_operation, operation_id(1));
    assert_eq!(bound.effect.psi_operation, operation_id(2));

    let check = installation_record_check(&image);
    let matrix = OneFieldSubstitutionMatrix {
        family: "installation port effect",
        fields: PortEffectFieldForTest::INVENTORY,
        honest: &honest_port_effect_record,
        donor: foreign_stored_dynamic_call_record(),
        custody: &installation_record_custody,
        substitute: &substitute_port_effect,
        check: &check,
        outcome: &port_effect_outcome,
        joined_replay: None,
    };
    run_one_field_substitution_matrix(&matrix);
}
