use super::*;

fn predecessor() -> Machine {
    machine(
        vec![u32_type()],
        vec![returns(strict(
            Strict::Subtract,
            slot(0, u32_type()),
            unsigned(1),
        ))],
    )
}

fn guarded_calls(other: u128) -> Machine {
    machine(
        vec![u32_type()],
        vec![
            Statement::Arm {
                guard: Some(expression(
                    ExpressionKind::Strict {
                        operation: Strict::Less,
                        policy: None,
                        left: Box::new(unsigned(0)),
                        right: Box::new(slot(0, u32_type())),
                    },
                    Some(ScalarType::Boolean),
                )),
                target: Target::Value(call(2, vec![slot(0, u32_type())])),
                source: nowhere(),
            },
            returns(call(2, vec![unsigned(other)])),
        ],
    )
}

fn with_private_inputs(
    machines: Vec<Machine>,
    private: &[u64],
) -> typed_trees_to_terminal_psi::Elaboration {
    let mut elaboration = lower(machines).unwrap();
    elaboration.private_inputs = private.iter().copied().map(id).collect();
    elaboration
}

#[test]
fn every_private_arrival_establishes_the_inferred_input() {
    let RouteVerdict::Accepted(accepted) = judge(Ok(with_private_inputs(
        vec![guarded_calls(1), predecessor()],
        &[2],
    )))
    .unwrap() else {
        panic!("both callers establish a positive input")
    };
    assert_eq!(
        accepted.elaboration.module.machines[1]
            .contract
            .requires
            .len(),
        1
    );
    assert!(!verdicts(&accepted.elaboration, &accepted.discharge, 1, requirement).is_empty());
    for (machine, result) in accepted.discharge.machines.iter() {
        for (obligation, verdict) in result.verdicts.iter() {
            if let Ok(Discharged::Certificate(evidence)) = verdict {
                assert_kernel_accepted(&accepted.elaboration, machine.get(), obligation, evidence);
            }
        }
    }
}

#[test]
fn one_unsafe_private_arrival_is_not_hidden_by_a_guarded_arrival() {
    let RouteVerdict::Rejected { unproved, .. } = judge(Ok(with_private_inputs(
        vec![guarded_calls(0), predecessor()],
        &[2],
    )))
    .unwrap() else {
        panic!("the zero arrival cannot justify subtracting one")
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(2) && safety(&row.kind))
    );
}

#[test]
fn private_inputs_cross_an_omitted_helper_chain() {
    // Each forwarder has two possible positive inputs. The predecessor's
    // input joins those already-disjunctive conditions into one contract.
    let entry = machine(
        vec![],
        vec![
            Statement::discard(call(2, vec![unsigned(1)])),
            Statement::discard(call(2, vec![unsigned(2)])),
            Statement::discard(call(3, vec![unsigned(3)])),
            returns(call(3, vec![unsigned(4)])),
        ],
    );
    let forward = || {
        machine(
            vec![u32_type()],
            vec![returns(call(4, vec![slot(0, u32_type())]))],
        )
    };
    let verdict = judge(Ok(with_private_inputs(
        vec![entry, forward(), forward(), predecessor()],
        &[2, 3, 4],
    )))
    .unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("every forwarded input permits subtracting one: {verdict:?}")
    };
    for (machine, result) in accepted.discharge.machines.iter() {
        for (obligation, verdict) in result.verdicts.iter() {
            if let Ok(Discharged::Certificate(evidence)) = verdict {
                assert_kernel_accepted(&accepted.elaboration, machine.get(), obligation, evidence);
            }
        }
    }
    accepted
        .publish()
        .expect("the proved input contract must encode canonically");
}

fn judge_source_arrivals(source: &str, expected_private: usize) -> RouteVerdict {
    let typed = super::boundary_calls::typed(source);
    let entry = super::boundary_calls::entry(&typed);
    let elaboration = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        super::build_purpose::roster(&[entry], &[], source::DependencyScope::Product),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    assert_eq!(elaboration.private_inputs.len(), expected_private);
    judge(Ok(elaboration)).unwrap()
}

#[test]
fn view_extent_facts_follow_only_the_guarded_input_across_private_arrivals() {
    let source = r#"
machine entry(bytes: &[u8], other: &[u8], index: u64) -> u8 {
    transition index < bytes.len { true -> next(INPUT, index) false -> 255u8 }
    machine next(bytes: &[u8], index: u64) -> u8 {
        transition { _ -> fetch(bytes, index) }
    }
    machine fetch(bytes: &[u8], index: u64) -> u8 {
        bytes[index]
    }
OTHER_BODY
}
"#;
    let judge_input = |input, other_body, expected_private| {
        let source = source
            .replace("INPUT", input)
            .replace("OTHER_BODY", other_body);
        judge_source_arrivals(&source, expected_private)
    };
    let verdict = judge_input("bytes", "", 2);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the guarded descriptor reaches both helpers: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    for input in ["other", "&bytes[0..index]"] {
        let verdict = judge_input(input, "", 2);
        let RouteVerdict::Rejected { unproved, .. } = verdict else {
            panic!("the guard cannot justify indexing {input}: {verdict:?}");
        };
        assert!(
            unproved
                .iter()
                .any(|row| row.machine == id(3) && safety(&row.kind))
        );
    }

    let outside = "    machine unchecked(bytes: &[u8], index: u64) -> u8 { fetch(bytes, index) }";
    let verdict = judge_input("bytes", outside, 1);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("the unread sibling also calls fetch: {verdict:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(3) && safety(&row.kind))
    );
}

#[test]
fn private_arrival_retains_a_guard_on_the_computed_argument() {
    let source = r#"
machine entry(bytes: &[u8], index: u64) -> u8 {
    transition index < bytes.len { true -> lead(bytes, index) false -> 255u8 }
    machine lead(bytes: &[u8], index: u64) -> u8 {
        let first: u8 = bytes[index];
        transition first > 0 && first < 128 {
            true -> first
            false -> following(bytes, index, first)
        }
    }
    machine following(bytes: &[u8], index: u64, first: u8) -> u8 {
        let next: u64 = index + 1;
        transition first >= 194 && first <= 244 && next < bytes.len {
            true -> fetch(bytes, first, INPUT)
            false -> 255u8
        }
    }
    machine fetch(bytes: &[u8], first: u8, index: u64) -> u8 {
        bytes[index]
    }
}
"#;
    let verdict = judge_source_arrivals(&source.replace("INPUT", "next"), 3);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the checked computed index reaches the helper: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    let verdict = judge_source_arrivals(&source.replace("INPUT", "bytes.len"), 3);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("the original guard does not justify a different index: {verdict:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(4) && safety(&row.kind))
    );
}

#[test]
fn private_arrivals_preserve_the_guarded_field_of_an_owned_argument() {
    let source = r#"
data Character { point: u32; sibling: u32; }
machine entry(character: Character) -> u32 {
    transition character.GUARDED_FIELD < 65536 { true -> 0u32 false -> forward(character) }
    machine forward(character: Character) -> u32 {
        transition { _ -> encode(character) }
    }
    machine encode(character: Character) -> u32 {
        character.point - 65536
    }
}
"#;
    let verdict = judge_source_arrivals(&source.replace("GUARDED_FIELD", "point"), 2);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the field guard crosses both owned arguments: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    let verdict = judge_source_arrivals(&source.replace("GUARDED_FIELD", "sibling"), 2);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("guarding the sibling leaves the subtraction unproved: {verdict:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(3) && safety(&row.kind))
    );
}

#[test]
fn a_fixed_input_boundary_does_not_receive_its_callers_stronger_fact() {
    let entry = machine(vec![], vec![returns(call(2, vec![unsigned(2)]))]);
    let forward = machine(
        vec![u32_type()],
        vec![returns(call(3, vec![slot(0, u32_type())]))],
    );
    let RouteVerdict::Rejected { unproved, .. } = judge(Ok(with_private_inputs(
        vec![entry, forward, predecessor()],
        &[3],
    )))
    .unwrap() else {
        panic!("the complete forwarding contract permits zero")
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(3) && safety(&row.kind))
    );
}

#[test]
fn a_private_call_without_normal_return_still_owes_its_inferred_input() {
    let mut bodies = IdTable::default();
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![returns(call(2, vec![slot(0, u32_type())]))],
    });
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![
            Statement::discard(strict(Strict::Subtract, slot(0, u32_type()), unsigned(1))),
            Statement::Arm {
                guard: None,
                target: Target::Crash(semantic_vocabulary::CrashCause::Trap),
                source: nowhere(),
            },
        ],
    });
    let mut signatures = IdTable::default();
    for requires in [vec![at_least(1, 1)], Vec::new()] {
        signatures.push(Signature {
            operational: Default::default(),
            parameters: vec![u32_type().into()],
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(u32_type().into()),
            requires,
            ensures: Vec::new(),
            reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
            invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(trap_when(vec![terminal_psi::CrashRouteGuard::Truth])),
            termination: Termination::Silent,
            ranking: None,
        });
    }
    let mut elaboration =
        typed_trees_to_terminal_psi::lower_bodies(id(1), &bodies, &signatures, &IdTable::default())
            .unwrap();
    elaboration.private_inputs = vec![id(2)];
    let RouteVerdict::Accepted(accepted) = judge(Ok(elaboration)).unwrap() else {
        panic!("the non-returning call has a positive input")
    };
    assert!(!verdicts(&accepted.elaboration, &accepted.discharge, 1, requirement).is_empty());
}

#[test]
fn a_private_helper_that_returns_nothing_receives_its_inferred_input() {
    // The helper computes `value - 1` and returns no value, so its call is
    // the Unit-result call operation, which owes the inferred input too.
    let mut bodies = IdTable::default();
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![Statement::discard(expression(
            ExpressionKind::Call {
                required_contract: None,
                static_reach_binding: None,
                callee: id::<MachineId>(2).into(),
                arguments: vec![slot(0, u32_type())],
                designated: None,
            },
            None,
        ))],
    });
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![Statement::discard(strict(
            Strict::Subtract,
            slot(0, u32_type()),
            unsigned(1),
        ))],
    });
    let signatures = |entry_requires: Vec<Proposition>| {
        let mut signatures = IdTable::default();
        for requires in [entry_requires, Vec::new()] {
            signatures.push(Signature {
                operational: Default::default(),
                parameters: vec![u32_type().into()],
                erased: Vec::new(),
                borrowed: Vec::new(),
                result: None,
                requires,
                ensures: Vec::new(),
                reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
                invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
                crashes: CrashCeiling::Authored(Vec::new()),
                termination: Termination::Silent,
                ranking: None,
            });
        }
        signatures
    };
    let judged = |entry_requires| {
        let mut elaboration = typed_trees_to_terminal_psi::lower_bodies(
            id(1),
            &bodies,
            &signatures(entry_requires),
            &IdTable::default(),
        )
        .unwrap();
        elaboration.private_inputs = vec![id(2)];
        judge(Ok(elaboration)).unwrap()
    };
    let RouteVerdict::Accepted(accepted) = judged(vec![at_least(1, 1)]) else {
        panic!("the caller's positive input reaches the helper")
    };
    assert_eq!(
        accepted.elaboration.module.machines[1]
            .contract
            .requires
            .len(),
        1
    );
    let RouteVerdict::Rejected { unproved, .. } = judged(Vec::new()) else {
        panic!("nothing makes the helper's input positive")
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(2) && safety(&row.kind))
    );
}

#[test]
fn private_arrivals_can_discharge_compound_guards_on_owned_arguments() {
    let source = r#"
data OpenOptions {
    read: bool;
    write: bool;
    append: bool;
    truncate: bool;
    create: bool;
    create_new: bool;
}
machine entry(options: OpenOptions) -> u32
{
    // Truncation requires random-write access. Append-only access cannot
    // satisfy it, and must not be upgraded just to make a flag combination fit.
    transition options.truncate && (!options.write || options.append) && !options.create_new {
        true -> 0u32
        false -> access(options)
    }
    machine access(options: OpenOptions) -> u32 {
        transition options.read || (!options.write && !options.append) {
            true -> writable(options, 2147483648)
            false -> writable(options, 0)
        }
    }
    machine writable(options: OpenOptions, desired: u32) -> u32 {
        transition options.append {
            true -> disposition(options, desired | 1179924)
            false -> ordinary_write(options, desired)
        }
    }
    machine ordinary_write(options: OpenOptions, desired: u32) -> u32 {
        transition options.write {
            true -> disposition(options, desired | 1073741824)
            false -> disposition(options, desired)
        }
    }
    machine disposition(options: OpenOptions, desired: u32) -> u32 {
        transition options.create_new {
            true -> acquire(desired, 1)
            false -> creating(options, desired)
        }
    }
    machine creating(options: OpenOptions, desired: u32) -> u32 {
        transition options.create {
            true -> create_mode(options.truncate, desired)
            false -> existing_mode(options.truncate, desired)
        }
    }
    machine create_mode(truncate: bool, desired: u32) -> u32 {
        transition truncate { true -> acquire(desired, 2) false -> acquire(desired, 4) }
    }
    machine existing_mode(truncate: bool, desired: u32) -> u32 {
        transition truncate { true -> acquire(desired, 5) false -> acquire(desired, 3) }
    }
    machine acquire(desired: u32, disposition: u32) -> u32 {
        desired | disposition
    }
}
"#;
    let verdict = judge_source_arrivals(source, 8);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("each option route establishes one inferred arrival: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();
}

#[test]
fn a_recursive_call_retains_the_guard_on_its_qualified_scalar_input() {
    let source = r#"
domain i64::FileHandle
requires self != -1;

machine walk(handle: i64 in FileHandle, again: bool) -> i64 {
    transition { _ -> directory(handle, again) }
    machine directory(handle: i64 in FileHandle, again: bool) -> i64
    requires true {
        transition (handle as -FileHandle) != 0 {
            true -> query(handle, again)
            false -> -3i64
        }
    }
    machine query(handle: i64 in FileHandle, again: bool) -> i64
    requires (handle as -FileHandle) != 0 {
        transition again {
            true -> directory(handle, false)
            false -> (handle as -FileHandle)
        }
    }
}

machine entry() crashes Trap {
    let result: i64 = walk(41i64 as +FileHandle, true);
    transition result == 41 { true -> done() false -> fail() }
    machine done() {}
    machine fail() { crash Trap; }
}
"#;
    let judge_source = |source: &str| {
        let typed = super::boundary_calls::typed(source);
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = typed_trees_to_terminal_psi::elaborate_for(
            &typed,
            super::build_purpose::roster(&[entry], &[], source::DependencyScope::Product),
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap();
        judge(Ok(elaboration)).unwrap()
    };
    let verdict = judge_source(source);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the guard applies to the recursive caller's actual handle: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    let verdict = judge_source(&source.replace(
        "transition (handle as -FileHandle) != 0",
        "transition (handle as -FileHandle) == 0",
    ));
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("the zero edge cannot prove the call's nonzero condition: {verdict:?}");
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.statement.contains("call query from directory")),
        "the zero edge must fail the query input obligation: {unproved:?}"
    );
}

/// Two callers reach `report` knowing different elements: `entry` read
/// `cells[0]` and `second_step` read `cells[1]`, after `fill` wrote both
/// through the borrowed board. The inferred input is their join, so each
/// caller owes a disjunction one of whose disjuncts names an element it never
/// read and so has no value for. That caller proves the disjunct it can state.
#[test]
fn a_caller_proves_the_inferred_disjunct_it_can_state() {
    let source = r#"
data Board { cells: [u32; 2]; }
machine entry(board: &mut Board) -> u32 {
    fill(board, 1);
    transition board.cells[0] == 1 { true -> second_step(board) false -> report(board) }
    machine second_step(board: &mut Board) -> u32 {
        transition board.cells[1] == 1 { true -> 0u32 false -> report(board) }
    }
    machine report(board: &mut Board) -> u32 {
        7u32
    }
}
machine fill(board: &mut Board, value: u32) {
    board.cells[0] = value;
    board.cells[1] = value;
}
"#;
    let verdict = judge_source_arrivals(source, 2);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("each caller states the disjunct it established: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();
}

/// A written disjunctive requirement whose second disjunct names a field the
/// caller never read. The caller may prove the first disjunct, but an
/// unstated disjunct is never taken as true: a looser guard refuses.
#[test]
fn an_unstated_disjunct_is_proved_false_not_assumed() {
    let source = r#"
data Pair { first: u32; second: u32; }
machine entry(pair: &mut Pair) -> u32 {
    transition pair.first < BOUND { true -> check(pair) false -> 0u32 }
}
machine check(pair: &mut Pair) -> u32 requires pair.first < 10 || pair.second < 3 {
    7u32
}
"#;
    for (bound, accepted) in [("10", true), ("20", false)] {
        match judge_source_arrivals(&source.replace("BOUND", bound), 0) {
            RouteVerdict::Accepted(proved) if accepted => {
                proved.publish_invocation().unwrap();
            }
            RouteVerdict::Rejected { unproved, .. } if !accepted => assert!(
                unproved
                    .iter()
                    .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))
            ),
            verdict => panic!("guard bound {bound} decides the requirement: {verdict:?}"),
        }
    }
}
