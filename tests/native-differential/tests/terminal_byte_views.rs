//! Native length observation starts from encoded, independently verified Terminal.
//! Source helper closure and byte reads/subslices are not admitted by this fixture.

use native_realization::{compiler_baseline_request_v1, optimize_artifact_sections};
use optimization_core::OptimizationSelections;
use proof_admission::AdmissionProfile;
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout, stage_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId,
    ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use terminal_psi::{
    Block, ByteSequenceCarrier, MachineContract, Operation, OperationKind, OperationResult,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};

#[cfg(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "aarch64"),
))]
#[path = "common/native_function.rs"]
mod native_function;

fn byte_view_length_module() -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(2).unwrap();
    let source = PlaceId::new(3).unwrap();
    let structural_type = StructuralTypeId::new(4).unwrap();
    let length = ValueId::new(5).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::ImmutableBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(6).unwrap(),
                scalar_type,
            }),
            structural_places: vec![StructuralPlaceDeclaration {
                id: source,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(7).unwrap(),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: length,
                        scalar_type,
                    }),
                    kind: OperationKind::ByteSequenceLength { source },
                }],
                terminator: Terminator::Return {
                    edge: EdgeId::new(8).unwrap(),
                    value: length,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(9).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn stage_byte_view_length(target: NativeTarget) -> StagedOptimizedResolvedSelectedFormLayout {
    let semantic = terminal_codec::encode_module(&byte_view_length_module()).unwrap();
    let proof =
        terminal_codec::encode_proof_bundle(&terminal_verifier::ProofBundle::default()).unwrap();
    let selections = OptimizationSelections::new([]).unwrap();
    // This public boundary decodes and verifies the artifact before projection.
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        compiler_baseline_request_v1(&selections),
    )
    .expect("verified byte length reaches the ordinary optimizer input");
    let target = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized, target,
    )
    .expect("byte length reaches target operations");
    let environment =
        register_environment::baseline_target_register_environment(target.target()).unwrap();
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            target,
            environment,
        )
        .expect("byte descriptor length selects native instructions");
    let liveness =
        selected_instructions_to_register_homes::stage_optimized_liveness(selected).unwrap();
    let ranges =
        selected_instructions_to_register_homes::stage_optimized_live_ranges(liveness).unwrap();
    let legality =
        selected_instructions_to_register_homes::stage_optimized_allocation_legality(ranges)
            .unwrap();
    let homes =
        selected_instructions_to_register_homes::stage_optimized_register_homes(legality).unwrap();
    let machine =
        register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(
            &homes,
        )
        .unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let encoding = post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(), &machine, physical, None,
    ).unwrap();
    let layout = stage_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        physical,
        &encoding,
    )
    .unwrap();
    validate_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        physical,
        &encoding,
        &layout,
    )
    .expect("byte length's retained machine bytes independently replay");
    layout
}

#[test]
fn byte_view_length_cross_lowers_on_hosted_targets() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let layout = stage_byte_view_length(target);
        assert_eq!(layout.functions().len(), 1);
        assert!(layout.functions()[0].byte_count > 0);
    }
}

#[test]
fn byte_view_length_executes_with_dynamic_native_descriptors() {
    #[cfg(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    ))]
    native_function::assert_c_driver(
        &stage_byte_view_length(NativeTarget::host()),
        r#"
        #include <stdint.h>
        #include <stddef.h>
        struct ByteView { const uint8_t *bytes; uint64_t length; };
        _Static_assert(sizeof(void *) == 8, "parameter is one eight-byte pointer");
        _Static_assert(sizeof(struct ByteView) == 16, "referent is sixteen bytes");
        _Static_assert(_Alignof(struct ByteView) == 8, "referent alignment is eight");
        _Static_assert(offsetof(struct ByteView, length) == 8, "length lives at offset eight");
        extern uint64_t omega_entry(const struct ByteView *);
        int main(void) {
            const uint8_t text[] = { 'O', 'm', 'e', 'g', 'a' };
            const uint8_t raw[] = { 0xff, 0x00, 0x80, 0xfe };
            uint8_t longer[257] = { 0xff };
            struct ByteView view = { NULL, 0 };
            if (omega_entry(&view) != 0) return 1;
            view.bytes = text; view.length = sizeof(text);
            if (omega_entry(&view) != 5) return 2;
            view.bytes = raw; view.length = sizeof(raw);
            if (omega_entry(&view) != 4) return 3;
            view.bytes = longer; view.length = sizeof(longer);
            if (omega_entry(&view) != 257) return 4;
            view.length = 0;
            if (omega_entry(&view) != 0) return 5;
            return 0;
        }
    "#,
    );
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    eprintln!(
        "SKIP byte-view native execution: the existing cc/assembly harness supports Linux x86-64/AArch64 and macOS AArch64; Windows requires a separate host execution route"
    );
}

#[test]
fn byte_view_length_rejects_an_unavailable_descriptor_source() {
    let mut module = byte_view_length_module();
    module.machines[0].blocks[0].operations[0].kind = OperationKind::ByteSequenceLength {
        source: PlaceId::new(99).unwrap(),
    };
    assert_eq!(
        terminal_codec::encode_module(&module),
        Err(terminal_codec::CodecError::InvalidModule(
            terminal_verifier::ModuleError::InvalidByteSequenceLengthSource {
                operation: OperationId::new(7).unwrap(),
                source: PlaceId::new(99).unwrap(),
            },
        )),
        "a missing descriptor cannot enter a canonical artifact",
    );
}
