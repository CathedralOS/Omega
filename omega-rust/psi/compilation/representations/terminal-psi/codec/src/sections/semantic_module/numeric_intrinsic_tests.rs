use super::module_wire::encode_raw;
use super::target_family_tests::{identity_table, unit_machine};
use crate::{decode_module, encode_module, semantic_fingerprint};
use language_semantics::numeric_intrinsics::{NumericIntrinsic, NumericResult, NumericType};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, QualifiedScalarType, ScalarQualificationSetId, ScalarType,
};
use terminal_psi::*;

pub(super) fn module(meaning: NumericIntrinsic) -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine);
    module.machines.push(unit_machine(machine));
    module.identities = identity_table(3);
    let signature = meaning.signature().unwrap();
    let result = match signature.result {
        NumericResult::Primitive(primitive) => primitive.scalar_type().unwrap(),
        NumericResult::Boolean => ScalarType::Boolean,
        NumericResult::FloatClass => panic!("this helper creates scalar boundaries"),
    };
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(1).unwrap(),
        numeric_intrinsic: Some(meaning),
        external_providers: Vec::new(),
        declaration: None,
        role: BoundaryDeclarationRole::Callable,
        kind: BoundaryRequirementKind::Machine,
        completion: BoundaryCompletion::NormalReturn,
        unconditional_termination: false,
        identity: TerminalIdentityIndex(3),
        owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
        attachment: None,
        parameter_order: vec![BoundaryParameterKind::Scalar; signature.operand_count],
        scalar_parameters: vec![signature.operand.scalar_type().unwrap(); signature.operand_count],
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Scalar(QualifiedScalarType {
            scalar_type: result,
            qualifications: ScalarQualificationSetId::ZERO,
        }),
        normal_qualifications: Vec::new(),
        scalar_domain_requirements: Vec::new(),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        scalar_requires: Vec::new(),
        scalar_ensures: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        reference_outputs: Vec::new(),
        fixed_service_reach: Vec::new(),
        owning_service: None,
        published_service_ceiling: Vec::new(),
    });
    module
}

#[test]
fn numeric_meaning_survives_portable_round_trip_and_changes_program_identity() {
    // A format-polymorphic predicate produces Boolean; the format is retained
    // by its meaning, not guessed from that result.
    let meaning = NumericIntrinsic::from_canonical_encoding([0, 5, 1, 0]).unwrap();
    let module = module(meaning);
    let bytes = encode_module(&module).unwrap();
    let decoded = decode_module(&bytes).unwrap();
    assert_eq!(decoded, module);
    assert_eq!(
        decoded.boundary_machines[0].numeric_intrinsic,
        Some(meaning)
    );
    assert_eq!(encode_module(&decoded).unwrap(), bytes);
    let mut changed = module.clone();
    changed.boundary_machines[0].numeric_intrinsic = None;
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&changed).unwrap()
    );
}

#[test]
fn numeric_boundary_refuses_substituted_parameters_results_and_evidence_roles() {
    let meaning = NumericIntrinsic::from_canonical_encoding([3, 0, 0, 0]).unwrap();
    for mutation in 0..4 {
        let mut module = module(meaning);
        let boundary = &mut module.boundary_machines[0];
        match mutation {
            0 => boundary.scalar_parameters[0] = NumericType::F64.scalar_type().unwrap(),
            1 => boundary.result = BoundaryMachineResult::Unit,
            2 => boundary.role = BoundaryDeclarationRole::ContractEvidence,
            3 => {
                boundary
                    .scalar_parameters
                    .push(NumericType::F32.scalar_type().unwrap());
                boundary.parameter_order.push(BoundaryParameterKind::Scalar);
            }
            _ => unreachable!(),
        }
        let error = terminal_psi::validate_numeric_intrinsic_signature(
            &module.boundary_machines[0],
            &module.structural_types,
        )
        .unwrap_err();
        assert!(error.contains("numeric intrinsic"), "{error}");
        let bytes = encode_raw(&module).unwrap();
        let error = decode_module(&bytes).unwrap_err().to_string();
        assert!(error.contains("numeric intrinsic"), "{error}");
    }
}

#[test]
fn numeric_meaning_wire_rejects_noncanonical_padding_unknown_rows_and_truncation() {
    let meaning = NumericIntrinsic::from_canonical_encoding([3, 0, 0, 0]).unwrap();
    let boundary = &module(meaning).boundary_machines[0];
    let mut writer = super::wire::Writer::default();
    super::structural_signature_wire::encode_boundary_machine(&mut writer, boundary).unwrap();
    let bytes = writer.finish();
    for length in 0..bytes.len() {
        assert!(
            super::structural_signature_wire::decode_boundary_machine(
                &mut super::wire::Reader::new(&bytes[..length])
            )
            .is_err()
        );
    }
    for bytes in [
        [0, 255, 0, 0],
        [0, 0, 2, 0],
        [1, 255, 0, 0],
        [2, 0, 8, 0],
        [3, 0, 0, 1],
        [4, 8, 0, 1],
        [255, 0, 0, 0],
    ] {
        assert!(
            NumericIntrinsic::from_canonical_encoding(bytes).is_none(),
            "{bytes:?}"
        );
    }
}
