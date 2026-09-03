use psi_core::{
    BlockId, CanonicalStructuralPathSegment, ContractId, EdgeId, IeeeFloatFormat,
    IeeeFloatStructuralField, MachineId, OperationId, PlaceId, PsiSemanticId, StructuralFieldId,
    ValueId,
};
use psi_terminal::{
    Block, DirectBlockFloatParameter, DirectCallFloatResult, DirectMachineFloatResult,
    DirectOperationFloatResult, DirectStructuralFloatLeaf, FloatMeaningProjection,
    FloatMeaningProjectionOperation, FloatMeaningSource, MachineContract, ProofOnlyValueType,
    ProofValueDeclaration, ProofValueId, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, VocabularyMarker,
};

use super::{decode_module, encode_module};
use crate::module_wire::encode_legacy_result_path_raw;

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).expect("test ids are nonzero")
}

fn unit_module() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id::<MachineId>(1),
        structural_types: Vec::new(),
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
            id: id::<MachineId>(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id::<BlockId>(1),
            blocks: vec![Block {
                id: id::<BlockId>(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: id::<EdgeId>(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: id::<ContractId>(1),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
                crash_routes: Vec::new(),
            },
        }],
    }
}

#[test]
fn v56_v59_reconstructs_absent_result_path_rosters_as_current_empty_rows() {
    let module = unit_module();
    let legacy = encode_legacy_result_path_raw(&module).expect("legacy compatibility bytes");
    assert_eq!(&legacy[8..10], &56_u16.to_le_bytes());
    assert_eq!(&legacy[10..12], &59_u16.to_le_bytes());
    assert_eq!(decode_module(&legacy), Ok(module.clone()));

    let current = encode_module(&module).expect("current result-path bytes");
    assert_eq!(&current[8..10], &76_u16.to_le_bytes());
    assert_eq!(&current[10..12], &79_u16.to_le_bytes());

    let mut crossed_pair = legacy;
    crossed_pair[10..12].copy_from_slice(&72_u16.to_le_bytes());
    assert!(decode_module(&crossed_pair).is_err());
}

#[test]
fn v56_v59_rejects_current_only_direct_float_sources() {
    let operation = psi_numerics::float_projection::FloatProjectionOperation::Meaning32;
    let contract = operation.contract_identity();
    let contract = psi_terminal::FloatProjectionContractIdentity {
        format: contract.format,
        operation: contract.operation,
        declaration: contract.declaration,
        catalog_version: contract.catalog_version,
        commitment: contract.commitment,
    };
    let mut module = unit_module();
    module.float_meaning_projections = vec![FloatMeaningProjection {
        result: ProofValueDeclaration {
            id: ProofValueId(0),
            value_type: ProofOnlyValueType::FloatMeaning,
        },
        source: FloatMeaningSource::DirectMachineResult(DirectMachineFloatResult {
            owner: id::<MachineId>(1),
            result: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        }),
        operation: FloatMeaningProjectionOperation::Meaning32,
        contract,
    }];
    let mut legacy = encode_legacy_result_path_raw(&module).expect("legacy direct result bytes");
    let source_prefix = [0, 0, 0, 0, 1, 5];
    let source_offset = legacy
        .windows(source_prefix.len())
        .position(|window| window == source_prefix)
        .expect("legacy direct result source is unique");
    legacy[source_offset + 5] = 6;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 6))
    );
    legacy[source_offset + 5] = 7;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 7))
    );
    legacy[source_offset + 5] = 8;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 8))
    );
    legacy[source_offset + 5] = 9;
    assert_eq!(
        decode_module(&legacy),
        Err(super::CodecError::InvalidTag("FloatMeaningSource", 9))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectOperationResult(DirectOperationFloatResult {
            owner: id::<MachineId>(1),
            producer: id::<OperationId>(1),
            result: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            6
        ))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectBlockParameter(DirectBlockFloatParameter {
            owner: id::<MachineId>(1),
            block: id::<BlockId>(1),
            parameter: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            7
        ))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectCallResult(DirectCallFloatResult {
            owner: id::<MachineId>(1),
            producer: id::<OperationId>(1),
            result: id::<ValueId>(1),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            8
        ))
    );

    module.float_meaning_projections[0].source =
        FloatMeaningSource::DirectStructuralLeaf(DirectStructuralFloatLeaf {
            owner: id::<MachineId>(1),
            field: IeeeFloatStructuralField::new(
                id::<PlaceId>(1),
                vec![CanonicalStructuralPathSegment::Field(
                    id::<StructuralFieldId>(1),
                )],
            )
            .expect("nonempty structural path"),
            format: IeeeFloatFormat::Binary32,
        });
    assert_eq!(
        encode_legacy_result_path_raw(&module),
        Err(super::CodecError::InvalidTag(
            "legacy FloatMeaningSource",
            9
        ))
    );
}
