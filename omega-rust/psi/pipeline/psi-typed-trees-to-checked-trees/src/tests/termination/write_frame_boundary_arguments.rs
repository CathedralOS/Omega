use super::*;

#[test]
fn boundary_method_names_do_not_acquire_builtin_empty_frames() {
    for target in [
        "min",
        "max",
        "sqrt",
        "as_slice",
        "as_mut_slice",
        "as_view",
        "bytes",
    ] {
        let source = format!(
            "boundary trait Device {{ machine {target}(value: u64) -> u64; }}
            data Main {{ device: Device; value: u64; audit: u64; }}
            machine compute(value: &mut u64) -> u64 {{ value = 1; 1 }}
            machine Main::run(&mut self) {{
                self.value = self.device.{target}(compute(&mut self.audit));
            }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
        let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let statement = typed
            .statement_table
            .statements(state.statement_nodes)
            .first()
            .expect("assignment");
        let mut paths = resolver
            .statement_value_may_write_paths(machine, statement)
            .unwrap_or_else(|| panic!("{target} must have a complete value-call frame"));
        paths.sort();
        assert_eq!(paths, ["self.audit", "self.device"], "{target}");
    }
}

#[test]
fn constrained_boundary_reference_parameters_keep_their_write_reach() {
    let source = r#"
    boundary trait Device {
        machine overwrite(value: &mut u64);
        machine overwrite_slice(values: &mut [u64]);
    }
    data Main { device: Device; value: u64; cells: [u64; 2]; }
    machine Main::run(&mut self) { self.device.overwrite(&mut self.value); }
    machine Main::slice(&mut self) { self.device.overwrite_slice(&mut self.cells[0..2]); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let boundary = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Device")
        .expect("Device");
    let signature = typed
        .trait_machine_signatures(boundary)
        .first()
        .expect("overwrite");
    let parameter_type = typed
        .state_signature_parameters(signature)
        .first()
        .expect("value")
        .type_reference;
    let reference = typed
        .type_reference_table
        .type_reference(parameter_type)
        .clone();
    assert!(matches!(
        reference,
        psi_typed_trees::types::TypeReferenceNode::Reference { .. }
    ));
    let base_type = typed.type_reference_table.insert(reference);
    // Normalization can put transparent constraints around the whole reference.
    // Do not erase its access mode while peeling those constraints.
    typed.type_reference_table.substitute_node(
        parameter_type,
        psi_typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints: Default::default(),
        },
    );
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::run")
        .expect("caller");
    let state = typed.machine_states(machine).first().expect("entry");
    let frame = resolver.inferred_state_write_frame(machine, state);
    let mut actual = frame
        .complete_paths()
        .expect("complete constrained out-parameter frame")
        .to_vec();
    actual.sort();
    assert_eq!(actual, ["self.device", "self.value"]);
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::slice")
        .expect("slice caller");
    let state = typed.machine_states(machine).first().expect("entry");
    let frame = resolver.inferred_state_write_frame(machine, state);
    let mut actual = frame
        .complete_paths()
        .expect("complete primitive slice frame")
        .to_vec();
    actual.sort();
    assert_eq!(actual, ["self.cells", "self.device"]);
}

#[test]
fn boundary_arguments_publish_declared_reach_and_all_producer_writes() {
    let cases = [
        (
            "scalar",
            "self.device.scalar(compute(&mut self.audit) + 1);",
            true,
            false,
        ),
        (
            "record",
            "self.device.record(Pair { first: compute(&mut self.audit) + 1, second: compute(&mut self.other) });",
            true,
            true,
        ),
        (
            "array",
            "self.device.array([compute(&mut self.audit) + 1, compute(&mut self.other) + 1]);",
            true,
            true,
        ),
        (
            "choice",
            "self.device.choice(Choice::Filled { pair: Pair { first: compute(&mut self.audit), second: compute(&mut self.other) + 1 } });",
            true,
            true,
        ),
        (
            "out_argument",
            "self.device.output(compute(&mut self.audit) + 1, &mut self.other);",
            true,
            true,
        ),
        (
            "nested_call",
            "self.value = identity(self.device.scalar_value(compute(&mut self.audit) + 1));",
            true,
            false,
        ),
        (
            "carrier_literal",
            "self.device.carrier(Carrier { value: &mut self.other });",
            false,
            false,
        ),
        (
            "carrier_call",
            "self.device.carrier(make_carrier(&mut self.other));",
            false,
            false,
        ),
        (
            "carrier_array",
            "self.device.references([Carrier { value: &mut self.other }]);",
            false,
            false,
        ),
        (
            "exclusive_carrier",
            "self.device.mutate_carrier(&mut self.carrier);",
            false,
            false,
        ),
        ("missing_argument", "self.device.output(0);", false, false),
        (
            "extra_argument",
            "self.device.scalar(0, &mut self.other);",
            false,
            false,
        ),
        (
            "shared_out_argument",
            "self.device.output(0, &self.other);",
            false,
            false,
        ),
        (
            "ambiguous_signature",
            "self.device.ambiguous(&mut self.other);",
            false,
            false,
        ),
        (
            "generic_member",
            "self.device.generic(compute(&mut self.audit) + 1);",
            false,
            false,
        ),
        (
            "foreign_receiver",
            "self.nested.device.scalar(&mut self.other);",
            false,
            false,
        ),
        (
            "reborrow",
            "let mut alias: &mut u64 = &mut self.other; self.device.scalar(compute(&mut alias) + 1);",
            false,
            false,
        ),
        (
            "recursive",
            "self.device.array([compute(&mut self.audit), recursive(&mut self.other) + 1]);",
            false,
            false,
        ),
        (
            "indexed_argument",
            "self.device.output(0, &mut self.cells[index(&mut self.audit)]);",
            false,
            false,
        ),
        (
            "wrong_nominal",
            "self.device.record(OtherPair { first: compute(&mut self.audit), second: 0 });",
            false,
            false,
        ),
        (
            "wrong_length",
            "self.device.array([compute(&mut self.audit)]);",
            false,
            false,
        ),
    ];
    let mut source = r#"
    data Pair { first: u64; second: u64; }
    data OtherPair { first: u64; second: u64; }
    data Choice { case Filled(pair: Pair); case Empty; }
    data Carrier { value: &mut u64; }
    boundary trait Device {
        machine scalar(value: u64);
        machine scalar_value(value: u64) -> u64;
        machine record(value: Pair);
        machine array(value: [u64; 2]);
        machine choice(value: Choice);
        machine output(value: u64, output: &mut u64);
        machine carrier(value: Carrier);
        machine mutate_carrier(value: &mut Carrier);
        machine references(value: [Carrier; 1]);
        machine generic<T>(value: u64);
        machine ambiguous(value: u64);
        machine ambiguous(value: &mut u64);
    }
    boundary trait OtherDevice { machine scalar(value: &mut u64); }
    data Nested { device: OtherDevice; }
    data Main { device: Device; nested: Nested; value: u64; audit: u64; other: u64; cells: [u64; 2]; carrier: Carrier; }
    machine compute(value: &mut u64) -> u64 { value = 1; 1 }
    machine recursive(value: &mut u64) -> u64 { recursive(value) }
    machine index(value: &mut u64) -> u64 [0..=1] { value = 1; 0 }
    machine identity(value: u64) -> u64 { value }
    machine make_carrier(value: &mut u64) -> Carrier { Carrier { value: value } }
    "#.to_owned();
    for (name, statement, _, _) in cases {
        source.push_str(&format!(
            "machine Main::after_{name}(&mut self) -> &mut u64 {{
                {statement}
                &mut self.value
            }}
            machine Main::{name}(&mut self) {{
                let alias: &mut u64 = self.after_{name}();
                alias = 2;
            }}
            machine Main::direct_{name}(&mut self) {{
                {statement}
            }}"
        ));
    }
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    // Malformed argument contexts deliberately reach the pre-validation frame
    // query. Neither argument typing nor boundary reach may be guessed here.
    let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("symbol cache");
    for name in [
        "carrier_literal",
        "carrier_call",
        "carrier_array",
        "exclusive_carrier",
        "missing_argument",
        "extra_argument",
        "shared_out_argument",
        "ambiguous_signature",
        "generic_member",
        "foreign_receiver",
    ] {
        let qualified = format!("Main::direct_{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified)
            .expect("direct caller");
        let state = typed.machine_states(machine).first().expect("entry");
        assert!(
            !resolver
                .inferred_state_write_frame(machine, state)
                .is_complete(),
            "{name} must not regain a complete state frame through fallback"
        );
        let call = typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                if let psi_typed_trees::statement::StatementNode::Call(call) = statement {
                    Some(call)
                } else {
                    None
                }
            })
            .expect("boundary statement");
        assert!(
            !resolver.may_write_frame(machine, call).is_complete(),
            "{name} must not regain a complete direct call frame through fallback"
        );
    }
    for (name, _, complete, writes_other) in cases {
        let qualified = format!("Main::{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified)
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let frame = resolver.inferred_state_write_frame(machine, state);
        if complete {
            let mut expected = vec!["self.audit", "self.device", "self.value"];
            if writes_other {
                expected.push("self.other");
            }
            expected.sort();
            let mut actual = frame
                .complete_paths()
                .unwrap_or_else(|| panic!("{name} must be complete"))
                .to_vec();
            actual.sort();
            assert_eq!(
                actual, expected,
                "{name} must retain receiver, exclusive arguments, and producer writes"
            );
        } else {
            assert!(!frame.is_complete(), "{name} must remain opaque");
        }
    }
}
