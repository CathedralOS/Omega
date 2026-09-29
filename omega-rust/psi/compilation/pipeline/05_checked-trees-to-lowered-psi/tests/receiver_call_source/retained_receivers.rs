//! Declared borrowed receivers retained by scalar callees and rejoined at
//! statement calls.

use super::{CheckedStructuralAccess, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::{LoweringError, TerminalMachineSelection};

fn lower(
    checked: &checked_trees::CheckedTrees,
    name: &str,
) -> Result<lowered_psi::LoweredPsi, LoweringError> {
    checked_trees_to_lowered_psi::lower_machine(checked, TerminalMachineSelection::Name(name))
}

fn verify(lowered: &lowered_psi::LoweredPsi) {
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("retained receiver program independently verifies");
}

const STATEMENT_SOURCE: &str = r#"
    data Tally { calls: u64; }
    data Main { tally: Tally; spare: Tally; }
    machine Tally::bump(&mut self, amount: u64) -> bool {
        self.calls = amount;
        transition { _ -> true }
    }
    machine Main::roll(&mut self) -> bool {
        self.tally.calls = 1;
        transition { _ -> true }
    }
    machine Main::enter(&mut self) {
        _ = RECEIVER;
    }
"#;

/// A discarded statement call keeps no receiver expression. Its retained
/// receiver operand must still name the place the statement spells: the
/// caller's whole `self`, or the field path beneath it.
#[test]
fn statement_call_receiver_rejoins_its_authored_place() {
    for (call, path) in [
        ("self.roll()", Vec::new()),
        ("self.tally.bump(3)", vec!["tally".to_owned()]),
    ] {
        let source = STATEMENT_SOURCE.replace("RECEIVER", call);
        let checked = crate::front_end::checked_program(&source);
        verify(
            &lower(&checked, "Main::enter")
                .unwrap_or_else(|error| panic!("{call}: statement receiver lowers: {error:?}")),
        );
        let plan = super::unit_plan(&checked, "Main::enter");
        let (index, receiver) = plan
            .operations
            .iter()
            .enumerate()
            .find_map(|(index, operation)| match operation {
                CheckedUnitEffectOperationPlan::ScalarCall {
                    structural_arguments,
                    ..
                } => Some((index, structural_arguments.first()?.clone())),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{call}: the discard keeps its scalar call"));
        assert_eq!(receiver.access, CheckedStructuralAccess::MutableBorrow);
        assert_eq!(
            receiver.path,
            path.into_iter()
                .map(checked_trees::CheckedUnitStructuralPathSegment::Field)
                .collect::<Vec<_>>(),
            "{call}"
        );
        let machine = plan.machine;
        for mutation in ["path", "access"] {
            let mut changed = checked.clone();
            let plan = changed
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == machine)
                .unwrap();
            let CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            } = &mut plan.operations[index]
            else {
                unreachable!()
            };
            let argument = &mut structural_arguments[0];
            match mutation {
                "path" => {
                    argument.path = vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
                        "spare".to_owned(),
                    )]
                }
                _ => argument.access = CheckedStructuralAccess::SharedBorrow,
            }
            assert!(
                lower(&changed, "Main::enter").is_err(),
                "{call}: {mutation} substitution must not publish"
            );
        }
    }
}
