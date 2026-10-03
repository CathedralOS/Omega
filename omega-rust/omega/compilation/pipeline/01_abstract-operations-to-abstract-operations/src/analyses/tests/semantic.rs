//! Semantic facts, effects, and liveness coverage.

use super::fixtures::*;
use crate::analyses::analysis_dependencies;
use crate::analyses::{
    CallResultOrigin, ExecutableEdgeKnowledge, PlaceAccessStep, PlaceAccessView,
    PlaceAliasRelation, PlaceView, ResultSourceResolution, ScalarConstantSupport, declared_access,
};
use crate::{AnalysisProduct, EffectKnowledge, compute_analysis};
use abstract_operations::{AbstractOperation as O, AbstractResult, AbstractStructuralCallResult};
use optimization_core::*;
use optimization_unit::*;
use semantic_vocabulary::*;

fn structural_parameter(
    place: u64,
    position: u32,
    access: terminal_psi::StructuralAccess,
) -> terminal_psi::StructuralParameterDeclaration {
    terminal_psi::StructuralParameterDeclaration {
        place: id(place, PlaceId::new),
        position,
        is_self: false,
        structural_type: id(9000 + position as u64, StructuralTypeId::new),
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn argument_view(
    place: u64,
    path: Vec<terminal_psi::StructuralPathSegment>,
    access: terminal_psi::StructuralAccess,
) -> terminal_psi::StructuralArgument {
    terminal_psi::StructuralArgument {
        place: id(place, PlaceId::new),
        path,
        access,
    }
}

fn call_unit(callee: u64, structural_arguments: Vec<terminal_psi::StructuralArgument>) -> O {
    O::CallStructural {
        psi_operation: id(700, OperationId::new),
        callee: id(callee, MachineId::new),
        arguments: Vec::new(),
        structural_arguments,
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        result: AbstractStructuralCallResult::Unit,
    }
}

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
    let mut call = node(O::CallStructural {
        psi_operation: call_support,
        callee: callee.machine,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        dynamic_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
        result: AbstractStructuralCallResult::Unit,
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

fn scalar_result(raw: u64) -> AbstractResult {
    AbstractResult {
        value: id(raw, ValueId::new),
        scalar_type: ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
        ),
    }
}

fn runtime_step(identity: &str) -> PlaceAccessStep {
    PlaceAccessStep::Runtime(terminal_psi::StructuralPathSegment::from(identity))
}

#[test]
fn place_access_summaries_project_observed_surfaces_through_calls() {
    use terminal_psi::StructuralAccess;
    let store_field = id(810, StructuralFieldId::new);

    // The leaf reads parameter 0's `len` field and writes parameter 1's `pos`
    // field — observed bounds its declared access rows alone cannot state.
    let mut leaf = function(200, 1, vec![(1, Terminator::Return)]);
    leaf.structural_parameters = vec![
        structural_parameter(200, 0, StructuralAccess::SharedBorrow),
        structural_parameter(201, 1, StructuralAccess::MutableBorrow),
    ];
    leaf.declared_places = [id(200, PlaceId::new), id(201, PlaceId::new)]
        .into_iter()
        .collect();
    leaf.blocks[0].nodes.insert(
        0,
        node(O::PrimitiveScalarRead {
            psi_operation: id(701, OperationId::new),
            result: scalar_result(702),
            source: id(200, PlaceId::new),
            path: vec![terminal_psi::StructuralPathSegment::from("len")],
        }),
    );
    leaf.blocks[0].nodes.insert(
        1,
        node(O::StructuralScalarFieldStore {
            psi_operation: id(703, OperationId::new),
            destination: leaf.structural_parameters[1].clone().into(),
            path: vec![terminal_psi::StructuralPathSegment::from("pos")],
            field: store_field,
            value: scalar_result(704),
            range_obligation: None,
        }),
    );

    // The mid caller lends two views of its own parameter to the leaf; the
    // root lends the mid one view of its parameter in turn.
    let mut mid = function(300, 1, vec![(1, Terminator::Return)]);
    mid.structural_parameters = vec![structural_parameter(300, 0, StructuralAccess::Owned)];
    mid.blocks[0].nodes.insert(
        0,
        node(call_unit(
            200,
            vec![
                argument_view(
                    300,
                    vec![terminal_psi::StructuralPathSegment::from("a")],
                    StructuralAccess::SharedBorrow,
                ),
                argument_view(
                    300,
                    vec![terminal_psi::StructuralPathSegment::from("b")],
                    StructuralAccess::MutableBorrow,
                ),
            ],
        )),
    );
    let mut root = function(100, 1, vec![(1, Terminator::Return)]);
    root.structural_parameters = vec![structural_parameter(100, 0, StructuralAccess::Owned)];
    root.blocks[0].nodes.insert(
        0,
        node(call_unit(
            300,
            vec![argument_view(
                100,
                vec![terminal_psi::StructuralPathSegment::from("m")],
                StructuralAccess::Owned,
            )],
        )),
    );

    let unit = unit(vec![root, leaf, mid], b"place-access-calls");
    let AnalysisProduct::PlaceAccessSummaries(summaries) =
        compute_analysis(&unit, AnalysisKind::PlaceAccessSummaries).unwrap()
    else {
        unreachable!()
    };

    let leaf_summary = summaries.function(id(200, MachineId::new)).unwrap();
    assert!(leaf_summary.parameters[0].is_read());
    assert!(!leaf_summary.parameters[0].is_written());
    assert_eq!(
        leaf_summary.parameters[0].reads,
        vec![vec![runtime_step("len")]]
    );
    assert!(leaf_summary.parameters[1].is_written());
    assert_eq!(
        leaf_summary.parameters[1].writes,
        vec![vec![
            runtime_step("pos"),
            PlaceAccessStep::Canonical(CanonicalStructuralPathSegment::Field(store_field)),
        ]]
    );
    assert_eq!(leaf_summary.revision, unit.identity);
    assert!(leaf_summary.result_sources.is_empty());
    assert!(!leaf_summary.unsummarized_targets);

    // A caller's summary keeps the may-bound at the lent argument view — the
    // callee's deeper spelling never composes onto it, which is what keeps
    // the transitive join finite through recursive components.
    let mid_summary = summaries.function(id(300, MachineId::new)).unwrap();
    assert_eq!(
        mid_summary.parameters[0].reads,
        vec![vec![runtime_step("a")]]
    );
    assert_eq!(
        mid_summary.parameters[0].writes,
        vec![vec![runtime_step("b")]]
    );
    let root_summary = summaries.function(id(100, MachineId::new)).unwrap();
    assert_eq!(
        root_summary.parameters[0].reads,
        vec![vec![runtime_step("m")]]
    );
    assert_eq!(
        root_summary.parameters[0].writes,
        vec![vec![runtime_step("m")]]
    );

    // The call-site query composes the callee's observed paths beneath the
    // call's own argument views.
    let arguments = vec![argument_view(
        100,
        vec![terminal_psi::StructuralPathSegment::from("m")],
        StructuralAccess::Owned,
    )];
    let call = summaries
        .call_accesses(id(100, MachineId::new), id(300, MachineId::new), &arguments)
        .unwrap();
    assert_eq!(call.arguments.len(), 1);
    assert!(call.arguments[0].signature_bound);
    assert_eq!(
        call.arguments[0].reads,
        vec![PlaceAccessView {
            owner: id(100, MachineId::new),
            root: id(100, PlaceId::new),
            path: vec![runtime_step("m"), runtime_step("a")],
        }]
    );
    assert_eq!(
        call.arguments[0].writes,
        vec![PlaceAccessView {
            owner: id(100, MachineId::new),
            root: id(100, PlaceId::new),
            path: vec![runtime_step("m"), runtime_step("b")],
        }]
    );
    assert!(call.result_references.is_empty());
    assert!(call.external_reads.is_empty() && call.external_writes.is_empty());
    assert!(!call.unsummarized_targets);

    // A machine the unit does not carry has no summary to project through.
    assert!(
        summaries
            .call_accesses(id(100, MachineId::new), id(999, MachineId::new), &[])
            .is_none()
    );
}

#[test]
fn place_access_summaries_resolve_aliases_external_roots_and_result_sources() {
    use terminal_psi::StructuralAccess;
    let index_value = id(720, ValueId::new);
    let index_obligation = id(721, ObligationId::new);
    let runtime_index = terminal_psi::StructuralPathSegment::RuntimeIndex {
        index: index_value,
        obligation: index_obligation,
    };

    let mut callee = function(
        200,
        1,
        vec![(1, Terminator::Jump(2)), (2, Terminator::Return)],
    );
    callee.structural_parameters =
        vec![structural_parameter(200, 0, StructuralAccess::SharedBorrow)];
    let view_place = id(210, PlaceId::new);
    let carrier_place = id(211, PlaceId::new);
    let local_place = id(212, PlaceId::new);
    let result_place = id(220, PlaceId::new);
    callee.structural_places = vec![
        terminal_psi::StructuralPlaceDeclaration {
            id: view_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id(730, OperationId::new),
                structural_type: id(9010, StructuralTypeId::new),
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: carrier_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id(731, OperationId::new),
                structural_type: id(9011, StructuralTypeId::new),
            },
        },
        terminal_psi::StructuralPlaceDeclaration {
            id: local_place,
            kind: StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: 0,
                structural_type: id(9012, StructuralTypeId::new),
                construction: None,
            },
        },
    ];
    callee.declared_places = [
        id(200, PlaceId::new),
        view_place,
        carrier_place,
        local_place,
        result_place,
    ]
    .into_iter()
    .collect();
    let result_type = id(9013, StructuralTypeId::new);
    callee.result = abstract_operations::AbstractFunctionResult::Structural(
        terminal_psi::StructuralResultDeclaration {
            place: result_place,
            structural_type: result_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: vec![terminal_psi::StructuralReferenceResultSource {
                path: vec![terminal_psi::StructuralPathSegment::from("out")],
                sources: vec![argument_view(
                    200,
                    vec![terminal_psi::StructuralPathSegment::from("inner")],
                    StructuralAccess::SharedBorrow,
                )],
            }],
        },
    );

    callee.blocks[0].nodes.insert(
        0,
        node(O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::WholeCollection,
            psi_operation: id(730, OperationId::new),
            result: terminal_psi::StructuralOperationResult {
                place: view_place,
                structural_type: id(9010, StructuralTypeId::new),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            destination: local_place,
            source: argument_view(
                200,
                vec![terminal_psi::StructuralPathSegment::from("items")],
                StructuralAccess::SharedBorrow,
            ),
            element: id(9014, StructuralTypeId::new),
        }),
    );
    callee.blocks[0].nodes.insert(
        1,
        node(O::EstablishReference {
            psi_operation: id(731, OperationId::new),
            result: terminal_psi::StructuralOperationResult {
                place: carrier_place,
                structural_type: result_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            },
            source: argument_view(
                200,
                vec![terminal_psi::StructuralPathSegment::from("cell")],
                StructuralAccess::SharedBorrow,
            ),
        }),
    );
    // The element view composes its source prefix beneath the read path; the
    // runtime index stays inside the callee's own summary row.
    callee.blocks[0].nodes.insert(
        2,
        node(O::PrimitiveScalarRead {
            psi_operation: id(732, OperationId::new),
            result: scalar_result(733),
            source: view_place,
            path: vec![
                runtime_index.clone(),
                terminal_psi::StructuralPathSegment::from("x"),
            ],
        }),
    );
    // Crossing the carrier's referent boundary resolves to the borrowed
    // parameter's storage beneath the retained source projection.
    callee.blocks[0].nodes.insert(
        3,
        node(O::PrimitiveScalarRead {
            psi_operation: id(734, OperationId::new),
            result: scalar_result(735),
            source: carrier_place,
            path: vec![
                terminal_psi::StructuralPathSegment::Referent,
                terminal_psi::StructuralPathSegment::from("val"),
            ],
        }),
    );
    // A root outside the declared roster is external evidence, not silence.
    callee.blocks[0].nodes.insert(
        4,
        node(O::PrimitiveScalarRead {
            psi_operation: id(736, OperationId::new),
            result: scalar_result(737),
            source: id(888, PlaceId::new),
            path: Vec::new(),
        }),
    );
    let O::Jump {
        structural_bindings,
        ..
    } = &mut callee.blocks[0].nodes[5].operation
    else {
        unreachable!()
    };
    structural_bindings.push(abstract_operations::AbstractStructuralBinding {
        parameter: id(230, PlaceId::new),
        argument: argument_view(
            200,
            vec![terminal_psi::StructuralPathSegment::from("bound")],
            StructuralAccess::SharedBorrow,
        ),
    });
    // The jump binds block 2's structural parameter to `param0.bound`; a read
    // of the block parameter resolves to the bound caller-side storage.
    callee.blocks[1].structural_parameters =
        vec![structural_parameter(230, 0, StructuralAccess::SharedBorrow)];
    callee.blocks[1].nodes.insert(
        0,
        node(O::PrimitiveScalarRead {
            psi_operation: id(738, OperationId::new),
            result: scalar_result(739),
            source: id(230, PlaceId::new),
            path: Vec::new(),
        }),
    );
    let O::ReturnUnit { .. } = callee.blocks[1].nodes[1].operation else {
        unreachable!()
    };
    callee.blocks[1].nodes[1].operation = O::ReturnStructural {
        psi_edge: id(740, EdgeId::new),
        source: local_place,
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    // An unreachable block's accesses never pollute the summary.
    callee.blocks.push(OptimizationBlock {
        id: id(3, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        nodes: vec![node(O::PrimitiveScalarRead {
            psi_operation: id(741, OperationId::new),
            result: scalar_result(742),
            source: id(200, PlaceId::new),
            path: vec![terminal_psi::StructuralPathSegment::from("dead")],
        })],
    });

    let mut caller = function(100, 1, vec![(1, Terminator::Return)]);
    caller.structural_parameters = vec![structural_parameter(100, 0, StructuralAccess::Owned)];
    caller.blocks[0].nodes.insert(
        0,
        node(call_unit(
            200,
            vec![argument_view(
                100,
                vec![terminal_psi::StructuralPathSegment::from("m")],
                StructuralAccess::SharedBorrow,
            )],
        )),
    );
    // A machine the unit does not carry bounds its argument lane by the
    // declared access alone and marks the external surface a lower bound.
    caller.blocks[0].nodes.insert(
        1,
        node(call_unit(
            999,
            vec![argument_view(
                100,
                vec![terminal_psi::StructuralPathSegment::from("opaque")],
                StructuralAccess::MutableBorrow,
            )],
        )),
    );

    let unit = unit(vec![caller, callee], b"place-access-aliases");
    let AnalysisProduct::PlaceAccessSummaries(summaries) =
        compute_analysis(&unit, AnalysisKind::PlaceAccessSummaries).unwrap()
    else {
        unreachable!()
    };

    let callee_summary = summaries.function(id(200, MachineId::new)).unwrap();
    let mut expected_reads = vec![
        vec![
            runtime_step("items"),
            PlaceAccessStep::Runtime(runtime_index),
            runtime_step("x"),
        ],
        vec![runtime_step("cell"), runtime_step("val")],
        vec![runtime_step("bound")],
    ];
    expected_reads.sort();
    assert_eq!(callee_summary.parameters[0].reads, expected_reads);
    assert!(!callee_summary.parameters[0].is_written());
    assert_eq!(
        callee_summary.external_reads,
        vec![PlaceAccessView {
            owner: id(200, MachineId::new),
            root: id(888, PlaceId::new),
            path: Vec::new(),
        }]
    );
    assert_eq!(
        callee_summary.result_sources.len(),
        1,
        "one declared result reference leaf resolves once at compute time"
    );
    assert_eq!(
        callee_summary.result_sources[0].origins,
        vec![ResultSourceResolution::Parameter {
            position: 0,
            path: vec![runtime_step("inner")],
        }]
    );

    let caller_summary = summaries.function(id(100, MachineId::new)).unwrap();
    assert!(caller_summary.unsummarized_targets);
    assert_eq!(
        caller_summary.parameters[0].writes,
        vec![vec![runtime_step("opaque")]],
        "the unsummarized callee's argument lane keeps only its declared access bound"
    );
    assert!(
        caller_summary
            .external_reads
            .iter()
            .any(|view| view.owner == id(200, MachineId::new) && view.root == id(888, PlaceId::new)),
        "callee external rows propagate to the caller's external surface"
    );

    let arguments = vec![argument_view(
        100,
        vec![terminal_psi::StructuralPathSegment::from("m")],
        StructuralAccess::SharedBorrow,
    )];
    let call = summaries
        .call_accesses(id(100, MachineId::new), id(200, MachineId::new), &arguments)
        .unwrap();
    // The runtime index quotes a callee-local scalar: the caller-facing bound
    // truncates just before it.
    assert!(
        call.arguments[0].reads.contains(&PlaceAccessView {
            owner: id(100, MachineId::new),
            root: id(100, PlaceId::new),
            path: vec![runtime_step("m"), runtime_step("items")],
        }),
        "callee read paths truncate at their first runtime segment"
    );
    assert_eq!(
        call.result_references[0].origins,
        vec![CallResultOrigin::Argument(PlaceAccessView {
            owner: id(100, MachineId::new),
            root: id(100, PlaceId::new),
            path: vec![runtime_step("m"), runtime_step("inner")],
        })]
    );
}

#[test]
fn place_access_summaries_declared_bounds_cover_unobserved_lanes() {
    use terminal_psi::StructuralAccess;
    assert_eq!(
        declared_access(StructuralAccess::SharedBorrow),
        (true, false)
    );
    assert_eq!(
        declared_access(StructuralAccess::MutableBorrow),
        (true, true)
    );
    assert_eq!(
        declared_access(StructuralAccess::WriteOnlyBorrow),
        (false, true)
    );
    assert_eq!(declared_access(StructuralAccess::Owned), (true, true));

    // A self-recursive callee still converges: the bound inserts the lent
    // argument view itself, so the fixpoint cannot manufacture deeper rows
    // beyond the callee's own observed ones.
    let mut spinner = function(400, 1, vec![(1, Terminator::Return)]);
    spinner.structural_parameters =
        vec![structural_parameter(400, 0, StructuralAccess::SharedBorrow)];
    spinner.blocks[0].nodes.insert(
        0,
        node(O::PrimitiveScalarRead {
            psi_operation: id(760, OperationId::new),
            result: scalar_result(761),
            source: id(400, PlaceId::new),
            path: Vec::new(),
        }),
    );
    spinner.blocks[0].nodes.insert(
        1,
        node(call_unit(
            400,
            vec![argument_view(
                400,
                Vec::new(),
                StructuralAccess::SharedBorrow,
            )],
        )),
    );
    let unit = unit(vec![spinner], b"place-access-recursion");
    let AnalysisProduct::PlaceAccessSummaries(summaries) =
        compute_analysis(&unit, AnalysisKind::PlaceAccessSummaries).unwrap()
    else {
        unreachable!()
    };
    let spinner_summary = summaries.function(id(400, MachineId::new)).unwrap();
    assert_eq!(spinner_summary.parameters[0].reads, vec![Vec::new()]);
    assert!(!spinner_summary.unsummarized_targets);
}
