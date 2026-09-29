//! A `let` after a call-result field store keeps two ordinals apart. Its
//! plan's dense binding ordinal counts every scalar result the state binds
//! first, including a stored call result the planner binds; its source roles
//! (`LocalInitializer`, `CallArgument`) carry the authored local ordinal, the
//! count of declared immutable locals before it. Lowering must rejoin each
//! local's source through the authored ordinal whichever way the store was
//! planned. The planner binds a store's result for a selected operator
//! application, which this front end does not select; the native corpus
//! fixture `pass/float/local_after_float_call_store_exit` covers that shape.
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};

fn lowers_and_verifies(source: &str) {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("locals after a call-result store lower");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("locals after a call-result store verify");
}

/// A checked-body call stored into a field binds no scalar result of its own,
/// so the locals after it take dense ordinals 0 and 1 and authored ordinals
/// 0 and 1: counting the store would key both source roles one slot late.
#[test]
fn locals_after_a_checked_body_call_store_rejoin_their_sources() {
    lowers_and_verifies(
        r#"
        machine echo(value: i32) -> i32 { value }
        machine seven() -> i32 { 7 }
        data Main { a: i32; stored: i32; kept_second: i32; kept_third: i32; }
        machine Main::run(&mut self) {
            self.a = 4;
            self.stored = echo(self.a);
            let second: i32 = echo(self.a);
            let third: i32 = seven();
            self.kept_second = second;
            self.kept_third = third;
        }
    "#,
    );
}
