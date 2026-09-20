//! CFG, dominance, SCC, loop, and call-graph coverage.

use super::fixtures::*;
use crate::{AnalysisProduct, EffectClass, EffectKnowledge, ExitKind, compute_analysis};
use abstract_operations::AbstractOperation as O;
use optimization_core::*;
use semantic_vocabulary::*;

#[test]
fn cfg_products_cover_crash_exits_disconnected_machines_and_dominance() {
    let unit = unit(
        vec![
            function(
                100,
                1,
                vec![
                    (1, Terminator::Branch(2, 3)),
                    (2, Terminator::Jump(4)),
                    (3, Terminator::Jump(4)),
                    (4, Terminator::Branch(5, 6)),
                    (5, Terminator::Return),
                    (6, Terminator::Crash),
                ],
            ),
            function(200, 11, vec![(11, Terminator::Return)]),
        ],
        b"cfg",
    );
    let AnalysisProduct::ControlFlowGraph(cfg) =
        compute_analysis(&unit, AnalysisKind::ControlFlowGraph).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(cfg.functions.len(), 2);
    assert_eq!(cfg.functions[0].blocks[4].exits, vec![ExitKind::Normal]);
    assert_eq!(cfg.functions[0].blocks[5].exits, vec![ExitKind::Crash]);
    assert!(
        cfg.functions
            .iter()
            .all(|function| { function.blocks.iter().all(|block| block.reachable) })
    );

    let AnalysisProduct::Dominators(dominators) =
        compute_analysis(&unit, AnalysisKind::Dominators).unwrap()
    else {
        unreachable!()
    };
    let join = &dominators.functions[0].1[3];
    assert_eq!(join.0, id(4, BlockId::new));
    assert_eq!(join.1, vec![id(1, BlockId::new), id(4, BlockId::new)]);

    let AnalysisProduct::PostDominators(post) =
        compute_analysis(&unit, AnalysisKind::PostDominators).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        post.functions[0].1[0],
        (
            id(1, BlockId::new),
            vec![id(1, BlockId::new), id(4, BlockId::new)],
        )
    );
}

#[test]
fn irreducible_loop_and_scc_are_reported_canonically() {
    let unit = unit(
        vec![function(
            100,
            1,
            vec![
                (1, Terminator::Branch(2, 3)),
                (2, Terminator::Jump(4)),
                (3, Terminator::Jump(4)),
                (4, Terminator::Branch(2, 5)),
                (5, Terminator::Return),
            ],
        )],
        b"loop",
    );
    let AnalysisProduct::LoopForest(loops) =
        compute_analysis(&unit, AnalysisKind::LoopForest).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(loops.functions[0].1.len(), 1);
    assert_eq!(
        loops.functions[0].1[0].blocks,
        vec![id(2, BlockId::new), id(4, BlockId::new)]
    );
    assert!(loops.functions[0].1[0].irreducible);
    assert_eq!(loops.functions[0].1[0].header, None);
    let AnalysisProduct::StronglyConnectedComponents(components) =
        compute_analysis(&unit, AnalysisKind::StronglyConnectedComponents).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        components.functions[0].1,
        vec![
            vec![id(1, BlockId::new)],
            vec![id(2, BlockId::new), id(4, BlockId::new)],
            vec![id(3, BlockId::new)],
            vec![id(5, BlockId::new)],
        ]
    );
}

#[test]
fn call_graph_marks_mutual_recursion() {
    let mut first = function(100, 1, vec![(1, Terminator::Return)]);
    let mut second = function(200, 2, vec![(2, Terminator::Return)]);
    first.blocks[0].nodes.insert(
        0,
        node(O::CallUnit {
            psi_operation: id(501, semantic_vocabulary::OperationId::new),
            callee: second.machine,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }),
    );
    second.blocks[0].nodes.insert(
        0,
        node(O::CallUnit {
            psi_operation: id(502, semantic_vocabulary::OperationId::new),
            callee: first.machine,
            arguments: Vec::new(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }),
    );
    let unit = unit(vec![first, second], b"calls");
    let AnalysisProduct::EffectSummaries(effects) =
        compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(effects.nodes[0].class, EffectClass::InternalCall);
    assert_eq!(effects.nodes[0].observable, EffectKnowledge::May);
    assert_eq!(effects.nodes[0].suspension, EffectKnowledge::May);
    let AnalysisProduct::CallGraph(calls) =
        compute_analysis(&unit, AnalysisKind::CallGraph).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(calls.recursive_components, calls.components);
    assert_eq!(calls.recursive_components.len(), 1);
}

#[test]
fn call_graph_tracks_cleanup_and_stored_dynamic_transitions() {
    use abstract_operations::{
        AbstractResult, AbstractStoredDynamicDescriptor, AbstractStoredDynamicDispatch,
    };
    use terminal_psi::{
        ClosedConformanceApplication, NominalAffineCleanup, StructuralAccess, StructuralArgument,
        TerminalAffineCleanupAction, TerminalDynamicConformanceSelection,
        TerminalStoredDynamicDescriptor, TerminalStoredDynamicDispatch,
    };

    let caller_machine = id(100, MachineId::new);
    let cleanup_machine = id(200, MachineId::new);
    let stored_target = id(300, MachineId::new);
    let call_operation = id(501, OperationId::new);
    let application = ClosedConformanceApplication {
        owner: caller_machine,
        declaration_identity: "test::CarrierImplementsScanner".into(),
        telescope: Vec::new(),
        subject_identity: Some("test::Carrier".into()),
        trait_identity: "test::Scanner".into(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: Vec::new(),
        rows: Vec::new(),
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    let stored_dispatch = AbstractStoredDynamicDispatch {
        stored: AbstractStoredDynamicDescriptor {
            selection: TerminalDynamicConformanceSelection {
                owner: caller_machine,
                ordinal: 0,
                source: StructuralArgument {
                    place: id(502, PlaceId::new),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                },
                conformance_application_report_fingerprint: application.report_fingerprint,
                conformance_application_commitment: application.commitment,
            },
            descriptor: TerminalStoredDynamicDescriptor {
                owner: caller_machine,
                ordinal: 5,
                establishment_operation: call_operation,
                selection_ordinal: 0,
                aggregate_type_identity: "test::Holder".into(),
                field_identity: "handler".into(),
            },
            application,
        },
        dispatch: TerminalStoredDynamicDispatch {
            owner: caller_machine,
            operation: call_operation,
            descriptor_ordinal: 5,
            declaring_trait_identity: "test::Scanner".into(),
            public_requirement_identity: "test::Scanner::scan()".into(),
            family_tuple: Vec::new(),
            requirement_identity: "test::Scanner::scan".into(),
            realization_identity: "test::Carrier::scan".into(),
            realization_callable_identity: "test::Carrier::scan::callable".into(),
            realization: stored_target,
        },
    };

    let mut caller = function(100, 1, vec![(1, Terminator::Return)]);
    caller.blocks[0].nodes.insert(
        0,
        node(O::CallStoredDynamicScalar {
            psi_operation: call_operation,
            result: AbstractResult {
                value: id(503, ValueId::new),
                scalar_type: ScalarType::Boolean,
            },
            dynamic_dispatch: stored_dispatch,
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        }),
    );
    let O::ReturnUnit {
        cleanup_actions, ..
    } = &mut caller.blocks[0].nodes[1].operation
    else {
        unreachable!("fixture return")
    };
    *cleanup_actions = vec![TerminalAffineCleanupAction::InvokeNominal(
        NominalAffineCleanup {
            place: id(504, PlaceId::new),
            structural_type: id(505, StructuralTypeId::new),
            cleanup_machine,
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    )];

    let unit = unit(
        vec![
            caller,
            function(200, 2, vec![(2, Terminator::Return)]),
            function(300, 3, vec![(3, Terminator::Return)]),
        ],
        b"transitions",
    );
    let AnalysisProduct::CallGraph(calls) =
        compute_analysis(&unit, AnalysisKind::CallGraph).unwrap()
    else {
        unreachable!()
    };
    let transitions = calls
        .callees
        .iter()
        .find_map(|(machine, callees)| (*machine == caller_machine).then_some(callees))
        .expect("caller call graph row");
    assert_eq!(transitions, &[cleanup_machine, stored_target]);
}
