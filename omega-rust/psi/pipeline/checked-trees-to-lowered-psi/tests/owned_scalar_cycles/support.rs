use std::collections::BTreeSet;

use semantic_vocabulary::{BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, ValueId};
use terminal_psi::{
    Operation, OperationKind, ProofBundle, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, TerminalMachine, TerminalModule, Terminator,
};

pub fn publish(source: &str) -> (TerminalModule, ProofBundle, Vec<u8>, Vec<u8>) {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(std::path::PathBuf::from("main.omg"), source.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize walk");
    let mut syntax = syntax_trees::SyntaxTrees::new(source_id);
    tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
        .expect("parse unchanged walk syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve with source/debug custody");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type walk");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|error| panic!("check unhoisted walk: {error:#?}\n{source}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap();
    let states = checked.machine_states(machine);
    assert_eq!(
        states.len(),
        1,
        "no synthetic source state for the selected reset operand"
    );
    // A free machine's body is its implicit `entry` state; `walk` names
    // the machine, and the recursive successor resolves to this exact child.
    assert_eq!(states[0].name.as_str(), "entry");
    assert_eq!(
        checked
            .typed
            .symbols
            .find_child_by_name(machine.symbol, "entry"),
        Some(states[0].symbol),
        "the sole state belongs to the authored walk machine"
    );
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .expect("checked cyclic scalar plan");
    assert_eq!(graph.states.len(), 1);
    assert_eq!(graph.machine, machine.symbol);
    let state = &graph.states[0];
    assert_eq!(state.state, states[0].symbol);
    assert_eq!(
        state
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(state.structural_parameters.len(), 1);
    assert_eq!(state.structural_parameters[0].position, 1);
    assert_eq!(
        state.structural_parameters[0].multiplicity,
        language_semantics::Multiplicity::Affine
    );
    assert_eq!(
        state.primitive_locals.len(),
        1,
        "scratch remains owned by the authored state"
    );
    let checked_trees::CheckedScalarStateTerminator::Conditional {
        when_true,
        when_false,
        ..
    } = &state.terminator
    else {
        panic!("authored selective transition");
    };
    let checked_trees::CheckedScalarBranchDestination::Jump(successor) = when_true else {
        panic!("selected recursive successor");
    };
    assert_eq!(successor.target, states[0].symbol);
    assert_eq!(successor.argument_count, 3);
    assert!(matches!(
        when_false,
        checked_trees::CheckedScalarBranchDestination::Return { .. }
    ));

    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "walk")
        .unwrap_or_else(|error| panic!("lower owned scalar cycle: {error:?}\n{source}"));
    let debug = lowered.debug_map.expect("source-backed cycle debug map");
    assert!(!debug.sites.is_empty());
    let debug_bytes = terminal_codec::encode_debug_map(&lowered.semantic_module, &debug).unwrap();
    assert_eq!(
        terminal_codec::decode_debug_map(&lowered.semantic_module, &debug_bytes).unwrap(),
        debug
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "walk")
        .produce_artifact()
        .expect("publish cyclic walk and reset closure");
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("canonical cycle decode");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent cyclic artifact verification");
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    assert_eq!(
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
        artifact.proof_bytes()
    );
    assert_eq!(
        module.machines.len(),
        2,
        "only authored walk and reset; no wrapper"
    );
    let root = walk(&module);
    assert_eq!(root.parameters.len(), 2);
    assert_eq!(root.structural_parameters.len(), 1);
    let limits = &root.structural_parameters[0];
    assert_eq!(
        limits.position, 0,
        "Terminal structural inputs use dense positions"
    );
    assert_eq!(limits.access, StructuralAccess::Owned);
    assert_eq!(limits.multiplicity, StructuralMultiplicity::Affine);
    assert!(limits.qualifications.is_empty());
    assert!(limits.projected_qualifications.is_empty());
    (
        module,
        proof,
        artifact.semantic_bytes().to_vec(),
        artifact.proof_bytes().to_vec(),
    )
}

pub fn walk(module: &TerminalModule) -> &TerminalMachine {
    module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap()
}

pub fn unsigned_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

pub fn operation(
    machine: &TerminalMachine,
    predicate: impl Fn(&OperationKind) -> bool,
) -> &Operation {
    let mut matches = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| predicate(&operation.kind));
    let operation = matches.next().expect("authored operation survives");
    assert!(matches.next().is_none(), "one authored operation site");
    operation
}

pub fn successor(
    machine: &TerminalMachine,
    edge: EdgeId,
) -> (BlockId, &[ValueId], &[StructuralArgument]) {
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump {
                edge: candidate,
                target,
                arguments,
                structural_arguments,
                ..
            } if *candidate == edge => return (*target, arguments, structural_arguments),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    if successor.edge == edge {
                        return (
                            successor.target,
                            &successor.arguments,
                            &successor.structural_arguments,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    panic!("missing actual successor {edge:?}");
}

pub fn has_cycle(machine: &TerminalMachine) -> bool {
    machine.blocks.iter().any(|start| {
        let mut pending = vec![start.id];
        let mut visited = BTreeSet::new();
        while let Some(current) = pending.pop() {
            if !visited.insert(current) {
                continue;
            }
            let block = machine
                .blocks
                .iter()
                .find(|block| block.id == current)
                .unwrap();
            if matches!(
                block.terminator,
                Terminator::Jump { .. } | Terminator::Conditional { .. }
            ) {
                for edge in block.terminator.edges() {
                    let target = successor(machine, edge).0;
                    if target == start.id {
                        return true;
                    }
                    pending.push(target);
                }
            }
        }
        false
    })
}

// Follow only simultaneous SSA bindings, never arithmetic. Keep both entry and
// backedge origins so a preheader value cannot impersonate the loop-carried rank.
pub fn scalar_origins(machine: &TerminalMachine, value: ValueId) -> BTreeSet<ValueId> {
    let mut pending = vec![value];
    let mut visited = BTreeSet::new();
    let mut origins = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !visited.insert(value) {
            continue;
        }
        let parameter = machine.blocks.iter().find_map(|block| {
            block
                .parameters
                .iter()
                .position(|parameter| parameter.id == value)
                .map(|position| (block.id, position))
        });
        if let Some((target, position)) = parameter {
            for block in &machine.blocks {
                if matches!(
                    block.terminator,
                    Terminator::Jump { .. } | Terminator::Conditional { .. }
                ) {
                    for edge in block.terminator.edges() {
                        let (destination, arguments, _) = successor(machine, edge);
                        if destination == target {
                            pending.push(arguments[position]);
                        }
                    }
                }
            }
        } else {
            origins.insert(value);
        }
    }
    origins
}

pub fn assert_constant(machine: &TerminalMachine, value: ValueId, expected: u128) {
    let origins = scalar_origins(machine, value);
    assert!(!origins.is_empty());
    for origin in origins {
        assert!(machine.blocks.iter().flat_map(|block| &block.operations).any(|operation| operation.result.scalar().is_some_and(|result| result.id == origin) && matches!(operation.kind, OperationKind::IntegerConstant { value: IntegerValue::Unsigned(value) } if value == expected)), "exact integer constant {expected}");
    }
}
