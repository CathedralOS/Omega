//! Declared borrowed receivers retained by scalar callees: rejoined at
//! statement calls, shared across tail-fused bodies and lent by direct
//! result calls.

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

const FUSED_SOURCE: &str = r#"
    data Main { bias: u64; }
    machine Main::ping(&mut self, n: u64)
    terminates by n;
    -> u64
    {
        transition n == 0 {
            true -> (self.bias)
            _ -> self.pong(n - 1)
        }
    }
    machine Main::pong(&mut self, n: u64)
    terminates by n;
    -> u64
    {
        transition n == 0 {
            true -> (0)
            _ -> self.ping(n - 1)
        }
    }
"#;

/// Tail-fused bodies share the entry's one machine-scope receiver. A member
/// whose retained roster differs cannot borrow that declaration.
#[test]
fn tail_fused_bodies_share_one_retained_receiver() {
    let checked = crate::front_end::checked_program(FUSED_SOURCE);
    let lowered = lower(&checked, "Main::ping").expect("fused receiver cycle lowers");
    verify(&lowered);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("fused entry machine");
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("the fused graph declares one receiver");
    };
    assert!(receiver.is_self);
    assert!(
        entry
            .blocks
            .iter()
            .all(|block| block.structural_parameters.is_empty()),
        "no edge carries a lane for the shared receiver"
    );

    let ping = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::ping")
        .unwrap()
        .symbol;
    let mut changed = checked.clone();
    let graph = changed
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter_mut()
        .find(|graph| graph.machine == ping)
        .expect("fused scalar graph");
    let [_, member] = graph.states.as_mut_slice() else {
        panic!("two fused members");
    };
    member.structural_parameters[0].access = CheckedStructuralAccess::SharedBorrow;
    assert!(
        lower(&changed, "Main::ping").is_err(),
        "a member with another receiver roster must not share the entry's"
    );
}

const DIRECT_SOURCE: &str = r#"
    data Check {}
    data Main { check: Check; seen: u64; }
    machine Check::flag(&mut self, ok: bool, code: u64) -> u64 {
        transition ok { true -> 0 false -> code }
    }
    machine Main::enter(&mut self) {
        self.seen = 3;
        let first: u64 = self.check.flag(self.seen == 3, 1);
        let second: u64 = self.check.flag(self.seen == 4, 2);
        self.seen = second;
    }
"#;

/// A scalar local bound directly to a receiver method is the caller's
/// ordinary result operation: each call lends its receiver in one operation
/// of the caller's block instead of staging every operand in its own block.
#[test]
fn direct_receiver_initializers_stay_single_call_operations() {
    let checked = crate::front_end::checked_program(DIRECT_SOURCE);
    let plan = super::unit_plan(&checked, "Main::enter");
    let calls = plan
        .operations
        .iter()
        .filter(|operation| {
            matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments, ..
            } if structural_arguments.len() == 1
                && structural_arguments[0].access == CheckedStructuralAccess::MutableBorrow)
        })
        .count();
    assert_eq!(
        calls, 2,
        "both initializers are receiver-lending result calls"
    );
    assert!(
        !plan.operations.iter().any(|operation| matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                value: checked_trees::CheckedCallScalarArgument::Computation(_),
                ..
            }
        )),
        "no initializer is re-rooted as a whole-call computation"
    );
    let lowered = lower(&checked, "Main::enter").expect("direct receiver initializers lower");
    verify(&lowered);
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("caller entry");
    let [block] = entry.blocks.as_slice() else {
        panic!("receiver calls do not split the caller's straight-line body");
    };
    assert_eq!(
        block
            .operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::CallStructuralScalar { .. }
            ))
            .count(),
        2
    );
}
