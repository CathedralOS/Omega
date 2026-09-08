//! Source-produced sum inspection through the independent abstract validator.

use super::*;

fn inspection_unit() -> PsiOptimizationUnit {
    let source = r#"
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine write_byte(value: i32) reaches Console;
            machine exit_process(code: i32) reaches Console;
        }
        data Main {}
        machine Main::main() {
            let result: ByteRead = Console::read_byte();
            transition result {
                ByteRead::Byte { value } -> byte(value)
                ByteRead::Eof -> eof()
            }
            state byte(value: i32) {
                Console::write_byte(value);
                Console::exit_process(70);
            }
            state eof() { Console::exit_process(70); }
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check");
    let terminal =
        checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main").expect("Terminal");
    let semantic =
        terminal_codec::encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = terminal_codec::encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verified abstract lowering");
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("reconstruct abstract unit");
    unit.services = terminal.semantic_module.services.clone().into();
    unit.root_service_reach = terminal.semantic_module.root_service_reach.clone();
    refresh_identity(&mut unit);
    unit
}

#[test]
fn source_case_dispatch_preserves_payload_edges_and_cleanup() {
    let unit = inspection_unit();
    validate_psi_optimization_unit(&unit).expect("source case dispatch must validate");
    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == unit.entry)
        .unwrap();
    let node = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find(|node| matches!(node.operation, AbstractOperation::StructuralCase { .. }))
        .unwrap();
    let AbstractOperation::StructuralCase { source, cases } = &node.operation else {
        unreachable!()
    };
    assert_eq!(node.successors.len(), 2);
    assert!(
        node.uses.is_empty(),
        "payloads are edge-produced, not dominating scalar arguments"
    );
    for (case, edge) in cases.iter().zip(&node.successors) {
        assert_eq!(case.psi_edge, edge.psi_edge);
        assert_eq!(case.target, edge.target);
        assert_eq!(edge.trivial_affine_discards, [*source]);
        assert_eq!(edge.fuel.len(), 1);
        assert_eq!(edge.fuel[0].units, 1);
        let target = function
            .blocks
            .iter()
            .find(|block| block.id == case.target)
            .unwrap();
        assert_eq!(case.payloads.len(), target.parameters.len());
    }
}

fn case_location(unit: &PsiOptimizationUnit) -> (usize, usize, usize) {
    unit.functions
        .iter()
        .enumerate()
        .find_map(|(function_index, function)| {
            function
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    block
                        .nodes
                        .iter()
                        .position(|node| {
                            matches!(node.operation, AbstractOperation::StructuralCase { .. })
                        })
                        .map(|node_index| (function_index, block_index, node_index))
                })
        })
        .expect("case terminator")
}

#[test]
fn case_dispatch_rejects_reauthenticated_roster_and_payload_substitutions() {
    let baseline = inspection_unit();
    let (function_index, block_index, node_index) = case_location(&baseline);
    for mutation in [
        "omitted case",
        "reordered cases",
        "unknown case",
        "missing payload",
        "wrong field",
        "wrong parameter",
        "wrong type",
        "wrong target",
    ] {
        let mut changed = baseline.clone();
        let operation =
            &mut changed.functions[function_index].blocks[block_index].nodes[node_index].operation;
        let AbstractOperation::StructuralCase { cases, .. } = operation else {
            unreachable!()
        };
        match mutation {
            "omitted case" => {
                cases.pop();
            }
            "reordered cases" => cases.swap(0, 1),
            "unknown case" => cases[0].case = id(999, semantic_vocabulary::StructuralCaseId::new),
            "missing payload" => cases
                .iter_mut()
                .find(|case| !case.payloads.is_empty())
                .unwrap()
                .payloads
                .clear(),
            "wrong field" => {
                cases
                    .iter_mut()
                    .flat_map(|case| &mut case.payloads)
                    .next()
                    .unwrap()
                    .field = id(999, semantic_vocabulary::StructuralFieldId::new)
            }
            "wrong parameter" => {
                cases
                    .iter_mut()
                    .flat_map(|case| &mut case.payloads)
                    .next()
                    .unwrap()
                    .parameter = id(999, ValueId::new)
            }
            "wrong type" => {
                cases
                    .iter_mut()
                    .flat_map(|case| &mut case.payloads)
                    .next()
                    .unwrap()
                    .scalar_type = ScalarType::Boolean
            }
            "wrong target" => {
                let target = cases[0].target;
                cases[0].target = cases[1].target;
                cases[1].target = target;
            }
            _ => unreachable!(),
        }
        refresh_node_derivatives(&mut changed, function_index, block_index, node_index);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn case_dispatch_rejects_detached_edges_fuel_and_cleanup() {
    let baseline = inspection_unit();
    let (function_index, block_index, node_index) = case_location(&baseline);
    for mutation in [
        "missing edge",
        "wrong edge",
        "free edge",
        "lost cleanup",
        "duplicate cleanup",
    ] {
        let mut changed = baseline.clone();
        let node = &mut changed.functions[function_index].blocks[block_index].nodes[node_index];
        match mutation {
            "missing edge" => {
                node.successors.pop();
            }
            "wrong edge" => node.successors[0].psi_edge = id(999, EdgeId::new),
            "free edge" => node.successors[0].fuel[0].units = 0,
            "lost cleanup" | "duplicate cleanup" => {
                let AbstractOperation::StructuralCase { cases, .. } = &mut node.operation else {
                    unreachable!()
                };
                if mutation == "lost cleanup" {
                    cases[0].trivial_affine_discards.clear();
                } else {
                    let discard = cases[0].trivial_affine_discards[0];
                    cases[0].trivial_affine_discards.push(discard);
                }
                refresh_node_derivatives(&mut changed, function_index, block_index, node_index);
            }
            _ => unreachable!(),
        }
        refresh_identity(&mut changed);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn case_dispatch_rejects_unknown_and_nondominating_sources() {
    let baseline = inspection_unit();
    let (function_index, block_index, node_index) = case_location(&baseline);
    let mut unknown = baseline.clone();
    let AbstractOperation::StructuralCase { source, .. } =
        &mut unknown.functions[function_index].blocks[block_index].nodes[node_index].operation
    else {
        unreachable!()
    };
    *source = id(999, PlaceId::new);
    refresh_node_derivatives(&mut unknown, function_index, block_index, node_index);
    assert!(matches!(
        validate_psi_optimization_unit(&unknown),
        Err(OptimizationUnitValidationError::InvalidStructuralCaseDispatch { .. })
    ));

    // Move the real producer to one arm, preserving its identity and updating
    // derivative metadata. The dispatch can no longer observe it on entry.
    let mut unavailable = baseline;
    let function = &mut unavailable.functions[function_index];
    let producer = function.blocks[block_index].nodes.remove(0);
    let destination = (0..function.blocks.len())
        .find(|candidate| *candidate != block_index)
        .unwrap();
    function.blocks[destination].nodes.insert(0, producer);
    let mut effect = 0;
    for block in &mut function.blocks {
        for node in &mut block.nodes {
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect + 1,
            };
            effect += 1;
        }
    }
    for current_block in 0..unavailable.functions[function_index].blocks.len() {
        for current_node in 0..unavailable.functions[function_index].blocks[current_block]
            .nodes
            .len()
        {
            refresh_node_derivatives(
                &mut unavailable,
                function_index,
                current_block,
                current_node,
            );
        }
    }
    assert!(matches!(
        validate_psi_optimization_unit(&unavailable),
        Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { .. })
    ));
}

#[test]
fn case_edge_replays_owned_result_discard_against_retained_frontiers() {
    use optimization_unit::{
        OwnershipFrontierOwnedPlace, OwnershipFrontierSite, OwnershipFrontierSnapshot,
    };
    let mut unit = inspection_unit();
    let (function_index, block_index, node_index) = case_location(&unit);
    let function = &unit.functions[function_index];
    let AbstractOperation::StructuralCase { source, cases } =
        &function.blocks[block_index].nodes[node_index].operation
    else {
        unreachable!()
    };
    let entry = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        partial_custody: Vec::new(),
        owned_places: vec![OwnershipFrontierOwnedPlace {
            place: *source,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        }],
    };
    let exit = OwnershipFrontierSnapshot {
        owned_places: Vec::new(),
        ..entry.clone()
    };
    assert!(valid_edge_affine_transition(
        function,
        &entry,
        &exit,
        &[*source]
    ));
    assert!(!valid_edge_affine_transition(function, &entry, &exit, &[]));
    assert!(!valid_edge_affine_transition(
        function,
        &entry,
        &entry,
        &[*source]
    ));
    assert!(!valid_edge_affine_transition(
        function,
        &entry,
        &exit,
        &[*source, *source]
    ));
    for case in cases {
        for (site, snapshot) in [
            (
                OwnershipFrontierSite::EdgeEntry(case.psi_edge),
                entry.clone(),
            ),
            (OwnershipFrontierSite::EdgeExit(case.psi_edge), exit.clone()),
        ] {
            unit.ownership_frontier_facts
                .push(OwnershipFrontierFact::new(
                    unit.psi,
                    function.machine,
                    site,
                    snapshot,
                ));
        }
    }
    unit.ownership_frontier_facts
        .sort_by_key(|fact| (fact.machine, fact.site));
    refresh_identity(&mut unit);
    validate_psi_optimization_unit(&unit).expect("case cleanup rejoins retained ownership");
}

#[test]
fn case_dispatch_requires_readable_borrowed_sources() {
    let baseline = inspection_unit();
    let (function_index, block_index, node_index) = case_location(&baseline);
    for access in [
        terminal_psi::StructuralAccess::SharedBorrow,
        terminal_psi::StructuralAccess::MutableBorrow,
        terminal_psi::StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut unit = baseline.clone();
        let function = &mut unit.functions[function_index];
        let AbstractOperation::StructuralCase { source, .. } =
            &mut function.blocks[block_index].nodes[node_index].operation
        else {
            unreachable!()
        };
        let original = *source;
        let borrowed = id(999, PlaceId::new);
        *source = borrowed;
        let structural_type = function
            .structural_places
            .iter()
            .find_map(|place| match place.kind {
                StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } if place.id == original => Some(structural_type),
                _ => None,
            })
            .unwrap();
        function
            .structural_parameters
            .push(terminal_psi::StructuralParameterDeclaration {
                place: borrowed,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            });
        function
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: borrowed,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            });
        function.declared_places.insert(borrowed);
        refresh_node_derivatives(&mut unit, function_index, block_index, node_index);
        let result = validate_psi_optimization_unit(&unit);
        if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
            assert!(matches!(
                result,
                Err(OptimizationUnitValidationError::InvalidStructuralCaseDispatch { .. })
            ));
        } else {
            result.expect("readable case source retains referent observation");
        }
    }
}

#[test]
fn case_dispatch_cannot_observe_a_result_discarded_on_an_earlier_edge() {
    let mut unit = inspection_unit();
    let (function_index, block_index, node_index) = case_location(&unit);
    let function = &mut unit.functions[function_index];
    let mut dispatch = function.blocks[block_index].nodes[node_index].clone();
    let AbstractOperation::StructuralCase { source, cases } = &mut dispatch.operation else {
        unreachable!()
    };
    let discarded = *source;
    for case in cases {
        case.trivial_affine_discards.clear();
    }
    let dispatch_block = id(999, BlockId::new);
    function.blocks[block_index].nodes[node_index].operation = AbstractOperation::Jump {
        psi_edge: id(999, EdgeId::new),
        target: dispatch_block,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: vec![discarded],
        residual_affine_discards: Vec::new(),
    };
    function.blocks.push(optimization_unit::OptimizationBlock {
        id: dispatch_block,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        nodes: vec![dispatch],
    });
    let mut effect = 0;
    for block in &mut function.blocks {
        for node in &mut block.nodes {
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect + 1,
            };
            effect += 1;
        }
    }
    for current_block in 0..unit.functions[function_index].blocks.len() {
        for current_node in 0..unit.functions[function_index].blocks[current_block]
            .nodes
            .len()
        {
            refresh_node_derivatives(&mut unit, function_index, current_block, current_node);
        }
    }
    assert!(
        matches!(validate_psi_optimization_unit(&unit), Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { place, .. }) if place == discarded)
    );
}
