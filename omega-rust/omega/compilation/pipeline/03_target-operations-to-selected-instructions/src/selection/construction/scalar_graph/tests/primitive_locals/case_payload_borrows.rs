//! A case-qualified field load takes its offset from the named case's overlay.
use super::super::{LegalizedScalarReturnValue, LegalizedScalarTerminator};
use super::{
    CallSignature, CallingPolicy, IntegerSign, IntegerType, LegalizedScalarFunction,
    LegalizedScalarInstructionKind, PlaceId, ScalarType, SelectedFunction, SelectedInstructionKind,
    SelectedSelectionConstraints, StructuralAccess, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, ValueId, ValueShape, build, evaluate_call_plan, fixture,
};
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};

fn field(
    id: u64,
    identity: &str,
    field_type: terminal_psi::StructuralFieldType,
) -> terminal_psi::StructuralFieldDeclaration {
    terminal_psi::StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type,
    }
}

/// `Owner { prefix: u64, mode: Mode }`, with `Mode` holding `Idle { level }`
/// or `Run { distance: u64, speed }`. Both payload fields 2 share the read's
/// scalar type, so the two cases differ only in where the leaf lies: the tag
/// takes bytes 0..4 of `Mode`, the 8-aligned payload base is byte 8, and
/// `mode` begins at byte 8 of `Owner`. `Run.speed` is byte 24 of the
/// referent and `Idle.level` byte 16.
fn case_read_fixture(native: target::NativeTarget, scalar: ScalarType) -> LegalizedScalarFunction {
    let mut source = fixture(native, 0);
    let place = PlaceId::new(1).unwrap();
    let owner = StructuralTypeId::new(1).unwrap();
    let mode = StructuralTypeId::new(2).unwrap();
    let wide = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![ValueShape::borrowed_reference(32, 8)],
            result: Some(crate::selection::scalar_call_abi::scalar_shape(scalar).unwrap()),
        },
    )
    .unwrap();
    let parameter = terminal_psi::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: true,
        structural_type: owner,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    source.structural = Some(legalized_operations::LegalizedStructuralContract {
        result: None,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: owner,
                identity: "Owner".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        field(1, "prefix", terminal_psi::StructuralFieldType::Scalar(wide)),
                        field(
                            2,
                            "mode",
                            terminal_psi::StructuralFieldType::Structural(mode),
                        ),
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: mode,
                identity: "Mode".into(),
                shape: StructuralTypeShape::Sum {
                    cases: vec![
                        terminal_psi::StructuralCaseDeclaration {
                            id: StructuralCaseId::new(1).unwrap(),
                            identity: "Idle".into(),
                            fields: vec![field(
                                2,
                                "level",
                                terminal_psi::StructuralFieldType::Scalar(scalar),
                            )],
                        },
                        terminal_psi::StructuralCaseDeclaration {
                            id: StructuralCaseId::new(2).unwrap(),
                            identity: "Run".into(),
                            fields: vec![
                                field(
                                    1,
                                    "distance",
                                    terminal_psi::StructuralFieldType::Scalar(wide),
                                ),
                                field(
                                    2,
                                    "speed",
                                    terminal_psi::StructuralFieldType::Scalar(scalar),
                                ),
                            ],
                        },
                    ],
                },
            },
        ]
        .into(),
        structural_places: vec![StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        }],
        parameters: vec![legalized_operations::LegalizedCallUnitParameter {
            semantic: parameter,
            target: target_operations::TargetStructuralParameter {
                place,
                structural_type: owner,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                projected_qualifications: Vec::new(),
                shape: ValueShape::borrowed_reference(32, 8),
                placement: source.call_plan.parameters[0].clone(),
            },
        }],
    });
    source.blocks[0].instructions.truncate(1);
    source.provenance.operations.truncate(1);
    source.blocks[0].instructions[0]
        .result
        .as_mut()
        .unwrap()
        .scalar_type = scalar;
    source.blocks[0].instructions[0].kind =
        LegalizedScalarInstructionKind::StructuralScalarFieldRead {
            source: terminal_psi::StructuralArgument {
                place,
                access: StructuralAccess::SharedBorrow,
                path: vec![terminal_psi::StructuralPathSegment::Field("mode".into())],
            },
            case: Some(StructuralCaseId::new(2).unwrap()),
            field: StructuralFieldId::new(2).unwrap(),
        };
    let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator else {
        panic!("return");
    };
    returned.value = LegalizedScalarReturnValue::Value {
        value: ValueId::new(1).unwrap(),
        scalar_type: scalar,
    };
    source
}

fn read_offsets(selected: &SelectedFunction, place: PlaceId) -> Vec<u32> {
    selected
        .memory_accesses
        .iter()
        .filter(|access| {
            access.place == place
                && access.role == selected_instructions::SelectedMemoryAccessRole::ReadPlace
        })
        .map(|access| access.byte_offset)
        .collect()
}

#[test]
fn case_payload_load_replays_case_field_and_overlay_offset() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for scalar in [
            ScalarType::Boolean,
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
        ] {
            let source = case_read_fixture(native, scalar);
            let place = PlaceId::new(1).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                fixed_inputs: Vec::new(),
            };
            let construct = |source: &LegalizedScalarFunction| {
                build(
                    0,
                    source,
                    native,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    source,
                    selected,
                    native,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            let selected = construct(&source).unwrap();
            validate(&source, &selected).unwrap();
            assert_eq!(read_offsets(&selected, place), vec![24], "{native:?}");

            // The same row naming `Idle` is a different observation: it
            // selects the other overlay at byte 16, and the `Run` selection
            // does not replay against it.
            let mut idle = source.clone();
            let LegalizedScalarInstructionKind::StructuralScalarFieldRead { case, .. } =
                &mut idle.blocks[0].instructions[0].kind
            else {
                panic!("read");
            };
            *case = Some(StructuralCaseId::new(1).unwrap());
            let idle_selected = construct(&idle).unwrap();
            validate(&idle, &idle_selected).unwrap();
            assert_eq!(read_offsets(&idle_selected, place), vec![16]);
            assert!(validate(&idle, &selected).is_err());
            assert!(validate(&source, &idle_selected).is_err());

            // Dropping the case reads a record field of a sum, an undeclared
            // case has no overlay, and `Run.distance` is not the read's type.
            for mutation in 0..3 {
                let mut changed = source.clone();
                let LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    case, field, ..
                } = &mut changed.blocks[0].instructions[0].kind
                else {
                    panic!("read");
                };
                match mutation {
                    0 => *case = None,
                    1 => *case = Some(StructuralCaseId::new(3).unwrap()),
                    _ => *field = StructuralFieldId::new(1).unwrap(),
                }
                assert!(
                    construct(&changed).is_err(),
                    "construction mutation {mutation}"
                );
                assert!(
                    validate(&changed, &selected).is_err(),
                    "replay mutation {mutation}"
                );
            }

            // A selected load moved to another overlay is rejected.
            let mut changed = selected.clone();
            let load = changed.blocks[0]
                .instructions
                .iter_mut()
                .find(|row| {
                    matches!(
                        row.kind,
                        SelectedInstructionKind::Load8 { .. }
                            | SelectedInstructionKind::Load16 { .. }
                    )
                })
                .unwrap();
            match &mut load.kind {
                SelectedInstructionKind::Load8 { byte_offset }
                | SelectedInstructionKind::Load16 { byte_offset } => *byte_offset = 16,
                _ => panic!("load"),
            }
            assert!(validate(&source, &changed).is_err());
        }
    }
}
