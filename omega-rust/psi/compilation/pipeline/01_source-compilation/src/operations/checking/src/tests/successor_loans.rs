//! Passing storage to a successor must keep its original borrow authority.

use super::*;
use language_core::ReferenceAccess;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_check_tables::edges::{Destination, EdgeRow};
use terminal_check_tables::obligations::ObligationRow;
use terminal_psi::{
    Block, StructuralAccess, StructuralArgument, StructuralPlaceDeclaration, Terminator,
};

fn duplicated_borrow() -> crate::lowering::Elaboration {
    let mut body = super::references::unit(vec![super::references::pair()], Vec::new());
    body.borrowed.push((0, ReferenceAccess::Mutable));
    let mut input = super::references::lower_lending(vec![body]).unwrap();
    let machine = &mut input.module.machines[0];
    let source = machine.structural_parameters[0].clone();
    let argument = StructuralArgument::whole(source.place, StructuralAccess::MutableBorrow);
    let arrival = id(1);
    machine.blocks[0].terminator = Terminator::Jump {
        edge: arrival,
        target: id(2),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: vec![argument.clone(), argument],
        cleanup_actions: Vec::new(),
    };
    let mut target = Block::plain(
        id(2),
        Vec::new(),
        Terminator::ReturnUnit {
            edge: id(2),
            cleanup_actions: Vec::new(),
        },
    );
    for position in 0..2 {
        let mut parameter = source.clone();
        parameter.place = id(2 + u64::from(position));
        parameter.position = position;
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::BlockParameter {
                block: target.id,
                position,
            },
        });
        target.structural_parameters.push(parameter);
    }
    machine.blocks.push(target);
    let tables = input.tables.machines.get_mut(machine.id).unwrap();
    tables.edges.get_mut(arrival).unwrap().destination = Destination::Block(id(2));
    tables.edges.push(EdgeRow {
        source: id(2),
        destination: Destination::Return,
        dispositions: Vec::new(),
    });
    // Replacing the return with an argument-passing jump makes the new
    // arrival owe the same obligation as one emitted by lowering.
    let obligation = tables.obligations.push(ObligationRow {
        created_at: ProgramPoint::Edge(arrival),
        kind: ObligationKind::ArrivalInvariant { edge: arrival },
    });
    let sources = input.sources.get_mut(machine.id).unwrap();
    let source = *sources.edges.get(arrival).unwrap();
    assert_eq!(sources.obligations.push(source), obligation);
    input
}

#[test]
fn a_structural_successor_owes_its_arrival_even_without_scalar_arguments() {
    let mut input = duplicated_borrow();
    let tables = input.tables.machines.get_mut(id(1)).unwrap();
    assert_eq!(tables.obligations.len(), 1);
    tables.obligations = IdTable::default();
    input.sources.get_mut(id(1)).unwrap().obligations = IdTable::default();
    let refusal = judge(Ok(input)).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert_eq!(refusal.concerns, Construct::Block(id(1)));
    assert_eq!(refusal.detail, "a site owes an obligation with no row");
}

#[test]
fn a_successor_cannot_duplicate_one_exclusive_borrow_into_two_headers() {
    let verdict = judge(Ok(duplicated_borrow()));
    assert!(
        matches!(verdict, Err(Refusal { reason: RefusalReason::Conflict, ref detail, .. })
            if detail == "successor arguments duplicate an exclusive borrow"),
        "the successor must reject the overlapping exclusive cohort"
    );
}

#[test]
fn shared_successor_names_may_continue_one_shared_ingress() {
    let mut input = duplicated_borrow();
    let machine = &mut input.module.machines[0];
    machine.contract.structural_parameters[0].access = StructuralAccess::SharedBorrow;
    for parameter in &mut machine.blocks[1].structural_parameters {
        parameter.access = StructuralAccess::SharedBorrow;
    }
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    for actual in structural_arguments {
        actual.access = StructuralAccess::SharedBorrow;
    }
    assert!(
        matches!(judge(Ok(input)), Ok(RouteVerdict::Accepted(_))),
        "shared continuation must retain its ingress"
    );
}

#[test]
fn an_attenuated_successor_still_conflicts_with_an_exclusive_sibling() {
    let mut input = duplicated_borrow();
    let machine = &mut input.module.machines[0];
    machine.blocks[1].structural_parameters[1].access = StructuralAccess::SharedBorrow;
    let Terminator::Jump {
        structural_arguments,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    structural_arguments[1].access = StructuralAccess::SharedBorrow;
    let tables = input.tables.machines.get_mut(machine.id).unwrap();
    let captured = tables.roots.get(id(1)).copied().unwrap();
    tables.obligations.push(ObligationRow {
        created_at: ProgramPoint::Edge(id(1)),
        kind: ObligationKind::Loan {
            captured,
            access: ReferenceAccess::Shared,
            lender: terminal_check_tables::obligations::Lender::Incoming,
        },
    });
    assert!(
        matches!(judge(Ok(input)), Err(Refusal { reason: RefusalReason::Conflict, ref detail, .. })
        if detail == "successor arguments overlap shared and exclusive borrows"),
        "attenuation cannot hide overlapping siblings"
    );
}

#[test]
fn loop_element_reborrows_keep_distinct_incoming_backing() {
    let program = r#"
        machine update(left: @mut u32, right: @mut u32) {
            left = 1u32;
            right = 2u32;
        }
        machine walk(items: @mut [u32], other: @mut [u32], index: u64) {
            transition index < 4u64 && index < items.len && index < other.len {
                true -> step(items, other, index)
                false -> done()
            }
            machine step(items: @mut [u32], other: @mut [u32], index: u64)
            requires index < 4u64 && index < items.len && index < other.len
            {
                update(@mut items[index], @mut other[index]);
                transition { _ -> walk(items, other, index + 1u64) }
            }
            machine done() { }
        }
    "#;
    for (program, accepts) in [
        (program.to_owned(), true),
        (
            program.replace("@mut other[index]", "@mut items[index]"),
            false,
        ),
    ] {
        let typed = super::boundary_calls::typed(&program);
        let verdict = super::entryless::judged_typed_result(&typed, false);
        if accepts {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "{verdict:?}"
            );
        } else {
            match verdict {
                Ok(RouteVerdict::Rejected { conflicts, .. }) => assert!(
                    conflicts.iter().any(|conflict| matches!(
                        conflict.conflict,
                        crate::checking::LoanConflict::TwiceExclusive
                    )),
                    "{conflicts:?}"
                ),
                Err(crate::checking::CompilationRefusal::Elaboration(refusal)) => assert!(
                    refusal.reason == RefusalReason::Conflict
                        && refusal
                            .detail
                            .starts_with("successor borrow conflicts with an original live loan"),
                    "duplicating the input must fail its actual alias check: {refusal:?}"
                ),
                other => panic!("duplicating an exclusive element borrow must reject: {other:?}"),
            }
        }
    }
}

#[test]
fn shared_array_views_keep_their_backing_when_sliced() {
    for (start, accepted) in [(1, true), (3, false)] {
        let source = format!(
            r#"
machine entry() -> u8 {{
    let bytes: [u8; 3] = [17, 42, 99];
    let view: @[u8] = @bytes[..];
    let suffix: @[u8] = view[{start}..];
    suffix[0]
}}
"#
        );
        let results =
            super::boundary_calls::compilation_results_of(&super::boundary_calls::typed(&source));
        assert!(!results.is_empty());
        if accepted {
            assert!(
                results
                    .iter()
                    .all(|result| matches!(result, Ok(RouteVerdict::Accepted(_)))),
                "a shared array view must retain its source through slicing"
            );
        } else {
            assert!(results.iter().any(|result| matches!(result, Ok(RouteVerdict::Rejected { unproved, .. })
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. })))),
                "reading an empty suffix must fail its bounds obligation");
        }
    }
}

#[test]
fn cleanup_must_keep_backing_on_the_edge_that_still_uses_its_view() {
    let typed = super::boundary_calls::typed(
        r#"
data Packet { before: u8; bytes: [u8; 3]; after: u8; }
machine inspect(bytes: @[u8]) -> bool { bytes.len == 3 }
machine entry(first: bool) -> bool {
    let packet: Packet = Packet { before: 11, bytes: [17, 42, 99], after: 97 };
    let view: @[u8] = @packet.bytes[..];
    first && inspect(view) && packet.before == 11 && packet.after == 97
}
"#,
    );
    let mut input = super::boundary_calls::compilation_results_of(&typed)
        .into_iter()
        .map(|result| match result {
            Ok(RouteVerdict::Accepted(accepted)) => accepted.elaboration,
            other => panic!("the short-circuit path must accept: {other:?}"),
        })
        .next()
        .expect("the entry has a checked body");
    let machine = input
        .module
        .machines
        .iter_mut()
        .find(|machine| {
            machine.blocks.iter().any(|block| {
                matches!(
                    &block.terminator,
                    Terminator::Conditional { when_false, .. }
                        if !when_false.cleanup_actions.is_empty()
                )
            })
        })
        .expect("the false edge retires the unused packet");
    let identity = machine.id;
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| {
            matches!(
                &block.terminator,
                Terminator::Conditional { when_false, .. } if !when_false.cleanup_actions.is_empty()
            )
        })
        .unwrap();
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &mut block.terminator
    else {
        unreachable!();
    };
    // Discarding the packet is valid on the false path, but would
    // invalidate the view before inspect on the continuing path. The false
    // edge also retires the unused view itself; that retirement is not
    // copied, so the conflict tested is the packet's.
    when_true.cleanup_actions = when_false
        .cleanup_actions
        .iter()
        .filter(|action| {
            matches!(
                action,
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(_)
            )
        })
        .cloned()
        .collect();
    assert!(!when_true.cleanup_actions.is_empty());
    let mut activity = crate::checking::ProofSearchActivity::default();
    let mut scope = crate::checking::prepare_replay(&mut input, &mut activity).unwrap();
    let (formed, domains, checking, _, _) = scope.checking();
    let module = formed.source();
    let tables = checking.machines.get_mut(identity).unwrap();
    let prepared = crate::checking::prepare(formed, domains, identity, tables).unwrap();
    let mut references = prepared.references(&module.structural_types);
    let machine = module.machine(identity).unwrap();
    let storage = crate::checking::places::Storage::new(
        identity,
        machine,
        module,
        domains,
        &prepared.graph,
        tables,
        &prepared.types,
        &mut references,
    )
    .unwrap();
    let (_, conflicts) = storage
        .loans(&mut references, module, tables, None)
        .unwrap();
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.conflict == crate::checking::LoanConflict::ChangeWhileBorrowed),
        "cleanup on the continuing edge must preserve the packet's live view"
    );
}

#[test]
fn successor_capture_precedes_disposal_of_its_owned_backing() {
    let source = r#"data Packet {
    left: u8;
    bytes: [u8; 3];
    value: u8;
    right: u8;
}

machine read_value(packet: @Packet) -> u8 {
    packet.value
}

machine main() crashes Trap {
    transition { _ -> block round(false) }

    machine round(second: bool) {
        let expected: u8 = match second { true -> 42u8 _ -> 17u8 };
        let packet: Packet = Packet {
            left: 11u8,
            bytes: [2u8, 3u8, 5u8],
            value: expected,
            right: 99u8
        };
        transition { _ -> block visit(second, expected, @packet) }
    }

    machine visit(second: bool, expected: u8, packet: @Packet) {
        let actual: u8 = read_value(packet);
        transition actual == expected && packet.left == 11u8 && packet.right == 99u8 {
            true -> block next(second)
            _ -> bad()
        }
    }

    machine next(second: bool) {
        transition second {
            true -> done()
            _ -> block round(true)
        }
    }

    machine done() {}
    machine bad() crashes Trap { crash Trap; }
}
"#;
    let prepare = |text: &str| {
        let typed = super::boundary_calls::typed(text);
        let main = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "main")
            .expect("the cycle declares main")
            .symbol;
        crate::lowering::prepare_compilation(
            &typed,
            crate::lowering::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[main],
                subjects: &[],
                purpose: declaration_contracts::source_identity::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program
    };
    let direct_field = source.replace("read_value(packet);", "packet.value;");
    let forwarding_cycle = source
        .replace(
            "block visit(second, expected, @packet)",
            "block forward(second, expected, @packet)",
        )
        .replace(
            "    machine visit(",
            r#"    machine copy_expected(expected: u8) -> u8 {
        expected
    }

    machine forward(second: bool, expected: u8, packet: @Packet) {
        let next_expected: u8 = copy_expected(expected);
        transition second {
            true -> block visit(true, next_expected, packet)
            _ -> block visit(false, next_expected, packet)
        }
    }

    machine visit("#,
        );
    for (text, forwards) in [
        (source, false),
        (direct_field.as_str(), false),
        (forwarding_cycle.as_str(), true),
    ] {
        let positive = prepare(text);
        let terminal_check_tables::compilation::TerminalCompilation::Prepared(program) = &positive
        else {
            panic!("the shared borrow cycle must lower");
        };
        if forwards {
            let closure = program
                .closure
                .as_ref()
                .expect("the forwarding cycle lowers");
            let machine = |name| {
                closure
                    .names
                    .iter()
                    .find_map(|(id, names)| (names.machine == name).then_some(id))
                    .expect("the cycle declares this machine")
            };
            let forward = machine("forward");
            let visit = machine("visit");
            assert!(closure.sources.get(forward).unwrap().heads.is_empty());
            assert!(
                closure
                    .module
                    .machine(forward)
                    .unwrap()
                    .blocks
                    .iter()
                    .any(|block| block
                        .operations
                        .iter()
                        .any(|operation| matches!(operation.kind,
                    terminal_psi::OperationKind::CallUnit { callee, .. } if callee == visit)))
            );
            let folded_forward = closure
                .sources
                .get(visit)
                .unwrap()
                .heads
                .iter()
                .find_map(|(block, member)| (*member == forward).then_some(*block))
                .expect("the receiver retains the forwarder's cyclic head");
            assert!(closure.module.machine(visit).unwrap().blocks.iter().any(|block|
                matches!(block.terminator, Terminator::Jump { target, .. } if target == folded_forward)));
        }
        let expected_verdicts = 1 + program.forms.len() + program.witnesses.len();
        let verdicts = crate::checking::judge_compilation(positive);
        assert_eq!(verdicts.len(), expected_verdicts);
        assert!(
            verdicts
                .iter()
                .all(|result| matches!(result, Ok(RouteVerdict::Accepted(_)))),
            "the shared borrow cycle must accept: {verdicts:?}"
        );
    }

    // Accepted elaborations carry successful checking knowledge. Prepare a
    // fresh input for the mutation instead of reusing or clearing that evidence.
    let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) =
        prepare(source)
    else {
        panic!("the unchanged cycle must prepare its actual closure");
    };
    let mut input = prepared
        .closure
        .expect("the unchanged cycle must lower without a source refusal");
    assert!(
        input
            .tables
            .machines
            .iter()
            .all(|(_, tables)| tables.knowledge.is_empty()),
        "the negative must start from fresh, unpopulated checking tables"
    );
    let (identity, selected, owner) = input
        .module
        .machines
        .iter()
        .find_map(|machine| {
            machine.blocks.iter().find_map(|block| {
                let Terminator::Jump {
                    edge,
                    target,
                    structural_arguments,
                    ..
                } = &block.terminator
                else {
                    return None;
                };
                let target = machine.blocks.iter().find(|block| block.id == *target)?;
                structural_arguments
                    .iter()
                    .zip(&target.structural_parameters)
                    .find_map(|(argument, parameter)| {
                        let source = machine
                            .structural_places
                            .iter()
                            .find(|place| place.id == argument.place)?;
                        (argument.access == StructuralAccess::SharedBorrow
                            && parameter.access == StructuralAccess::SharedBorrow
                            && argument.path.is_empty()
                            && matches!(
                                source.kind,
                                StructuralPlaceKind::OperationResult { .. }
                                    | StructuralPlaceKind::TrivialAffineLocal { .. }
                            ))
                        .then_some((
                            machine.id,
                            *edge,
                            argument.place,
                        ))
                    })
            })
        })
        .expect("real lowering lends a fresh owned Packet to a shared block header");
    let machine = input
        .module
        .machines
        .iter_mut()
        .find(|machine| machine.id == identity)
        .unwrap();
    let cleanup = machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::Jump {
                edge,
                cleanup_actions,
                ..
            } if *edge == selected => Some(cleanup_actions),
            _ => None,
        })
        .unwrap();
    assert!(cleanup.iter().all(|action| action.place() != owner));
    // The actual has already been captured for the destination header when
    // this cleanup runs. Keep every other operation, argument and table row.
    cleanup.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
        owner,
    ));

    let mut activity = crate::checking::ProofSearchActivity::default();
    let mut scope = crate::checking::prepare_replay(&mut input, &mut activity).unwrap();
    let (formed, domains, checking, _, _) = scope.checking();
    let module = formed.source();
    let tables = checking.machines.get_mut(identity).unwrap();
    let prepared = crate::checking::prepare(formed, domains, identity, tables).unwrap();
    let mut references = prepared.references(&module.structural_types);
    let storage = crate::checking::places::Storage::new(
        identity,
        module.machine(identity).unwrap(),
        module,
        domains,
        &prepared.graph,
        tables,
        &prepared.types,
        &mut references,
    )
    .unwrap();
    // Exercise the independent edge guard, rather than rejecting the forged
    // affine schedule before loan checking or borrowing a pre-mutation plan.
    let refusal = storage
        .loans(&mut references, module, tables, None)
        .expect_err("cleanup cannot destroy a newly captured borrow's owner");
    assert_eq!(refusal.reason, RefusalReason::Conflict);
    assert_eq!(refusal.machine, identity);
    assert_eq!(refusal.concerns, Construct::Edge(selected));
    assert_eq!(
        refusal.detail,
        "successor cleanup changes the backing of a captured borrow"
    );
}

#[test]
fn an_empty_shared_payload_keeps_its_owned_permission_at_a_call() {
    let typed = super::boundary_calls::typed(
        r#"
data Holder<'held> { bytes: @'held [u8]; }
machine inspect<'a>(value: @'a [u8]) -> u64 { value.len }
machine entry() -> u64 {
    let mut holder: Holder;
    inspect(holder.bytes)
}
"#,
    );
    let results = super::boundary_calls::compilation_results_of(&typed);
    assert!(!results.is_empty());
    let mut owned_loan = false;
    for result in results {
        match result {
            Ok(RouteVerdict::Accepted(accepted)) => {
                owned_loan |= accepted
                    .elaboration
                    .tables
                    .machines
                    .iter()
                    .any(|(_, tables)| {
                        tables.obligations.iter().any(|(_, row)| {
                            matches!(
                                row.kind,
                                ObligationKind::Loan {
                                    lender: terminal_check_tables::obligations::Lender::Owner,
                                    ..
                                }
                            )
                        })
                    });
            }
            Ok(RouteVerdict::FallBack(refusal))
            | Err(crate::checking::CompilationRefusal::Elaboration(refusal)) => {
                panic!("empty shared payload was refused: {}", refusal.detail);
            }
            _ => panic!("empty shared payload call was not accepted"),
        }
    }
    assert!(
        owned_loan,
        "the local empty payload must retain an owner-backed loan"
    );
}

#[test]
fn loop_windows_keep_the_exact_original_input_permission() {
    use terminal_semantics::descriptor_lifetimes::descriptor_location;
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, terminal_borrowed_storage_graph_from,
    };
    let program = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/fixtures/run/views/ranked_slice_iteration/main.omg"
    ));
    let typed = super::rankings::typed_with_ranking_catalog(program);
    let checksum = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "checksum")
        .unwrap()
        .symbol;
    let prepared = crate::lowering::prepare_compilation(
        &typed,
        crate::lowering::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[checksum],
            subjects: &[],
            purpose: declaration_contracts::source_identity::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .program;
    let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) = prepared
    else {
        panic!("the unchanged checksum must form a Terminal compilation");
    };
    let input = prepared.closure.unwrap_or_else(|refusal| {
        panic!("source preparation refused checksum: {}", refusal.detail)
    });
    let graph = terminal_borrowed_storage_graph_from(
        &input.module,
        input.module.machines.iter().map(|machine| machine.id),
        &[],
    )
    .unwrap_or_else(|error| panic!("{}", error.in_module(&input.module)));
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&input.module.structural_types, &graph);
    let mut checked = 0;
    for machine in &input.module.machines {
        for block in &machine.blocks {
            let at = (block.id, u32::try_from(block.operations.len()).unwrap());
            for ((edge, target), actuals) in block
                .terminator
                .successor_edges()
                .into_iter()
                .zip(block.terminator.successor_block_arguments())
            {
                for (actual, formal) in actuals
                    .iter()
                    .zip(&machine.block(&target).unwrap().structural_parameters)
                {
                    if !terminal_semantics::descriptor_results::is_descriptor(
                        input.module.structural_types.iter(),
                        formal.structural_type,
                    ) {
                        continue;
                    }
                    let location = descriptor_location(
                        &input.module.structural_types,
                        actual.place,
                        machine.structural_place_type(actual.place).unwrap(),
                        &actual.path,
                        None,
                    )
                    .unwrap();
                    let origins = analysis
                        .symbolic_source_origins(machine.id, at, &location)
                        .expect("an actual checksum descriptor must have an origin roster");
                    let result = terminal_semantics::borrowed_loan_source(
                        &input.module,
                        machine,
                        &mut analysis,
                        at,
                        &location,
                        std::iter::empty(),
                    );
                    assert_eq!(
                        result,
                        Ok(terminal_semantics::BorrowedLoanSource::Incoming),
                        "machine {:?}, edge {:?}, source {:?}, origin count {}, first origins {:?}",
                        machine.id,
                        edge,
                        location,
                        origins.len(),
                        origins.iter().take(6).collect::<Vec<_>>()
                    );
                    checked += 1;
                }
            }
        }
    }
    assert!(
        checked >= 2,
        "the entry and feedback both transport the real view"
    );
}

fn permission_inputs(text: &str) -> crate::lowering::Elaboration {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    let program = crate::lowering::prepare_compilation(
        &typed,
        crate::lowering::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: declaration_contracts::source_identity::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .program;
    let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) = program
    else {
        panic!("the permission query's declarations must prepare");
    };
    prepared
        .closure
        .unwrap_or_else(|refusal| panic!("permission query input refused: {}", refusal.detail))
}

#[test]
fn incoming_permission_query_keeps_different_inputs_and_leaves_ambiguous() {
    use terminal_semantics::descriptor_lifetimes::{DescriptorLocation, DescriptorPathSegment};
    use terminal_semantics::descriptor_results::{DescriptorCallOrigin, DescriptorSourceOrigin};
    let input = permission_inputs("machine entry(first: @[u8], second: @[u8]) {}");
    let machine = &input.module.machines[0];
    let origins = machine
        .structural_parameters
        .iter()
        .map(|parameter| {
            DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                source: parameter.place,
                projection: terminal_semantics::borrowed_storage::Projection::new(&[]),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(origins.len(), 2);
    for origin in &origins {
        let occurrence = terminal_semantics::incoming_view_occurrence(
            &input.module,
            machine,
            std::slice::from_ref(origin),
        );
        assert!(
            occurrence.is_ok(),
            "the exact input must identify its own borrow: {origin:?}: {occurrence:?}"
        );
    }
    assert_eq!(
        terminal_semantics::incoming_view_occurrence(&input.module, machine, &origins),
        Err("the view can name different incoming borrows or fields"),
        "two different input borrows cannot be claimed as one"
    );

    let input = permission_inputs(
        r#"
data Pair<'held> { first: @'held [u8]; second: @'held [u8]; }
machine entry<'a>(pair: @Pair<'a>) {}
"#,
    );
    let machine = &input.module.machines[0];
    let parameter = &machine.structural_parameters[0];
    let fields = input
        .module
        .structural_type(parameter.structural_type)
        .unwrap()
        .shape
        .direct_fields()
        .unwrap();
    let origins = fields
        .iter()
        .map(|field| {
            DescriptorSourceOrigin::IncomingSlot(DescriptorLocation {
                root: parameter.place,
                path: vec![DescriptorPathSegment::Field(field.id)],
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(origins.len(), 2);
    for origin in &origins {
        let occurrence = terminal_semantics::incoming_view_occurrence(
            &input.module,
            machine,
            std::slice::from_ref(origin),
        );
        let DescriptorSourceOrigin::IncomingSlot(expected) = origin else {
            unreachable!("these inputs are exact field occurrences");
        };
        assert!(
            occurrence.as_ref() == Ok(expected),
            "the exact field must identify its own borrow: {origin:?}: {occurrence:?}"
        );
    }
    assert_eq!(
        terminal_semantics::incoming_view_occurrence(&input.module, machine, &origins),
        Err("the view can name different incoming borrows or fields"),
        "two fields still name different borrows"
    );
}

#[test]
fn indexed_successors_keep_their_original_incoming_loan() {
    use terminal_semantics::descriptor_lifetimes::DescriptorLocation;
    use terminal_semantics::descriptor_results::{
        DescriptorAnalysis, DescriptorCallOrigin, terminal_borrowed_storage_graph_from,
    };
    // Tail-call cycles become state headers; acyclic helpers remain ordinary calls.
    // Keep this real cycle so the test exercises captured storage on arrivals.
    const SOURCE: &str = r#"
data Shape [copy] { value: u64; }
machine entry(shapes: @[Shape; 2], sibling: @[Shape; 2], index: u64, again: bool) -> u64
requires index < 2 {
    transition { _ -> selected(shapes, sibling, index, again, @shapes[index]) }
    machine selected(shapes: @[Shape; 2], sibling: @[Shape; 2], index: u64,
        again: bool, shape: @Shape) -> u64
    requires index < 2 {
        transition { _ -> forwarded(shapes, sibling, index, again, shape) }
    }
    machine forwarded(shapes: @[Shape; 2], sibling: @[Shape; 2], index: u64,
        again: bool, shape: @Shape) -> u64
    requires index < 2 {
        transition again {
            true -> entry(shapes, sibling, index, false)
            false -> shape.value
        }
    }
}
"#;
    let mut input = permission_inputs(SOURCE);
    let graph_from_source = || {
        terminal_borrowed_storage_graph_from(
            &input.module,
            input.module.machines.iter().map(|machine| machine.id),
            &[],
        )
        .unwrap()
    };
    let graph = graph_from_source();
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&input.module.structural_types, &graph);
    let machine = input
        .module
        .machines
        .iter()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .any(|block| !block.structural_parameters.is_empty())
        })
        .expect("the actual source has borrowed successor headers");
    let original = machine.structural_parameters[0].place;
    let sibling = machine.structural_parameters[1].place;
    assert_ne!(original, sibling);
    let mut observed = Vec::new();
    for block in &machine.blocks {
        let at = (block.id, u32::try_from(block.operations.len()).unwrap());
        for parameter in &block.structural_parameters {
            let descriptor = DescriptorLocation::whole(parameter.place);
            let Some(DescriptorCallOrigin::Argument { source, projection }) =
                analysis.storage_projection(machine.id, at, &descriptor)
            else {
                continue;
            };
            if projection.captured_at.is_none() {
                continue;
            }
            assert_eq!(
                source, original,
                "the sibling input cannot replace the selected source"
            );
            assert!(
                projection
                    .captured_storage_path()
                    .unwrap()
                    .iter()
                    .any(|step| step.runtime_index().is_some())
            );
            assert_eq!(
                terminal_semantics::borrowed_loan_source(
                    &input.module,
                    machine,
                    &mut analysis,
                    at,
                    &descriptor,
                    std::iter::empty(),
                ),
                Ok(terminal_semantics::BorrowedLoanSource::Incoming)
            );
            observed.push((at, descriptor));
        }
    }
    assert!(
        observed.len() >= 2,
        "both selected and forwarded retain the captured address"
    );
    let (at, descriptor) = &observed[0];
    let mut missing = graph_from_source();
    let mut removed = 0;
    for machine in &mut missing {
        for step in machine.flow.values_mut() {
            for edge in &mut step.successors {
                for binding in &mut edge.bindings {
                    if let Some((_, projection)) = &mut binding.captured_storage {
                        *projection =
                            terminal_semantics::borrowed_storage::Projection::new(&projection.path);
                        removed += 1;
                    }
                }
            }
        }
    }
    assert!(removed > 0);
    let mut missing =
        DescriptorAnalysis::with_borrowed_storage_slots(&input.module.structural_types, &missing);
    assert!(
        terminal_semantics::borrowed_loan_source(
            &input.module,
            machine,
            &mut missing,
            *at,
            descriptor,
            std::iter::empty(),
        )
        .is_err(),
        "an indexed path without its actual capture cannot establish a loan"
    );
    let mut stale = graph_from_source();
    let mut changed = 0;
    for candidate in &mut stale {
        let stale_point = (candidate.entry.0, u32::MAX);
        assert!(!candidate.flow.contains_key(&stale_point));
        for step in candidate.flow.values_mut() {
            for edge in &mut step.successors {
                for binding in &mut edge.bindings {
                    if let Some((_, capture)) = &mut binding.captured_storage {
                        // Keep the complete source path, but name a capture
                        // operation that does not exist in this machine.
                        *capture = terminal_semantics::borrowed_storage::Projection::new(
                            &capture.captured_address_path().unwrap(),
                        )
                        .at(stale_point);
                        changed += 1;
                    }
                }
            }
        }
    }
    assert!(changed > 0);
    let mut stale =
        DescriptorAnalysis::with_borrowed_storage_slots(&input.module.structural_types, &stale);
    assert!(
        stale
            .captured_storage_family(machine.id, *at, descriptor)
            .is_err()
    );
    assert!(
        terminal_semantics::borrowed_loan_source(
            &input.module,
            machine,
            &mut stale,
            *at,
            descriptor,
            std::iter::empty(),
        )
        .is_err(),
        "a stale Some capture cannot establish an incoming loan"
    );
    let mut ambiguous = graph_from_source();
    let mut crossed = 0;
    for candidate in &mut ambiguous {
        if candidate.id != machine.id {
            continue;
        }
        for step in candidate.flow.values_mut() {
            let alternatives = step
                .successors
                .iter()
                .filter_map(|edge| {
                    let mut alternative = edge.clone();
                    let mut changed = false;
                    for binding in &mut alternative.bindings {
                        if binding.parameter != *descriptor {
                            continue;
                        }
                        if let Some((captured, projection)) = &mut binding.captured_storage
                            && projection.captured_storage_path().is_some_and(|path| {
                                path.iter().any(|step| step.runtime_index().is_some())
                            })
                        {
                            // The producer captures the current array name. Its replay
                            // resolves that name to the original input; the sibling is
                            // a same-typed but different incoming resource.
                            captured.root = sibling;
                            changed = true;
                        }
                    }
                    if changed {
                        crossed += 1;
                        Some(alternative)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            step.successors.extend(alternatives);
        }
    }
    assert!(crossed > 0);
    let mut ambiguous =
        DescriptorAnalysis::with_borrowed_storage_slots(&input.module.structural_types, &ambiguous);
    assert!(
        terminal_semantics::borrowed_loan_source(
            &input.module,
            machine,
            &mut ambiguous,
            *at,
            descriptor,
            std::iter::empty(),
        )
        .is_err(),
        "competing original and sibling arrivals cannot establish one parent"
    );
    let mut widened = machine.clone();
    widened
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.structural_parameters)
        .find(|parameter| parameter.place == descriptor.root)
        .unwrap()
        .access = StructuralAccess::MutableBorrow;
    assert_eq!(
        terminal_semantics::borrowed_loan_source(
            &input.module,
            &widened,
            &mut analysis,
            *at,
            descriptor,
            std::iter::empty(),
        ),
        Err("borrowed successor exceeds its original incoming access")
    );
    // Calling a nested machine directly gives it its own input references.
    // On a later cycle, shape can come from shapes[index]. Both must permit
    // forwarding without becoming one parent loan.
    let mut transported =
        DescriptorAnalysis::with_borrowed_storage_slots(&input.module.structural_types, &graph);
    let mut multiple_sources = 0;
    for candidate in &input.module.machines {
        for block in &candidate.blocks {
            let at = (block.id, u32::try_from(block.operations.len()).unwrap());
            for parameter in &block.structural_parameters {
                if parameter.access == StructuralAccess::Owned
                    || terminal_semantics::descriptor_results::is_descriptor(
                        input.module.structural_types.iter(),
                        parameter.structural_type,
                    )
                {
                    continue;
                }
                let descriptor = DescriptorLocation::whole(parameter.place);
                let family = transported
                    .captured_storage_family(candidate.id, at, &descriptor)
                    .expect("every borrowed header retains its complete input addresses");
                if family.len() < 2 {
                    continue;
                }
                terminal_semantics::borrowed_loan_sources(
                    &input.module,
                    candidate,
                    &mut transported,
                    at,
                    &descriptor,
                    std::iter::empty(),
                )
                .expect("every actual incoming resource must permit transport");
                assert!(
                    terminal_semantics::borrowed_loan_source(
                        &input.module,
                        candidate,
                        &mut transported,
                        at,
                        &descriptor,
                        std::iter::empty(),
                    )
                    .is_err(),
                    "different incoming resources cannot establish one immediate parent"
                );
                multiple_sources += 1;
            }
        }
    }
    assert!(
        multiple_sources > 0,
        "the independent member must exercise distinct incoming resources"
    );
    drop(transported);
    let identity = machine.id;
    drop(analysis);
    drop(missing);
    drop(stale);
    drop(ambiguous);
    // Check the entered machine's complete loan judgment, including both
    // successors, with the full source module and its actual tables intact.
    let mut activity = crate::checking::ProofSearchActivity::default();
    let mut scope = crate::checking::prepare_replay(&mut input, &mut activity).unwrap();
    let (formed, domains, checking, _, _) = scope.checking();
    let module = formed.source();
    let tables = checking.machines.get_mut(identity).unwrap();
    let prepared = crate::checking::prepare(formed, domains, identity, tables).unwrap();
    let mut references = prepared.references(&module.structural_types);
    let storage = crate::checking::places::Storage::new(
        identity,
        module.machine(identity).unwrap(),
        module,
        domains,
        &prepared.graph,
        tables,
        &prepared.types,
        &mut references,
    )
    .unwrap();
    let (loans, conflicts) = storage
        .loans(&mut references, module, tables, None)
        .unwrap();
    assert!(
        loans.is_empty(),
        "continuing an input must not create a loan"
    );
    assert!(conflicts.is_empty(), "{conflicts:?}");

    // Every independently lowered member must check too. Use fresh input:
    // the preparation above has already populated its checking tables.
    let whole = judge(Ok(permission_inputs(SOURCE)));
    assert!(
        matches!(&whole, Ok(RouteVerdict::Accepted(_))),
        "the unchanged indexed-reference cycle must check as a whole: {whole:?}"
    );
}
