//! Verified scalar literal calls still stop before unsupported native literal storage.

use semantic_vocabulary::{BlockId, MachineId, OperationId, StructuralPlaceKind};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalModule};

const LITERALS: [&[u8]; 3] = [
    &[],
    &[0xff, 0x00, 0x80],
    &[
        0x4f, 0x6d, 0x65, 0x67, 0x61, 0x00, 0xff, 0x80, 0x17, 0xfe, 0x41,
    ],
];

fn literal_call_module(bytes: &[u8]) -> TerminalModule {
    let mut module = super::fixtures::byte_view_read_call_module();
    let caller = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = caller.structural_parameters.remove(0);
    let place = caller
        .structural_places
        .iter_mut()
        .find(|place| place.id == parameter.place)
        .unwrap();
    place.kind = StructuralPlaceKind::ByteSequenceLiteral {
        declaration_ordinal: 0,
        structural_type: parameter.structural_type,
    };
    caller.blocks[0].operations.insert(
        0,
        Operation {
            id: OperationId::new(104).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place.id,
                bytes: bytes.to_vec(),
            },
        },
    );
    module
}

#[test]
fn verified_literal_byte_view_calls_reject_before_native_projection() {
    for bytes in LITERALS {
        let module = literal_call_module(bytes);
        let proof = super::fixtures::byte_view_read_proof(&module);
        let semantic = terminal_codec::encode_module(&module).unwrap();
        let proof = terminal_codec::encode_proof_bundle(&proof).unwrap();
        let selections = optimization_core::OptimizationSelections::new([]).unwrap();
        match native_realization::optimize_artifact_sections(
            &semantic, &proof, &proof_admission::AdmissionProfile::default(),
            native_realization::compiler_baseline_request_v1(&selections),
        ) {
            Err(native_realization::OptimizationPipelineError::Run(
                abstract_operations_to_abstract_operations::OptimizationRunError::InitialValidation(
                    optimization_unit_semantics::OptimizationUnitValidationError::StructuralCallContractMismatch { machine, block, node },
                ),
            )) => {
                assert_eq!(machine, MachineId::new(100).unwrap());
                assert_eq!(block, BlockId::new(101).unwrap());
                assert_eq!(node, 1);
            }
            Err(error) => panic!("unexpected native literal admission failure: {error:?}"),
            Ok(_) => panic!("native literal support advanced; replace this fence with the runtime oracle"),
        }
    }
}
