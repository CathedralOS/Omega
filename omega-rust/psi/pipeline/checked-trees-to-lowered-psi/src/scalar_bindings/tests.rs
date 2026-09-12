use super::*;
use crate::scalar_graph_lowering::lower_checked_boolean_expression;
use checked_trees::CheckedScalarBindingDestination;

#[test]
fn owned_structural_locals_reuse_exact_published_payloads_and_reject_invalid_custody() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let other = symbols::SymbolHandle::from_arena_index(2);
    let source = StructuralArgument {
        place: PlaceId::new(41).unwrap(),
        path: Vec::new(),
        access: StructuralAccess::Owned,
    };
    let argument = checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        ..Default::default()
    };
    let bindings = ScalarBindings::new(0).with_structural_locals(&[(symbol, source.clone())]);
    for _ in 0..2 {
        assert_eq!(bindings.owned_argument(&argument).unwrap(), source);
    }
    for locals in [
        vec![],
        vec![(other, source.clone())],
        vec![(symbol, source.clone()), (symbol, source.clone())],
        vec![(
            symbol,
            StructuralArgument {
                access: StructuralAccess::SharedBorrow,
                ..source.clone()
            },
        )],
        vec![(
            symbol,
            StructuralArgument {
                path: vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
                ..source.clone()
            },
        )],
    ] {
        assert!(
            ScalarBindings::new(0)
                .with_structural_locals(&locals)
                .owned_argument(&argument)
                .is_err()
        );
    }
    let mut changed = argument.clone();
    changed.access = checked_trees::CheckedStructuralAccess::SharedBorrow;
    assert!(bindings.owned_argument(&changed).is_err());
    changed.access = checked_trees::CheckedStructuralAccess::Owned;
    changed
        .path
        .push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
            0,
        ));
    assert!(bindings.owned_argument(&changed).is_err());
    changed.path.clear();
    let invalid = symbols::SymbolHandle::invalid();
    changed.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: invalid };
    changed.access = checked_trees::CheckedStructuralAccess::Owned;
    assert!(
        ScalarBindings::new(0)
            .with_structural_locals(&[(invalid, source)])
            .owned_argument(&changed)
            .is_err()
    );
}

#[test]
fn primitive_places_remain_separate_from_immutable_and_current_ssa_storage() {
    let primitive_symbol = symbols::SymbolHandle::from_arena_index(1);
    let ssa_symbol = symbols::SymbolHandle::from_arena_index(2);
    let source = PlaceId::new(41).unwrap();
    let scalar_type = terminal_scalar_type(PrimitiveType::U64).unwrap();
    let mut bindings = ScalarBindings::new(1);
    for symbol in [primitive_symbol, ssa_symbol] {
        bindings
            .append(
                CheckedScalarBindingDestination::StorageInitialize { symbol },
                scalar_type,
                1,
            )
            .unwrap();
    }
    bindings
        .append(CheckedScalarBindingDestination::Immutable, scalar_type, 2)
        .unwrap();
    let mut bindings = bindings.with_primitive_storage(&[(primitive_symbol, source, scalar_type)]);
    for current_position in [3, 4] {
        for symbol in [primitive_symbol, ssa_symbol] {
            bindings
                .append(
                    CheckedScalarBindingDestination::StorageAssign { symbol },
                    scalar_type,
                    current_position,
                )
                .unwrap();
        }
        assert_eq!(
            bindings
                .expression(&CheckedScalarExpression::StorageRead {
                    symbol: primitive_symbol,
                    primitive_type: PrimitiveType::U64,
                })
                .unwrap(),
            LoweredDirectExpression::PrimitiveRead {
                source,
                scalar_type
            },
        );
        assert_eq!(
            bindings
                .expression(&CheckedScalarExpression::StorageRead {
                    symbol: ssa_symbol,
                    primitive_type: PrimitiveType::U64,
                })
                .unwrap(),
            LoweredDirectExpression::Local {
                position: current_position,
                scalar_type
            },
        );
        assert_eq!(
            bindings
                .expression(&CheckedScalarExpression::Local {
                    position: 1,
                    primitive_type: PrimitiveType::U64,
                })
                .unwrap(),
            LoweredDirectExpression::Local {
                position: 2,
                scalar_type
            },
        );
    }
    assert!(bindings.immutable_position(2).is_err());
}

#[test]
fn primitive_storage_reads_reject_missing_duplicate_invalid_and_wrong_type_rows() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let other = symbols::SymbolHandle::from_arena_index(2);
    let source = PlaceId::new(41).unwrap();
    let integer_type = terminal_scalar_type(PrimitiveType::U64).unwrap();
    let boolean_read =
        CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::StorageRead {
            symbol,
        }));
    for storage in [
        vec![],
        vec![(other, source, ScalarType::Boolean)],
        vec![(symbol, source, integer_type)],
        vec![
            (symbol, source, ScalarType::Boolean),
            (symbol, source, ScalarType::Boolean),
        ],
    ] {
        assert!(
            ScalarBindings::new(0)
                .with_primitive_storage(&storage)
                .expression(&boolean_read)
                .is_err()
        );
    }
    let invalid = symbols::SymbolHandle::invalid();
    let bindings =
        ScalarBindings::new(0).with_primitive_storage(&[(invalid, source, ScalarType::Boolean)]);
    assert!(
        bindings
            .expression(&CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::StorageRead { symbol: invalid },
            )))
            .is_err()
    );

    // A matching old SSA type cannot rescue a wrong primitive place type.
    let mut bindings = ScalarBindings::new(0);
    bindings
        .append(
            CheckedScalarBindingDestination::StorageInitialize { symbol },
            ScalarType::Boolean,
            0,
        )
        .unwrap();
    let bindings = bindings.with_primitive_storage(&[(symbol, source, integer_type)]);
    assert!(bindings.expression(&boolean_read).is_err());
    assert!(
        bindings
            .expression(&CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type: PrimitiveType::Bool,
            })
            .is_err()
    );
}

#[test]
fn primitive_reads_emit_fresh_typed_results_after_intervening_stores() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let source = PlaceId::new(41).unwrap();
    for primitive_type in [
        PrimitiveType::U64,
        PrimitiveType::Bool,
        PrimitiveType::F32,
        PrimitiveType::F64,
    ] {
        let scalar_type = terminal_scalar_type(primitive_type).unwrap();
        let bindings =
            ScalarBindings::new(0).with_primitive_storage(&[(symbol, source, scalar_type)]);
        let expression = bindings
            .expression(&CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            })
            .unwrap();
        assert_eq!(expression.scalar_type(), scalar_type);
        let mut operations = OperationBuffer::new(0);
        let mut next_value = 1;
        let first = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
        let store = operations.allocate();
        operations.push(Operation {
            id: store,
            result: terminal_psi::OperationResult::Unit,
            kind: OperationKind::WriteOnlyPrimitiveStore {
                destination: source,
                value: first,
            },
        });
        let second = emit_direct_expression(&expression, &[], &mut next_value, &mut operations);
        assert_ne!(first, second);
        assert_eq!(operations.len(), 3);
        for (operation_position, value) in [(0, first), (2, second)] {
            assert_eq!(
                operations[operation_position].kind,
                OperationKind::PrimitiveScalarRead { source }
            );
            assert_eq!(
                operations[operation_position].result,
                terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value,
                    scalar_type
                },)
            );
        }
    }
}

#[test]
fn recursive_integer_and_boolean_readers_keep_operand_evaluation_order() {
    let left_symbol = symbols::SymbolHandle::from_arena_index(1);
    let right_symbol = symbols::SymbolHandle::from_arena_index(2);
    let left_place = PlaceId::new(41).unwrap();
    let right_place = PlaceId::new(42).unwrap();
    let scalar_type = terminal_scalar_type(PrimitiveType::U8).unwrap();
    let bindings = ScalarBindings::new(0).with_primitive_storage(&[
        (left_symbol, left_place, scalar_type),
        (right_symbol, right_place, scalar_type),
    ]);
    let expression = bindings
        .expression(&CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::IntegerComparison {
                kind: CheckedIntegerComparisonKind::Equal,
                left: Box::new(CheckedScalarExpression::IntegerWiden {
                    primitive_type: PrimitiveType::U64,
                    operand: Box::new(CheckedScalarExpression::IntegerBitwiseNot {
                        primitive_type: PrimitiveType::U8,
                        operand: Box::new(CheckedScalarExpression::StorageRead {
                            symbol: left_symbol,
                            primitive_type: PrimitiveType::U8,
                        }),
                    }),
                }),
                right: Box::new(CheckedScalarExpression::IntegerWiden {
                    primitive_type: PrimitiveType::U64,
                    operand: Box::new(CheckedScalarExpression::IntegerBinary {
                        kind: CheckedIntegerBinaryKind::WrappingAdd,
                        primitive_type: PrimitiveType::U8,
                        left: Box::new(CheckedScalarExpression::StorageRead {
                            symbol: right_symbol,
                            primitive_type: PrimitiveType::U8,
                        }),
                        right: Box::new(CheckedScalarExpression::StorageRead {
                            symbol: left_symbol,
                            primitive_type: PrimitiveType::U8,
                        }),
                    }),
                }),
            },
        )))
        .unwrap();
    validate_direct_parameter_types(&expression, &[]).unwrap();
    let mut operations = OperationBuffer::new(0);
    emit_direct_expression(&expression, &[], &mut 1, &mut operations);
    let sources = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::PrimitiveScalarRead { source } => Some(source),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sources, [left_place, right_place, left_place]);
}

#[test]
fn primitive_boolean_reads_stay_in_selected_short_circuit_blocks() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let source = PlaceId::new(41).unwrap();
    let bindings =
        ScalarBindings::new(1).with_primitive_storage(&[(symbol, source, ScalarType::Boolean)]);
    for conjunction in [true, false] {
        let left = Box::new(CheckedBooleanExpression::Parameter { position: 0 });
        let right = Box::new(CheckedBooleanExpression::Not(Box::new(
            CheckedBooleanExpression::StorageRead { symbol },
        )));
        let checked = if conjunction {
            CheckedBooleanExpression::And { left, right }
        } else {
            CheckedBooleanExpression::Or { left, right }
        };
        let LoweredDirectExpression::Boolean { expression } = bindings
            .expression(&CheckedScalarExpression::Boolean(Box::new(checked)))
            .unwrap()
        else {
            panic!("Boolean expression retains its carrier");
        };
        let decision = lower_boolean_value_decision(&expression);
        let parameter = ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        };
        let mut operations = OperationBuffer::new(0);
        let (root, branches) = emit_inlined_boolean_value_blocks(
            &decision,
            &[parameter],
            vec![parameter],
            LoweredBooleanDecisionExit::Return,
            block_id(1),
            block_id(3),
            &mut 2,
            &mut 1,
            &mut operations,
        );
        assert!(
            root.operations.is_empty(),
            "the right read must not precede selection"
        );
        let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = root.terminator
        else {
            panic!("short-circuit root selects a successor");
        };
        let (selected, skipped) = if conjunction {
            (when_true.target, when_false.target)
        } else {
            (when_false.target, when_true.target)
        };
        let reads = |block: &Block| {
            block
                .operations
                .iter()
                .filter(|operation| operation.kind == OperationKind::PrimitiveScalarRead { source })
                .count()
        };
        assert_eq!(
            reads(branches.iter().find(|block| block.id == selected).unwrap()),
            1
        );
        assert_eq!(
            reads(branches.iter().find(|block| block.id == skipped).unwrap()),
            0
        );
    }
}

#[test]
fn equal_boolean_read_syntax_emits_two_distinct_observations() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let source = PlaceId::new(41).unwrap();
    let bindings =
        ScalarBindings::new(0).with_primitive_storage(&[(symbol, source, ScalarType::Boolean)]);
    let read = CheckedBooleanExpression::StorageRead { symbol };
    let expression = bindings
        .expression(&CheckedScalarExpression::Boolean(Box::new(
            CheckedBooleanExpression::Equal {
                left: Box::new(read.clone()),
                right: Box::new(read),
            },
        )))
        .unwrap();
    let mut operations = OperationBuffer::new(0);
    emit_direct_expression(&expression, &[], &mut 1, &mut operations);
    assert_eq!(operations.len(), 3);
    for operation in &operations[..2] {
        assert_eq!(
            operation.kind,
            OperationKind::PrimitiveScalarRead { source }
        );
        assert_eq!(
            operation.result.expect_scalar().scalar_type,
            ScalarType::Boolean
        );
    }
    let OperationKind::BooleanEqual { left, right } = operations[2].kind else {
        panic!("Boolean equality consumes the completed reads");
    };
    assert_ne!(left, right);
    assert_eq!(left, operations[0].result.expect_scalar().id);
    assert_eq!(right, operations[1].result.expect_scalar().id);
}

#[test]
fn mutable_parameters_have_current_storage_without_an_immutable_entry_alias() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let mut bindings = ScalarBindings::new(3);
    bindings
        .initialize_parameter(symbol, ScalarType::Boolean, 1)
        .unwrap();
    assert_eq!(bindings.immutable_position(0).unwrap(), 0);
    assert!(bindings.immutable_position(1).is_err());
    assert_eq!(bindings.immutable_position(2).unwrap(), 2);
    assert_eq!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .unwrap(),
        1
    );
    bindings
        .append(
            CheckedScalarBindingDestination::Immutable,
            ScalarType::Boolean,
            3,
        )
        .unwrap();
    bindings
        .append(
            CheckedScalarBindingDestination::StorageAssign { symbol },
            ScalarType::Boolean,
            4,
        )
        .unwrap();
    assert_eq!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .unwrap(),
        4
    );
    assert_eq!(bindings.immutable_position(3).unwrap(), 3);
    assert!(
        bindings
            .expression(&CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::Bool,
            })
            .is_err()
    );
    assert!(
        bindings
            .initialize_parameter(symbol, ScalarType::Boolean, 1)
            .is_err()
    );
}

#[test]
fn storage_and_immutable_namespaces_resolve_distinct_current_values() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let mut bindings = ScalarBindings::new(1);
    assert!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .is_err()
    );
    bindings
        .append(
            CheckedScalarBindingDestination::StorageInitialize { symbol },
            ScalarType::Boolean,
            1,
        )
        .unwrap();
    bindings
        .append(
            CheckedScalarBindingDestination::Immutable,
            ScalarType::Boolean,
            2,
        )
        .unwrap();
    bindings
        .append(
            CheckedScalarBindingDestination::StorageAssign { symbol },
            ScalarType::Boolean,
            3,
        )
        .unwrap();
    assert_eq!(bindings.immutable_position(0).unwrap(), 0);
    assert_eq!(bindings.immutable_position(1).unwrap(), 2);
    assert!(bindings.immutable_position(2).is_err());
    assert_eq!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .unwrap(),
        3
    );
}

#[test]
fn storage_mapping_rejects_missing_duplicate_and_wrong_type_custody() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let other = symbols::SymbolHandle::from_arena_index(2);
    let integer = terminal_scalar_type(typed_trees::types::PrimitiveType::U8).unwrap();
    let mut bindings = ScalarBindings::new(0);
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageAssign { symbol },
                ScalarType::Boolean,
                0
            )
            .is_err()
    );
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageInitialize {
                    symbol: symbols::SymbolHandle::invalid()
                },
                ScalarType::Boolean,
                0
            )
            .is_err()
    );
    bindings
        .append(
            CheckedScalarBindingDestination::StorageInitialize { symbol },
            ScalarType::Boolean,
            0,
        )
        .unwrap();
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageInitialize { symbol },
                ScalarType::Boolean,
                1
            )
            .is_err()
    );
    assert!(
        bindings
            .append(
                CheckedScalarBindingDestination::StorageAssign { symbol },
                integer,
                1
            )
            .is_err()
    );
    assert!(bindings.storage_position(symbol, integer).is_err());
    assert!(
        bindings
            .storage_position(other, ScalarType::Boolean)
            .is_err()
    );
    assert_eq!(
        bindings
            .storage_position(symbol, ScalarType::Boolean)
            .unwrap(),
        0
    );
    assert!(
        lower_checked_scalar_expression(&CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type: typed_trees::types::PrimitiveType::Bool
        })
        .is_err()
    );
    assert!(
        lower_checked_boolean_expression(&CheckedBooleanExpression::StorageRead { symbol })
            .is_err()
    );
}
