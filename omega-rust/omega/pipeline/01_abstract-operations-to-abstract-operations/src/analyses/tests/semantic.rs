//! Semantic facts, effects, and liveness coverage.

use super::fixtures::*;
use crate::analyses::analysis_dependencies;
use crate::analyses::{
    ExecutableEdgeKnowledge, PlaceAliasRelation, PlaceView, ScalarConstantSupport,
};
use crate::{AnalysisProduct, EffectClass, EffectKnowledge, ScalarConstant, compute_analysis};
use abstract_operations::AbstractOperation as O;
use optimization_core::*;
use optimization_unit::*;
use semantic_vocabulary::*;

#[test]
fn sccp_case_paths_and_payloads_do_not_invent_join_constants() {
    // These are analysis fixtures, not claims of source-produced structural custody.
    for payload_arrival in [false, true] {
        let (mut input, parameter, _, _) =
            block_parameter_constant_unit(None, IntegerValue::Unsigned(8));
        let function = &mut input.functions[0];
        let scalar_type = function.blocks[3].parameters[0].scalar_type;
        let case_edge = id(501, EdgeId::new);
        let second_case_edge = id(502, EdgeId::new);
        let payloads = if payload_arrival {
            vec![abstract_operations::AbstractStructuralCasePayloadBinding {
                parameter,
                field: id(504, StructuralFieldId::new),
                scalar_type,
            }]
        } else {
            Vec::new()
        };
        let target = if payload_arrival { 4 } else { 3 };
        let case = O::StructuralCase {
            source: id(505, PlaceId::new),
            cases: [case_edge, second_case_edge]
                .into_iter()
                .enumerate()
                .map(
                    |(index, psi_edge)| abstract_operations::AbstractStructuralCaseSuccessor {
                        psi_edge,
                        target: id(target, BlockId::new),
                        case: id(506 + index as u64, StructuralCaseId::new),
                        payloads: payloads.clone(),
                        trivial_affine_discards: Vec::new(),
                    },
                )
                .collect(),
        };
        if payload_arrival {
            function.blocks[2].nodes = vec![node(case)];
        } else {
            let O::Conditional { when_false, .. } = &mut function.blocks[0].nodes[0].operation
            else {
                unreachable!()
            };
            when_false.target = id(5, BlockId::new);
            function.blocks.push(OptimizationBlock {
                id: id(5, BlockId::new),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                nodes: vec![node(case)],
            });
        }
        input.identity = recompute_psi_optimization_unit_identity(&input);
        let AnalysisProduct::ScalarConstants(constants) =
            compute_analysis(&input, AnalysisKind::ScalarConstants).unwrap()
        else {
            unreachable!()
        };
        assert!(
            constants.facts.iter().all(|fact| fact.value != parameter),
            "a case arrival must participate in the mixed join lattice"
        );
        let AnalysisProduct::ExecutableEdges(edges) =
            compute_analysis(&input, AnalysisKind::ExecutableEdges).unwrap()
        else {
            unreachable!()
        };
        for case_edge in [case_edge, second_case_edge] {
            assert!(edges.edges.iter().any(|edge| {
                edge.edge == case_edge && edge.knowledge == ExecutableEdgeKnowledge::KnownExecutable
            }));
        }
    }
}

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
fn place_aliases_classify_views_from_declared_roots_and_live_claims() {
    let parameter_a = id(80, PlaceId::new);
    let parameter_b = id(81, PlaceId::new);
    let attachment = id(82, PlaceId::new);
    let mut machine_fn = function(100, 1, vec![(1, Terminator::Return)]);
    machine_fn.structural_places = vec![
        terminal_psi::StructuralPlaceDeclaration {
            id: parameter_a,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: parameter_b,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: attachment,
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment: id(83, StructuralTypeId::new),
                field: id(84, StructuralFieldId::new),
                boundary: id(85, BoundaryMachineId::new),
            },
        },
    ];
    machine_fn.declared_places = [parameter_a, parameter_b, attachment].into_iter().collect();
    let mut input = unit(vec![machine_fn], b"place-aliases");
    let site = OwnershipFrontierSite::BlockEntry(id(1, BlockId::new));
    let exit = OwnershipFrontierSite::OperationExit(id(2, OperationId::new));
    input.ownership_frontier_facts = vec![
        OwnershipFrontierFact::new(
            input.psi,
            input.entry,
            site,
            OwnershipFrontierSnapshot {
                claims: vec![
                    OwnershipFrontierLiveClaim {
                        claim: id(90, ClaimId::new),
                        input: Some(parameter_a),
                        path: vec![terminal_psi::StructuralPathSegment::from("field")],
                        multiplicity: Some(terminal_psi::StructuralMultiplicity::Affine),
                    },
                    OwnershipFrontierLiveClaim {
                        claim: id(91, ClaimId::new),
                        input: None,
                        path: Vec::new(),
                        multiplicity: None,
                    },
                ],
                owned_places: Vec::new(),
                partial_custody: Vec::new(),
            },
        ),
        // The same view at a second site dedupes and joins its evidence.
        OwnershipFrontierFact::new(
            input.psi,
            input.entry,
            exit,
            OwnershipFrontierSnapshot {
                claims: vec![OwnershipFrontierLiveClaim {
                    claim: id(92, ClaimId::new),
                    input: Some(parameter_a),
                    path: vec![terminal_psi::StructuralPathSegment::from("field")],
                    multiplicity: Some(terminal_psi::StructuralMultiplicity::Affine),
                }],
                owned_places: Vec::new(),
                partial_custody: Vec::new(),
            },
        ),
    ];
    input.identity = recompute_psi_optimization_unit_identity(&input);

    let AnalysisProduct::PlaceAliases(aliases) =
        compute_analysis(&input, AnalysisKind::PlaceAliases).unwrap()
    else {
        unreachable!()
    };
    let machine = aliases.function(input.entry).unwrap();
    assert_eq!(
        machine
            .roots
            .iter()
            .map(|root| root.place)
            .collect::<Vec<_>>(),
        vec![parameter_a, parameter_b, attachment]
    );
    assert_eq!(machine.claims.len(), 1);
    assert_eq!(
        machine.claims[0].sites,
        vec![site, exit],
        "one deduplicated claim view keeps both frontier sites as evidence"
    );
    assert_eq!(machine.unrooted_claims, 1);

    let view = |root: PlaceId, path: &[terminal_psi::StructuralPathSegment]| PlaceView {
        root,
        path: path.to_vec(),
    };
    let field = terminal_psi::StructuralPathSegment::from("field");
    let other = terminal_psi::StructuralPathSegment::from("other");
    // Distinct declared roots are disjoint storage sites.
    assert_eq!(
        machine.relation(
            &view(parameter_a, std::slice::from_ref(&field)),
            &view(parameter_b, &[])
        ),
        PlaceAliasRelation::Disjoint
    );
    // A prefix view contains the longer one.
    assert_eq!(
        machine.relation(
            &view(parameter_a, &[]),
            &view(parameter_a, std::slice::from_ref(&field))
        ),
        PlaceAliasRelation::Overlapping
    );
    // Diverging fields on one root name disjoint extents.
    assert_eq!(
        machine.relation(
            &view(parameter_a, std::slice::from_ref(&field)),
            &view(parameter_a, std::slice::from_ref(&other))
        ),
        PlaceAliasRelation::Disjoint
    );
    // A continuation crossing the referent boundary leaves the carrier's
    // storage rather than extending it.
    assert_eq!(
        machine.relation(
            &view(parameter_a, std::slice::from_ref(&field)),
            &view(
                parameter_a,
                &[field.clone(), terminal_psi::StructuralPathSegment::Referent]
            )
        ),
        PlaceAliasRelation::Disjoint
    );
    // A referent crossing can reach storage owned through another root.
    assert_eq!(
        machine.relation(
            &view(
                parameter_a,
                &[terminal_psi::StructuralPathSegment::Referent]
            ),
            &view(parameter_b, &[])
        ),
        PlaceAliasRelation::Unknown
    );
    // Provider attachments are evidence without a placeable layout.
    assert_eq!(
        machine.relation(&view(parameter_a, &[]), &view(attachment, &[])),
        PlaceAliasRelation::Unknown
    );
    assert_eq!(
        machine.relation(&view(id(999, PlaceId::new), &[]), &view(parameter_a, &[])),
        PlaceAliasRelation::Unknown,
        "a root missing from the declared roster cannot be reasoned about"
    );

    // The roster-wide premise fails only because the attachment has no layout.
    assert!(!machine.declared_roots_disjoint());
    let mut plain = function(100, 1, vec![(1, Terminator::Return)]);
    plain.structural_places = vec![
        terminal_psi::StructuralPlaceDeclaration {
            id: parameter_a,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: parameter_b,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: attachment,
            kind: StructuralPlaceKind::Result,
        },
    ];
    plain.declared_places = [parameter_a, parameter_b, attachment].into_iter().collect();
    let plain_unit = unit(vec![plain], b"place-aliases-plain");
    let AnalysisProduct::PlaceAliases(aliases) =
        compute_analysis(&plain_unit, AnalysisKind::PlaceAliases).unwrap()
    else {
        unreachable!()
    };
    assert!(
        aliases
            .function(plain_unit.entry)
            .unwrap()
            .declared_roots_disjoint()
    );
    assert!(
        analysis_dependencies(AnalysisKind::PlaceAliases)
            .unwrap()
            .contains(AnalysisKind::OwnershipFrontiers),
        "frontier claims are the producer's evidence, so invalidation must cascade"
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
