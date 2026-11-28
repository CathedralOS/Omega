//! A machine passed as a value is selected by the declaration the source
//! names, and a call through a machine value owes the requirement of the
//! callable type it is called through. Stage 05 writes the selection as a
//! descriptor that names one machine and writes the call's debt as one
//! obligation per `requires` clause; the lowered-form checker trusts both.
//!
//! The spec rule, `wiki/spec/language/machine_values.md`: "Named machines
//! can be stored in ordinary data and selected at runtime. A callable field
//! publishes the complete contract its caller may rely on."

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, operation_mut, the_operation};
use terminal_check_tables::obligations::ObligationKind;
use terminal_psi::OperationKind;

const CHOOSER: &str = "data Step = machine(x: u8) -> u8 reaches none crashes none; \
    machine action(x: u8) -> u8 reaches none crashes none { x } \
    machine second(x: u8) -> u8 reaches none crashes none { x +% 1u8 } \
    machine run(f: Step, x: u8) -> u8 { f(x) } \
    machine main() -> u8 { run(action, 3u8) +% run(second, 3u8) }";

/// `run(action, 3u8)` passes a descriptor that names `action`, and
/// `run(second, 3u8)` in the same body one that names `second`, although
/// both machines have the same type.
#[test]
fn a_named_machine_value_selects_the_machine_the_source_names() {
    let program = Program::of(CHOOSER);
    let main = program.machine("main");
    let descriptors = &program.elaboration().module.execution.callables.descriptors;
    let mut selected: Vec<_> = main
        .operations()
        .filter_map(|operation| match &operation.kind {
            OperationKind::SelectMachine { descriptor } => Some((operation.id, *descriptor)),
            _ => None,
        })
        .map(|(operation, descriptor)| {
            let named = descriptors
                .iter()
                .find(|candidate| candidate.id == descriptor)
                .expect("the descriptor is declared");
            (
                program.written(main, operation),
                program.machine_named(named.machine),
            )
        })
        .collect();
    selected.sort();
    assert_eq!(
        selected,
        [
            ("action".to_owned(), "action".to_owned()),
            ("second".to_owned(), "second".to_owned()),
        ]
    );
    program.judged(|_| {}).accepted();

    // Both machines satisfy `Step`, so a descriptor redirected to the other
    // one is still a valid program and the checker accepts it: only stage
    // 05's choice decides which machine runs.
    let action = program.elaboration().module.machines[program.position("action")].id;
    program
        .judged(|lowered| {
            for candidate in &mut lowered.module.execution.callables.descriptors {
                candidate.machine = action;
            }
        })
        .accepted();
}

const READER: &str = "data ByteReader = machine(offset: u32) -> u8 requires offset < 256 \
    ensures result < 200 reaches none crashes none;";

/// `reader(offset)` owes the callable type's `requires offset < 256`, as
/// one call-requirement obligation on the call. With the same clause on
/// `read` the checker proves it; without it the program is rejected for
/// that obligation. Dropping the obligation from the lowered call is
/// refused.
#[test]
fn a_call_through_a_machine_value_owes_its_callable_types_requirement() {
    let read = |requires: &str| {
        Program::of(&format!(
            "{READER} machine read(reader: ByteReader, offset: u32) -> u8 {requires} \
             {{ reader(offset) }}"
        ))
    };
    let program = read("requires offset < 256");
    let machine = program.machine("read");
    let call = the_operation(machine, |kind| {
        matches!(kind, OperationKind::CallMachine { .. })
    });
    let OperationKind::CallMachine {
        requirement_obligations,
        ..
    } = &call.kind
    else {
        unreachable!()
    };
    let [obligation] = requirement_obligations.as_slice() else {
        panic!("one requirement: {requirement_obligations:?}")
    };
    let tables = program.tables("read");
    assert!(
        matches!(
            tables.obligations.get(*obligation).map(|row| &row.kind),
            Some(ObligationKind::CallRequirement { clause: 0, .. })
        ),
        "{:?}",
        tables.obligations.get(*obligation)
    );
    program.judged(|_| {}).accepted();

    let rejection = read("").judged(|_| {}).rejected();
    assert_eq!(
        rejection.unproved,
        ["CallRequirement { invocation: InvocationId(1), clause: 0 }"],
        "{rejection:?}"
    );

    let id = call.id;
    let refusal = program
        .judged_in("read", |machine| {
            let OperationKind::CallMachine {
                requirement_obligations,
                ..
            } = &mut operation_mut(machine, id).kind
            else {
                unreachable!()
            };
            requirement_obligations.clear();
        })
        .refused();
    assert!(
        refusal
            .detail
            .contains("a call does not owe one requirement per requires clause of its callee"),
        "{refusal:?}"
    );
}

#[test]
fn a_returned_reader_can_call_its_own_state_of_the_same_concrete_type() {
    let source = r#"data File { byte: u8; }
data ReaderFn<S> = machine(state: @S) -> u8 reaches none crashes none;
data Reader<'a, some S> { state: @'a S; read: ReaderFn<S>; }
machine from_file(state: @File) -> u8 reaches none crashes none { state.byte }
machine pack<'a>(file: @'a File) -> Reader<'a> {
    Reader { state: file, read: from_file }
}
machine main() crashes Trap {
    let file: File = File { byte: 17u8 };
    let other_file: File = File { byte: 42u8 };
    let reader: Reader = pack(@file);
    let other: Reader = pack(@other_file);
    let value: u8 = (reader.read)(reader.state);
    transition value == 17u8 { true -> done(), false -> wrong() }
    machine done() {}
    machine wrong() { crash Trap; }
}
"#;
    Program::of(source).judged(|_| ()).accepted();

    let crossed = source.replace("(reader.read)(reader.state)", "(reader.read)(other.state)");
    let Err(refusal) = Program::try_of(&[("main.omg", &crossed)]) else {
        panic!("the two returned File readers must keep distinct openings");
    };
    assert!(refusal.message.contains("argument types"), "{refusal:?}");
}
