use crate::front_end::typed_program;
use crate::interpreter::evaluator::{Evaluator, Halt};

#[test]
fn explicit_crash_does_not_complete_as_unit() {
    for cause in ["Trap", "Abort"] {
        let program = typed_program(&format!(
            "machine main() crashes {cause} {{ crash {cause}; }}"
        ));
        let result = Evaluator::new(&program, &[]).run_entry("main", None);
        assert!(
            matches!(result, Err(Halt::Trap(message)) if message == format!("explicit crash {cause}"))
        );
    }
}

#[test]
fn explicit_crash_propagates_through_static_call() {
    let program = typed_program(
        "machine fail() crashes Trap { crash Trap; }
         machine main() crashes Trap { fail(); }",
    );
    assert!(matches!(
        Evaluator::new(&program, &[]).run_entry("main", None),
        Err(Halt::Trap(message)) if message == "explicit crash Trap"
    ));
}

#[test]
fn untaken_crash_preserves_ordinary_result() {
    let program = typed_program(
        "machine main() -> i32 crashes Trap {
             transition true { true -> (7) false -> failed() }
             state failed() -> i32 { crash Trap; }
         }",
    );
    assert!(matches!(
        Evaluator::new(&program, &[]).run_const_machine("main"),
        Ok(7)
    ));
}
