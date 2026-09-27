use crate::CallFrameResolver;
use facts::NormalizedWriteFrame;

fn inspection_frame(source: &str) -> NormalizedWriteFrame {
    let program = crate::front_end::typed_program(source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("inspect"))
        .expect("inspection machine");
    CallFrameResolver::new(&program)
        .expect("resolved declarations")
        .inferred_state_write_frame(machine, &program.machine_states(machine)[0])
}

// These queries test the caller-storage ceiling, independently of executable
// recursion admission. An incomplete recursive body is not an empty frame;
// the selected signature must account for every route back to caller storage.
#[test]
fn static_recursive_scalar_call_does_not_invent_namespace_storage() {
    let frame = inspection_frame(
        "data Arithmetic {} data Counter { count: u64; }
         machine Arithmetic::walk(value: u64) -> u64 { Arithmetic::walk(value) }
         machine Counter::inspect(&mut self) {
             self.count = 1;
             let result: u64 = Arithmetic::walk(0);
         }",
    );
    assert_eq!(
        frame.complete_paths(),
        Some(["self.count".to_owned()].as_slice())
    );
}

#[test]
fn static_recursive_exclusive_argument_keeps_its_actual_storage_ceiling() {
    let frame = inspection_frame(
        "data Arithmetic {} data Counter { count: u64; }
         machine Arithmetic::walk(value: &mut u64) -> u64 {
             value = 2;
             Arithmetic::walk(value)
         }
         machine Counter::inspect(&mut self) {
             let result: u64 = Arithmetic::walk(&mut self.count);
         }",
    );
    assert_eq!(
        frame.complete_paths(),
        Some(["self.count".to_owned()].as_slice())
    );
}

#[test]
fn static_recursive_ceiling_does_not_erase_operand_evaluation_writes() {
    let frame = inspection_frame(
        "data Arithmetic {} data Counter { count: u64; }
         machine bump(value: &mut u64) -> u64 { value = 2; 0 }
         machine Arithmetic::walk(value: u64) -> u64 { Arithmetic::walk(value) }
         machine Counter::inspect(&mut self) {
             let result: u64 = Arithmetic::walk(bump(&mut self.count));
         }",
    );
    assert_eq!(
        frame.complete_paths(),
        Some(["self.count".to_owned()].as_slice())
    );
}

#[test]
fn static_recursive_carried_references_do_not_acquire_an_empty_ceiling() {
    for (source, expected) in [
        (
            "data Arithmetic {} data Carrier { slot: &mut u64; }
         machine Arithmetic::walk(carrier: Carrier) -> u64 { Arithmetic::walk(carrier) }
         machine inspect(carrier: Carrier) { let result: u64 = Arithmetic::walk(carrier); }",
            None,
        ),
        (
            "data Carrier { slot: &mut u64; }
         machine Carrier::walk(self) -> u64 { self.walk() }
         machine inspect(carrier: Carrier) { let result: u64 = carrier.walk(); }",
            Some(vec!["$P0".to_owned()]),
        ),
    ] {
        assert_eq!(
            inspection_frame(source).into_complete_paths(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn static_recursive_transitive_service_reach_remains_opaque() {
    let frame = inspection_frame(
        "data Arithmetic {} boundary trait Device { machine touch(); }
         machine effect() reaches Device { Device::touch(); }
         machine Arithmetic::walk(value: u64) -> u64 {
             effect();
             Arithmetic::walk(value)
         }
         machine inspect() { let result: u64 = Arithmetic::walk(0); }",
    );
    assert!(!frame.is_complete());
}

#[test]
fn static_ceiling_queries_share_the_frozen_program_call_plans() {
    let program = crate::front_end::typed_program("machine inspect() {}");
    let _scope = crate::enter_frozen_program_scope(&program);
    let resolver = CallFrameResolver::new(&program).unwrap();
    let resolver_plans = resolver.call_plans();
    let recursive_plans = super::operand_call_plans(&program, None);
    assert!(std::sync::Arc::ptr_eq(&resolver_plans, &recursive_plans));

    let unrelated = program.clone();
    let unrelated_plans = super::operand_call_plans(&unrelated, None);
    assert!(!std::sync::Arc::ptr_eq(&resolver_plans, &unrelated_plans));
}
