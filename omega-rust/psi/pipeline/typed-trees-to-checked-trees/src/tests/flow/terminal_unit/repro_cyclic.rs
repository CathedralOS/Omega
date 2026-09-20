//! Temporary reproduction for the print_squares cyclic Unit plan gap.
use crate::tests::flow::terminal_unit::checked_with_service;
use crate::tests::flow::terminal_unit::machine_named;

fn assert_has_plan(source: &str, label: &str) {
    let checked = checked_with_service(source);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let main = machine_named(&checked, "Main::main");
    let ordinary = plans.for_machine(main).is_some();
    let composed = plans.composed_for_machine(main).is_some();
    assert!(
        ordinary || composed,
        "{label}: Main::main has no checked machine plan"
    );
}

const CONSOLE: &str = r#"
    pub boundary trait Console {
        machine exit_process(return_code: i32) reaches Console;
        machine write_line(text: &[u8]) reaches Console;
    }
"#;

// B0 control: scalar field, scalar guard, projected call (known FAIL).
#[test]
fn b0_scalar_guard_projected_call() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        self.k = 0;
        transition { _ -> loop_state() }
        state loop_state(&mut self) {
            transition self.k < 9 { true -> step() _ -> finish() }
        }
        state step(&mut self) {
            self.k = self.k + 1;
            transition { _ -> loop_state() }
        }
        state finish(&mut self) {
            self.console.exit_process(0);
        }
    }
"#
        ),
        "b0_scalar_guard_projected_call",
    );
}

// B1: scalar field writes but NO scalar guard (unconditional cycle).
#[test]
fn b1_scalar_writes_unconditional() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        self.k = 0;
        transition { _ -> step() }
        state step(&mut self) {
            self.k = self.k + 1;
            transition { _ -> finish() }
        }
        state finish(&mut self) {
            self.k = self.k + 1;
            transition { _ -> step() }
        }
    }
"#
        ),
        "b1_scalar_writes_unconditional",
    );
}

// B2: scalar guard on a FIELD, no scalar writes.
#[test]
fn b2_field_guard_no_writes() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        transition self.k < 9 { true -> finish() _ -> finish() }
        state finish(&mut self) {
            self.console.exit_process(0);
        }
    }
"#
        ),
        "b2_field_guard_no_writes",
    );
}

// B3: scalar field write in entry only; projected call; unconditional 2-state.
#[test]
fn b3_entry_scalar_write() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        self.k = 0;
        transition { _ -> finish() }
        state finish(&mut self) {
            self.console.exit_process(0);
        }
    }
"#
        ),
        "b3_entry_scalar_write",
    );
}

// B4: scalar ops, free (non-projected) boundary call — known PASS analog.
#[test]
fn b4_scalar_guard_free_call() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        self.k = 0;
        transition { _ -> loop_state() }
        state loop_state(&mut self) {
            transition self.k < 9 { true -> step() _ -> finish() }
        }
        state step(&mut self) {
            self.k = self.k + 1;
            transition { _ -> loop_state() }
        }
        state finish(&mut self) {
            Console::exit_process(0);
        }
    }
"#
        ),
        "b4_scalar_guard_free_call",
    );
}

// B5: projected call AND scalar guard on a state PARAMETER (not field).
#[test]
fn b5_parameter_guard_projected_call() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; }
    machine Main::main(&mut self) reaches Console {
        transition { _ -> loop_state(0) }
        state loop_state(&mut self, k: u32) {
            transition k < 9 { true -> step(k) _ -> finish() }
        }
        state step(&mut self, k: u32) {
            transition { _ -> loop_state(k + 1) }
        }
        state finish(&mut self) {
            self.console.exit_process(0);
        }
    }
"#
        ),
        "b5_parameter_guard_projected_call",
    );
}

// B7: same as B0 but step writes a constant (no field read in RHS).
#[test]
fn b7_constant_write_step() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        self.k = 0;
        transition { _ -> loop_state() }
        state loop_state(&mut self) {
            transition self.k < 9 { true -> step() _ -> finish() }
        }
        state step(&mut self) {
            self.k = 7;
            transition { _ -> loop_state() }
        }
        state finish(&mut self) {
            self.console.exit_process(0);
        }
    }
"#
        ),
        "b7_constant_write_step",
    );
}

// B8: same as B0 but the guard reads a local-free parameterless shape:
// scalar field read in a non-guard statement (entry), projected call, cycle.
#[test]
fn b8_field_read_entry_projected() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        self.k = self.k + 1;
        transition { _ -> finish() }
        state finish(&mut self) {
            self.console.exit_process(0);
        }
    }
"#
        ),
        "b8_field_read_entry_projected",
    );
}

// B6: scalar field ops, projected call, NO cycle (DAG control).
#[test]
fn b6_scalar_ops_dag_projected_call() {
    assert_has_plan(
        &format!(
            "{CONSOLE}{}",
            r#"
    data Main { console: Service<Console>; k: u32 in Wrapping; }
    machine Main::main(&mut self) reaches Console {
        self.k = 0;
        transition { _ -> step() }
        state step(&mut self) {
            self.k = self.k + 1;
            transition { _ -> finish() }
        }
        state finish(&mut self) {
            self.console.exit_process(0);
        }
    }
"#
        ),
        "b6_scalar_ops_dag_projected_call",
    );
}
