//! Where a call-containing scalar requirement stops on the way to Terminal.
//!
//! OPERATOR-MACHINE-SUPPLY asks for this refusal to be reproduced before its
//! repair is assigned. Checked interpretation already executes
//! `restricted(saved, value)` under `requires observe(left) == observe(right)`
//! -- `compiler/tests/contract_application_terms.rs::runtime_body_calls_execute_with_checked_premises`
//! runs it to its documented exits -- and this is the next step failing.
//!
//! These pin a BOUNDARY, not desired behavior. `covered_requires` keeps an
//! explicit `None` requires row for a clause it cannot express as a closed
//! scalar contract, and `graph_preparation` refuses on it. When the premise
//! route reaches Terminal these tests are expected to fail and be replaced by
//! the agreement the row asks for: canonical reload, independent verification,
//! interpretation and native execution.

use checked_trees_to_lowered_psi::{LoweringError, TerminalMachineSelection, lower_machine};

const DECLARATIONS: &str = r#"
    pub machine observe(value: bool) -> bool
    terminates;
    { transition { _ -> (value == true) } }
    pub machine restricted(left: bool, right: bool) -> bool
    requires observe(left) == observe(right);
    terminates;
    { left }
"#;

const CALLER: &str =
    "machine caller(value: bool) -> bool { let saved: bool = value; restricted(saved, value) }";

fn lowering(machine: &str) -> Result<(), LoweringError> {
    let checked = crate::front_end::checked_program(&format!("{DECLARATIONS}\n{CALLER}\n"));
    lower_machine(&checked, TerminalMachineSelection::Name(machine)).map(|_| ())
}

#[test]
fn a_call_containing_requires_stops_at_the_scalar_contract() {
    for machine in ["restricted", "caller"] {
        assert!(
            matches!(
                lowering(machine),
                Err(LoweringError::Unsupported(
                    "scalar contract contains an unsupported clause"
                ))
            ),
            "{machine} should still stop at the scalar contract, got {:?}",
            lowering(machine)
        );
    }
}

#[test]
fn the_same_program_lowers_the_machine_without_a_call_premise() {
    assert!(
        lowering("observe").is_ok(),
        "the refusal is the call-containing clause, not this program"
    );
}
