use super::{ScalarBindings, terminal_scalar_type};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedRuntimeIndex, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};
use semantic_vocabulary::{PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

fn observation_bindings(
    view: bool,
    access: StructuralAccess,
    primitive: PrimitiveType,
) -> (ScalarBindings, CheckedUnitStructuralArgumentPlan) {
    let sum = StructuralTypeId::new(1).unwrap();
    let record = StructuralTypeId::new(2).unwrap();
    let collection = StructuralTypeId::new(3).unwrap();
    let declarations = vec![
        StructuralTypeDeclaration {
            id: sum,
            identity: "Mode".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    id: StructuralCaseId::new(7).unwrap(),
                    identity: "Ready".into(),
                    fields: Vec::new(),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: record,
            identity: "Entry".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(5).unwrap(),
                    identity: "mode".into(),
                    relevance: language_core::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(sum),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: collection,
            identity: "Collection".into(),
            shape: if view {
                StructuralTypeShape::ElementView { element: record }
            } else {
                StructuralTypeShape::FixedArray {
                    element: record,
                    length: 4,
                }
            },
        },
    ];
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(19).unwrap(),
        position: 0,
        is_self: false,
        structural_type: collection,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let scalar_type = terminal_scalar_type(primitive).unwrap();
    let bindings = ScalarBindings::for_computation_operands(6, [scalar_type])
        .with_structural_parameters(&[(3, parameter)])
        .with_structural_observations(&declarations);
    let argument = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 },
        path: vec![
            CheckedUnitStructuralPathSegment::RuntimeIndex(CheckedRuntimeIndex::Parameter {
                position: 0,
            }),
            CheckedUnitStructuralPathSegment::Field("mode".into()),
        ],
        type_identity: "Mode".into(),
        access: CheckedStructuralAccess::SharedBorrow,
    };
    (bindings, argument)
}

#[test]
fn case_observation_keeps_array_or_view_root_and_mapped_integer_parameter() {
    for view in [false, true] {
        for primitive in [PrimitiveType::U16, PrimitiveType::I32] {
            let (bindings, argument) =
                observation_bindings(view, StructuralAccess::SharedBorrow, primitive);
            let result = bindings
                .parameter_element_case_observation(&argument, "Ready", &[])
                .unwrap();
            let LoweredBooleanReturnExpression::IndexedCaseMembership {
                source,
                observes_element_view_length,
                path,
                index,
                element_path,
                case,
            } = result
            else {
                panic!("indexed case observation");
            };
            assert_eq!(source, PlaceId::new(19).unwrap());
            assert_eq!(observes_element_view_length, view);
            assert!(path.is_empty());
            assert_eq!(
                element_path,
                [terminal_psi::StructuralPathSegment::Field("mode".into())]
            );
            assert_eq!(case, StructuralCaseId::new(7).unwrap());
            assert!(
                matches!(*index, LoweredDirectExpression::Parameter { position: 6, scalar_type }
                if scalar_type == terminal_scalar_type(primitive).unwrap()),
                "signed selectors keep their signed carrier so the bound owes nonnegativity"
            );
        }
    }
}

#[test]
fn case_observation_rejects_unreadable_foreign_or_noninteger_bindings() {
    for (access, primitive) in [
        (StructuralAccess::WriteOnlyBorrow, PrimitiveType::U64),
        (StructuralAccess::SharedBorrow, PrimitiveType::Addr),
        (StructuralAccess::SharedBorrow, PrimitiveType::Bool),
    ] {
        let (bindings, argument) = observation_bindings(true, access, primitive);
        assert!(
            bindings
                .parameter_element_case_observation(&argument, "Ready", &[])
                .is_err()
        );
    }
    let (bindings, argument) =
        observation_bindings(true, StructuralAccess::SharedBorrow, PrimitiveType::U64);
    assert!(
        bindings
            .parameter_element_case_observation(&argument, "Foreign", &[])
            .is_err()
    );
    for drift in 0..4 {
        let mut changed = argument.clone();
        match drift {
            0 => {
                changed.source =
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
            }
            1 => changed.path[1] = CheckedUnitStructuralPathSegment::Field("other".into()),
            2 => {
                changed.path[0] =
                    CheckedUnitStructuralPathSegment::RuntimeIndex(CheckedRuntimeIndex::Parameter {
                        position: 1,
                    })
            }
            _ => changed.access = CheckedStructuralAccess::Owned,
        }
        assert!(
            bindings
                .parameter_element_case_observation(&changed, "Ready", &[])
                .is_err()
        );
    }
}

#[test]
fn case_observation_binds_formal_symbols_after_receiver_retention() {
    let symbol = symbols::SymbolHandle::from_arena_index;
    let machine = checked_trees::machine::Machine {
        symbol: symbol(10),
        ..Default::default()
    };
    let mut parameters = (0..4)
        .map(|position| checked_trees::signature::StateParameter {
            symbol: symbol(20 + position),
            is_self: position == 0,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    for retain_receiver in [false, true] {
        let (mut bindings, mut argument) =
            observation_bindings(true, StructuralAccess::SharedBorrow, PrimitiveType::U64);
        if retain_receiver {
            let mut receiver = bindings.structural_parameters[0].1.clone();
            receiver.place = PlaceId::new(18).unwrap();
            receiver.is_self = true;
            bindings.structural_parameters.insert(0, (0, receiver));
        }
        argument.source = CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: parameters[3].symbol,
        };
        let resolved = bindings
            .case_parameter_argument(&argument, &machine, &parameters)
            .unwrap()
            .unwrap();
        assert!(matches!(resolved.source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
                if parameter_index == u32::from(retain_receiver)));
        let result = bindings
            .parameter_element_case_observation(&resolved, "Ready", &[])
            .unwrap();
        assert!(
            matches!(result, LoweredBooleanReturnExpression::IndexedCaseMembership {
            source, observes_element_view_length: true, ..
        } if source == PlaceId::new(19).unwrap())
        );

        argument.source = CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: machine.symbol,
        };
        if retain_receiver {
            let resolved = bindings
                .case_parameter_argument(&argument, &machine, &parameters)
                .unwrap()
                .unwrap();
            assert!(matches!(
                resolved.source,
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
            ));
        } else {
            assert!(
                bindings
                    .case_parameter_argument(&argument, &machine, &parameters)
                    .is_err()
            );
        }
    }

    let (bindings, mut argument) =
        observation_bindings(true, StructuralAccess::SharedBorrow, PrimitiveType::U64);
    argument.source =
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: symbol(99) };
    assert!(
        bindings
            .case_parameter_argument(&argument, &machine, &parameters)
            .unwrap()
            .is_none(),
        "an actual local must retain its separate local observation route"
    );
    argument.source = CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
        symbol: parameters[3].symbol,
    };
    parameters[2].symbol = parameters[3].symbol;
    assert!(
        bindings
            .case_parameter_argument(&argument, &machine, &parameters)
            .is_err()
    );
}

#[test]
fn case_observation_emits_only_declared_view_length_before_membership() {
    use crate::emission::operation_emission::boolean::emit_boolean_expression;
    use crate::emission::operation_emission::buffer::OperationBuffer;
    for view in [false, true] {
        let (bindings, argument) =
            observation_bindings(view, StructuralAccess::SharedBorrow, PrimitiveType::U64);
        let expression = bindings
            .parameter_element_case_observation(&argument, "Ready", &[])
            .unwrap();
        let parameters = (1..=7)
            .map(|position| terminal_psi::ValueDeclaration {
                id: semantic_vocabulary::ValueId::new(position).unwrap(),
                scalar_type: terminal_scalar_type(PrimitiveType::U64).unwrap(),
                qualifications: Default::default(),
            })
            .collect::<Vec<_>>();
        let mut operations = OperationBuffer::new(0);
        let mut next_value = 8;
        emit_boolean_expression(&expression, &parameters, &mut next_value, &mut operations);
        assert_eq!(operations.operations.len(), if view { 2 } else { 1 });
        if view {
            assert!(matches!(operations.operations[0].kind,
                terminal_psi::OperationKind::ElementViewLength { source }
                    if source == PlaceId::new(19).unwrap()));
        }
        let terminal_psi::OperationKind::StructuralCaseMembership { source, path, .. } =
            &operations.operations.last().unwrap().kind
        else {
            panic!("case observation");
        };
        assert_eq!(*source, PlaceId::new(19).unwrap());
        assert!(
            matches!(path[0], terminal_psi::StructuralPathSegment::RuntimeIndex { index, .. }
            if index == parameters[6].id)
        );
    }
}
