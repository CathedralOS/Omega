use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;
use crate::tests::front_end::typed_program_with_core_service;

#[test]
fn output_predicates_survive_read_only_boundary_arguments() {
    for receiver in ["console: Service<Console>", "console: &mut Console"] {
        let source = format!(
            r#"
            domain [u8; 4]::Utf8 requires valid_utf8(self);
            pub boundary trait Console {{ machine write(text: &[u8]); }}
            machine fill({receiver}, output: &mut [u8; 4]) reaches Console
            ensures output in Utf8 {{
                output = "okay";
                console.write(output);
            }}
        "#
        );
        // The `Service<Console>` receiver needs the same fused-service
        // erasure authorizations `settle_checked_providers` binds in real
        // builds; without them the carrier parameter stays unshaped.
        let mut typed = typed_program_with_core_service(&source);
        crate::tests::bind_fixture_fused_service_erasures(&mut typed);
        lower_typed_trees(typed, &CheckingRequest::settled())
            .unwrap_or_else(|diagnostics| panic!("{receiver}: {diagnostics:#?}"));
    }
}

#[test]
fn output_predicates_do_not_survive_writable_boundary_arguments() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Device { machine read(output: &mut [u8]); }
        machine fill(device: &mut Device, output: &mut [u8; 4])
        reaches Device ensures output in Utf8 {
            output = "okay";
            device.read(output);
        }
    "#;
    assert!(lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()).is_err());
}

#[test]
fn output_predicates_survive_read_only_boundary_expression_arguments() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Console { machine write(text: &[u8]) -> u64; }
        machine fill(console: &mut Console, output: &mut [u8; 4])
        reaches Console ensures output in Utf8 {
            output = "okay";
            let count: u64 = console.write(output);
        }
    "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("expression calls use the same selected readonly formal frame");
}

/// A writable boundary argument that names an indexed element lends exactly
/// that element: the durable frame coarsens the write to its collection path,
/// but the recorded borrow access retains `cells[1].out`, so the sibling
/// element and same-element sibling fields keep their declared coverage.
#[test]
fn writable_indexed_boundary_argument_preserves_sibling_element_coverage() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Device { machine read(output: &mut [u8]); }
        data Cell { out: [u8; 4] in Utf8; other: [u8; 4] in Utf8; }
        data Record { cells: [Cell; 2]; }
        machine Record::run(&mut self) reaches Device {
            self.cells[0].out = "aa";
            self.cells[0].other = "bb";
            self.cells[1].out = "cc";
            self.cells[1].other = "dd";
            Device::read(&mut self.cells[1].out);
            self.cells[1].out = "ee";
        }
    "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("the lent element is re-established and every sibling survives");
}

/// Without the re-establishing store the return fails, but only on the field
/// the boundary call actually lent — the coarsened collection path must not
/// retire `cells[0]` or `cells[1].other`.
#[test]
fn writable_indexed_boundary_argument_retires_only_the_lent_field() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Device { machine read(output: &mut [u8]); }
        data Cell { out: [u8; 4] in Utf8; other: [u8; 4] in Utf8; }
        data Record { cells: [Cell; 2]; }
        machine Record::run(&mut self) reaches Device {
            self.cells[0].out = "aa";
            self.cells[0].other = "bb";
            self.cells[1].out = "cc";
            self.cells[1].other = "dd";
            Device::read(&mut self.cells[1].out);
        }
    "#;
    let Err(diagnostics) =
        lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
    else {
        panic!("the lent field's coverage was handed to a &mut [u8] writer");
    };
    let field_requirements = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove default-domain field requirement")
        })
        .collect::<Vec<_>>();
    assert!(
        !field_requirements.is_empty()
            && field_requirements.iter().all(|diagnostic| {
                diagnostic.message.contains("self.cells[1].out")
                    && !diagnostic.message.contains("cells[0]")
                    && !diagnostic.message.contains("other")
            }),
        "expected only the lent `self.cells[1].out` to retire: {diagnostics:#?}"
    );
}

/// A shared borrow of a sibling element contributes no write path: it records
/// only a read access, so the exclusive argument's exact referent still refines
/// the coarsened collection path and the read element keeps its coverage.
#[test]
fn readonly_sibling_argument_does_not_widen_the_boundary_write() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Device { machine mix(output: &mut [u8], source: &[u8; 4]); }
        data Cell { out: [u8; 4] in Utf8; other: [u8; 4] in Utf8; }
        data Record { cells: [Cell; 2]; }
        machine Record::run(&mut self) reaches Device {
            self.cells[0].out = "aa";
            self.cells[0].other = "bb";
            self.cells[1].out = "cc";
            self.cells[1].other = "dd";
            Device::mix(&mut self.cells[1].out, &self.cells[0].out);
            self.cells[1].out = "ee";
        }
    "#;
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .expect("the shared read lends nothing and the lent field is restored");
}

/// A runtime index never names one element: `cells[pick()].out` may alias any
/// element, so every element's `out` coverage retires while the sibling `other`
/// fields keep theirs — unresolved indexes do not collapse to universal
/// coverage, and they still overlap every fixed element.
#[test]
fn runtime_indexed_boundary_argument_retires_every_elements_field() {
    let source = r#"
        domain [u8; 4]::Utf8 requires valid_utf8(self);
        boundary trait Device { machine read(output: &mut [u8]); }
        data Cell { out: [u8; 4] in Utf8; other: [u8; 4] in Utf8; }
        data Record { cells: [Cell; 2]; }
        machine Record::run(&mut self, index: u64[0..2]) reaches Device {
            self.cells[0].out = "aa";
            self.cells[0].other = "bb";
            self.cells[1].out = "cc";
            self.cells[1].other = "dd";
            Device::read(&mut self.cells[index].out);
            self.cells[0].out = "ee";
        }
    "#;
    let Err(diagnostics) =
        lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
    else {
        panic!("a runtime-indexed loan must retire every element's field coverage");
    };
    let field_requirements = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove default-domain field requirement")
        })
        .collect::<Vec<_>>();
    assert!(
        !field_requirements.is_empty()
            && field_requirements.iter().all(|diagnostic| {
                diagnostic.message.contains("out") && !diagnostic.message.contains("other")
            })
            && field_requirements
                .iter()
                .any(|diagnostic| diagnostic.message.contains("self.cells[1].out")),
        "expected only the `out` fields of both elements to retire: {diagnostics:#?}"
    );
}

#[test]
fn boundary_parameter_frames_require_exact_receiver_scope_and_signature() {
    use symbols::SymbolHandle;
    use typed_trees::statement::StatementNode;
    let original = parse_typed_trees(
        r#"
        boundary trait Console { machine write(text: &[u8]); }
        boundary trait Device { machine write(text: &mut [u8]); }
        machine fill(console: &mut Console, output: &mut [u8; 4]) reaches Console + Device {
            console.write(output);
            state other(console: &mut Device, output: &mut [u8; 4]) {
                console.write(output);
            }
        }
    "#,
    );
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "fill")
        .expect("fill");
    let entry = &original.machine_states(machine)[0];
    let other = &original.machine_states(machine)[1];
    let statements = entry.statement_nodes;
    let StatementNode::Call(call) = &original.statement_table.statements(statements)[0] else {
        panic!("entry call");
    };
    let StatementNode::Call(other_call) =
        &original.statement_table.statements(other.statement_nodes)[0]
    else {
        panic!("other call");
    };
    for (receiver, target, exact) in [
        (call.receiver_symbol, call.target_symbol, true),
        (SymbolHandle::invalid(), call.target_symbol, false),
        (
            SymbolHandle::from_parts(
                call.receiver_symbol.arena_index(),
                call.receiver_symbol.generation() + 1,
            ),
            call.target_symbol,
            false,
        ),
        (call.receiver_symbol, SymbolHandle::invalid(), false),
        (
            call.receiver_symbol,
            SymbolHandle::from_parts(
                call.target_symbol.arena_index(),
                call.target_symbol.generation() + 1,
            ),
            false,
        ),
        (call.receiver_symbol, other_call.target_symbol, false),
        (other_call.receiver_symbol, other_call.target_symbol, false),
    ] {
        let mut program = original.clone();
        let StatementNode::Call(call) = &mut program.statement_table.statements_mut(statements)[0]
        else {
            panic!("entry call");
        };
        call.receiver_symbol = receiver;
        call.target_symbol = target;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "fill")
            .expect("fill");
        let StatementNode::Call(call) = &program.statement_table.statements(statements)[0] else {
            panic!("entry call");
        };
        let paths = validation::CallFrameResolver::new(&program)
            .expect("resolver")
            .may_write_frame(machine, call)
            .into_complete_paths();
        if exact {
            assert_eq!(paths, Some(vec!["console".to_owned()]));
        } else {
            assert!(
                paths.is_none_or(|paths| paths.iter().any(|path| path == "output")),
                "invalid receiver/signature identity cannot manufacture a readonly frame"
            );
        }
    }
}

#[test]
fn boundary_parameter_methods_do_not_acquire_builtin_empty_frames() {
    use typed_trees::statement::StatementNode;
    let program = parse_typed_trees(
        r#"
        boundary trait Console { machine bytes() -> u64; }
        machine run(console: &mut Console) reaches Console { let count: u64 = console.bytes(); }
    "#,
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run");
    let state = &program.machine_states(machine)[0];
    let statement = &program.statement_table.statements(state.statement_nodes)[0];
    assert!(matches!(statement, StatementNode::LocalData(_)));
    let paths = validation::CallFrameResolver::new(&program)
        .expect("resolver")
        .statement_value_may_write_paths(machine, statement);
    assert_eq!(paths, Some(vec!["console".to_owned()]));
    let StatementNode::LocalData(local) = statement else {
        unreachable!();
    };
    let expression = local.initial_value;
    let mut missing = program.clone();
    let typed_trees::expression::ExpressionNode::Call(call) =
        missing.expression_table.expression_mut(expression)
    else {
        panic!("value call");
    };
    call.target_symbol = symbols::SymbolHandle::invalid();
    let machine = missing
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .expect("run");
    let state = &missing.machine_states(machine)[0];
    let statement = &missing.statement_table.statements(state.statement_nodes)[0];
    let paths = validation::CallFrameResolver::new(&missing)
        .expect("resolver")
        .statement_value_may_write_paths(machine, statement);
    assert!(
        paths.is_none_or(|paths| paths.iter().any(|path| path == "console")),
        "a missing method identity must not turn a known boundary receiver into a builtin"
    );
}
