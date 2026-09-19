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
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
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
fn static_boundary_result_bound_to_local_keeps_its_single_origin() {
    let program = typed(
        "data Main { value: u64; } boundary trait Sink { machine reference(value: &mut u64) -> &mut u64; machine touch(record: &mut u64); } machine Main::inspect(&mut self) { let r: &mut u64 = Sink::reference(&mut self.value); Sink::touch(r); }",
    );
    assert_eq!(
        frame(&program).complete_paths(),
        Some(["self.value".to_owned()].as_slice())
    );
}

#[test]
fn boundary_results_bound_to_locals_join_their_proven_single_origin() {
    for (name, body, expected) in [
        // A static signature admits only the exclusive argument's storage,
        // so the bound local forwards that single origin.
        (
            "static_argument",
            "let r: &mut u64 = Device::reference(&mut self.value); self.device.touch(r);",
            Some(&["self.device", "self.value"][..]),
        ),
        // A receiver-only result must point into the opaque receiver storage.
        (
            "receiver_only",
            "let r: &mut u64 = self.device.make(); self.device.touch(r);",
            Some(&["self.device"][..]),
        ),
        // A second admitted route gives the bound result a divergent
        // referent set; lending it through the call argument unions every
        // proven route.
        (
            "receiver_and_argument",
            "let r: &mut u64 = self.device.reference(&mut self.value); self.device.touch(r);",
            Some(&["self.device", "self.value"][..]),
        ),
        (
            "two_arguments",
            "let r: &mut u64 = Device::pick(&mut self.value, &mut self.other); self.device.touch(r);",
            Some(&["self.device", "self.other", "self.value"][..]),
        ),
        // A result with no admitted caller route cannot prove a referent.
        (
            "no_route",
            "let r: &mut u64 = Device::empty(); self.device.touch(r);",
            None,
        ),
        // An interior referent claims the coarse storage root rather than a
        // fabricated member subpath.
        (
            "interior_referent",
            "let r: &mut u64 = Device::project(&mut self.cell); self.device.touch(r);",
            Some(&["self.cell", "self.device"][..]),
        ),
        // A nested boundary result transports its own single origin.
        (
            "nested_result",
            "let r: &mut u64 = Device::reference(Device::reference(&mut self.value)); self.device.touch(r);",
            Some(&["self.device", "self.value"][..]),
        ),
        // A carrier whose stored exclusive reference could still reach the
        // referent keeps the result opaque.
        (
            "carrier_route",
            "let r: &mut u64 = self.device.carrier_reference(&mut self.carrier); self.device.touch(r);",
            None,
        ),
        // Rebinding the bound local redirects later writes to the new origin.
        (
            "rebound",
            "let mut r: &mut u64 = Device::reference(&mut self.value); r = &mut self.other; self.device.touch(r);",
            Some(&["self.device", "self.other", "self.value"][..]),
        ),
        // A bound-local argument canonicalizes through that local's origin,
        // so the new binding keeps the referent held at bind time even after
        // the argument binding is rebound elsewhere.
        (
            "bound_from_local",
            "let alias: &mut u64 = &mut self.value; let r: &mut u64 = Device::reference(alias); self.device.touch(r);",
            Some(&["self.device", "self.value"][..]),
        ),
        (
            "bound_from_rebound_local",
            "let mut alias: &mut u64 = &mut self.value; let r: &mut u64 = Device::reference(alias); alias = &mut self.other; self.device.touch(r);",
            Some(&["self.device", "self.value"][..]),
        ),
        // Writing through the bound result lands on its proven referent.
        (
            "write_through",
            "let r: &mut u64 = Device::reference(&mut self.value); r = 1;",
            Some(&["self.value"][..]),
        ),
        // An exact aggregate referent keeps member projections exact.
        (
            "aggregate_root",
            "let r: &mut Cell = Device::cell_ref(&mut self.cell); r.value = 1;",
            Some(&["self.cell", "self.cell.value"][..]),
        ),
    ] {
        let program = typed(&format!(
            "data Cell {{ value: u64; }} data Carrier {{ value: &mut u64; }} data Main {{ device: Device; value: u64; other: u64; cell: Cell; carrier: Carrier; }} boundary trait Device {{ machine reference(value: &mut u64) -> &mut u64; machine pick(hit: &mut u64, other: &mut u64) -> &mut u64; machine project(cell: &mut Cell) -> &mut u64; machine cell_ref(cell: &mut Cell) -> &mut Cell; machine make() -> &mut u64; machine empty() -> &mut u64; machine carrier_reference(carrier: &mut Carrier) -> &mut u64; machine touch(record: &mut u64); }} machine Main::inspect(&mut self) {{ {body} }}"
        ));
        let mut actual = frame(&program).complete_paths().map(|paths| paths.to_vec());
        if let Some(paths) = &mut actual {
            paths.sort();
        }
        let expected = expected.map(|paths| {
            paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn boundary_result_bound_to_local_forwards_a_parameter_origin() {
    let program = typed(
        "data Main { device: Device; } boundary trait Device { machine reference(value: &mut u64) -> &mut u64; machine touch(record: &mut u64); } machine Main::inspect(&mut self, record: &mut u64) { let r: &mut u64 = Device::reference(record); self.device.touch(r); }",
    );
    assert_eq!(
        frame(&program).complete_paths(),
        Some(["$P0".to_owned(), "self.device".to_owned()].as_slice())
    );
}

#[test]
fn boundary_result_with_a_stale_return_type_cannot_bind_an_origin() {
    let program = typed(
        "data Main { device: Device; value: u64; } boundary trait Device { machine reference(value: &mut u64) -> &mut u64; machine touch(record: &mut u64); } machine Main::inspect(&mut self) { let r: &mut u64 = Device::reference(&mut self.value); self.device.touch(r); }",
    );
    let signature_span = program.traits()[0].machines;
    let valid = program
        .trait_machine_signatures
        .span_or_empty(signature_span)[0]
        .return_type;
    for stale in [
        TypeReferenceHandle::invalid(),
        TypeReferenceHandle::from_parts(valid.arena_index(), valid.generation() + 1),
    ] {
        let mut invalid = program.clone();
        invalid
            .trait_machine_signatures
            .span_mut_or_empty(signature_span)[0]
            .return_type = stale;
        assert!(
            !frame(&invalid).is_complete(),
            "stale result type {stale:?}"
        );
    }
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

/// A signature `Type` parameter the call's actual arguments pin concretely
/// instantiates the boundary route: the same single-origin and owned-storage
/// gates then run against the caller's storage, not the formal name.
#[test]
fn generic_boundary_signature_result_binds_the_caller_actual() {
    for (name, source, expected) in [
        // `project<T>(carrier: &mut T) -> &mut T` instantiated at `Cell`:
        // the single admitted route is the argument's exact storage, so the
        // projected write stays a precise subpath and the re-exported
        // reference keeps that origin through the boundary receiver's frame.
        (
            "result_root_exact",
            "data Cell { value: u64; } data Main { device: Device; cell: Cell; } boundary trait Device { machine project<T>(carrier: &mut T) -> &mut T; machine output(value: &mut Cell); } machine Main::inspect(&mut self) { let r: &mut Cell = Device::project(&mut self.cell); r.value = 1; self.device.output(r); }",
            Some(&["self.cell", "self.cell.value", "self.device"][..]),
        ),
        // The referent sits inside a `Wrap<Cell>` at an offset the caller
        // cannot name, so the routed origin claims the coarse storage root.
        (
            "carrier_interior_result",
            "data Cell { value: u64; } data Wrap<T> { inner: T; } data Main { device: Device; wrap: Wrap<Cell>; } boundary trait Device { machine project(wrap: &mut Wrap<Cell>) -> &mut Cell; } machine Main::inspect(&mut self) { let r: &mut Cell = Device::project(&mut self.wrap); r.value = 1; }",
            Some(&["self.wrap"][..]),
        ),
        // The same substitution reaches a scalar referent one member deep.
        (
            "carrier_scalar_interior",
            "data Wrap<T> { inner: T; } data Main { device: Device; wrap: Wrap<u64>; } boundary trait Device { machine project(wrap: &mut Wrap<u64>) -> &mut u64; } machine Main::inspect(&mut self) { let r: &mut u64 = Device::project(&mut self.wrap); r = 1; }",
            Some(&["self.wrap"][..]),
        ),
        // A nested application binds the inner parameter under its own
        // scope, so `Wrap<Wrap<u64>>` still resolves the `u64` interior.
        (
            "nested_application",
            "data Wrap<T> { inner: T; } data Main { device: Device; wrap: Wrap<Wrap<u64>>; } boundary trait Device { machine project(wrap: &mut Wrap<Wrap<u64>>) -> &mut u64; } machine Main::inspect(&mut self) { let r: &mut u64 = Device::project(&mut self.wrap); r = 1; }",
            Some(&["self.wrap"][..]),
        ),
        // An exclusive `&mut Wrap<Cell>` parameter writes its argument's
        // origin through the same substitution the result route uses.
        (
            "generic_argument_footprint",
            "data Cell { value: u64; } data Wrap<T> { inner: T; } data Main { device: Device; wrap: Wrap<Cell>; } boundary trait Device { machine consume(wrap: &mut Wrap<Cell>); } machine Main::inspect(&mut self) { self.device.consume(&mut self.wrap); }",
            Some(&["self.device", "self.wrap"][..]),
        ),
        // A generic helper's member projection composes under the call's
        // instantiation, keeping the exact leaf through the boundary write.
        (
            "helper_carrier_projection",
            "data Cell { value: u64; } data Wrap<T> { inner: T; } data Main { device: Device; wrap: Wrap<Cell>; } boundary trait Device { machine output(value: &mut u64); } machine project(wrap: &mut Wrap<Cell>) -> &mut u64 { &mut wrap.inner.value } machine Main::inspect(&mut self) { self.device.output(project(&mut self.wrap)); }",
            Some(&["self.device", "self.wrap.inner.value"][..]),
        ),
    ] {
        let program = typed(source);
        let mut actual = frame(&program).complete_paths().map(|paths| paths.to_vec());
        if let Some(paths) = &mut actual {
            paths.sort();
        }
        let expected = expected.map(|paths| {
            paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, expected, "{name}");
    }
}

/// Substitution never widens admission: a route whose parameter stays
/// unbound, whose carrier stores an exclusive reference, recurses without a
/// finite proof, or admits several caller origins keeps the frame opaque.
#[test]
fn generic_boundary_carriers_still_fail_closed() {
    for (name, source) in [
        // No argument binds `T`, so the result's referent stays unproven.
        (
            "unbound_result_parameter",
            "data Cell { value: u64; } data Main { device: Device; cell: Cell; } boundary trait Device { machine spawn<T>() -> &mut T; machine output(value: &mut Cell); } machine Main::inspect(&mut self) { let r: &mut Cell = Device::spawn(); self.device.output(r); }",
        ),
        // Both exclusive arguments admit the referent: the divergent
        // referent set converges through the call argument (covered in
        // `boundary_results_bound_to_locals_join_their_proven_single_origin`).
        // A carrier whose stored exclusive reference may already reach the
        // referent cannot name where the result lands.
        (
            "stored_exclusive_carrier",
            "data Cell { value: u64; } data Pocket<T> { held: &mut T; } data Main { device: Device; pocket: Pocket<Cell>; } boundary trait Device { machine project(pocket: &mut Pocket<Cell>) -> &mut Cell; } machine Main::inspect(&mut self) { let r: &mut Cell = Device::project(&mut self.pocket); r.value = 1; }",
        ),
        // A by-value carrier is judged on the instantiated storage: once
        // `T` binds `Cell`, the stored exclusive reference still fails.
        (
            "by_value_reference_carrier",
            "data Cell { value: u64; } data Pocket<T> { held: &mut T; } data Main { device: Device; pocket: Pocket<Cell>; } boundary trait Device { machine consume<T>(pocket: Pocket<T>); } machine Main::inspect(&mut self) { self.device.consume(self.pocket); }",
        ),
        // The recursive member walk cannot finish, so the route stays
        // opaque instead of guessing which link holds the referent.
        (
            "recursive_carrier",
            "data Link<T> { next: &mut Link<T>; value: T; } data Main { device: Device; link: Link<u64>; } boundary trait Device { machine project(link: &mut Link<u64>) -> &mut u64; } machine Main::inspect(&mut self) { let r: &mut u64 = Device::project(&mut self.link); r = 1; }",
        ),
        // `T` instantiated at a dynamic carrier still fails the
        // owned-storage gate: the referent cannot be proven isolated.
        (
            "dynamic_carrier",
            "trait Shape {} data Cell { value: u64; } data Main { device: Device; shape: dyn Shape; } boundary trait Device { machine project<T>(carrier: &mut T); } machine Main::inspect(&mut self) { self.device.project(&mut self.shape); }",
        ),
    ] {
        let program = typed(source);
        assert!(
            !frame(&program).is_complete(),
            "{name} unexpectedly completed"
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

/// A resolved non-boundary requirement call keeps the runtime receiver's
/// proven origin and every exclusive argument's origin: the retained
/// `target_symbol` selects the requirement signature exactly, while the
/// receiver place — a `dyn` field, a reference field, a nested member, a
/// bound local, or the explicit `self` argument on a declaration-qualified
/// call — still names caller storage. No implementor path is invented.
#[test]
fn requirement_receiver_calls_write_their_proven_origins() {
    for (name, body, expected) in [
        (
            "dyn_field",
            "self.handler.code();",
            Some(&["self.handler"][..]),
        ),
        (
            "dyn_field_exclusive_argument",
            "self.handler.apply(&mut self.audit);",
            Some(&["self.audit", "self.handler"][..]),
        ),
        ("shared_self", "self.handler.peek();", Some(&[][..])),
        (
            "shared_self_exclusive_argument",
            "self.handler.view(&mut self.audit);",
            Some(&["self.audit"][..]),
        ),
        (
            "reference_field",
            "self.ref_handler.code();",
            Some(&["self.ref_handler"][..]),
        ),
        (
            "nested_receiver",
            "self.group.handler.code();",
            Some(&["self.group.handler"][..]),
        ),
        (
            "bound_local",
            "let s: &mut dyn Shape = &mut self.handler; s.code();",
            Some(&["self.handler"][..]),
        ),
        (
            "qualified_self_argument",
            "Shape::apply(&mut self.handler, &mut self.audit);",
            Some(&["self.audit", "self.handler"][..]),
        ),
        (
            "qualified_shared_self",
            "Shape::peek(&self.handler);",
            Some(&[][..]),
        ),
        (
            "qualified_no_self",
            "Shape::touch(&mut self.audit);",
            Some(&["self.audit"][..]),
        ),
    ] {
        let program = typed(&format!(
            "trait Shape {{ machine code(&mut self) -> i32; machine apply(&mut self, value: &mut u64); machine peek(&self) -> i32; machine view(&self, value: &mut u64); machine touch(value: &mut u64); }}
             data Group {{ handler: dyn Shape; }}
             data Main {{ handler: dyn Shape; ref_handler: &mut dyn Shape; audit: u64; group: Group; }}
             machine Main::inspect(&mut self) {{ {body} }}"
        ));
        let mut actual = frame(&program).complete_paths().map(|paths| paths.to_vec());
        if let Some(paths) = &mut actual {
            paths.sort();
        }
        let expected = expected.map(|paths| {
            paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, expected, "{name}");
    }
}

/// A `dyn` receiver that arrives through a parameter writes the parameter's
/// own origin, and a requirement spelled like a receiver-bearing builtin
/// keeps its resolved signature rather than acquiring the builtin's empty
/// frame.
#[test]
fn requirement_parameter_receivers_write_their_proven_origins() {
    for (name, body, expected) in [
        ("dyn_parameter", "s.code();", Some(&["$P0"][..])),
        (
            "dyn_parameter_exclusive_argument",
            "s.apply(&mut self.audit);",
            Some(&["$P0", "self.audit"][..]),
        ),
        (
            "builtin_spelling_stays_resolved",
            "let n: i32 = s.bytes();",
            Some(&["$P0"][..]),
        ),
    ] {
        let program = typed(&format!(
            "trait Shape {{ machine code(&mut self) -> i32; machine apply(&mut self, value: &mut u64); machine bytes(&mut self) -> i32; }}
             data Main {{ audit: u64; }}
             machine Main::inspect(&mut self, s: &mut dyn Shape) {{ {body} }}"
        ));
        let mut actual = frame(&program).complete_paths().map(|paths| paths.to_vec());
        if let Some(paths) = &mut actual {
            paths.sort();
        }
        let expected = expected.map(|paths| {
            paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, expected, "{name}");
    }
}

/// A requirement's exclusive result joins the caller-visible routes its
/// retained signature admits — the runtime receiver or an exclusive
/// argument — under the same candidate rule as boundary results: one proven
/// origin forwards exactly, several proven routes union into the bound
/// local's divergent set so a write through it lands on every candidate,
/// while a route that reaches untracked storage or no route at all stays
/// opaque.
#[test]
fn requirement_results_bound_to_locals_join_their_proven_origins() {
    for (name, body, expected) in [
        (
            "receiver_route",
            "let r: &mut u64 = self.handler.get(); r = 1;",
            Some(&["self.handler"][..]),
        ),
        (
            "qualified_route",
            "let r: &mut u64 = Shape::get(&mut self.handler); r = 1;",
            Some(&["self.handler"][..]),
        ),
        (
            "forwarded",
            "let r: &mut u64 = self.handler.get(); self.handler.consume(r);",
            Some(&["self.handler"][..]),
        ),
        (
            "nested_result_argument",
            "self.handler.consume(self.handler.get());",
            Some(&["self.handler"][..]),
        ),
        (
            "interior_referent",
            "let r: &mut u64 = Shape::project(&mut self.cell); r = 1;",
            Some(&["self.cell"][..]),
        ),
        (
            "aggregate_root",
            "let r: &mut Cell = Shape::cell_ref(&mut self.cell); r.value = 1;",
            Some(&["self.cell", "self.cell.value"][..]),
        ),
        (
            "rebound",
            "let mut r: &mut u64 = self.handler.get(); r = &mut self.other; r = 1;",
            Some(&["self.handler", "self.other"][..]),
        ),
        (
            "receiver_and_argument_routes",
            "let r: &mut u64 = self.handler.lend(&mut self.audit); r = 1;",
            Some(&["self.audit", "self.handler"][..]),
        ),
        (
            "two_argument_routes",
            "let r: &mut u64 = Shape::pick(&mut self.audit, &mut self.other); r = 1;",
            Some(&["self.audit", "self.other"][..]),
        ),
        ("no_route", "let r: &mut u64 = Shape::spawn(); r = 1;", None),
        (
            "carrier_route",
            "let r: &mut u64 = Shape::carrier_project(&mut self.carrier); r = 1;",
            None,
        ),
    ] {
        let program = typed(&format!(
            "data Cell {{ value: u64; }} data Carrier {{ held: &mut u64; }}
             trait Shape {{ machine get(&mut self) -> &mut u64; machine consume(&mut self, value: &mut u64); machine project(cell: &mut Cell) -> &mut u64; machine cell_ref(cell: &mut Cell) -> &mut Cell; machine lend(&mut self, value: &mut u64) -> &mut u64; machine pick(hit: &mut u64, other: &mut u64) -> &mut u64; machine spawn() -> &mut u64; machine carrier_project(carrier: &mut Carrier) -> &mut u64; }}
             data Main {{ handler: dyn Shape; cell: Cell; carrier: Carrier; audit: u64; other: u64; }}
             machine Main::inspect(&mut self) {{ {body} }}"
        ));
        let mut actual = frame(&program).complete_paths().map(|paths| paths.to_vec());
        if let Some(paths) = &mut actual {
            paths.sort();
        }
        let expected = expected.map(|paths| {
            paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, expected, "{name}");
    }
}

/// The requirement rung keys on the retained `target_symbol`: a stale
/// generation is authoritative rejection and the trait-typed receiver keeps
/// the conservative floor from manufacturing storage. An absent annotation
/// still re-derives the requirement from the receiver's declared leaf trait
/// — the same signature the dispatch contract names.
#[test]
fn requirement_wrong_and_stale_targets_stay_consistent() {
    let program = typed(
        "trait Shape { machine code(&mut self) -> i32; machine peek(&self) -> i32; }
         data Main { handler: dyn Shape; }
         machine Main::inspect(&mut self) { self.handler.code(); }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("inspect"))
        .unwrap();
    let statements = program.machine_states(machine)[0].statement_nodes;
    let StatementNode::Call(call) = &program.statement_table.statements(statements)[0] else {
        panic!("statement call");
    };
    let target = call.target_symbol;
    let mut unannotated = program.clone();
    let StatementNode::Call(call) = &mut unannotated.statement_table.statements_mut(statements)[0]
    else {
        unreachable!()
    };
    call.target_symbol = SymbolHandle::invalid();
    assert_eq!(
        frame(&unannotated).complete_paths(),
        Some(["self.handler".to_owned()].as_slice())
    );
    let mut stale = program.clone();
    let StatementNode::Call(call) = &mut stale.statement_table.statements_mut(statements)[0] else {
        unreachable!()
    };
    call.target_symbol = SymbolHandle::from_parts(target.arena_index(), target.generation() + 1);
    assert!(!frame(&stale).is_complete());
}

/// Requirement frames fail closed when the receiver or an argument names no
/// proven caller storage: an indexed collection element, a computed receiver,
/// a generic trait whose substitution cannot be closed at the call site, or
/// an exclusive argument whose referent was never tracked all stay opaque
/// rather than inventing a path.
#[test]
fn requirement_calls_without_proven_storage_stay_opaque() {
    for (name, body) in [
        (
            "unproven_result_argument",
            "let r: &mut u64 = Shape::spawn(); self.handler.apply(r);",
        ),
        ("indexed_receiver", "self.handlers[0].code();"),
        ("call_receiver", "self.make().code();"),
        ("generic_trait_receiver", "self.ghandler.gcode();"),
        ("unrelated_requirement", "self.handler.nope();"),
    ] {
        let program = typed(&format!(
            "trait Shape {{ machine code(&mut self) -> i32; machine apply(&mut self, value: &mut u64); machine spawn() -> &mut u64; }}
             trait G<T> {{ machine gcode(&mut self) -> i32; }}
             machine Main::make(&mut self) -> &mut dyn Shape {{ &mut self.handler }}
             data Main {{ handler: dyn Shape; ghandler: dyn G; handlers: [dyn Shape; 2]; audit: u64; }}
             machine Main::inspect(&mut self) {{ {body} }}"
        ));
        assert!(!frame(&program).is_complete(), "{name}");
    }
}

#[test]
fn generic_boundary_owner_bindings_preserve_signature_argument_access() {
    for method in [
        "machine consume(carrier: &mut T, metadata: u64);",
        "machine consume<Value>(carrier: &mut T, metadata: Value);",
    ] {
        let source = format!(
            "data Cell {{ value: u64; }} boundary trait Device<T> {{ {method} }} data Main {{ device: Device<Cell>; cell: Cell; untouched: u64; }} machine Main::inspect(&mut self) {{ self.device.consume(&mut self.cell, self.untouched); }}"
        );
        let program = typed(&source);
        let mut paths = frame(&program)
            .into_complete_paths()
            .expect("exact owner application");
        paths.sort();
        assert_eq!(paths, ["self.cell", "self.device"]);
    }
}

#[test]
fn generic_boundary_owner_failures_cannot_use_signature_free_frames() {
    for (name, source) in [
        (
            "wrong_actual",
            "data Cell { value: u64; } data Other { value: u64; } boundary trait Device<T> { machine consume(carrier: &mut T); } data Main { device: Device<Cell>; other: Other; } machine Main::inspect(&mut self) { self.device.consume(&mut self.other); }",
        ),
        (
            "interior_reference",
            "data Cell { value: u64; } data Pocket { held: &mut Cell; } boundary trait Device<T> { machine consume(carrier: T); } data Main { device: Device<Pocket>; pocket: Pocket; } machine Main::inspect(&mut self) { self.device.consume(self.pocket); }",
        ),
        (
            "unbound_static_owner",
            "data Cell { value: u64; } boundary trait Device<T> { machine consume(carrier: &mut T); } data Main { cell: Cell; } machine Main::inspect(&mut self) { Device::consume(&mut self.cell); }",
        ),
        (
            "unbound_method",
            "data Cell { value: u64; } boundary trait Device<T> { machine consume<Value>(carrier: &mut T); } data Main { device: Device<Cell>; cell: Cell; } machine Main::inspect(&mut self) { self.device.consume(&mut self.cell); }",
        ),
    ] {
        assert!(
            !frame(&typed(source)).is_complete(),
            "{name} must remain opaque"
        );
    }
}

#[test]
fn generic_boundary_owner_result_uses_existing_candidate_origins() {
    let program = typed(
        "data Cell { value: u64; } data Main { device: Device<Cell>; cell: Cell; } boundary trait Device<T> { machine project(carrier: &mut T) -> &mut T; } machine Main::inspect(&mut self) { let r: &mut Cell = self.device.project(&mut self.cell); r.value = 1; }",
    );
    let paths = frame(&program)
        .into_complete_paths()
        .expect("closed owner result candidates");
    assert!(paths.iter().any(|path| path == "self.device"));
    assert!(paths.iter().any(|path| path == "self.cell"));
    assert!(
        !paths.iter().any(|path| path == "self.device.value"),
        "opaque receiver origin cannot acquire field precision"
    );
}

#[test]
fn generic_boundary_method_shadow_remains_opaque_when_its_binder_is_unbound() {
    let program = typed(
        "data Cell { value: u64; } data Other { value: u32; } boundary trait Device<T> { machine consume<T>(carrier: &mut T); } data Main { device: Device<Cell>; other: Other; } machine Main::inspect(&mut self) { self.device.consume(&mut self.other); }",
    );
    // The typed formal currently selects the owner's T while the distinct,
    // same-spelled method binder remains unbound. Never guess a substitution
    // from spelling to manufacture a complete frame.
    assert!(!frame(&program).is_complete());
}
