use validation::validate_program;

fn diagnostics(source: &str) -> Vec<String> {
    let typed = crate::front_end::typed_program(source);
    validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn mixed_boundary_return_initializers_admit_nested_calls_with_immutable_scalar_formals() {
    for boundary_crash in ["", "crashes Abort"] {
        let source = format!(
            r#"
            machine abort() -> u16 crashes Abort {{ crash Abort; }}
            boundary trait PortIo {{}}
            pub data Receipt [linear] {{ value: u64; }}
            boundary machine Receipt::settle(self, first: u16, second: u16) -> u16
            reaches PortIo {boundary_crash} ensures true;
            data Wrapper {{}}
            machine Wrapper::measure(receipt: Receipt, value: u16) -> u16
            reaches PortIo crashes Abort
            {{ let accepted: u16 = receipt.settle(abort(), value); accepted }}
        "#
        );
        let messages = diagnostics(&source);
        assert!(messages.is_empty(), "{boundary_crash}: {messages:?}");
        let messages = diagnostics(&source.replace(
            "measure(receipt: Receipt, value: u16)",
            "measure(receipt: Receipt, mut value: u16)",
        ));
        assert!(
            messages
                .iter()
                .any(|message| message
                    .contains("value-call argument cannot itself be a machine call")),
            "mutable scalar formal remains outside this source family: {messages:?}"
        );
    }
}

#[test]
fn free_scalar_local_assignments_admit_nested_call_evaluation() {
    let diagnostics = diagnostics(
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let mut saved: bool = input;
             saved = identity(identity(input));
             saved
         }",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn unit_scalar_store_assignments_admit_nested_call_evaluation() {
    // The unit statement sequence owns these assignments: the nested operand is
    // a checked `AssignmentValue` computation argument evaluated exactly once
    // before the outer call and the store.
    for source in [
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let mut saved: bool = input;
             saved = identity(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(saved: &mut bool, input: bool) {
             saved = identity(identity(input));
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::value(&mut self, input: bool) {
             let mut saved: bool = input;
             saved = identity(identity(input));
             self.flag = saved;
         }",
        "machine identity(input: bool) -> bool { input }
         machine choose(left: bool, right: bool) -> bool { left }
         machine value(input: bool) {
             let mut saved: bool = input;
             saved = choose(identity(input), identity(input));
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    }
}

#[test]
fn member_scalar_store_assignments_admit_nested_call_evaluation() {
    // Member destinations on a borrowed structural parameter or the attached
    // receiver sequence through the same checked structural field-store
    // producer as a scalar source: the nested operand is an `AssignmentValue`
    // computation evaluated exactly once before the outer call and the store.
    for source in [
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::value(&mut self, input: bool) -> bool {
             self.flag = identity(identity(input));
             self.flag
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine value(param: &mut Container, input: bool) {
             param.flag = identity(identity(input));
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine value(param: &write Container, input: bool) {
             param.flag = identity(identity(input));
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::write(&write self, input: bool) {
             self.flag = identity(identity(input));
         }",
        "data Inner { flag: bool; }
         data Outer { inner: Inner; }
         machine identity(input: bool) -> bool { input }
         machine value(param: &mut Outer, input: bool) {
             param.inner.flag = identity(identity(input));
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    }
}

#[test]
fn unserved_assignment_destinations_keep_nested_call_realization_fence() {
    for source in [
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let mut saved: [bool; 1] = [input];
             saved[0] = identity(identity(input));
             input
         }",
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let mut saved: Container = Container { flag: input };
             saved.flag = identity(identity(input));
             input
         }",
        // A shared borrow carries no store authority, so `param.flag` stays
        // outside the realized member-store family.
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine value(param: &Container, input: bool) {
             param.flag = identity(identity(input));
         }",
        // A shared primitive borrow sibling is unplannable in the
        // attached/no-scalar-parameter signature variant, so the receiver
        // member store keeps the fence rather than outrun the producer.
        "data Container { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine Container::value(&mut self, observer: &bool) {
             self.flag = identity(identity(false));
         }",
        // The leaf's declared primitive must be exactly the callee's result.
        "data Container { flag: i32; }
         machine identity(input: bool) -> bool { input }
         machine value(param: &mut Container, input: bool) {
             param.flag = identity(identity(input));
         }",
        "machine identity(input: i32) -> i32 { input }
         machine value(input: i32) {
             let mut saved: i32 in Wrapping = 0;
             saved = identity(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) -> bool {
             let saved: bool = input;
             saved = identity(identity(input));
             saved
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(
            diagnostics
                .iter()
                .any(|message| message
                    .contains("value-call argument cannot itself be a machine call")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn immutable_unit_result_initializers_admit_computed_scalar_operands() {
    for source in [
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let saved: bool = identity(identity(input));
         }",
        "machine identity(input: bool) -> bool { input }
         machine value(input: bool) {
             let before: bool = input;
             let saved: bool = identity(identity(before));
         }",
        "data Scalar {}
         machine identity(input: bool) -> bool { input }
         machine Scalar::read(input: bool) -> bool { input }
         data Root {}
         machine Root::value(&mut self, input: bool) {
             let saved: bool = Scalar::read(identity(input));
         }",
        "boundary trait Host { machine read(input: bool) -> bool reaches Host; }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) reaches Host {
             let saved: bool = Host::read(identity(input));
         }",
        "pub data Packet { flag: bool; }
         boundary trait Host { machine read(input: bool) -> Packet reaches Host; }
         machine identity(input: bool) -> bool { input }
         machine value(input: bool) reaches Host {
             let saved: Packet = Host::read(identity(input));
         }",
    ] {
        let diagnostics = diagnostics(source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    }
}

#[test]
fn nominal_boundary_parameter_result_initializers_keep_exact_requirement_eligibility() {
    for (declaration, result) in [("", "bool"), ("pub data Packet { flag: bool; }", "Packet")] {
        let source = format!(
            "{declaration}
             boundary trait Host {{ machine read(input: bool) -> {result} reaches Host; }}
             machine identity(input: bool) -> bool {{ input }}
             machine value<machine Read>(input: bool)
             where machine Read satisfies Host::read;
             reaches Host {{
                 let saved: {result} = Read(identity(input));
             }}"
        );
        let diagnostics = diagnostics(&source);
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    }
}
