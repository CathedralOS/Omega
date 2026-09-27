//! Resolved receiver calls on parameters, locals, nested value paths, type
//! names, and boundary parameters run the SAME result-use, arity, and
//! type-parameter-bound validation as every other call. The resolved
//! `target_symbol` locates the exact callee; it is evidence about which
//! declaration the call invokes, never a waiver of the caller's
//! obligations.

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
