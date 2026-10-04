mod construction;

use super::*;
use crate::proof_contracts::arithmetic_domains::{
    requires_value_environment, validate_value_range,
};
use layout_plans::{IntegerInterpretation, LayoutPlacementReport};
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::data::DataMember;
use typed_trees::statement::StatementNode;

fn program_with_storage(source: &str, width: u16, sign: IntegerSign) -> TypedTrees {
    let mut program = crate::front_end::typed_program(source);
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Cell")
        .unwrap();
    let symbol = data.symbol;
    let [DataMember::Field(field)] = program.data_members(data) else {
        panic!("one field");
    };
    let field = field.symbol;
    let interpretation = match sign {
        IntegerSign::Signed => IntegerInterpretation::Signed,
        IntegerSign::Unsigned => IntegerInterpretation::Unsigned,
    };
    // This query consumes the exact normalized geometry published by layout
    // evaluation. The public recast corpus fixture exercises that producer.
    program.plan_laid_layouts.push(typed_trees::PlanLaidLayout {
        data_name: "Cell".to_owned(),
        data_symbol: symbol,
        field_symbols: vec![field],
        schema_symbol: symbol,
        schema_field_symbols: vec![field],
        policy_symbol: symbols::SymbolHandle::invalid(),
        policy_plan_machine_symbol: symbols::SymbolHandle::invalid(),
        validated_layout: layout_plans::LayoutPlanReport {
            schema_report_fingerprint: 0,
            entries: vec![layout_plans::LayoutFieldEntryReport {
                field: "value".to_owned(),
                member_identity: None,
                placement: LayoutPlacementReport::IntegerAt {
                    offset: 0,
                    stored_width: u64::from(width),
                    interpretation,
                },
            }],
            offsets: None,
            size: Some(u64::from(width / 8)),
            align: 1,
        },
        private_callback_demands: Vec::new(),
        offsets: vec![0],
        bit_fields: Vec::new(),
        integer_fields: vec![typed_trees::PlanLaidIntegerField {
            field_index: 0,
            stored_width_bits: width,
            interpretation,
            write_is_total: false,
        }],
        repeated_fields: Vec::new(),
        size: usize::from(width / 8),
        align: 1,
    });
    program
}

fn discharge(source: &str, width: u16, sign: IntegerSign) -> bool {
    let program = program_with_storage(source, width, sign);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "store")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let assignment = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
        .find_map(|statement| match statement {
            StatementNode::Assignment(assignment) => Some(assignment),
            _ => None,
        })
        .unwrap();
    let mut environment = requires_value_environment(&program, machine, state);
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let StatementNode::LocalData(local) = statement
            && program.primitive_type_reference(local.type_reference) == Some(PrimitiveType::Bool)
        {
            environment = crate::proof_contracts::arithmetic_domains::guard_narrowed_environment(
                &program,
                machine,
                Some(state),
                &typed_trees::statement::TransitionGuardNode::When(local.initial_value),
                &environment,
            );
        }
    }
    let mut diagnostics = Vec::new();
    let primitive = program.primitive_type_reference(places::assignment_value_type(
        &program,
        places::declared_place_type_raw(&program, machine, Some(state), assignment.target).unwrap(),
    ));
    let (interval, source, _) = validate_value_range(
        &program,
        machine,
        Some(state),
        assignment.value,
        &environment,
        primitive,
        ArithmeticDomain::Exact,
        "store",
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "RHS: {diagnostics:?}");
    let frames = crate::CallFrameResolver::new(&program).expect("typed call frames");
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .rev()
        .find(|statement| matches!(statement, StatementNode::Assignment(_)))
        .unwrap();
    let origins = frames.local_write_origins_before_statement(machine, statement);
    let result = validate_stored_integer_write(
        &program,
        machine,
        Some(state),
        assignment.target,
        assignment.value,
        origins.as_deref(),
        interval,
        source,
        &environment,
        "store",
        &mut diagnostics,
    );
    if let Some(result) = result.first() {
        assert_eq!(
            result.site,
            StoredIntegerWriteSite::Assignment {
                destination: assignment.target
            }
        );
        assert_eq!(result.value, assignment.value);
        assert_eq!(result.stored_type, IntegerType::new(sign, width).unwrap());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        true
    } else {
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("requires a value proven to fit")),
            "a missing geometry or skipped store is not a range rejection: {diagnostics:?}"
        );
        false
    }
}

#[test]
fn stored_integer_write_checks_signed_boundaries_and_unknown_values() {
    for (value, accepted) in [
        ("-129", false),
        ("-128", true),
        ("127", true),
        ("128", false),
        ("input", false),
    ] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: i64; }} machine store(cell: &mut Cell, input: i64) {{ cell.value = {value}; }}"
                ),
                8,
                IntegerSign::Signed
            ),
            accepted,
            "{value}"
        );
    }
}

#[test]
fn stored_integer_write_retains_current_parameter_bounds() {
    for (bound, accepted) in [("127", true), ("128", false)] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: i64; }} machine store(cell: &mut Cell, input: i64) requires 0 <= input, input <= {bound} {{ cell.value = input; }}"
                ),
                8,
                IntegerSign::Signed
            ),
            accepted
        );
    }
}

#[test]
fn stored_integer_write_retains_declared_rhs_range() {
    assert!(discharge(
        "data Cell { value: i64; } machine store(cell: &mut Cell, input: i64[0..=127]) { cell.value = input; }",
        8,
        IntegerSign::Signed,
    ));
}

#[test]
fn stored_integer_write_uses_the_selected_guard_environment() {
    assert!(discharge(
        "data Cell { value: i64; } machine store(cell: &mut Cell, input: i64) {
            let eligible: bool = 0 <= input && input <= 127;
            cell.value = input;
        }",
        8,
        IntegerSign::Signed,
    ));
}

#[test]
fn stored_integer_write_preserves_unsigned_64_boundaries() {
    for (width, sign, value, accepted) in [
        (8, IntegerSign::Unsigned, "255", true),
        (8, IntegerSign::Unsigned, "256", false),
        (64, IntegerSign::Unsigned, "18446744073709551615", true),
        (64, IntegerSign::Signed, "18446744073709551615", false),
        (64, IntegerSign::Unsigned, "input", true),
        (64, IntegerSign::Signed, "input", false),
    ] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: u64; }} machine store(cell: &mut Cell, input: u64) {{ cell.value = {value}; }}"
                ),
                width,
                sign
            ),
            accepted,
            "{value}/{width}/{sign:?}"
        );
    }
}

#[test]
fn stored_integer_write_cannot_use_storage_width_as_an_unknown_rhs_bound() {
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert!(!super::interval_fits(byte, Interval::UNBOUNDED, None));
    assert!(!super::interval_fits(
        byte,
        Interval::UNBOUNDED,
        Some(PrimitiveType::U64)
    ));
    assert!(super::interval_fits(
        byte,
        Interval::UNBOUNDED,
        Some(PrimitiveType::U8)
    ));
}

#[test]
fn stored_integer_write_checks_local_and_forwarded_loans() {
    for declarations in [
        "let leaf: &mut i64 = &mut cell.value;",
        "let first: &mut i64 = &mut cell.value; let leaf: &mut i64 = &mut first;",
    ] {
        for (value, accepted) in [(127, true), (128, false)] {
            assert_eq!(
                discharge(
                    &format!(
                        "data Cell {{ value: i64; }} machine store(cell: &mut Cell) {{ {declarations} leaf = {value}; }}"
                    ),
                    8,
                    IntegerSign::Signed
                ),
                accepted,
                "{declarations} / {value}"
            );
        }
    }
}

#[test]
fn stored_integer_write_keeps_encoded_view_geometry_through_a_field_loan() {
    for (value, accepted) in [(127, true), (128, false)] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: i64; }}
             machine store(bytes: &mut [u8; 1]) {{
                 let view: &mut Cell = &mut bytes[0] as &mut Cell;
                 let leaf: &mut i64 = &mut view.value;
                 leaf = {value};
             }}"
                ),
                8,
                IntegerSign::Signed
            ),
            accepted
        );
    }
}

#[test]
fn stored_integer_write_rebinding_keeps_the_current_field_origin() {
    // The original target is ordinary i64; the replacement is encoded i8.
    // Reusing the initializer's origin would silently admit the second value.
    for (value, accepted) in [(127, true), (128, false)] {
        assert_eq!(
            discharge(
                &format!(
                    "data Cell {{ value: i64; }}
             machine store(cell: &mut Cell, ordinary: &mut i64) {{
                 let mut leaf: &mut i64 = &mut ordinary;
                 leaf = &mut cell.value;
                 leaf = {value};
             }}"
                ),
                8,
                IntegerSign::Signed
            ),
            accepted
        );
    }
}

#[test]
fn stored_integer_write_preserves_all_candidate_encodings_and_unknown_origins() {
    let program = program_with_storage(
        "data Cell { value: i64; }
         machine store(cell: &mut Cell, ordinary: &mut i64) {
             let leaf: &mut i64 = &mut cell.value;
             leaf = 127;
         }",
        8,
        IntegerSign::Signed,
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap();
    let StatementNode::Assignment(assignment) = statement else {
        panic!("write");
    };
    let frames = crate::CallFrameResolver::new(&program).unwrap();
    let origins = frames
        .local_write_origins_before_statement(machine, statement)
        .unwrap();
    let encoded = origins
        .iter()
        .find(|origin| {
            origin
                .source
                .as_ref()
                .is_some_and(|source| !source.segments.is_empty())
        })
        .unwrap()
        .clone();
    let mut plain = encoded.clone();
    let source = plain.source.as_mut().unwrap();
    source.root = program.state_parameters(state)[1].symbol;
    source.segments.clear();
    source.representation = None;
    let projections = super::destination_projections(
        &program,
        machine,
        state,
        assignment.target,
        Some(&[plain, encoded.clone()]),
    )
    .expect("all known candidates remain represented");
    assert_eq!(
        projections.len(),
        1,
        "an ordinary candidate cannot hide the encoded one"
    );
    assert_eq!(projections[0].1.bits(), 8);
    let mut unknown = encoded.clone();
    unknown.source = None;
    assert!(
        super::destination_projections(
            &program,
            machine,
            state,
            assignment.target,
            Some(&[encoded, unknown]),
        )
        .is_err(),
        "an unknown alternative is not ordinary scalar storage"
    );
}

#[test]
fn unrelated_integer_at_does_not_change_returned_reference_store_admission() {
    assert_returned_record_field_is_ordinary(
        "data Cell { value: i64; }
         data CellId [copy] { case First; case Second; }
         data Room { value: u64; }
         data Level { rooms: [Room; 2]; count: u64; }
         machine Level::room_mut(level: &mut Level, cell: CellId) -> &mut Room {
             let slots: &mut [Room] = level.rooms.as_mut_slice();
             transition cell {
                 CellId::First -> &mut slots[0]
                 _ -> &mut slots[1]
             }
         }
         machine exercise(level: &mut Level, cell: CellId) {
             let room: &mut Room = Level::room_mut(level, cell);
             room.value = 1;
         }",
    );
}

#[test]
fn unrelated_integer_at_preserves_cyclic_returned_reference_field_store() {
    assert_returned_record_field_is_ordinary(
        "data Cell { value: i64; }
         data CellId [copy] { case First; case Second; }
         data Room { value: u64; }
         data Level { rooms: [Room; 2]; count: u64; }
         machine Level::room_mut(level: &mut Level, cell: CellId) -> &mut Room {
             transition { _ -> again(level, cell, 1) }
             state again(level: &mut Level, cell: CellId, remaining: u64) {
                 transition remaining > 0 {
                     true -> again(level, cell, remaining - 1)
                     _ -> select(level, cell)
                 }
             }
             state select(level: &mut Level, cell: CellId) {
                 let slots: &mut [Room] = level.rooms.as_mut_slice();
                 transition cell {
                     CellId::First -> &mut slots[0]
                     _ -> &mut slots[1]
                 }
             }
         }
         machine exercise(level: &mut Level, cell: CellId) {
             let room: &mut Room = Level::room_mut(level, cell);
             room.value = 1;
         }",
    );
}

fn assert_returned_record_field_is_ordinary(source: &str) {
    let program = program_with_storage(source, 8, IntegerSign::Signed);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap();
    let StatementNode::Assignment(assignment) = statement else {
        panic!("store");
    };
    let resolver = crate::CallFrameResolver::new(&program).unwrap();
    let origins = resolver.local_write_origins_before_statement(machine, statement);
    let diagnostic_origins = origins.as_ref().map(|origins| {
        origins
            .iter()
            .map(|origin| {
                (
                    origin.local_symbol,
                    origin.local_segments.clone(),
                    origin.source.as_ref().map(|source| {
                        (
                            source.root,
                            source.segments.clone(),
                            source.representation.clone(),
                        )
                    }),
                )
            })
            .collect::<Vec<_>>()
    });
    let result = super::destination_projections(
        &program,
        machine,
        state,
        assignment.target,
        origins.as_deref(),
    );
    assert_eq!(
        result,
        Ok(Vec::new()),
        "the returned Room has no encoded field; prefix origins: {diagnostic_origins:?}"
    );
}

#[test]
fn declaration_field_projection_does_not_cross_unknown_stored_references() {
    let program = program_with_storage(
        "data Cell { value: i64; }
         data Holder { cell: &mut Cell; }
         machine store(holder: &mut Holder) { holder.cell.value = 127; }",
        8,
        IntegerSign::Signed,
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap();
    let StatementNode::Assignment(assignment) = statement else {
        panic!("store");
    };
    assert!(
        super::destination_projections(&program, machine, state, assignment.target, None,).is_err(),
        "a stored reference needs its contents' origin, not the enclosing declaration"
    );
}

#[test]
fn stored_integer_source_geometry_authenticates_fields_and_recast_coordinates() {
    let program = program_with_storage(
        "data Cell { value: i64; }
         data Ordinary { value: i64; }
         machine inspect(cell: &mut Cell, ordinary: &mut Ordinary, raw: &mut [u8; 1]) {}",
        8,
        IntegerSign::Signed,
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let parameters = program.state_parameters(state);
    let field = |name: &str| {
        let data = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)
            .unwrap();
        let [DataMember::Field(field)] = program.data_members(data) else {
            panic!("field");
        };
        field.symbol
    };
    let mut source = crate::LocalWriteSource {
        representation: None,
        path: "not a source of declaration identity".to_owned(),
        collection_coarse: false,
        root: parameters[0].symbol,
        segments: Vec::new(),
    };
    let selected = [PlaceSegment::Field {
        symbol: field("Cell"),
    }];
    assert_eq!(
        stored_integer_source_projection(&program, machine, state, &source, &selected),
        Ok(Some((
            field("Cell"),
            IntegerType::new(IntegerSign::Signed, 8).unwrap()
        )))
    );
    assert!(
        stored_integer_source_projection(
            &program,
            machine,
            state,
            &source,
            &[PlaceSegment::Field {
                symbol: field("Ordinary")
            }]
        )
        .is_err()
    );
    source.collection_coarse = true;
    assert!(
        stored_integer_source_projection(&program, machine, state, &source, &selected).is_err()
    );
    source.collection_coarse = false;
    source.root = parameters[2].symbol;
    source.segments = vec![PlaceSegment::FixedIndex { index: 0 }];
    source.representation = Some((parameters[0].type_reference, selected.to_vec()));
    assert_eq!(
        stored_integer_source_projection(&program, machine, state, &source, &[]),
        Ok(Some((
            field("Cell"),
            IntegerType::new(IntegerSign::Signed, 8).unwrap()
        ))),
        "the recast's logical carrier selects encoding, not its raw byte backing"
    );
    source.root = SymbolHandle::invalid();
    assert!(stored_integer_source_projection(&program, machine, state, &source, &[]).is_err());
}

#[test]
fn stored_integer_source_effect_reach_includes_owned_descendants_and_unknown_referents() {
    let program = program_with_storage(
        "data Cell { value: i64; }
         data Ordinary { value: i64; }
         data Nested { cells: [Cell; 2]; }
         data Borrowed { value: &mut i64; }
         machine inspect(nested: &mut Nested, ordinary: &mut Ordinary, borrowed: &mut Borrowed) {}",
        8,
        IntegerSign::Signed,
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let sources = program
        .state_parameters(state)
        .iter()
        .map(|parameter| crate::LocalWriteSource {
            representation: None,
            path: String::new(),
            collection_coarse: false,
            root: parameter.symbol,
            segments: Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        stored_integer_source_projection(&program, machine, state, &sources[0], &[]),
        Ok(None)
    );
    assert_eq!(
        stored_integer_source_may_require_fit(&program, machine, state, &sources[0]),
        Ok(true),
        "a nonencoded aggregate itself can contain encoded integer leaves"
    );
    assert_eq!(
        stored_integer_source_may_require_fit(&program, machine, state, &sources[1]),
        Ok(false)
    );
    assert_eq!(
        stored_integer_source_may_require_fit(&program, machine, state, &sources[2]),
        Err(()),
        "a stored reference's semantic i64 type does not establish its referent encoding"
    );
    let mut total = program.clone();
    total.plan_laid_layouts[0].integer_fields[0].write_is_total = true;
    assert_eq!(
        stored_integer_source_may_require_fit(&total, machine, state, &sources[0]),
        Ok(false),
        "normalized total encoding requires no narrower concrete-value obligation"
    );
}
