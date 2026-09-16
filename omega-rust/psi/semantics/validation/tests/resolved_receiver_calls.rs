//! Resolved receiver calls on parameters, locals, nested value paths, type
//! names, and boundary parameters run the SAME result-use, arity, and
//! type-parameter-bound validation as every other call. The resolved
//! `target_symbol` locates the exact callee; it is evidence about which
//! declaration the call invokes, never a waiver of the caller's
//! obligations.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolved source");
    lower_symbol_resolved_trees(&resolved).expect("typed source")
}

fn rejects(source: &str, fragment: &str) {
    let diagnostics = validation::validate_program(&typed(source))
        .expect_err("invalid resolved receiver call must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}"
    );
}

fn accepts(source: &str) {
    validation::validate_program(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

#[test]
fn parameter_receiver_calls_obey_result_use_and_arity() {
    for (body, expected) in [
        // An implicit statement-position discard of a non-unit result is an
        // error even though the call's resolved target is exact.
        ("counter.bump(); 0", "discards its non-unit `u64` result"),
        // The callee's non-`self` parameters set the callable arity: `bump`
        // takes zero authored arguments, so extra operands cannot bind.
        (
            "counter.bump(1); 0",
            "state `bump` expects 0 argument(s), got 1",
        ),
    ] {
        rejects(
            &format!(
                "data Counter {{ count: u64; }}
                 machine Counter::bump(&mut self) -> u64 {{ self.count = 1; 0 }}
                 machine inspect(counter: &mut Counter) -> u64 {{ {body} }}"
            ),
            expected,
        );
    }
    // Explicit `_ =` discard and the receiver-bound `self` still validate.
    accepts(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) -> u64 { self.count = 1; 0 }
         machine inspect(counter: &mut Counter) -> u64 { _ = counter.bump(); 0 }",
    );
}

#[test]
fn local_receiver_calls_obey_result_use() {
    rejects(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) -> u64 { self.count = 1; 0 }
         machine inspect() -> u64 {
             let mut counter: Counter = Counter { count: 0 };
             counter.bump();
             0
         }",
        "discards its non-unit `u64` result",
    );
    accepts(
        "data Counter { count: u64; }
         machine Counter::bump(&mut self) -> u64 { self.count = 1; 0 }
         machine inspect() -> u64 {
             let mut counter: Counter = Counter { count: 0 };
             _ = counter.bump();
             0
         }",
    );
}

/// A nested value-path receiver (`outer.inner.read()`) keeps its resolved
/// endpoint and target symbols open until `lower_typed_trees` runs
/// `resolve_projected_receiver_calls` ahead of validation -- emulate that
/// resolved shape directly, the same way the authored-selection tests
/// substitute exact symbols into the call node.
fn with_resolved_nested_receiver_call(body: &str) -> TypedTrees {
    use typed_trees::statement::StatementNode;

    let mut program = typed(&format!(
        "data Inner {{ value: u64; }}
         machine Inner::read(&self) -> u64 {{ self.value }}
         data Outer {{ inner: Inner; }}
         machine inspect(outer: &Outer) -> u64 {{ {body} }}"
    ));
    let read_state = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.name.as_str() == "read")
        .expect("attached Inner::read state")
        .symbol;
    let inner = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .and_then(|definition| {
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == "inner" =>
                    {
                        Some(field.symbol)
                    }
                    _ => None,
                })
        })
        .expect("Outer::inner field");
    let (statement_nodes, outer) = {
        let inspect = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .expect("caller machine");
        let state = &program.machine_states(inspect)[0];
        (
            state.statement_nodes,
            program.state_parameters(state)[0].symbol,
        )
    };
    let StatementNode::Call(call) = &mut program.statement_table.statements_mut(statement_nodes)[0]
    else {
        panic!("nested statement receiver call")
    };
    // The pipeline's receiver resolution binds these exact identities before
    // validation runs; the unresolved typed form stays silent by design.
    call.receiver_root_symbol = outer;
    call.receiver_symbol = inner;
    call.target_symbol = read_state;
    program
}

#[test]
fn nested_receiver_path_calls_obey_result_use_and_arity() {
    for (body, expected) in [
        (
            "outer.inner.read(); 0",
            "discards its non-unit `u64` result",
        ),
        (
            "outer.inner.read(1, 2); 0",
            "state `read` expects 0 argument(s), got 2",
        ),
    ] {
        let diagnostics = validation::validate_program(&with_resolved_nested_receiver_call(body))
            .expect_err("invalid resolved receiver call must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "expected {expected:?}: {diagnostics:#?}"
        );
    }
    let accepted = with_resolved_nested_receiver_call("_ = outer.inner.read(); 0");
    validation::validate_program(&accepted)
        .unwrap_or_else(|diagnostics| panic!("explicit discard: {diagnostics:#?}"));
}

#[test]
fn type_name_receiver_calls_treat_the_first_argument_as_explicit_self() {
    // `Envelope::seal(&mut envelope)` spells the declaration: the first
    // authored argument IS the callee's `self` operand, exactly as the
    // lowering's `explicit_self` arity rule binds it.
    accepts(
        "data Envelope { sealed: bool; }
         machine Envelope::seal(&mut self) -> u64 { self.sealed = true; 0 }
         machine inspect() -> u64 {
             let mut envelope: Envelope = Envelope { sealed: false };
             _ = Envelope::seal(&mut envelope);
             0
         }",
    );
    for (body, expected) in [
        // Omitting the explicit self operand is an arity error, not a
        // receiver place.
        (
            "Envelope::seal(); 0",
            "state `seal` expects 1 argument(s), got 0",
        ),
        // A static-carrier result obeys the same strict result use.
        (
            "Envelope::seal(&mut envelope); 0",
            "discards its non-unit `u64` result",
        ),
    ] {
        rejects(
            &format!(
                "data Envelope {{ sealed: bool; }}
                 machine Envelope::seal(&mut self) -> u64 {{ self.sealed = true; 0 }}
                 machine inspect() -> u64 {{
                     let mut envelope: Envelope = Envelope {{ sealed: false }};
                     {body}
                 }}"
            ),
            expected,
        );
    }
}

#[test]
fn boundary_parameter_receiver_calls_obey_result_use_and_arity() {
    for (body, expected) in [
        ("probe.sample(7); 0", "discards its non-unit `bool` result"),
        (
            "probe.sample(); 0",
            "state `sample` expects 1 argument(s), got 0",
        ),
    ] {
        rejects(
            &format!(
                "boundary trait Probe {{
                     machine sample(code: i32) -> bool
                     reaches
                         Probe;
                 }}
                 machine use_probe(probe: &mut Probe) -> u64
                 reaches Probe
                 {{ {body} }}"
            ),
            expected,
        );
    }
    accepts(
        "boundary trait Probe {
             machine sample(code: i32) -> bool
             reaches
                 Probe;
         }
         machine use_probe(probe: &mut Probe) -> u64
         reaches Probe
         { _ = probe.sample(7); 0 }",
    );
}

#[test]
fn resolved_receiver_calls_enforce_type_parameter_bounds() {
    // `take<T [copy]>` pins `T` to the argument's declared place type through
    // the resolved receiver; a non-`copy` instantiation cannot satisfy it.
    rejects(
        "data Heavy { a: u64; b: u64; }
         data Carrier { count: u64; }
         machine Carrier::take<T [copy]>(&mut self, value: &T) -> u64 { 0 }
         machine inspect(carrier: &mut Carrier, heavy: &Heavy) -> u64 {
             _ = carrier.take(heavy);
             0
         }",
        "does not satisfy `[copy]`",
    );
    accepts(
        "data Marker [copy] { value: u64; }
         data Carrier { count: u64; }
         machine Carrier::take<T [copy]>(&mut self, value: &T) -> u64 { 0 }
         machine inspect(carrier: &mut Carrier, marker: &Marker) -> u64 {
             _ = carrier.take(marker);
             0
         }",
    );
}
