use super::*;
use semantic_vocabulary::IeeeFloatComparisonOperation;

fn comparison_module(
    comparison: IeeeFloatComparisonOperation,
    operands: [IeeeFloatValue; 2],
) -> TerminalModule {
    let mut module = unit_module();
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(4),
        scalar_type: ScalarType::Boolean,
        qualifications: Default::default(),
    });
    machine.blocks[0].operations = operands
        .into_iter()
        .enumerate()
        .map(|(ordinal, value)| Operation {
            id: operation_id(ordinal as u64 + 1),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(ordinal as u64 + 1),
                scalar_type: ScalarType::IeeeFloat(value.format()),
                qualifications: Default::default(),
            }),
            kind: OperationKind::IeeeFloatConstant { value },
        })
        .chain(std::iter::once(Operation {
            id: operation_id(3),
            result: OperationResult::Scalar(ValueDeclaration {
                id: value_id(3),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            }),
            kind: OperationKind::IeeeFloatCompare {
                comparison,
                left: value_id(1),
                right: value_id(2),
            },
        }))
        .collect();
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(3),
        cleanup_actions: Vec::new(),
    };
    module
}

#[test]
fn ieee_relations_round_trip_and_execute_both_formats_without_order_complements() {
    use IeeeFloatComparisonOperation::*;
    let comparisons = [Equal, NotEqual, Less, LessOrEqual, Greater, GreaterOrEqual];
    let equal = [true, false, false, true, false, true];
    let less = [false, true, true, true, false, false];
    let greater = [false, true, false, false, true, true];
    let unordered = [false, true, false, false, false, false];
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let values = match format {
            IeeeFloatFormat::Binary32 => [
                IeeeFloatValue::Binary32(0),
                IeeeFloatValue::Binary32(0x8000_0000),
                IeeeFloatValue::Binary32(0x3f80_0000),
                IeeeFloatValue::Binary32(0x4000_0000),
                IeeeFloatValue::Binary32(1),
                IeeeFloatValue::Binary32(0x7f80_0000),
                IeeeFloatValue::Binary32(0xff80_0000),
                IeeeFloatValue::Binary32(0x7fc0_0042),
                IeeeFloatValue::Binary32(0xff80_0001),
            ],
            IeeeFloatFormat::Binary64 => [
                IeeeFloatValue::Binary64(0),
                IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
                IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
                IeeeFloatValue::Binary64(0x4000_0000_0000_0000),
                IeeeFloatValue::Binary64(1),
                IeeeFloatValue::Binary64(0x7ff0_0000_0000_0000),
                IeeeFloatValue::Binary64(0xfff0_0000_0000_0000),
                IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042),
                IeeeFloatValue::Binary64(0xfff0_0000_0000_0001),
            ],
        };
        for (left, right, expected) in [
            (0, 1, equal),
            (2, 3, less),
            (3, 2, greater),
            (4, 0, greater),
            (5, 5, equal),
            (6, 2, less),
            (2, 7, unordered),
            (7, 2, unordered),
            (7, 7, unordered),
            (8, 8, unordered),
        ] {
            let mut encodings = Vec::new();
            for (comparison, expected) in comparisons.into_iter().zip(expected) {
                let module = comparison_module(comparison, [values[left], values[right]]);
                let semantic = encode_module(&module).unwrap();
                let decoded = decode_module(&semantic).unwrap();
                assert_eq!(decoded, module);
                assert_eq!(encode_module(&decoded).unwrap(), semantic);
                assert!(
                    !encodings.contains(&semantic),
                    "operation relation participates in identity"
                );
                encodings.push(semantic.clone());
                let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
                let measured = interpret_terminal_artifact_measured(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    &[],
                )
                .unwrap();
                assert_eq!(
                    measured.value(),
                    TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
                    "{format:?} {comparison:?} {left} {right}",
                );
                assert_eq!(measured.usage().total_units(), 4);
            }
        }
    }
}

#[test]
fn ieee_comparison_rejects_mixed_carriers_nonboolean_results_and_missing_operands() {
    let original = comparison_module(
        IeeeFloatComparisonOperation::Equal,
        [IeeeFloatValue::Binary32(0), IeeeFloatValue::Binary32(0)],
    );
    for mutation in ["format", "boolean", "result", "missing", "late", "unit"] {
        let mut module = original.clone();
        let operations = &mut module.machines[0].blocks[0].operations;
        match mutation {
            "format" | "boolean" => {
                let (scalar_type, kind) = if mutation == "format" {
                    (
                        ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
                        OperationKind::IeeeFloatConstant {
                            value: IeeeFloatValue::Binary64(0),
                        },
                    )
                } else {
                    (
                        ScalarType::Boolean,
                        OperationKind::BooleanConstant { value: false },
                    )
                };
                operations[1].result = OperationResult::Scalar(ValueDeclaration {
                    id: value_id(2),
                    scalar_type,
                    qualifications: Default::default(),
                });
                operations[1].kind = kind;
            }
            "result" => {
                operations[2].result = OperationResult::Scalar(ValueDeclaration {
                    id: value_id(3),
                    scalar_type: ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
                    qualifications: Default::default(),
                })
            }
            "missing" => {
                operations[2].kind = OperationKind::IeeeFloatCompare {
                    comparison: IeeeFloatComparisonOperation::Equal,
                    left: value_id(1),
                    right: value_id(999),
                }
            }
            "late" => operations.swap(1, 2),
            "unit" => operations[2].result = OperationResult::Unit,
            _ => unreachable!(),
        }
        assert!(
            verify_module(
                &module,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            )
            .is_err(),
            "{mutation} cannot donate IEEE comparison custody",
        );
    }
}

#[test]
fn ieee_comparison_wire_rejects_unknown_relation_and_previous_vocabulary() {
    let module = comparison_module(
        IeeeFloatComparisonOperation::Equal,
        [IeeeFloatValue::Binary32(0), IeeeFloatValue::Binary32(0)],
    );
    let bytes = encode_module(&module).unwrap();
    let mut needle = vec![65, 0];
    needle.extend(1_u64.to_le_bytes());
    needle.extend(2_u64.to_le_bytes());
    let positions = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(position, window)| (window == needle).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    let mut invalid = bytes.clone();
    invalid[positions[0] + 1] = 6;
    assert_eq!(
        decode_module(&invalid),
        Err(terminal_codec::CodecError::InvalidTag(
            "IeeeFloatComparisonOperation",
            6
        )),
    );
    let mut stale = bytes;
    stale[10..12].copy_from_slice(&96_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale),
        Err(terminal_codec::CodecError::UnsupportedVocabularyMarker(96))
    );
}
