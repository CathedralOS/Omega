//! Resolved receiver calls on parameters, locals, nested value paths, type
//! names, and boundary parameters run the SAME result-use, arity, and
//! type-parameter-bound validation as every other call. The resolved
//! `target_symbol` locates the exact callee; it is evidence about which
//! declaration the call invokes, never a waiver of the caller's
//! obligations.

mod namespace_calls;

fn rejects(source: &str, fragment: &str) {
    let diagnostics = validation::validate_program(&crate::front_end::typed_program(source))
        .expect_err("invalid resolved receiver call must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(fragment)),
        "expected {fragment:?}: {diagnostics:#?}"
    );
}

fn accepts(source: &str) {
    validation::validate_program(&crate::front_end::typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
}

#[test]
fn private_states_are_jump_destinations_not_callable_methods() {
    for call in ["finish(42)", "self.finish(42)", "Helper::run::finish(42)"] {
        rejects(
            &format!(
                "data Helper {{}}
             machine Helper::run(&self) -> u64 {{
                 let value: u64 = {call};
                 transition {{ _ -> finish(value) }}
                 state finish(&self, value: u64) -> u64 {{ value }}
             }}"
            ),
            "does not resolve",
        );
    }
    accepts(
        "machine finish(value: u64) -> u64 { value }
         data Helper {}
         machine Helper::run() -> u64 {
             let value: u64 = finish(42);
             transition { _ -> finish(value) }
             state finish(value: u64) -> u64 { value }
         }",
    );
}

#[test]
fn discarded_unit_calls_cannot_invoke_private_states_either() {
    for call in ["finish()", "self.finish()"] {
        rejects(
            &format!(
                "data Helper {{}}
             machine Helper::run(&self) {{
                 {call};
                 transition {{ _ -> finish() }}
                 state finish(&self) {{}}
             }}"
            ),
            "finish",
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
