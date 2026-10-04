use super::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTrees,
    PlaceId, ScalarBindings, ScalarType, StructuralAccess, StructuralArgument,
    terminal_scalar_type,
};
use crate::expression_preparation::prepare_expression::{
    lower_checked_boolean_expression, lower_checked_scalar_expression,
};
use checked_trees::CheckedScalarBindingDestination;
use checked_trees::types::PrimitiveType;

#[test]
fn runtime_parameter_selectors_retain_declared_type_and_mapped_position() {
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::expression_preparation::prepare_expression::exact_u64_index;
    let unsigned = terminal_scalar_type(PrimitiveType::U16).unwrap();
    let signed = terminal_scalar_type(PrimitiveType::I32).unwrap();
    let address = terminal_scalar_type(PrimitiveType::Addr).unwrap();
    let bindings = ScalarBindings::for_computation_operands(
        7,
        [unsigned, signed, ScalarType::Boolean, address],
    );
    assert!(matches!(bindings.runtime_index_parameter(0).unwrap(),
        LoweredDirectExpression::Parameter { position: 7, scalar_type } if scalar_type == unsigned));
    assert!(matches!(
        exact_u64_index(bindings.runtime_index_parameter(0).unwrap()).unwrap(),
        LoweredDirectExpression::IntegerWiden { .. }
    ));
    assert!(matches!(
        exact_u64_index(bindings.runtime_index_parameter(1).unwrap()).unwrap(),
        LoweredDirectExpression::IntegerExactCast { .. }
    ));
    for position in [2, 3, 4, u32::MAX] {
        assert!(bindings.runtime_index_parameter(position).is_err());
    }
    let mut bindings = ScalarBindings::new([unsigned]);
    bindings
        .initialize_parameter(symbols::SymbolHandle::from_arena_index(1), unsigned, 0)
        .unwrap();
    assert!(bindings.runtime_index_parameter(0).is_err());
    bindings
        .append(CheckedScalarBindingDestination::Immutable, unsigned, 1)
        .unwrap();
    assert!(
        bindings.runtime_index_parameter(1).is_err(),
        "locals cannot impersonate parameters"
    );
}

#[test]
fn nested_selectors_remap_each_scalar_operand_once() {
    let scalar_type = terminal_scalar_type(PrimitiveType::U64).unwrap();
    let bindings = ScalarBindings::for_computation_operands(9, [scalar_type, scalar_type]);
    let mut expression = CheckedScalarExpression::StructuralParameterIndexedRead {
        root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
        path: Vec::new(),
        index: Box::new(CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U64,
        }),
        inner_indexes: vec![CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U64,
        }],
        element_path: Vec::new(),
        primitive_type: PrimitiveType::I32,
    };
    bindings.scalar(&mut expression).unwrap();
    let CheckedScalarExpression::StructuralParameterIndexedRead {
        index,
        inner_indexes,
        ..
    } = expression
    else {
        panic!("read shape is retained");
    };
    assert!(matches!(
        *index,
        CheckedScalarExpression::Parameter { position: 9, .. }
    ));
    assert!(matches!(
        inner_indexes[0],
        CheckedScalarExpression::Parameter { position: 10, .. }
    ));
}

#[test]
fn closed_record_projections_replay_exact_sources_carriers_and_all_siblings() {
    use checked_trees::expression::ExpressionNode;
    let source = "data Config [copy] { size: u64; enabled: bool; }
        data Foreign [copy] { size: u64; enabled: bool; }
        data Outer [copy] { config: Config; bytes: [u8; 1]; }
        const CONFIG: Config = Config { size: 7, enabled: true };
        const NESTED: Outer = Outer { config: Config { size: 7, enabled: true }, bytes: [5] };
        machine read() -> u64 { CONFIG.size }
        machine donor() -> u64 { CONFIG.size }
        machine projected_with_parameter(other: Config) -> u64 { CONFIG.size }
        machine nested() -> u64 { NESTED.config.size + 0u64 }
        machine boolean() -> bool { !CONFIG.enabled }
        machine sibling() -> bool { true }
        machine caller() -> bool { sibling() }";
    let original = crate::front_end::checked_program(source);
    let state = |name: &str| {
        let machine = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("machine");
        original.machine_states(machine)[0].symbol
    };
    let read_state = state("read");
    let read_machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .unwrap();
    let read_entry = &original.machine_states(read_machine)[0];
    let checked_trees::statement::StatementNode::Expression(read_source) = original
        .statement_table
        .statements(read_entry.statement_nodes)[0]
    else {
        panic!("read expression");
    };
    assert!(
        validation::closed_record_scalar_projection(&original.typed, read_source).is_some(),
        "authored constructor projection selects a closed scalar leaf"
    );
    let bindings = ScalarBindings::new([]);
    let validate = |checked: &CheckedTrees, state| {
        bindings.expression_at(checked, state, 0, CheckedScalarExpressionRole::Return)
    };
    for name in ["read", "donor", "nested", "boolean"] {
        validate(&original, state(name))
            .expect("closed projection composes through ordinary scalar replay");
    }
    let plans = &original.facts.values.scalar_expressions;
    let (source_binding, retained) = plans
        .bound_expression_at(read_state, 0, CheckedScalarExpressionRole::Return)
        .expect("source-bound projection");
    assert!(
        matches!(retained, CheckedScalarExpression::IntegerLiteral { literal }
        if literal.value_u64() == Some(7)
        && literal.landing().unwrap().landed_type == numerics::literals::LandedIntegerType::U64)
    );
    let parameter_state = state("projected_with_parameter");
    let (parameter_binding, parameter_retained) = plans
        .bound_expression_at(parameter_state, 0, CheckedScalarExpressionRole::Return)
        .expect("projection with unrelated structural formal");
    let validate_parameter = |checked: &CheckedTrees, retained: &CheckedScalarExpression| {
        crate::expression_preparation::source_custody::validate_storage_read_expression(
            checked,
            parameter_state,
            0,
            parameter_binding.expression,
            retained,
        )
    };
    validate_parameter(&original, parameter_retained).expect("literal retains source custody");
    let substituted_field = CheckedScalarExpression::StructuralParameterField {
        parameter_position: 0,
        path: vec![checked_trees::CheckedStructuralPredicatePathSegment::Field(
            "size".into(),
        )],
        primitive_type: PrimitiveType::U64,
    };
    assert!(
        validate_parameter(&original, &substituted_field).is_err(),
        "an unrelated valid structural formal cannot replace a projected literal"
    );
    let mut changed = original.clone();
    let ExpressionNode::Member(parameter_member) = original
        .expression_table
        .expression(parameter_binding.expression)
    else {
        panic!("parameter machine projection");
    };
    *changed
        .typed
        .expression_table
        .expression_mut(parameter_member.receiver) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(7));
    assert!(
        validate_parameter(&changed, &substituted_field).is_err(),
        "receiver corruption cannot turn a value projection into a structural read"
    );
    let mut changed = original.clone();
    changed
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter_mut()
        .find(|expression| {
            expression.state == state("nested")
                && expression.role == CheckedScalarExpressionRole::Return
        })
        .expect("nested arithmetic plan")
        .expression = retained.clone();
    assert!(
        validate(&changed, state("nested")).is_err(),
        "a same-valued literal cannot replace the member's enclosing computation"
    );
    let mut changed = original.clone();
    let nested = changed
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter_mut()
        .find(|expression| {
            expression.state == state("nested")
                && expression.role == CheckedScalarExpressionRole::Return
        })
        .expect("nested arithmetic plan");
    let CheckedScalarExpression::IntegerBinary { kind, .. } = &mut nested.expression else {
        panic!("nested addition");
    };
    *kind = checked_trees::CheckedIntegerBinaryKind::ExactMultiply;
    assert!(
        validate(&changed, state("nested")).is_err(),
        "projection replay preserves its enclosing operator"
    );
    let expression = source_binding.expression;
    let ExpressionNode::Member(member) = original.expression_table.expression(expression) else {
        panic!("member");
    };
    let constructor = member.receiver;
    let ExpressionNode::StructLiteral(literal) = original.expression_table.expression(constructor)
    else {
        panic!("constructor");
    };
    let constructor_fields = literal.fields;
    let actuals = original.expression_table.struct_fields(constructor_fields);
    let selected = actuals
        .iter()
        .find(|actual| actual.name.as_str() == "size")
        .unwrap()
        .value;
    let sibling = actuals
        .iter()
        .find(|actual| actual.name.as_str() == "enabled")
        .unwrap()
        .value;
    let foreign = original
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Foreign")
        .unwrap();
    let checked_trees::data::DataMember::Field(foreign_field) = &original.data_members(foreign)[0]
    else {
        panic!("field");
    };

    for field_symbol in [symbols::SymbolHandle::invalid(), foreign_field.symbol] {
        let mut changed = original.clone();
        let ExpressionNode::Member(member) =
            changed.typed.expression_table.expression_mut(expression)
        else {
            panic!("member");
        };
        member.member_symbol = field_symbol;
        assert!(
            validate(&changed, read_state).is_err(),
            "invalid or foreign field cannot disable replay"
        );
    }
    let mut changed = original.clone();
    let ExpressionNode::StructLiteral(literal) =
        changed.typed.expression_table.expression_mut(constructor)
    else {
        panic!("constructor");
    };
    literal.type_symbol = foreign.symbol;
    assert!(
        validate(&changed, read_state).is_err(),
        "same-layout constructor substitution rejects"
    );

    let mut changed = original.clone();
    *changed.typed.expression_table.expression_mut(constructor) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(7));
    assert!(
        validate(&changed, read_state).is_err(),
        "replacing the constructor cannot disable projection replay"
    );

    let mut changed = original.clone();
    *changed.typed.expression_table.expression_mut(selected) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(8));
    assert!(
        validate(&changed, read_state).is_err(),
        "selected value must match retained value"
    );

    let call = original
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| matches!(node, ExpressionNode::Call(_)).then_some(node.clone()))
        .unwrap();
    let mut changed = original.clone();
    *changed.typed.expression_table.expression_mut(sibling) = call;
    assert!(
        validate(&changed, read_state).is_err(),
        "unselected call cannot disappear"
    );

    let mut changed = original.clone();
    let mut stale = actuals[0].clone();
    stale.value = checked_trees::expression::ExpressionHandle::from_parts(
        stale.value.arena_index(),
        stale.value.generation() + 1,
    );
    changed
        .typed
        .expression_table
        .set_struct_field_at_offset(constructor_fields, 0, stale);
    assert!(
        validate(&changed, read_state).is_err(),
        "stale constructor value rejects"
    );

    let mut changed = original.clone();
    let ExpressionNode::StructLiteral(changed_literal) =
        changed.typed.expression_table.expression_mut(constructor)
    else {
        panic!("constructor");
    };
    changed_literal.fields = arena::HandleSpan::from_parts(
        arena::Handle::from_parts(
            constructor_fields.start().arena_index(),
            constructor_fields.start().generation() + 1,
        ),
        constructor_fields.count(),
    );
    assert!(
        validate(&changed, read_state).is_err(),
        "stale constructor field span rejects"
    );

    let donor = plans
        .bound_expression_at(state("donor"), 0, CheckedScalarExpressionRole::Return)
        .unwrap()
        .0
        .expression;
    assert_ne!(donor, expression);
    let mut changed = original.clone();
    let row = changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find_map(|(handle, binding)| {
            (binding.state == read_state && binding.role == CheckedScalarExpressionRole::Return)
                .then_some(handle)
        })
        .unwrap();
    changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .get_mut(row)
        .expression = donor;
    assert!(
        validate(&changed, read_state).is_err(),
        "equal-valued same-typed source occurrence cannot substitute"
    );
}

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
    let local = |symbol, argument| super::StructuralLocalBinding {
        root: facts::PlaceRoot::Symbol(symbol),
        argument,
        structural_type: semantic_vocabulary::StructuralTypeId::new(7).unwrap(),
    };
    let bindings = ScalarBindings::new([]).with_structural_locals(&[local(symbol, source.clone())]);
    for _ in 0..2 {
        assert_eq!(bindings.owned_argument(&argument).unwrap(), source);
    }
    for locals in [
        vec![],
        vec![local(other, source.clone())],
        vec![local(symbol, source.clone()), local(symbol, source.clone())],
        vec![local(
            symbol,
            StructuralArgument {
                access: StructuralAccess::SharedBorrow,
                ..source.clone()
            },
        )],
        vec![local(
            symbol,
            StructuralArgument {
                path: vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
                ..source.clone()
            },
        )],
    ] {
        assert!(
            ScalarBindings::new([])
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
        ScalarBindings::new([])
            .with_structural_locals(&[local(invalid, source)])
            .owned_argument(&changed)
            .is_err()
    );
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
            ScalarBindings::new([])
                .with_primitive_storage(&storage)
                .expression(&boolean_read)
                .is_err()
        );
    }
    let invalid = symbols::SymbolHandle::invalid();
    let bindings =
        ScalarBindings::new([]).with_primitive_storage(&[(invalid, source, ScalarType::Boolean)]);
    assert!(
        bindings
            .expression(&CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::StorageRead { symbol: invalid },
            )))
            .is_err()
    );

    // A matching old SSA type cannot rescue a wrong primitive place type.
    let mut bindings = ScalarBindings::new([]);
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
fn mutable_parameters_have_current_storage_without_an_immutable_entry_alias() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let mut bindings = ScalarBindings::new([ScalarType::Boolean; 3]);
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
fn storage_mapping_rejects_missing_duplicate_and_wrong_type_custody() {
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let other = symbols::SymbolHandle::from_arena_index(2);
    let integer = terminal_scalar_type(typed_trees::types::PrimitiveType::U8).unwrap();
    let mut bindings = ScalarBindings::new([]);
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

#[test]
fn borrowed_structural_operands_cannot_upgrade_emitted_source_access() {
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    };
    use semantic_vocabulary::StructuralTypeId;
    let checked = crate::front_end::checked_program("machine value() -> bool { true }");
    let machine = &checked.machines()[0];
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let place = PlaceId::new(41).unwrap();
    for local in [false, true] {
        for access in [
            StructuralAccess::Owned,
            StructuralAccess::MutableBorrow,
            StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let bindings = if local {
                ScalarBindings::new([]).with_structural_locals(&[super::StructuralLocalBinding {
                    root: facts::PlaceRoot::Symbol(symbol),
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    argument: StructuralArgument {
                        place,
                        path: Vec::new(),
                        access,
                    },
                }])
            } else {
                ScalarBindings::new([]).with_structural_parameters(&[(
                    0,
                    super::StructuralParameterDeclaration {
                        place,
                        position: 0,
                        is_self: false,
                        structural_type: StructuralTypeId::new(1).unwrap(),
                        multiplicity: super::StructuralMultiplicity::Unrestricted,
                        access,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                    },
                )])
            };
            let mut argument = CheckedUnitStructuralArgumentPlan {
                source: if local {
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
                } else {
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
                },
                path: vec![CheckedUnitStructuralPathSegment::Field("child".into())],
                access: CheckedStructuralAccess::MutableBorrow,
                ..Default::default()
            };
            let lowered = bindings.borrowed_structural_argument(&argument, machine, &[]);
            if matches!(
                access,
                StructuralAccess::Owned | StructuralAccess::MutableBorrow
            ) {
                assert_eq!(
                    lowered.unwrap(),
                    StructuralArgument {
                        place,
                        path: vec![terminal_psi::StructuralPathSegment::Field("child".into())],
                        access: StructuralAccess::MutableBorrow,
                    }
                );
            } else {
                assert!(
                    lowered.is_err(),
                    "mutable borrow cannot use {access:?}, local={local}"
                );
            }
            assert!(
                bindings
                    .shared_structural_place(&argument, machine, &[])
                    .is_err()
            );
            assert!(
                bindings
                    .shared_structural_argument(&argument, machine, &[])
                    .is_err()
            );
            argument.access = CheckedStructuralAccess::SharedBorrow;
            assert_eq!(
                bindings
                    .shared_structural_argument(&argument, machine, &[])
                    .is_ok(),
                access != StructuralAccess::WriteOnlyBorrow,
                "shared observation preserves stored access, local={local}"
            );
        }
    }
}

/// A call result an assignment stores takes its binding-ordinal slot but no
/// authored slot, so a later local's authored `Local` read still names that
/// local. Resolving the read through binding ordinals would hand it the
/// stored call result instead.
#[test]
fn unnamed_call_results_keep_authored_local_reads_on_their_locals() {
    let integer = terminal_scalar_type(PrimitiveType::I32).unwrap();
    let mut bindings = ScalarBindings::new([integer]);
    // Dense value 1 is a call result stored into a field; value 2 is the
    // next declared immutable local.
    bindings.append_unnamed_result(1);
    bindings
        .append(CheckedScalarBindingDestination::Immutable, integer, 2)
        .unwrap();
    // Binding ordinals count both results after the formal.
    assert_eq!(bindings.immutable_position(1).unwrap(), 1);
    assert_eq!(bindings.immutable_position(2).unwrap(), 2);
    // The declared local is the first authored slot after the formal.
    let mut local = CheckedScalarExpression::Local {
        position: 1,
        primitive_type: PrimitiveType::I32,
    };
    bindings.scalar(&mut local).unwrap();
    assert_eq!(
        local,
        CheckedScalarExpression::Local {
            position: 2,
            primitive_type: PrimitiveType::I32,
        }
    );
    let mut boolean = CheckedBooleanExpression::Local { position: 1 };
    bindings.boolean(&mut boolean).unwrap();
    assert_eq!(boolean, CheckedBooleanExpression::Local { position: 2 });
    // No second local was declared, so authored slot 2 does not exist.
    assert!(
        bindings
            .scalar(&mut CheckedScalarExpression::Local {
                position: 2,
                primitive_type: PrimitiveType::I32,
            })
            .is_err()
    );
    // The formal keeps its own slot.
    let mut formal = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::I32,
    };
    bindings.scalar(&mut formal).unwrap();
    assert_eq!(
        formal,
        CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::I32,
        }
    );
}

#[test]
fn structural_local_types_preserve_reference_shells_and_reject_conflicting_carriers() {
    use super::StructuralLocalBinding;
    use super::structural_paths::{PreparedStructuralPathSegment, prepare_argument_path};
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let place = PlaceId::new(41).unwrap();
    let record_type = StructuralTypeId::new(3).unwrap();
    let reference_type = StructuralTypeId::new(7).unwrap();
    let local = StructuralLocalBinding {
        root: facts::PlaceRoot::Symbol(symbol),
        argument: StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        },
        structural_type: reference_type,
    };
    let types = [
        StructuralTypeDeclaration {
            id: record_type,
            identity: "record".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
        StructuralTypeDeclaration {
            id: reference_type,
            identity: "reference".into(),
            shape: StructuralTypeShape::Reference {
                referent: record_type,
                access: StructuralAccess::SharedBorrow,
            },
        },
    ];
    let bindings = ScalarBindings::new([]).with_structural_locals(std::slice::from_ref(&local));
    assert_eq!(bindings.structural_place_type(place), Some(reference_type));
    let path = [checked_trees::CheckedUnitStructuralPathSegment::Referent];
    assert_eq!(
        prepare_argument_path(
            &path,
            bindings.structural_place_type(place).unwrap(),
            &types
        )
        .unwrap(),
        [PreparedStructuralPathSegment::Static(
            terminal_psi::StructuralPathSegment::Referent
        )]
    );
    assert!(
        prepare_argument_path(&path, record_type, &types).is_err(),
        "access cannot substitute the referent for the produced reference carrier"
    );
    assert!(
        prepare_argument_path(&path, reference_type, &types[..1]).is_err(),
        "the exact published carrier must exist in this catalog"
    );
    let conflicting = StructuralLocalBinding {
        structural_type: record_type,
        ..local.clone()
    };
    assert_eq!(
        ScalarBindings::new([])
            .with_structural_locals(&[local, conflicting])
            .structural_place_type(place),
        None
    );
    assert_eq!(
        bindings.structural_place_type(PlaceId::new(42).unwrap()),
        None
    );
    let mut owned = ScalarBindings::new([]);
    owned
        .establish_structural_local(symbol, place, record_type)
        .unwrap();
    assert_eq!(owned.structural_place_type(place), Some(record_type));
    owned.retire_structural_local(symbol, place).unwrap();
    assert_eq!(owned.structural_place_type(place), None);
}

#[test]
fn argument_projection_retains_indexed_scalar_endpoint_and_every_reference_access() {
    use super::structural_paths::{
        PreparedArgumentTip, PreparedStructuralPathSegment, prepare_argument_path,
        prepare_argument_projection,
    };
    use checked_trees::CheckedUnitStructuralPathSegment as CheckedSegment;
    use semantic_vocabulary::{IntegerSign, IntegerType, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    let id = |value| StructuralTypeId::new(value).unwrap();
    let hits_type = StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
    ));
    let mut types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "shared_reference".into(),
            shape: StructuralTypeShape::Reference {
                referent: id(2),
                access: StructuralAccess::SharedBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "mutable_reference".into(),
            shape: StructuralTypeShape::Reference {
                referent: id(3),
                access: StructuralAccess::MutableBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: id(3),
            identity: "board".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: crate::terminal_identities::structural_field_id(1),
                    identity: "tallies".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(id(4)),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: id(4),
            identity: "tallies".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id(5),
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: id(5),
            identity: "tally".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: crate::terminal_identities::structural_field_id(2),
                    identity: "hits".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: hits_type.clone(),
                }],
            },
        },
    ];
    let mut path = vec![
        CheckedSegment::Referent,
        CheckedSegment::Referent,
        CheckedSegment::Field("tallies".into()),
        CheckedSegment::FixedIndex(1),
        CheckedSegment::Field("hits".into()),
    ];
    let projection = prepare_argument_projection(&path, id(1), &types).unwrap();
    assert!(matches!(projection.tip, PreparedArgumentTip::Inline(field) if *field == hits_type));
    // The final mutable reference must not hide the earlier shared crossing.
    // Resolving this geometry alone therefore cannot authorize a mutable loan.
    assert_eq!(
        projection.crossed_accesses,
        [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow
        ]
    );
    assert_eq!(
        projection.path,
        [
            StructuralPathSegment::Referent,
            StructuralPathSegment::Referent,
            StructuralPathSegment::Field("tallies".into()),
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Field("hits".into()),
        ]
        .map(PreparedStructuralPathSegment::Static)
    );
    assert_eq!(
        prepare_argument_path(&path, id(1), &types).unwrap(),
        projection.path
    );
    let direct = prepare_argument_projection(&path[2..], id(3), &types).unwrap();
    assert!(matches!(direct.tip, PreparedArgumentTip::Inline(field) if *field == hits_type));
    assert!(direct.crossed_accesses.is_empty());

    path[3] = CheckedSegment::FixedIndex(2);
    assert!(prepare_argument_projection(&path, id(1), &types).is_err());
    path[3] = CheckedSegment::FixedIndex(1);
    assert!(
        prepare_argument_projection(&path, id(1), &types[..4]).is_err(),
        "an array's element must retain its exact declaration"
    );
    let StructuralTypeShape::Record { fields } = &mut types[4].shape else {
        unreachable!()
    };
    fields[0].relevance = BindingRelevance::Erased;
    assert!(
        prepare_argument_projection(&path, id(1), &types).is_err(),
        "an erased field never becomes an executable scalar endpoint"
    );
}

#[test]
fn primitive_reference_arguments_keep_the_carrier_and_exact_referent_projection() {
    use super::StructuralLocalBinding;
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    };
    use semantic_vocabulary::StructuralTypeId;
    use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let place = PlaceId::new(41).unwrap();
    let scalar_type = terminal_scalar_type(PrimitiveType::U32).unwrap();
    let primitive = StructuralTypeId::new(2).unwrap();
    let reference = StructuralTypeId::new(3).unwrap();
    let types = vec![
        StructuralTypeDeclaration {
            id: primitive,
            identity: "u32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        },
        StructuralTypeDeclaration {
            id: reference,
            identity: "&u32".into(),
            shape: StructuralTypeShape::Reference {
                referent: primitive,
                access: StructuralAccess::SharedBorrow,
            },
        },
    ];
    let local = StructuralLocalBinding {
        root: facts::PlaceRoot::Symbol(symbol),
        argument: StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::SharedBorrow,
        },
        structural_type: reference,
    };
    let argument = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
        path: vec![CheckedUnitStructuralPathSegment::Referent],
        type_identity: "u32".into(),
        access: CheckedStructuralAccess::SharedBorrow,
    };
    let bindings = ScalarBindings::new([]).with_structural_locals(std::slice::from_ref(&local));
    let lowered = bindings
        .primitive_borrow(&argument, scalar_type, &types)
        .unwrap();
    assert_eq!(lowered.place, place);
    assert_eq!(lowered.path, [StructuralPathSegment::Referent]);
    assert_eq!(lowered.access, StructuralAccess::SharedBorrow);
    assert!(
        bindings
            .primitive_borrow(
                &argument,
                terminal_scalar_type(PrimitiveType::F32).unwrap(),
                &types
            )
            .is_err()
    );
    let mut malformed = argument.clone();
    malformed.path.clear();
    assert!(
        bindings
            .primitive_borrow(&malformed, scalar_type, &types)
            .is_err()
    );
    malformed = argument.clone();
    malformed.access = CheckedStructuralAccess::MutableBorrow;
    assert!(
        bindings
            .primitive_borrow(&malformed, scalar_type, &types)
            .is_err()
    );
    assert!(
        bindings
            .primitive_borrow(&argument, scalar_type, &types[..1])
            .is_err()
    );
    let mut duplicate_types = types.clone();
    duplicate_types.push(types[1].clone());
    assert!(
        bindings
            .primitive_borrow(&argument, scalar_type, &duplicate_types)
            .is_err()
    );
    assert!(
        ScalarBindings::new([])
            .primitive_borrow(&argument, scalar_type, &types)
            .is_err()
    );
    assert!(
        ScalarBindings::new([])
            .with_structural_locals(&[local.clone(), local.clone()])
            .primitive_borrow(&argument, scalar_type, &types)
            .is_err()
    );
    // A shared primitive join still presents its existing primitive home. It
    // must not manufacture a reference projection from the requested type.
    let direct = StructuralLocalBinding {
        structural_type: primitive,
        ..local
    };
    let direct_bindings = ScalarBindings::new([]).with_structural_locals(&[direct]);
    assert!(
        direct_bindings
            .primitive_borrow(&argument, scalar_type, &types)
            .is_err()
    );
    malformed = argument;
    malformed.path.clear();
    assert_eq!(
        direct_bindings
            .primitive_borrow(&malformed, scalar_type, &types)
            .unwrap()
            .path,
        Vec::<StructuralPathSegment>::new()
    );
}

#[test]
fn computed_structural_storage_requires_exact_live_identity_and_custody() {
    use super::StructuralLocalBinding;
    use checked_trees::{
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan,
    };
    let checked = crate::front_end::checked_program("machine value() -> bool { true }");
    let machine = &checked.machines()[0];
    let expression = typed_trees::expression::ExpressionHandle::from_arena_index(1);
    let other = typed_trees::expression::ExpressionHandle::from_arena_index(2);
    let symbol = symbols::SymbolHandle::from_arena_index(1);
    let place = PlaceId::new(41).unwrap();
    let structural_type = semantic_vocabulary::StructuralTypeId::new(7).unwrap();
    let source = StructuralLocalBinding {
        root: facts::PlaceRoot::Expression(expression),
        argument: StructuralArgument {
            place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
        structural_type,
    };
    let argument = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression },
        access: CheckedStructuralAccess::SharedBorrow,
        ..Default::default()
    };
    let bindings = ScalarBindings::new([]).with_structural_locals(&[source.clone()]);
    assert_eq!(
        bindings
            .shared_structural_place(&argument, machine, &[])
            .unwrap(),
        place
    );
    for locals in [
        vec![],
        vec![StructuralLocalBinding {
            root: facts::PlaceRoot::Expression(other),
            ..source.clone()
        }],
        vec![StructuralLocalBinding {
            root: facts::PlaceRoot::Symbol(symbol),
            ..source.clone()
        }],
        vec![source.clone(), source.clone()],
        vec![StructuralLocalBinding {
            argument: StructuralArgument {
                access: StructuralAccess::WriteOnlyBorrow,
                ..source.argument.clone()
            },
            ..source.clone()
        }],
        vec![
            source.clone(),
            StructuralLocalBinding {
                root: facts::PlaceRoot::Symbol(symbol),
                structural_type: semantic_vocabulary::StructuralTypeId::new(8).unwrap(),
                ..source.clone()
            },
        ],
    ] {
        assert!(
            ScalarBindings::new([])
                .with_structural_locals(&locals)
                .shared_structural_place(&argument, machine, &[])
                .is_err()
        );
    }
    let invalid = CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
            expression: typed_trees::expression::ExpressionHandle::invalid(),
        },
        ..argument.clone()
    };
    let invalid_source = StructuralLocalBinding {
        root: facts::PlaceRoot::Expression(typed_trees::expression::ExpressionHandle::invalid()),
        ..source.clone()
    };
    assert!(
        ScalarBindings::new([])
            .with_structural_locals(&[invalid_source])
            .shared_structural_place(&invalid, machine, &[])
            .is_err()
    );
    let mut aliases = ScalarBindings::new([]).with_structural_locals(&[
        source.clone(),
        StructuralLocalBinding {
            root: facts::PlaceRoot::Symbol(symbol),
            ..source
        },
    ]);
    aliases.retire_structural_local(symbol, place).unwrap();
    assert!(
        aliases
            .shared_structural_place(&argument, machine, &[])
            .is_err()
    );
}
