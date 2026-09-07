use crate::CallFrameResolver;
use facts::NormalizedWriteFrame;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn frame(program: &TypedTrees) -> NormalizedWriteFrame {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("inspect"))
        .unwrap();
    let frames = CallFrameResolver::new(program)
        .unwrap()
        .inferred_machine_state_write_frames(machine);
    assert_eq!(frames.len(), 1);
    frames.into_iter().next().unwrap()
}

#[test]
fn static_boundary_scalar_arguments_do_not_invent_receiver_storage() {
    for actual in ["true", "produce()"] {
        let program = typed(&format!(
            "boundary trait Sink {{ machine record(value: bool); }} machine produce() -> bool {{ true }} machine inspect() {{ Sink::record({actual}); }}"
        ));
        assert_eq!(
            frame(&program).complete_paths(),
            Some([].as_slice()),
            "{actual}"
        );
    }
    let program = typed(
        "data Flag { enabled: bool; } data Helper {} boundary trait Sink { machine record(value: bool); } machine trigger() -> bool crashes Trap { crash Trap; } machine Helper::inspect(record: &mut Flag)\nrequires record.enabled\ncrashes Trap record.enabled\n{ record.enabled = false; Sink::record(trigger()); }",
    );
    assert_eq!(
        frame(&program).complete_paths(),
        Some(["$P0.enabled".to_owned()].as_slice())
    );
}

#[test]
fn static_boundary_exclusive_arguments_and_nested_effects_keep_their_write_paths() {
    for body in ["Sink::touch(record);", "Sink::record(change(record));"] {
        let program = typed(&format!(
            "data Flag {{ enabled: bool; }} boundary trait Sink {{ machine record(value: bool); machine touch(record: &mut Flag); }} machine change(record: &mut Flag) -> bool {{ record.enabled = false; true }} machine inspect(record: &mut Flag) {{ {body} }}"
        ));
        let expected = if body.starts_with("Sink::touch") {
            "$P0"
        } else {
            "$P0.enabled"
        };
        assert_eq!(
            frame(&program).complete_paths(),
            Some([expected.to_owned()].as_slice()),
            "{body}"
        );
    }
}

#[test]
fn static_boundary_expression_calls_use_the_same_exact_signature_frame() {
    let program = typed(
        "boundary trait Sink { machine produce() -> bool; } machine inspect() -> bool { Sink::produce() }",
    );
    assert_eq!(frame(&program).complete_paths(), Some([].as_slice()));
}

#[test]
fn static_boundary_wrong_zero_and_stale_targets_remain_opaque() {
    let program = typed(
        "boundary trait Sink { machine record(value: bool); machine other(value: bool); } machine inspect() { Sink::record(true); }",
    );
    let machine = &program.machines()[0];
    let statements = program.machine_states(machine)[0].statement_nodes;
    let StatementNode::Call(call) = &program.statement_table.statements(statements)[0] else {
        panic!("statement call");
    };
    let target = call.target_symbol;
    let foreign = program.trait_machine_signatures(&program.traits()[0])[1].symbol;
    for wrong in [
        SymbolHandle::invalid(),
        SymbolHandle::from_parts(target.arena_index(), target.generation() + 1),
        foreign,
    ] {
        let mut invalid = program.clone();
        let StatementNode::Call(call) = &mut invalid.statement_table.statements_mut(statements)[0]
        else {
            unreachable!()
        };
        call.target_symbol = wrong;
        assert!(!frame(&invalid).is_complete());
    }
    let mut invalid = program.clone();
    let StatementNode::Call(call) = &mut invalid.statement_table.statements_mut(statements)[0]
    else {
        unreachable!()
    };
    call.receiver_symbol = SymbolHandle::invalid();
    assert!(!frame(&invalid).is_complete());
}

#[test]
fn static_boundary_stale_formal_types_cannot_erase_exclusive_argument_writes() {
    let program = typed(
        "data Flag { enabled: bool; } boundary trait Sink { machine touch(record: &mut Flag); } machine inspect(record: &mut Flag) { Sink::touch(record); }",
    );
    assert_eq!(
        frame(&program).complete_paths(),
        Some(["$P0".to_owned()].as_slice())
    );
    let parameter = program.trait_machine_signatures(&program.traits()[0])[0]
        .parameters
        .start();
    let reference = program.state_parameters.get(parameter).type_reference;
    let stale =
        TypeReferenceHandle::from_parts(reference.arena_index(), reference.generation() + 1);
    for wrapped in [false, true] {
        let mut invalid = program.clone();
        assert!(!invalid.type_reference_table.contains_type_reference(stale));
        let reference = if wrapped {
            invalid
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: stale,
                    constraints: Default::default(),
                })
        } else {
            stale
        };
        invalid.state_parameters.get_mut(parameter).type_reference = reference;
        assert!(
            !frame(&invalid).is_complete(),
            "stale formal, wrapped={wrapped}"
        );
    }
}

#[test]
fn static_boundary_missing_and_out_of_arena_formal_types_remain_opaque() {
    let program = typed(
        "data Flag { enabled: bool; } boundary trait Sink { machine touch(record: &mut Flag); } machine inspect(record: &mut Flag) { Sink::touch(record); }",
    );
    let parameter = program.trait_machine_signatures(&program.traits()[0])[0]
        .parameters
        .start();
    for reference in [
        TypeReferenceHandle::invalid(),
        TypeReferenceHandle::from_arena_index(u32::MAX),
    ] {
        let mut invalid = program.clone();
        invalid.state_parameters.get_mut(parameter).type_reference = reference;
        assert!(
            !frame(&invalid).is_complete(),
            "missing formal {reference:?}"
        );
    }
}

#[test]
fn static_boundary_finite_constraint_chains_preserve_exclusive_writes() {
    let program = typed(
        "data Flag { enabled: bool; } boundary trait Sink { machine touch(record: &mut Flag); } machine inspect(record: &mut Flag) { Sink::touch(record); }",
    );
    let parameter = program.trait_machine_signatures(&program.traits()[0])[0]
        .parameters
        .start();
    for depth in [0, 1, 64, 128] {
        let mut constrained = program.clone();
        let mut reference = constrained.state_parameters.get(parameter).type_reference;
        for _ in 0..depth {
            reference = constrained
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: reference,
                    constraints: Default::default(),
                });
        }
        constrained
            .state_parameters
            .get_mut(parameter)
            .type_reference = reference;
        assert_eq!(
            frame(&constrained).complete_paths(),
            Some(["$P0".to_owned()].as_slice()),
            "finite chain depth {depth}"
        );
    }
}

#[test]
fn static_boundary_cyclic_formal_constraints_remain_opaque() {
    let program = typed(
        "data Flag { enabled: bool; } boundary trait Sink { machine touch(record: &mut Flag); } machine inspect(record: &mut Flag) { Sink::touch(record); }",
    );
    let parameter = program.trait_machine_signatures(&program.traits()[0])[0]
        .parameters
        .start();
    for cycle_length in [1, 2] {
        let mut invalid = program.clone();
        let mut reference = invalid.state_parameters.get(parameter).type_reference;
        let first = invalid
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: reference,
                constraints: Default::default(),
            });
        reference = first;
        for _ in 1..cycle_length {
            reference = invalid
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: reference,
                    constraints: Default::default(),
                });
        }
        invalid.type_reference_table.substitute_node(
            first,
            TypeReferenceNode::Constrained {
                base_type: reference,
                constraints: Default::default(),
            },
        );
        invalid.state_parameters.get_mut(parameter).type_reference = first;
        assert!(
            !frame(&invalid).is_complete(),
            "constraint cycle length {cycle_length}"
        );
    }
}
