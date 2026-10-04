use super::{FloatInterval, Interval, ValueEnvironment};

#[test]
fn copied_local_bounds_survive_an_unresolved_returned_reference_write() {
    for (initialization, accepted) in [
        ("let index: u64 = 15;", true),
        ("let index: u64 = bounded();", true),
        (
            "level.count = 15; let index: u64 = level.count; level.count = 30;",
            true,
        ),
        ("let index: u64 = 18446744073709551615;", false),
    ] {
        let program = crate::front_end::typed_program(&format!(
            "data CellId [copy] {{ case First; case Second; }}
             data Room {{ value: u64; }}
             data Level {{ rooms: [Room; 2]; count: u64; }}
             machine bounded() -> u64 ensures result <= 15 {{ 15 }}
             machine Level::room_mut(&mut self, cell: CellId) -> &mut Room {{
                 let slots: &mut [Room] = self.rooms.as_mut_slice();
                 transition cell {{
                     CellId::First -> &mut slots[0]
                     _ -> &mut slots[1]
                 }}
             }}
             machine exercise(level: &mut Level, cell: CellId) {{
                 {initialization}
                 let room: &mut Room = level.room_mut(cell);
                 room.value = 1;
                 level.count = max(level.count, index + 1);
             }}"
        ));
        let result = crate::validate_program(&program);
        if accepted {
            result.unwrap_or_else(|diagnostics| panic!("{initialization}: {diagnostics:?}"));
        } else {
            let diagnostics = result.expect_err("a saved maximum value cannot be incremented");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("may overflow `u64`")),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn unknown_writes_retain_only_existing_immutable_scalar_snapshots() {
    let program = crate::front_end::typed_program(
        "data Pair { value: u64; }
         machine inspect(input: u64, pointer: &mut u64) {
             let saved: u64 = pointer;
             let mut changing: u64 = 1;
             let borrowed: &mut u64 = pointer;
             let aggregate: Pair = Pair { value: 1 };
             let fraction: f64 = 1.0;
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut environment = ValueEnvironment::new();
    for name in [
        "input",
        "pointer",
        "saved",
        "changing",
        "borrowed",
        "aggregate",
        "aggregate.value",
        "unknown",
    ] {
        environment.set(name.to_owned(), Interval::constant(15));
        environment.mark_known_u64(name.to_owned(), 15);
    }
    // The source storage's current value differs from the saved copy. Retaining
    // a snapshot must not re-evaluate `saved`'s initializer through this pointer.
    environment.set("pointer".to_owned(), Interval::constant(255));
    let floating = FloatInterval {
        low: Some(1.0),
        high: Some(1.0),
    };
    environment
        .float_intervals
        .insert("fraction".to_owned(), floating);
    environment
        .float_intervals
        .insert("aggregate.value".to_owned(), floating);
    environment
        .non_nan
        .extend(["fraction".to_owned(), "aggregate.value".to_owned()]);
    environment
        .joint_add_upper_bounds
        .insert(("saved".to_owned(), "pointer".to_owned()));

    use super::super::ordered_values::{Operand, Relation};
    let ceiling = Operand::Integer(numerics::literals::IntegerLiteral::from_value(255), None);
    for parameter in program.state_parameters(state) {
        environment.ordered_values.push(Relation {
            left: ceiling.clone(),
            right: Operand::parameter(parameter),
            constraint: super::super::ordered_values::RelationConstraint::MinimumDifference(1),
        });
    }
    for statement in program.statement_table.statements(state.statement_nodes) {
        let typed_trees::statement::StatementNode::LocalData(local) = statement else {
            continue;
        };
        if local.name.as_str() == "fraction" {
            continue;
        }
        environment.ordered_values.push(Relation {
            left: ceiling.clone(),
            right: Operand::Place {
                root: local.symbol,
                fields: Vec::new(),
                path: local.name.as_str().to_owned(),
            },
            constraint: super::super::ordered_values::RelationConstraint::MinimumDifference(1),
        });
    }
    // Matching display text cannot authorize an absent declaration or a field
    // projection. Unknown writes must discard both forged snapshot claims.
    let input = program.state_parameters(state)[0].symbol;
    for (root, fields) in [
        (symbols::SymbolHandle::invalid(), Vec::new()),
        (input, vec![input]),
        (machine.symbol, Vec::new()),
    ] {
        environment.ordered_values.push(Relation {
            left: ceiling.clone(),
            right: Operand::Place {
                root,
                fields,
                path: "input".to_owned(),
            },
            constraint: super::super::ordered_values::RelationConstraint::MinimumDifference(1),
        });
    }

    environment.invalidate_unknown_writes(&program, Some(state));
    assert_eq!(environment.get("saved"), Some(Interval::constant(15)));
    assert_eq!(environment.get("input"), Some(Interval::constant(15)));
    assert_eq!(environment.intervals.len(), 2);
    assert_eq!(environment.known_u64_values.len(), 2);
    assert_eq!(environment.float_intervals.get("fraction"), Some(&floating));
    assert_eq!(environment.float_intervals.len(), 1);
    assert_eq!(environment.non_nan.len(), 1);
    assert!(environment.joint_add_upper_bounds.is_empty());
    assert_eq!(environment.ordered_values.len(), 2);
    assert!(environment.ordered_values.iter().all(|relation| {
        matches!(&relation.right, Operand::Place { path, .. } if path == "input" || path == "saved")
    }));

    let retained = environment.clone();
    environment.invalidate_written_paths(&["input".to_owned()]);
    assert_eq!(environment.ordered_values.len(), 1);
    environment = environment.join(&ValueEnvironment::new());
    assert!(environment.ordered_values.is_empty());

    environment = retained;
    environment.invalidate_unknown_writes(&program, None);
    assert_eq!(environment, ValueEnvironment::new());
}
