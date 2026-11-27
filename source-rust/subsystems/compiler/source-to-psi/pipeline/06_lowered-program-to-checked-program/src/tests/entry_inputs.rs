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
                guard: ArmGuard::When(expression(
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

fn with_private_inputs(machines: Vec<Machine>, private: &[u64]) -> crate::lowering::Elaboration {
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

fn judge_source_arrivals(source: &str, expected_private: &[&str]) -> RouteVerdict {
    let typed = super::boundary_calls::typed(source);
    let entry = super::boundary_calls::entry(&typed);
    let package = typed
        .symbols
        .source_map()
        .unwrap()
        .files()
        .next()
        .unwrap()
        .package_identity;
    let elaboration = crate::lowering::elaborate_for(
        &typed,
        super::build_purpose::roster(
            &[entry],
            &[package],
            source_provenance::DependencyScope::Product,
        ),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let mut helpers: Vec<_> = elaboration
        .private_inputs
        .iter()
        .map(|id| elaboration.names.get(*id).unwrap().machine.as_str())
        .collect();
    helpers.sort_unstable();
    assert_eq!(helpers, expected_private);
    judge(Ok(elaboration)).unwrap()
}

#[test]
fn a_swapped_private_call_publishes_the_negated_equality_it_inherits() {
    let source = r"
machine entry() { caller(true, false); }
machine caller(first: bool, second: bool) requires first != second {
    _ = receiver(second, first);
}
machine receiver(x: bool, y: bool) -> bool { x == y }
";
    let RouteVerdict::Accepted(accepted) = judge_source_arrivals(source, &["receiver"]) else {
        panic!("the caller proves its Boolean arguments differ")
    };
    accepted
        .publish_invocation()
        .expect("the receiver's inferred inequality must encode in its own parameter order");
}

#[test]
fn a_private_result_receipt_matches_its_actual_return_and_never_its_written_clause() {
    let source = r"
machine entry() -> i32 { increment(7) }
machine increment(n: i32) -> i32 requires n >= 0 && n <= 9 ensures result == n + 1 { n + 1 }
";
    let RouteVerdict::Accepted(accepted) = judge_source_arrivals(source, &[]) else {
        panic!("the bounded addition satisfies its written guarantee");
    };
    let publication = accepted.publish_invocation().unwrap();
    let module = publication.artifact.view().module();
    let helper = &module.machines[1];
    assert!(
        !module.identities.proved_private_result(helper, 0),
        "written guarantee"
    );
    let i = (1..helper.ensures.len())
        .find(|&i| module.identities.proved_private_result(helper, i))
        .unwrap_or_else(|| {
            panic!(
                "the concrete invocation proves the result bounds: {:?}, summary owners: {:?}",
                helper.ensures,
                accepted.elaboration().local_results
            )
        });
    let mut stale = helper.clone();
    stale.contract.ensures[i].proposition = Proposition::Falsehood;
    assert!(!module.identities.proved_private_result(&stale, i));
    let mut stale = helper.clone();
    for block in &mut stale.blocks {
        if let terminal_psi::Terminator::Return { value, .. } = &mut block.terminator {
            *value = semantic_vocabulary::ValueId::new(value.get() + 1).unwrap();
        }
    }
    assert!(!module.identities.proved_private_result(&stale, i));
}

#[test]
fn published_private_inputs_remember_the_exact_calls_that_proved_them() {
    let program = r#"
machine entry() -> u32 {
    let first: u32 = next(7u32);
    next(7u32)
    machine next(n: u32) -> u32 WRITTEN { n }
}
"#;
    for (header, eligible) in [("", true), ("requires n == 7", false)] {
        let verdict = judge_source_arrivals(
            &program.replace("WRITTEN", header),
            if eligible { &["next"] } else { &[] },
        );
        let RouteVerdict::Accepted(accepted) = verdict else {
            panic!("the two calls pass seven: {verdict:?}");
        };
        let publication = accepted.publish_invocation().unwrap();
        let module = publication.artifact.view().module();
        let caller = &module.machines[0];
        let callee = &module.machines[1];
        let [predicate] = callee.requires.as_slice() else {
            panic!("one inferred or written input: {:?}", callee.requires);
        };
        assert_eq!(
            module.identities.inferred_input_clause(callee.id, 0),
            eligible
        );
        assert_eq!(
            module
                .identities
                .matches_inferred_input_clause(callee.id, 0, predicate),
            eligible
        );
        assert!(!module.identities.matches_inferred_input_clause(
            callee.id,
            0,
            &Proposition::Truth
        ));
        let calls = caller
            .blocks
            .iter()
            .flat_map(|b| &b.operations)
            .filter_map(|op| {
                let call = op.declared_invocation()?;
                (call.callee == semantic_vocabulary::TargetMachineCallable::Machine(callee.id))
                    .then(|| {
                        (
                            semantic_vocabulary::InvocationSite::Operation(op.id),
                            call.requirement_obligations[0],
                        )
                    })
            })
            .collect::<Vec<_>>();
        let [first, second] = calls.as_slice() else {
            panic!("both calls owe the input");
        };
        for (&(site, own), &(_, other)) in [(first, second), (second, first)] {
            assert_eq!(
                module
                    .identities
                    .proved_inferred_input(caller.id, site, callee.id, 0, own),
                eligible
            );
            assert!(
                !module
                    .identities
                    .proved_inferred_input(caller.id, site, callee.id, 0, other)
            );
        }
    }
}

#[test]
fn a_complete_byte_check_does_not_become_a_helper_requirement() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/calls/run/inferred_branch_inputs/main.omg"
    ));
    let typed =
        crate::preparation::prepare_typed_trees(super::boundary_calls::typed(source), &[], &[])
            .unwrap();
    let entry = super::build_purpose::symbol(typed.trees(), "main");
    let prepared = crate::lowering::prepare_compilation(
        &typed,
        super::build_purpose::roster(
            &[entry],
            &[None],
            source_provenance::DependencyScope::Product,
        ),
        terminal_codec::TerminalPublication::Invocation,
    )
    .expect("fixture must lower to Psi");
    let verdicts = crate::judge_compilation(prepared.program);
    assert!(
        !verdicts.is_empty()
            && verdicts
                .iter()
                .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}"
    );
}

#[test]
fn view_extent_facts_follow_only_the_guarded_input_across_private_arrivals() {
    let source = r#"
machine entry(bytes: @[u8], other: @[u8], index: u64) -> u8 {
    transition index < bytes.length { true -> next(INPUT, index), false -> 255u8 }
    machine next(bytes: @[u8], index: u64) -> u8 {
        transition { _ -> fetch(bytes, index) }
    }
    machine fetch(bytes: @[u8], index: u64) -> u8 {
        bytes[index]
    }
OTHER_BODY
}
"#;
    let judge_input = |input, other_body, expected_private: &[&str]| {
        let source = source
            .replace("INPUT", input)
            .replace("OTHER_BODY", other_body);
        judge_source_arrivals(&source, expected_private)
    };
    let helpers = ["fetch", "next"];
    let verdict = judge_input("bytes", "", &helpers);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the guarded descriptor reaches both helpers: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    for input in ["other", "@bytes[0..index]"] {
        let verdict = judge_input(input, "", &helpers);
        let RouteVerdict::Rejected { unproved, .. } = verdict else {
            panic!("the guard cannot justify indexing {input}: {verdict:?}");
        };
        assert!(
            unproved
                .iter()
                .any(|row| row.machine == id(3) && safety(&row.kind))
        );
    }

    let outside = "    machine unchecked(bytes: @[u8], index: u64) -> u8 { fetch(bytes, index) }";
    let verdict = judge_input("bytes", outside, &["fetch", "next", "unchecked"]);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("the unguarded caller also reaches fetch: {verdict:?}");
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
machine entry(bytes: @[u8], index: u64) -> u8 {
    transition index < bytes.length { true -> lead(bytes, index), false -> 255u8 }
    machine lead(bytes: @[u8], index: u64) -> u8 {
        let first: u8 = bytes[index];
        transition first > 0 && first < 128 {
            true -> first,
            false -> following(bytes, index, first)
        }
    }
    machine following(bytes: @[u8], index: u64, first: u8) -> u8 {
        let next: u64 = index + 1;
        transition first >= 194 && first <= 244 && next < bytes.length {
            true -> fetch(bytes, first, INPUT),
            false -> 255u8
        }
    }
    machine fetch(bytes: @[u8], first: u8, index: u64) -> u8 {
        bytes[index]
    }
}
"#;
    let helpers = ["fetch", "following", "lead"];
    let verdict = judge_source_arrivals(&source.replace("INPUT", "next"), &helpers);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the checked computed index reaches the helper: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    let verdict = judge_source_arrivals(&source.replace("INPUT", "bytes.length"), &helpers);
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
    transition character.GUARDED_FIELD < 65536 { true -> 0u32, false -> forward(character) }
    machine forward(character: Character) -> u32 {
        transition { _ -> encode(character) }
    }
    machine encode(character: Character) -> u32 {
        character.point - 65536
    }
}
"#;
    let helpers = ["encode", "forward"];
    let verdict = judge_source_arrivals(&source.replace("GUARDED_FIELD", "point"), &helpers);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the field guard crosses both owned arguments: {verdict:?}");
    };
    accepted.publish_invocation().unwrap();

    let verdict = judge_source_arrivals(&source.replace("GUARDED_FIELD", "sibling"), &helpers);
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
                guard: ArmGuard::Always,
                target: Target::Crash(semantic_vocabulary::CrashCause::Trap),
                source: nowhere(),
            },
        ],
    });
    let mut signatures = IdTable::default();
    for requires in [vec![at_least(1, 1)], Vec::new()] {
        signatures.push(Signature {
            membership_ensures: Vec::new(),
            content_ensures: Vec::new(),
            operational: Default::default(),
            parameters: vec![u32_type().into()],
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: Some(u32_type().into()),
            requires,
            ensures: Vec::new(),
            reaches: crate::lowering::Ceiling::Inferred,
            invokes: crate::lowering::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(trap_when(vec![terminal_psi::CrashRouteGuard::Truth])),
            termination: Termination::Silent,
            ranking: None,
        });
    }
    let mut elaboration =
        crate::lowering::lower_bodies(id(1), &bodies, &signatures, &IdTable::default()).unwrap();
    declare_body_models(&mut elaboration, &signatures);
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
                static_reach_arguments: Vec::new(),
                proof_arguments: Vec::new(),
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
                membership_ensures: Vec::new(),
                content_ensures: Vec::new(),
                operational: Default::default(),
                parameters: vec![u32_type().into()],
                erased: Vec::new(),
                borrowed: Vec::new(),
                result: None,
                requires,
                ensures: Vec::new(),
                reaches: crate::lowering::Ceiling::Inferred,
                invokes: crate::lowering::Ceiling::Inferred,
                crashes: CrashCeiling::Authored(Vec::new()),
                termination: Termination::Silent,
                ranking: None,
            });
        }
        signatures
    };
    let judged = |entry_requires| {
        let signatures = signatures(entry_requires);
        let mut elaboration =
            crate::lowering::lower_bodies(id(1), &bodies, &signatures, &IdTable::default())
                .unwrap();
        declare_body_models(&mut elaboration, &signatures);
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
        true -> 0u32,
        false -> access(options)
    }
    machine access(options: OpenOptions) -> u32 {
        transition options.read || (!options.write && !options.append) {
            true -> writable(options, 2147483648),
            false -> writable(options, 0)
        }
    }
    machine writable(options: OpenOptions, desired: u32) -> u32 {
        transition options.append {
            true -> disposition(options, desired | 1179924),
            false -> ordinary_write(options, desired)
        }
    }
    machine ordinary_write(options: OpenOptions, desired: u32) -> u32 {
        transition options.write {
            true -> disposition(options, desired | 1073741824),
            false -> disposition(options, desired)
        }
    }
    machine disposition(options: OpenOptions, desired: u32) -> u32 {
        transition options.create_new {
            true -> acquire(desired, 1),
            false -> creating(options, desired)
        }
    }
    machine creating(options: OpenOptions, desired: u32) -> u32 {
        transition options.create {
            true -> create_mode(options.truncate, desired),
            false -> existing_mode(options.truncate, desired)
        }
    }
    machine create_mode(truncate: bool, desired: u32) -> u32 {
        transition truncate { true -> acquire(desired, 2), false -> acquire(desired, 4) }
    }
    machine existing_mode(truncate: bool, desired: u32) -> u32 {
        transition truncate { true -> acquire(desired, 5), false -> acquire(desired, 3) }
    }
    machine acquire(desired: u32, disposition: u32) -> u32 {
        desired | disposition
    }
}
"#;
    let verdict = judge_source_arrivals(
        source,
        &[
            "access",
            "acquire",
            "create_mode",
            "creating",
            "disposition",
            "existing_mode",
            "ordinary_write",
            "writable",
        ],
    );
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
            true -> query(handle, again),
            false -> -3i64
        }
    }
    machine query(handle: i64 in FileHandle, again: bool) -> i64
    requires (handle as -FileHandle) != 0 {
        transition again {
            true -> directory(handle, false),
            false -> (handle as -FileHandle)
        }
    }
}

machine entry() crashes Trap {
    let result: i64 = walk(41i64 as +FileHandle, true);
    transition result == 41 { true -> done(), false -> fail() }
    machine done() {}
    machine fail() { crash Trap; }
}
"#;
    let judge_source = |source: &str| {
        let typed = super::boundary_calls::typed(source);
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = crate::lowering::elaborate_for(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[],
                source_provenance::DependencyScope::Product,
            ),
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

#[test]
fn a_guarded_field_survives_a_wrapping_call_cycle_and_rejects_unguarded_twins() {
    let advance = "input.index + 1";
    for (name, intermediate_write, write, contract, alternative, next_index, accepted) in [
        (
            "guarded_index_cycle",
            None,
            "input.mask = 1u32 as +Wrapping;",
            "",
            "0u32",
            advance,
            true,
        ),
        (
            "replaced_index_cycle",
            None,
            "input.index = 3u32 as +Wrapping;",
            "",
            "0u32",
            advance,
            false,
        ),
        (
            "unbounded_index_cycle",
            None,
            "input.mask = 1u32 as +Wrapping;",
            "requires true",
            "0u32",
            advance,
            false,
        ),
        (
            "unguarded_arrival_cycle",
            None,
            "input.mask = 1u32 as +Wrapping;",
            "",
            "block fetch(input)",
            advance,
            false,
        ),
        (
            "guard_survives_two_more_transfers",
            Some("input.mask = 1u32 as +Wrapping;"),
            "",
            "",
            "0u32",
            advance,
            true,
        ),
        (
            "reset_on_the_backedge_after_the_guarded_read",
            Some("input.mask = 1u32 as +Wrapping;"),
            "",
            "",
            "0u32",
            "0u32 as +Wrapping",
            true,
        ),
        (
            "a_write_between_transfers_invalidates_the_guard",
            Some("input.index = 3u32 as +Wrapping;"),
            "",
            "",
            "0u32",
            advance,
            false,
        ),
    ] {
        let (successor, intermediate) = match intermediate_write {
            None => ("fetch", String::new()),
            Some(write) => (
                "relay",
                format!(
                    "machine relay(input: @mutable Input) -> u32 {{
                    {write}
                    transition {{ _ -> block handoff(input) }}
                 }}
                 machine handoff(input: @mutable Input) -> u32 {{
                    transition {{ _ -> block fetch(input) }}
                 }}"
                ),
            ),
        };
        let source = format!(
            "public data Input {{ index: u32 in Wrapping; mask: u32 in Wrapping; values: [u32; 3]; }}
            public machine entry(input: @mutable Input) -> u32 requires true {{
                transition {{ _ -> block test(input) }}
                machine test(input: @mutable Input) -> u32 {{
                    transition input.index < 3u32 {{ true -> block {successor}(input), _ -> {alternative} }}
                }}
                {intermediate}
                machine fetch(input: @mutable Input) -> u32 {contract} {{
                    {write}
                    let found: u32 = input.values[input.index as -Wrapping];
                    input.index = {next_index};
                    transition found == 0u32 {{ true -> 0u32, _ -> block test(input) }}
                }}
            }}"
        );
        let typed = super::boundary_calls::typed(&source);
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = crate::lowering::elaborate_for(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[],
                source_provenance::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap();
        let verdict = judge(Ok(elaboration)).unwrap();
        if accepted {
            let RouteVerdict::Accepted(accepted) = verdict else {
                panic!("{name}: {verdict:?}");
            };
            accepted.publish_invocation().unwrap();
        } else {
            let RouteVerdict::Rejected { unproved, .. } = verdict else {
                panic!("{name}: {verdict:?}");
            };
            assert!(
                unproved
                    .iter()
                    .any(|row| row.statement.contains("index is within length 3")),
                "{name}: {unproved:?}"
            );
        }
    }
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
machine entry(board: @mutable Board) -> u32 {
    fill(board, 1);
    transition board.cells[0] == 1 { true -> second_step(board), false -> report(board) }
    machine second_step(board: @mutable Board) -> u32 {
        transition board.cells[1] == 1 { true -> 0u32, false -> report(board) }
    }
    machine report(board: @mutable Board) -> u32 {
        7u32
    }
}
machine fill(board: @mutable Board, value: u32) {
    board.cells[0] = value;
    board.cells[1] = value;
}
"#;
    let verdict = judge_source_arrivals(source, &["fill", "report", "second_step"]);
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("each caller states the disjunct it established: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let module = publication.artifact.view().module();
    let report = module.machine(id(3)).unwrap();
    let [predicate @ Proposition::Disjunction(_)] = report.requires.as_slice() else {
        panic!("report must accept the alternatives established by its two callers");
    };
    assert!(
        module
            .identities
            .matches_inferred_input_clause(report.id, 0, predicate)
    );
    for caller in [id(1), id(2)] {
        let mut calls = module
            .machine(caller)
            .unwrap()
            .module_invocations()
            .filter(|call| call.inputs.callee == report.id);
        let call = calls.next().expect("each caller invokes report");
        assert!(calls.next().is_none());
        let [obligation] = call.inputs.requirement_obligations else {
            panic!("the call must prove report's inferred requirement");
        };
        assert!(module.identities.proved_inferred_input(
            caller,
            call.site,
            report.id,
            0,
            *obligation
        ));
    }
}

/// The first caller reads cell zero. After a call that may write the board,
/// the recursive caller reads cell one. Each arrival must establish its own
/// disjunct even when the body never needs the other cell's entry snapshot.
#[test]
fn recursive_arrivals_prove_their_observed_input_alternative() {
    let source = r"
public data Board { cells: [u32; 2]; }
public machine entry(board: @mutable Board) -> u32 {
    fill(board, 1u32);
    transition board.cells[0] == 1u32 { true -> report(board, 3u32), false -> 6u32 }
}
machine report(board: @mutable Board, remaining: u32) -> u32
WRITTEN
terminates by remaining
{
    fill(board, remaining);
    transition board.cells[1] == 0u32 && remaining > 0u32 {
        true -> report(board, remaining - 1u32),
        false -> 7u32
    }
}
machine fill(board: @mutable Board, value: u32) {
    board.cells[0] = value;
    board.cells[1] = value;
}
";
    let written = "requires board.cells[0] == 1u32 || board.cells[1] == 0u32";
    for (header, helpers) in [("", &["fill", "report"][..]), (written, &["fill"][..])] {
        let verdict = judge_source_arrivals(&source.replace("WRITTEN", header), helpers);
        let RouteVerdict::Accepted(accepted) = verdict else {
            panic!("each arrival establishes one input alternative: {verdict:?}");
        };
        accepted.publish_invocation().unwrap();
    }
    let wrong = source
        .replace("WRITTEN", written)
        .replace("board.cells[1] == 0u32 &&", "board.cells[1] == 2u32 &&");
    let RouteVerdict::Rejected { unproved, .. } = judge_source_arrivals(&wrong, &["fill"]) else {
        panic!("the recursive edge establishes neither disjunct");
    };
    assert!(unproved.iter().any(|row| matches!(
        row.kind,
        ObligationKind::ArrivalInvariant { .. } | ObligationKind::CallRequirement { .. }
    )));
}

/// A written disjunctive requirement whose second disjunct names a field the
/// caller never read. The caller may prove the first disjunct, but an
/// unstated disjunct is never taken as true: a looser guard refuses.
#[test]
fn an_unstated_disjunct_is_proved_false_not_assumed() {
    let source = r#"
data Pair { first: u32; second: u32; }
machine entry(pair: @mutable Pair) -> u32 {
    transition pair.first < BOUND { true -> check(pair), false -> 0u32 }
}
machine check(pair: @mutable Pair) -> u32 requires pair.first < 10 || pair.second < 3 {
    7u32
}
"#;
    for (bound, accepted) in [("10", true), ("20", false)] {
        match judge_source_arrivals(&source.replace("BOUND", bound), &[]) {
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

#[test]
fn a_guard_on_a_field_written_before_the_call_reaches_the_helper() {
    // The stack samples write their stack pointer and then guard it on the
    // edge into the helper that indexes with it. The guard's bound must reach
    // that helper whichever field the guarded value was read from, and an
    // unrelated write must not discard it. Guarding a field that no longer
    // holds the index must still refuse.
    for (name, writes, guarded, accepted) in [
        (
            "copied_from_another_field",
            "main.sp = main.k;",
            "main.sp",
            true,
        ),
        (
            "guarded_through_its_source",
            "main.sp = main.k;",
            "main.k",
            true,
        ),
        ("unrelated_field_written", "main.k = 5;", "main.sp", true),
        ("nothing_written", "", "main.sp", true),
        (
            "source_rewritten_after_copy",
            "main.sp = main.k; main.k = 2;",
            "main.k",
            false,
        ),
    ] {
        let source = format!(
            "data Main {{ stack: [i32; 8]; sp: i32; k: i32; }}
            machine entry(main: @mutable Main) {{
                {writes}
                transition {guarded} >= 0 && {guarded} < 8 {{ true -> push(main), false -> done() }}
                machine push(main: @mutable Main) {{
                    main.stack[main.sp] = 3;
                }}
                machine done() {{}}
            }}"
        );
        let typed = super::boundary_calls::typed(&source);
        let entry = super::boundary_calls::entry(&typed);
        let elaboration = crate::lowering::elaborate_for(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[],
                source_provenance::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap();
        let verdict = judge(Ok(elaboration)).unwrap();
        if accepted {
            let RouteVerdict::Accepted(accepted) = verdict else {
                panic!("{name}: {verdict:?}");
            };
            accepted.publish_invocation().unwrap();
        } else {
            let RouteVerdict::Rejected { unproved, .. } = verdict else {
                panic!("{name}: {verdict:?}");
            };
            assert!(
                unproved
                    .iter()
                    .any(|row| row.machine == id(2) && safety(&row.kind)),
                "{name}: {unproved:?}"
            );
        }
    }
}

#[test]
fn a_guard_on_an_array_element_reaches_a_helper_that_borrows_the_array() {
    // A caller guards one element of a local array and lends the array to
    // a nested helper whose call needs that element's bound. The bound
    // crosses into the helper as the read `items[0]`, the form the helper's
    // call gives the same cell. A guard on the other element leaves the
    // helper's call unproved.
    for (guarded, accepted) in [("items[0]", true), ("items[1]", false)] {
        let source = format!(
            "machine need(items: @[u64; 2], index: u64) -> u64
            requires index < 2 requires items[index] < 5 {{ items[index] }}
            machine entry(first: u64, second: u64) -> u64 {{
                let items: [u64; 2] = [first, second];
                transition {guarded} < 5 {{ true -> finish(@items), false -> (0) }}
                machine finish(items: @[u64; 2]) -> u64 {{ need(items, 0) }}
            }}"
        );
        match judge_source_arrivals(&source, &["finish"]) {
            RouteVerdict::Accepted(proved) if accepted => {
                proved.publish_invocation().unwrap();
            }
            RouteVerdict::Rejected { unproved, .. } if !accepted => assert!(
                unproved
                    .iter()
                    .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. })),
                "{guarded}: {unproved:?}"
            ),
            verdict => panic!("a guard on {guarded} decides the helper's call: {verdict:?}"),
        }
    }
}

#[test]
fn a_guard_on_the_last_shared_record_reaches_its_exact_field() {
    for (guarded, supplied, accepts) in [
        (4095, "items", true),
        (4094, "items", false),
        (4095, "other", false),
    ] {
        let source = format!(
            "public data Cell {{ marker: u64; other: u64; }}
             machine need(items: @[Cell; 4096], index: u64) -> u64
             requires index < 4096 requires items[index].marker < 5 {{ items[index].marker }}
             public machine entry(items: @[Cell; 4096], other: @[Cell; 4096]) -> u64 {{
                 transition items[{guarded}].marker < 5 {{ true -> finish({supplied}), false -> (0) }}
                 machine finish(items: @[Cell; 4096]) -> u64 {{ need(items, 4095) }}
             }}"
        );
        match judge_source_arrivals(&source, &["finish"]) {
            RouteVerdict::Accepted(proved) if accepts => {
                proved.publish_invocation().unwrap();
            }
            RouteVerdict::Rejected { unproved, .. } if !accepts => assert!(
                unproved
                    .iter()
                    .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. })),
                "{unproved:?}"
            ),
            verdict => panic!("guard items[{guarded}] with {supplied} supplied: {verdict:?}"),
        }
    }
}

#[test]
fn an_uncovered_boundary_call_names_the_member_it_calls() {
    // A boundary requirement has no machine names of its own, so the
    // diagnostic for its uncovered crash route names the member the caller
    // wrote, as it names a machine callee. The twin covers the route with
    // `requires index != 0` and is accepted.
    for (requires, accepted) in [("", false), ("requires index != 0", true)] {
        let source = format!(
            "boundary trait Forward {{
                machine forward(index: u64) -> u64 reaches none crashes Trap index == 0;
            }}
            public machine entry(index: u64) -> u64 invokes Forward {requires} {{
                Forward::forward(index)
            }}"
        );
        match judge_source_arrivals(&source, &[]) {
            RouteVerdict::Accepted(_) if accepted => {}
            RouteVerdict::Rejected { unproved, .. } if !accepted => assert!(
                unproved.iter().any(|row| row
                    .statement
                    .contains("call from `entry` to `forward` has an uncovered Trap crash route")),
                "{unproved:?}"
            ),
            verdict => panic!("{requires:?} decides the boundary call: {verdict:?}"),
        }
    }
}

#[test]
fn a_private_helper_omitting_crashes_publishes_its_guarded_call_route() {
    // A private nested helper that omits `crashes` infers the guarded Trap
    // route of its `maybe_zero` call rather than unconditional `Truth`. The
    // public caller's `requires items[index] == 7` discharges that guard;
    // dropping the requirement refuses the call at the uncovered Trap route.
    for (requires, accepted) in [("requires items[index] == 7", true), ("", false)] {
        let source = format!(
            "machine maybe_zero(items: @[u64; 2], index: u64) -> u64
            requires index < 2
            crashes Trap items[index] == 0
            {{
                transition items[index] == 0 {{ true -> bad(), false -> (items[index]) }}
                machine bad() -> u64 {{ crash Trap; }}
            }}
            public machine entry(items: @[u64; 2], index: u64) -> u64
            requires index < 2
            {requires}
            {{
                transition {{ _ -> consume(items, index) }}
                machine consume(values: @[u64; 2], selected: u64) -> u64
                requires selected < 2
                {{ maybe_zero(values, selected) }}
            }}"
        );
        match judge_source_arrivals(&source, &["bad"]) {
            RouteVerdict::Accepted(_) if accepted => {}
            RouteVerdict::Rejected { unproved, .. } if !accepted => assert!(
                unproved.iter().any(|row| row
                    .statement
                    .contains("call from `entry` to `consume` has an uncovered Trap crash route")),
                "{unproved:?}"
            ),
            verdict => panic!("{requires:?} decides the inferred private crash route: {verdict:?}"),
        }
    }
}
