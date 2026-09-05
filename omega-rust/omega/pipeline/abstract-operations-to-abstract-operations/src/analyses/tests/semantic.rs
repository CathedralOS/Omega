//! Semantic facts, effects, and liveness coverage.

use super::fixtures::*;
use crate::*;
use abstract_operations::AbstractOperation as O;
use optimization_core::*;
use optimization_unit::*;
use semantic_vocabulary::*;

#[test]
fn function_effects_propagate_services_and_crashes_through_calls() {
    let mut caller = function(100, 1, vec![(1, Terminator::Return)]);
    let mut callee = function(200, 2, vec![(2, Terminator::Crash)]);
    let call_support = id(510, OperationId::new);
    let mut call = node(O::CallUnit {
        psi_operation: call_support,
        callee: callee.machine,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    });
    call.provenance = vec![PsiProvenance::Operation(call_support)];
    caller.blocks[0].nodes.insert(0, call);
    let service_support = id(511, OperationId::new);
    let service = id(512, ServiceId::new);
    let mut write = node(O::PortWrite {
        psi_operation: service_support,
        service,
        port: 7,
        value: 9,
    });
    write.provenance = vec![PsiProvenance::Operation(service_support)];
    callee.blocks[0].nodes.insert(0, write);
    let unit = unit(vec![caller, callee], b"transitive-effects");

    let AnalysisProduct::EffectSummaries(effects) =
        compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(effects.functions.len(), 2);
    for summary in &effects.functions {
        assert_eq!(summary.observable, EffectKnowledge::Yes);
        assert_eq!(summary.crash, EffectKnowledge::Yes);
        assert_eq!(summary.services, vec![service]);
        assert_eq!(summary.revision, unit.identity);
        assert!(
            summary
                .support
                .contains(&PsiProvenance::Operation(service_support))
        );
    }
}
#[test]
fn literal_semantic_facts_keep_support_and_revision_regions() {
    let mut function = function(
        100,
        1,
        vec![
            (1, Terminator::Branch(2, 3)),
            (2, Terminator::Return),
            (3, Terminator::Crash),
        ],
    );
    let condition = id(12, ValueId::new);
    let integer = id(99, ValueId::new);
    let boolean_support = id(600, OperationId::new);
    let integer_support = id(601, OperationId::new);
    let integer_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap();
    function.parameters = vec![
        ValueDefinition {
            value: condition,
            scalar_type: ScalarType::Boolean,
            site: ValueDefinitionSite::FunctionParameter(0),
        },
        ValueDefinition {
            value: integer,
            scalar_type: ScalarType::Integer(integer_type),
            site: ValueDefinitionSite::FunctionParameter(1),
        },
    ];
    function.facts = vec![
        OptimizationFact::BooleanConstant {
            value: condition,
            constant: true,
            support: boolean_support,
        },
        OptimizationFact::IntegerConstant {
            value: integer,
            constant: IntegerValue::Unsigned(7),
            support: integer_support,
        },
    ];
    let unit = unit(vec![function], b"semantic-facts");

    let AnalysisProduct::ScalarConstants(constants) =
        compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap()
    else {
        unreachable!()
    };
    assert!(constants.facts.iter().all(|fact| {
        fact.valid_in.revision == unit.identity
            && fact.valid_in.machine == unit.entry
            && fact.valid_in.value == fact.value
    }));

    let AnalysisProduct::ExecutableEdges(edges) =
        compute_analysis(&unit, AnalysisKind::ExecutableEdges).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        edges
            .edges
            .iter()
            .map(|edge| (edge.knowledge, edge.support.clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                ExecutableEdgeKnowledge::KnownExecutable,
                ScalarConstantSupport {
                    operations: vec![boolean_support],
                    edges: vec![id(13, EdgeId::new)],
                }
            ),
            (
                ExecutableEdgeKnowledge::KnownInexecutable,
                ScalarConstantSupport {
                    operations: vec![boolean_support],
                    edges: Vec::new(),
                }
            ),
        ]
    );

    let AnalysisProduct::ValueRanges(ranges) =
        compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(ranges.facts.len(), 1);
    assert_eq!(ranges.facts[0].minimum, IntegerValue::Unsigned(7));
    assert_eq!(ranges.facts[0].maximum, IntegerValue::Unsigned(7));
    assert_eq!(ranges.facts[0].scalar_type, integer_type);
    assert!(matches!(
        ranges.facts[0].support,
        ValueRangeSupport::ScalarConstant(_)
    ));
    assert_eq!(ranges.facts[0].valid_in.scope, ValueRangeScope::EntireValue);
}

#[test]
fn scalar_constants_merge_only_feasible_block_parameter_bindings() {
    let (selected, parameter, supports, edges) =
        block_parameter_constant_unit(Some(true), IntegerValue::Unsigned(8));
    let AnalysisProduct::ScalarConstants(constants) =
        compute_analysis(&selected, AnalysisKind::ScalarConstants).unwrap()
    else {
        unreachable!()
    };
    let fact = constants
        .facts
        .iter()
        .find(|fact| fact.value == parameter)
        .expect("selected incoming constant reaches block parameter");
    assert_eq!(
        fact.constant,
        ScalarConstant::Integer(IntegerValue::Unsigned(7))
    );
    assert!(fact.identity.is_some());
    assert_eq!(fact.support.operations, vec![supports[0], supports[1]]);
    assert_eq!(fact.support.edges, vec![edges[0], edges[2]]);
    let AnalysisProduct::ExecutableEdges(executable) =
        compute_analysis(&selected, AnalysisKind::ExecutableEdges).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(
        executable
            .edges
            .iter()
            .map(|fact| (fact.edge, fact.knowledge))
            .collect::<Vec<_>>(),
        vec![
            (edges[0], ExecutableEdgeKnowledge::KnownExecutable),
            (edges[1], ExecutableEdgeKnowledge::KnownInexecutable),
            (edges[2], ExecutableEdgeKnowledge::KnownExecutable),
            (edges[3], ExecutableEdgeKnowledge::KnownInexecutable),
        ]
    );

    let (same, parameter, supports, edges) =
        block_parameter_constant_unit(None, IntegerValue::Unsigned(7));
    let AnalysisProduct::ScalarConstants(constants) =
        compute_analysis(&same, AnalysisKind::ScalarConstants).unwrap()
    else {
        unreachable!()
    };
    let fact = constants
        .facts
        .iter()
        .find(|fact| fact.value == parameter)
        .expect("equal feasible incoming values meet to one constant");
    assert_eq!(
        fact.constant,
        ScalarConstant::Integer(IntegerValue::Unsigned(7))
    );
    assert!(fact.identity.is_some());
    assert_eq!(fact.support.operations, vec![supports[1], supports[2]]);
    assert_eq!(fact.support.edges, edges);
    let AnalysisProduct::ExecutableEdges(executable) =
        compute_analysis(&same, AnalysisKind::ExecutableEdges).unwrap()
    else {
        unreachable!()
    };
    assert!(
        executable
            .edges
            .iter()
            .all(|fact| { fact.knowledge == ExecutableEdgeKnowledge::KnownExecutable })
    );

    let (different, parameter, _, _) =
        block_parameter_constant_unit(None, IntegerValue::Unsigned(8));
    let AnalysisProduct::ScalarConstants(constants) =
        compute_analysis(&different, AnalysisKind::ScalarConstants).unwrap()
    else {
        unreachable!()
    };
    assert!(constants.facts.iter().all(|fact| fact.value != parameter));
    assert!(
        analysis_dependencies(AnalysisKind::ScalarConstants)
            .unwrap()
            .contains(AnalysisKind::ControlFlowGraph)
    );
}

#[test]
fn effects_are_conservative_and_liveness_reaches_fixed_point() {
    let mut function = function(
        100,
        1,
        vec![
            (1, Terminator::Branch(2, 3)),
            (2, Terminator::Jump(4)),
            (3, Terminator::Jump(4)),
            (4, Terminator::Crash),
        ],
    );
    let condition = id(12, ValueId::new);
    let support = id(700, OperationId::new);
    let mut constant = node(O::BooleanConstant {
        psi_operation: support,
        result: condition,
        value: true,
    });
    constant.provenance = vec![PsiProvenance::Operation(support)];
    constant.definitions = vec![ValueDefinition {
        value: condition,
        scalar_type: ScalarType::Boolean,
        site: ValueDefinitionSite::Node {
            block: id(1, BlockId::new),
            node: 0,
        },
    }];
    function.blocks[0].nodes.insert(0, constant);
    function.blocks[0].nodes[1].uses = vec![ValueUse {
        value: condition,
        block: id(1, BlockId::new),
        node: 1,
    }];
    let unit = unit(vec![function], b"effects-liveness");

    let AnalysisProduct::EffectSummaries(effects) =
        compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(effects.nodes[0].class, EffectClass::PureScalar);
    assert_eq!(effects.nodes[0].observable, EffectKnowledge::No);
    assert_eq!(
        effects.nodes[0].support,
        vec![PsiProvenance::Operation(support)]
    );
    let crash = effects.nodes.last().unwrap();
    assert_eq!(crash.crash, EffectKnowledge::Yes);
    assert_eq!(crash.observable, EffectKnowledge::Yes);

    let AnalysisProduct::ValueLiveness(liveness) =
        compute_analysis(&unit, AnalysisKind::ValueLiveness).unwrap()
    else {
        unreachable!()
    };
    assert!(liveness.blocks[0].entry.is_empty());
    assert_eq!(liveness.blocks[0].nodes[0].exit, vec![condition]);
    assert_eq!(liveness.blocks[0].nodes[1].entry, vec![condition]);
    assert!(liveness.blocks[0].nodes[1].exit.is_empty());
    let independent = optimization_unit_semantics::reconstruct_closed_scalar_node_boundary(
        &unit,
        optimization_unit::NodeLocation {
            machine: id(100, MachineId::new),
            block: id(1, BlockId::new),
            node: 1,
        },
    )
    .unwrap();
    assert_eq!(independent.live_in, liveness.blocks[0].nodes[1].entry);
    assert_eq!(independent.live_out, liveness.blocks[0].nodes[1].exit);
}
