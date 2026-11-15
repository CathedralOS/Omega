//! Passing storage to a successor must keep its original borrow authority.

use super::*;
use language_core::ReferenceAccess;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_check_tables::edges::{Destination, EdgeRow};
use terminal_check_tables::obligations::ObligationRow;
use terminal_psi::{
    Block, StructuralAccess, StructuralArgument, StructuralPlaceDeclaration, Terminator,
};

fn duplicated_borrow() -> typed_trees_to_terminal_psi::Elaboration {
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
fn shared_array_views_keep_their_backing_when_sliced() {
    for (start, accepted) in [(1, true), (3, false)] {
        let source = format!(
            r#"
machine entry() -> u8 {{
    let bytes: [u8; 3] = [17, 42, 99];
    let view: &[u8] = &bytes[..];
    let suffix: &[u8] = view[{start}..];
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
machine inspect(bytes: &[u8]) -> bool { bytes.len == 3 }
machine entry(first: bool) -> bool {
    let packet: Packet = Packet { before: 11, bytes: [17, 42, 99], after: 97 };
    let view: &[u8] = &packet.bytes[..];
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
    // The same cleanup is valid on the false path, but would invalidate
    // the view before inspect on the continuing path.
    when_true.cleanup_actions = when_false.cleanup_actions.clone();
    let mut activity = crate::ProofSearchActivity::default();
    let mut scope = crate::prepare_replay(&mut input, &mut activity).unwrap();
    let (formed, domains, checking, _, _) = scope.checking();
    let module = formed.source();
    let tables = checking.machines.get_mut(identity).unwrap();
    let prepared = crate::prepare(formed, domains, identity, tables).unwrap();
    let mut references = prepared.references(&module.structural_types);
    let machine = module.machine(identity).unwrap();
    let storage = crate::places::Storage::new(
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
    let (_, conflicts) = storage.loans(&mut references, module, tables).unwrap();
    assert!(
        conflicts
            .iter()
            .any(|conflict| conflict.conflict == crate::LoanConflict::ChangeWhileBorrowed),
        "cleanup on the continuing edge must preserve the packet's live view"
    );
}

#[test]
fn an_empty_shared_payload_keeps_its_owned_permission_at_a_call() {
    let typed = super::boundary_calls::typed(
        r#"
data Holder<'held> { bytes: &'held [u8]; }
machine inspect<'a>(value: &'a [u8]) -> u64 { value.len }
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
            | Err(crate::CompilationRefusal::Elaboration(refusal)) => {
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
    let prepared = typed_trees_to_terminal_psi::prepare_compilation(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[checksum],
            subjects: &[],
            purpose: source::DependencyScope::Product,
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

fn permission_inputs(text: &str) -> typed_trees_to_terminal_psi::Elaboration {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    let program = typed_trees_to_terminal_psi::prepare_compilation(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
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
    let input = permission_inputs("machine entry(first: &[u8], second: &[u8]) {}");
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
data Pair<'held> { first: &'held [u8]; second: &'held [u8]; }
machine entry<'a>(pair: &Pair<'a>) {}
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
