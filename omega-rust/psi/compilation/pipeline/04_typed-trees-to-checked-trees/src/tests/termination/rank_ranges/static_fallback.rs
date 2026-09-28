//! Static membership and relational descent remain separate obligations.

use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const DESCENDING: &str =
    "machine walk(n: u32 [0..=10]) terminates by n -> Nat::Descending in 0..=10; -> u32";
fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {diagnostics:#?}\n{source}"));
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("complete checking: {diagnostics:#?}\n{source}"));
}

#[test]
fn static_membership_can_consume_a_relational_two_step_decrease() {
    prove(&format!(
        "{DESCENDING} {{ transition n >= 2 {{ true -> walk(n - 2) false -> n }} }}"
    ));
}
