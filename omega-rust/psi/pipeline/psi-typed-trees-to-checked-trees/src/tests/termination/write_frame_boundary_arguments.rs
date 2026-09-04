use super::*;

#[test]
fn boundary_forwarded_reference_reaches_checked_trees() {
    for (argument, target, expected_diagnostic) in [
        ("output", "output", None),
        ("identity(output)", "output", None),
        ("identity(output)", "inspect", None),
        ("other(&mut self.other)", "inspect", Some("expects `&u64`")),
        ("identity(output)", "owned", Some("expects `u64`")),
        (
            "shared(&self.value)",
            "output",
            Some("caller lends only immutable access"),
        ),
        (
            "other(&mut self.other)",
            "output",
            Some("expects `&mut u64`"),
        ),
        (
            "identity(output)",
            "write",
            Some("requires explicit write-only attenuation"),
        ),
    ] {
        let source = r#"
        boundary trait Device {
            machine output(value: &mut u64);
            machine inspect(value: &u64);
            machine owned(value: u64);
        }
        data Main { device: Device; value: u64; other: u32; }
        machine identity(value: &mut u64) -> &mut u64 { value }
        machine shared(value: &u64) -> &u64 { value }
        machine other(value: &mut u32) -> &mut u32 { value }
        machine write(value: &write u64) { value = 1; }
        machine Main::run(&mut self) {
            let output: &mut u64 = &mut self.value;
            TARGET(ARGUMENT);
        }
    "#
        .replace("ARGUMENT", argument)
        .replace(
            "TARGET",
            &if target == "write" {
                "write".to_owned()
            } else {
                format!("self.device.{target}")
            },
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed);
        if let Some(expected) = expected_diagnostic {
            let diagnostics = checked.expect_err("reference result access and referee must match");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.to_string().contains(expected)),
                "{target}({argument}) must diagnose {expected}: {diagnostics:?}"
            );
        } else {
            checked.expect("forwarding uses the existing reference loan");
        }
    }
}

#[test]
fn boundary_reference_results_transport_proven_origins_and_producer_writes() {
    let cases = [
        (
            "identity",
            "self.device.output(identity(&mut self.value));",
            Some(vec!["self.device", "self.value"]),
        ),
        (
            "nested",
            "self.device.output(identity(identity(identity(&mut self.value))));",
            Some(vec!["self.device", "self.value"]),
        ),
        (
            "projected",
            "self.device.output(field(&mut self.cell));",
            Some(vec!["self.cell.value", "self.device"]),
        ),
        (
            "attached",
            "self.device.output(self.cell.field_reference());",
            Some(vec!["self.cell.value", "self.device"]),
        ),
        (
            "local",
            "let alias: &mut u64 = &mut self.value; self.device.output(identity(alias));",
            Some(vec!["self.device", "self.value"]),
        ),
        (
            "owned_local_receiver",
            "let local: Cell = Cell { value: 0 }; self.device.output(local.field_reference());",
            Some(vec!["self.device"]),
        ),
        (
            "parameter_receiver",
            "self.device.output(cell.field_reference());",
            Some(vec!["$P0.value", "self.device"]),
        ),
        // General method-receiver frames still require member chains. An
        // indexed receiver needs absorbing collection precision transported
        // through that shared instantiation, not index erasure at this boundary.
        (
            "indexed_receiver",
            "self.device.output(self.cell_array[0].field_reference());",
            None,
        ),
        (
            "reference_member_receiver",
            "self.device.output(self.holder.cell.field_reference());",
            None,
        ),
        (
            "shared_parameter_receiver",
            "self.device.output(cell.field_reference());",
            None,
        ),
        (
            "effectful",
            "self.device.output(audited(&mut self.value, &mut self.audit));",
            Some(vec!["self.audit", "self.device", "self.value"]),
        ),
        (
            "indexed_result",
            "self.device.output(element(&mut self.cells));",
            Some(vec!["self.cells", "self.device"]),
        ),
        (
            "indexed_actual",
            "self.device.output(identity(&mut self.cells[1]));",
            Some(vec!["self.cells", "self.device"]),
        ),
        (
            "value_call",
            "let result: u64 = self.device.output_value(identity(&mut self.value));",
            Some(vec!["self.device", "self.value"]),
        ),
        (
            "recursive",
            "self.device.output(recursive(&mut self.value));",
            None,
        ),
        (
            "boundary_cycle",
            "self.device.output(self.recursive_boundary());",
            None,
        ),
        (
            "opaque",
            "self.device.output(opaque(&mut self.value));",
            None,
        ),
        (
            "unknown_result",
            "self.device.output(unknown(&mut self.value));",
            None,
        ),
        (
            "carrier",
            "self.device.output_carrier(carrier(&mut self.carrier));",
            None,
        ),
        (
            "carrier_scalar",
            "self.device.output(carrier_scalar(&mut self.carrier));",
            None,
        ),
        ("shared", "self.device.output(shared(&self.value));", None),
        (
            "boundary_reference",
            "self.device.output(self.device.reference(&mut self.value));",
            None,
        ),
    ];
    let mut source = String::from(
        r#"
        data Cell { value: u64; }
        data Carrier { value: &mut u64; }
        data ReferenceHolder { cell: &mut Cell; }
        boundary trait Device {
            machine output(value: &mut u64);
            machine output_value(value: &mut u64) -> u64;
            machine output_carrier(value: &mut Carrier);
            machine reference(value: &mut u64) -> &mut u64;
        }
        data Main {
            device: Device; value: u64; audit: u64;
            cell: Cell; cells: [u64; 2]; carrier: Carrier;
            cell_array: [Cell; 2]; holder: ReferenceHolder;
        }
        machine identity(value: &mut u64) -> &mut u64 { value }
        machine field(value: &mut Cell) -> &mut u64 { &mut value.value }
        machine Cell::field_reference(&mut self) -> &mut u64 { &mut self.value }
        machine element(values: &mut [u64; 2]) -> &mut u64 { &mut values[0] }
        machine audited<'value, 'audit>(
            value: &'value mut u64, audit: &'audit mut u64
        ) -> &'value mut u64 { audit = 1; value }
        machine recursive(value: &mut u64) -> &mut u64 { recursive(value) }
        machine Main::recursive_boundary(&mut self) -> &mut u64 {
            self.device.output(self.recursive_boundary());
            &mut self.value
        }
        machine opaque(value: &mut u64) -> &mut u64 { unknown(value); value }
        machine carrier(value: &mut Carrier) -> &mut Carrier { value }
        machine carrier_scalar(value: &mut Carrier) -> &mut u64 { value.value }
        machine shared(value: &u64) -> &u64 { value }
    "#,
    );
    for (name, body, _) in &cases {
        let parameters = match *name {
            "parameter_receiver" => ", cell: &mut Cell",
            "shared_parameter_receiver" => ", cell: &Cell",
            _ => "",
        };
        source.push_str(&format!(
            "machine Main::case_{name}(&mut self{parameters}) {{ {body} }}"
        ));
    }
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("resolver");
    let mut failures = Vec::new();
    for (name, _, expected) in cases {
        let qualified = format!("Main::case_{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified)
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let statement = typed
            .statement_table
            .statements(state.statement_nodes)
            .last()
            .expect("boundary call statement");
        let direct = match statement {
            psi_typed_trees::statement::StatementNode::Call(call) => {
                if name == "attached" {
                    let argument = typed.statement_table.expression_handles(call.arguments)[0];
                    let psi_typed_trees::expression::ExpressionNode::Call(helper) =
                        typed.expression_table.expression(argument)
                    else {
                        panic!("attached helper");
                    };
                    let callee = typed
                        .machines()
                        .iter()
                        .find(|machine| machine.name.as_str() == "Cell::field_reference")
                        .expect("attached callee");
                    let callee_state = &typed.machine_states(callee)[0];
                    assert_eq!(
                        helper.target_symbol, callee_state.symbol,
                        "attached call uses exact resolved helper"
                    );
                }
                // The statement-call query excludes argument evaluation. The
                // production statement consumer joins these two frames.
                let boundary = resolver.may_write_paths(machine, call);
                let producers = resolver.statement_value_may_write_paths(machine, statement);
                if name == "effectful" {
                    let mut boundary_paths = boundary.clone().expect("boundary reach");
                    boundary_paths.sort();
                    assert_eq!(boundary_paths, ["self.device", "self.value"]);
                    assert_eq!(
                        producers.as_deref(),
                        Some(["self.audit".to_owned()].as_slice())
                    );
                }
                boundary.zip(producers).map(|(mut written, producers)| {
                    written.extend(producers);
                    written.sort();
                    written.dedup();
                    written
                })
            }
            psi_typed_trees::statement::StatementNode::LocalData(local) => {
                resolver.expression_may_write_paths(machine, local.initial_value)
            }
            _ => panic!("boundary call"),
        };
        for (query, frame) in [
            resolver
                .inferred_state_write_frame(machine, state)
                .into_complete_paths(),
            direct,
        ]
        .into_iter()
        .enumerate()
        {
            let actual = frame.map(|mut paths| {
                paths.sort();
                paths
            });
            let expected = expected.as_ref().map(|paths| {
                let mut paths: Vec<_> = paths.iter().map(|path| (*path).to_owned()).collect();
                if name == "local" && query == 1 {
                    paths.push("alias".to_owned());
                }
                if name == "owned_local_receiver" && query == 1 {
                    paths.push("local.value".to_owned());
                }
                if name == "parameter_receiver" && query == 1 {
                    paths.retain(|path| path != "$P0.value");
                    paths.push("cell.value".to_owned());
                }
                paths.sort();
                paths
            });
            if actual != expected {
                failures.push(format!(
                    "{name} query {query}: expected {expected:?}, actual {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn boundary_attached_result_requires_the_exact_caller_self_identity() {
    use psi_typed_trees::expression::ExpressionNode;
    use psi_typed_trees::statement::StatementNode;
    let source = r#"
        boundary trait Device { machine output(value: &mut u64); }
        data Cell { value: u64; }
        data Main { device: Device; cell: Cell; }
        machine Cell::field_reference(&mut self) -> &mut u64 { &mut self.value }
        machine Main::run(&mut self) { self.device.output(self.cell.field_reference()); }
        machine Main::foreign(&mut self) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let original = lower_symbol_resolved_trees(&resolved).expect("type");
    for foreign in [false, true] {
        let mut typed = original.clone();
        if foreign {
            let foreign_symbol = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Main::foreign")
                .expect("foreign machine")
                .symbol;
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Main::run")
                .expect("caller");
            let state = &typed.machine_states(machine)[0];
            let StatementNode::Call(call) =
                &typed.statement_table.statements(state.statement_nodes)[0]
            else {
                panic!("boundary call");
            };
            let argument = typed.statement_table.expression_handles(call.arguments)[0];
            let ExpressionNode::Call(helper) = typed.expression_table.expression(argument) else {
                panic!("helper call");
            };
            let ExpressionNode::Member(member) = typed.expression_table.expression(helper.receiver)
            else {
                panic!("owned field receiver");
            };
            let root = member.receiver;
            let ExpressionNode::Name(name) = typed.expression_table.expression_mut(root) else {
                panic!("self root");
            };
            name.symbol = foreign_symbol;
            name.head_symbol = foreign_symbol;
            let members = name.member_symbols;
            typed
                .expression_table
                .set_name_path_member_symbol_at_offset(members, 0, foreign_symbol);
        }
        let resolver = psi_validation::CallFrameResolver::new(&typed).expect("resolver");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let StatementNode::Call(call) = &typed.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("boundary call");
        };
        let frame = resolver.may_write_frame(machine, call);
        if foreign {
            assert!(
                !frame.is_complete(),
                "foreign machine cannot supply the caller's self origin"
            );
        } else {
            let mut paths = frame.into_complete_paths().expect("exact caller self");
            paths.sort();
            assert_eq!(paths, ["self.cell.value", "self.device"]);
        }
    }
}

#[test]
fn boundary_reference_binding_identity_requires_the_live_caller_declaration() {
    use psi_typed_trees::expression::ExpressionNode;
    use psi_typed_trees::statement::StatementNode;
    use psi_typed_trees::types::TypeReferenceNode;
    let source = r#"
        boundary trait Device { machine output(value: &mut u64); }
        data Main { device: Device; value: u64; }
        machine identity(value: &mut u64) -> &mut u64 { value }
        machine Main::current(&mut self, output: &mut u64) { self.device.output(output); }
        machine Main::wrapped(&mut self, output: &mut u64) {
            self.device.output(identity(identity(output)));
        }
        machine Main::foreign(&mut self, output: &mut u64) {}
        machine Main::early(&mut self) {
            self.device.output(alias);
            let alias: &mut u64 = &mut self.value;
        }
        machine Main::early_wrapped(&mut self) {
            self.device.output(identity(identity(alias)));
            let alias: &mut u64 = &mut self.value;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let original = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    for (wrapped, variant) in [false, true].into_iter().flat_map(|wrapped| {
        ["exact", "constrained", "foreign", "stale", "later_local"]
            .into_iter()
            .map(move |variant| (wrapped, variant))
    }) {
        let mut typed = original.clone();
        let machine_name = match (wrapped, variant) {
            (false, "later_local") => "Main::early",
            (true, "later_local") => "Main::early_wrapped",
            (false, _) => "Main::current",
            (true, _) => "Main::wrapped",
        };
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let StatementNode::Call(call) = &typed.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("boundary call")
        };
        let mut argument = typed.statement_table.expression_handles(call.arguments)[0];
        while let ExpressionNode::Call(inner) = typed.expression_table.expression(argument) {
            argument = typed.expression_table.expression_handles(inner.arguments)[0];
        }
        let replacement = match variant {
            "foreign" => {
                let foreign = typed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "Main::foreign")
                    .expect("foreign caller");
                let foreign_state = &typed.machine_states(foreign)[0];
                Some(
                    typed
                        .state_parameters(foreign_state)
                        .iter()
                        .find(|parameter| parameter.name.as_str() == "output")
                        .expect("same-spelling foreign parameter")
                        .symbol,
                )
            }
            "stale" => {
                let ExpressionNode::Name(name) = typed.expression_table.expression(argument) else {
                    panic!("named argument")
                };
                Some(psi_symbols::SymbolHandle::from_parts(
                    name.symbol.arena_index(),
                    name.symbol.generation() + 1,
                ))
            }
            "later_local" => {
                let StatementNode::LocalData(local) =
                    &typed.statement_table.statements(state.statement_nodes)[1]
                else {
                    panic!("later local")
                };
                Some(local.symbol)
            }
            "constrained" => {
                let parameter = typed
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.name.as_str() == "output")
                    .expect("parameter");
                let reference = parameter.type_reference;
                let node = typed.type_reference_table.type_reference(reference).clone();
                assert!(matches!(node, TypeReferenceNode::Reference { .. }));
                let base_type = typed.type_reference_table.insert(node);
                typed.type_reference_table.substitute_node(
                    reference,
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints: Default::default(),
                    },
                );
                None
            }
            _ => None,
        };
        if let Some(symbol) = replacement {
            let ExpressionNode::Name(name) = typed.expression_table.expression_mut(argument) else {
                panic!("named argument")
            };
            name.symbol = symbol;
            name.head_symbol = symbol;
            let members = name.member_symbols;
            if !members.is_empty() {
                typed
                    .expression_table
                    .set_name_path_member_symbol_at_offset(members, 0, symbol);
            }
        }
        let resolver = psi_validation::CallFrameResolver::new(&typed).expect("resolver");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == machine_name)
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let StatementNode::Call(call) = &typed.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("boundary call")
        };
        let frame = resolver.may_write_frame(machine, call);
        if matches!(variant, "exact" | "constrained") {
            let mut actual = frame
                .complete_paths()
                .expect("live declared reference forwards")
                .to_vec();
            actual.sort();
            assert_eq!(
                actual,
                ["output", "self.device"],
                "{variant}, wrapped={wrapped}"
            );
        } else {
            assert!(
                !frame.is_complete(),
                "{variant}, wrapped={wrapped} must not grant a caller binding frame by spelling"
            );
        }
    }
}

#[test]
fn boundary_reference_bindings_keep_exact_origins_without_reborrowing_slots() {
    let repeated_calls = format!(
        "let alias: &mut u64 = &mut self.value; {}",
        "self.device.output(alias);".repeat(24)
    );
    let cases = [
        (
            "parameter",
            ", output: &mut u64",
            "self.device.output(output);",
            Some(vec!["$P0", "self.device"]),
            vec![Some(vec!["output", "self.device"])],
        ),
        (
            "local",
            "",
            "let alias: &mut u64 = &mut self.value; self.device.output(alias);",
            Some(vec!["self.device", "self.value"]),
            vec![Some(vec!["alias", "self.device", "self.value"])],
        ),
        (
            "consecutive_calls",
            "",
            repeated_calls.as_str(),
            Some(vec!["self.device", "self.value"]),
            vec![Some(vec!["alias", "self.device", "self.value"]); 24],
        ),
        (
            "replaced",
            "",
            "let mut alias: &mut u64 = &mut self.value;
             let prior: &mut u64 = alias;
             alias = &mut self.other;
             self.device.output(prior);
             self.device.output(alias);",
            Some(vec!["self.device", "self.other", "self.value"]),
            vec![
                Some(vec!["prior", "self.device", "self.value"]),
                Some(vec!["alias", "self.device", "self.other"]),
            ],
        ),
        (
            "value_call",
            "",
            "let alias: &mut u64 = &mut self.value;
             let result: u64 = self.device.output_value(alias);",
            Some(vec!["self.device", "self.value"]),
            vec![Some(vec!["alias", "self.device", "self.value"])],
        ),
        (
            "shared",
            ", output: &u64",
            "self.device.output(output);",
            None,
            vec![None],
        ),
        (
            "missing",
            "",
            "self.device.output(missing);",
            None,
            vec![None],
        ),
        (
            "carrier_field",
            "",
            "self.device.output(self.carrier.value);",
            None,
            vec![None],
        ),
        (
            "reference_result",
            "",
            "self.device.output(identity(&mut self.value));",
            Some(vec!["self.device", "self.value"]),
            vec![Some(vec!["self.device", "self.value"])],
        ),
        (
            "unknown_prefix",
            "",
            "let alias: &mut u64 = &mut self.value;
             unknown([0]);
             self.device.output(alias);",
            None,
            vec![None],
        ),
    ];
    let mut source = String::from(
        r#"
        data Carrier { value: &mut u64; }
        boundary trait Device {
            machine output(value: &mut u64);
            machine output_value(value: &mut u64) -> u64;
        }
        data Main { device: Device; value: u64; other: u64; carrier: Carrier; }
        machine identity(value: &mut u64) -> &mut u64 { value }
    "#,
    );
    for (name, parameters, body, _, _) in &cases {
        source.push_str(&format!(
            "machine Main::{name}(&mut self{parameters}) {{ {body} }}"
        ));
    }
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("lower typed trees");
    let resolver = psi_validation::CallFrameResolver::new(&typed).expect("resolver");
    let mut failures = Vec::new();
    for (name, _, _, expected_state, expected_calls) in cases {
        let qualified = format!("Main::{name}");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == qualified)
            .expect("caller");
        let state = typed.machine_states(machine).first().expect("entry");
        let mut frames = vec![resolver.inferred_state_write_frame(machine, state)];
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                psi_typed_trees::statement::StatementNode::Call(call)
                    if call.target.as_str() == "output" =>
                {
                    frames.push(resolver.may_write_frame(machine, call));
                }
                psi_typed_trees::statement::StatementNode::LocalData(local)
                    if matches!(typed.expression_table.expression(local.initial_value),
                        psi_typed_trees::expression::ExpressionNode::Call(call) if call.target.as_str() == "output_value") =>
                {
                    frames.push(resolver.expression_write_frame(machine, local.initial_value));
                }
                _ => {}
            }
        }
        let expected: Vec<_> = std::iter::once(expected_state)
            .chain(expected_calls)
            .collect();
        assert_eq!(frames.len(), expected.len(), "{name} query count");
        for (query, (frame, expected)) in frames.into_iter().zip(expected).enumerate() {
            let actual = frame.complete_paths().map(|paths| {
                let mut paths = paths.to_vec();
                paths.sort();
                paths
            });
            let expected = expected.map(|paths| {
                let mut paths: Vec<_> = paths.into_iter().map(str::to_owned).collect();
                paths.sort();
                paths
            });
            if actual != expected {
                failures.push(format!(
                    "{name} query {query}: expected {expected:?}, actual {actual:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

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
