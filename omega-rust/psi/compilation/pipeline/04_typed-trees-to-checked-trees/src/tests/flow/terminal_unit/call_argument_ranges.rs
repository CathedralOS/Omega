//! Exclusive ranges passed as call arguments: a range over fixed-array field
//! storage keeps the collection projection its lowering views whole, a literal
//! byte range uses the same descriptor operation, a qualified byte view
//! narrows into an unqualified formal, and a range into a domain-qualified
//! formal names its Terminal boundary.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedConditionalReturnArm, CheckedStorageRoot,
    CheckedStructuralAccess, CheckedUnitEffectOperationPlan, CheckedUnitPlanOmissionStage,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment,
};

/// The structural arguments of the call a guarded arm of `machine`'s entry
/// state completes with.
fn arm_call_arguments(
    checked: &checked_trees::CheckedTrees,
    machine: &str,
) -> Vec<CheckedUnitStructuralArgumentPlan> {
    let symbol = machine_named(checked, machine);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(symbol)
        .unwrap_or_else(|| {
            panic!(
                "{machine} plans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(symbol)
            )
        });
    let CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
        return_arm: CheckedConditionalReturnArm::Call(operation),
        ..
    } = &plan.states[0].terminator
    else {
        panic!("a call arm beside a jump: {:?}", plan.states[0].terminator);
    };
    operation
        .call()
        .expect("an ordinary call")
        .structural_arguments
        .clone()
}

const FIELD_RANGES: &str = r#"
    data Root { items: [i32; 5]; seen: u64; }
    machine Root::consume(&mut self, sub: &[i32]) { self.seen = sub.len; }
    machine Root::view(&mut self, lo: u64, hi: u64) {
        transition lo <= hi && hi <= self.items.len {
            true -> self.consume(self.items[lo..hi])
            false -> rejected()
        }
        state rejected(&mut self) { self.seen = 0; }
    }
    machine Root::measure(&mut self, lo: u64, hi: u64)
    requires lo <= hi && hi <= 5;
    {
        self.consume(self.items[lo..hi]);
    }
"#;

#[test]
fn local_array_ranges_use_declared_storage_with_or_without_initializers() {
    for element in ["u8", "i32"] {
        for initializer in ["", " = [1, 2]"] {
            let checked = checked(&format!(
                "machine consume(values: &[{element}]) {{ let count: u64 = values.len; }}
                 machine run() {{
                     let values: [{element}; 2]{initializer};
                     consume(&values[..]);
                 }}"
            ));
            let symbol = machine_named(&checked, "run");
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(symbol)
                .unwrap_or_else(|| {
                    panic!(
                        "{element} local {initializer}: {:?}",
                        checked
                            .facts
                            .flow
                            .terminal_unit_effects
                            .omission_for_machine(symbol)
                    )
                });
            let argument = plan
                .operations
                .iter()
                .find_map(CheckedUnitEffectOperationPlan::call)
                .and_then(|call| call.structural_arguments.first())
                .expect("call receives its range descriptor");
            assert!(matches!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root: CheckedStorageRoot::OwnedLocal { .. },
                    start: None,
                    end: None,
                    ..
                } | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    root: CheckedStorageRoot::OwnedLocal { .. },
                    start: None,
                    end: None,
                    ..
                }
            ));
            assert!(argument.path.is_empty());
            assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
        }
    }
}

fn field_range(argument: &CheckedUnitStructuralArgumentPlan) -> bool {
    matches!(
        argument.source,
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root: CheckedStorageRoot::Parameter { index: 0 },
            start: Some(_),
            end: Some(_),
            ..
        }
    ) && matches!(
        argument.path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field(_)]
    ) && argument.access == CheckedStructuralAccess::SharedBorrow
}

/// A guarded terminal call passes a runtime-bounded range of its receiver's
/// array: the argument is an element range rooted at `self` whose path is
/// the field projection to the array, with both retained endpoints.
#[test]
fn guarded_tail_call_passes_a_field_range_of_its_receiver() {
    let checked = checked(FIELD_RANGES);
    let arguments = arm_call_arguments(&checked, "Root::view");
    assert!(
        arguments.iter().any(field_range),
        "the arm call passes the field range: {arguments:#?}"
    );
}

/// A statement call takes the same field range: the call-argument site
/// admits it without a ledger loan, since the view dies with its call.
#[test]
fn statement_call_passes_a_field_range() {
    let checked = checked(FIELD_RANGES);
    let symbol = machine_named(&checked, "Root::measure");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .unwrap_or_else(|| {
            panic!(
                "measure plans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(symbol)
            )
        });
    let call = plan
        .operations
        .iter()
        .find_map(CheckedUnitEffectOperationPlan::call)
        .expect("the consuming call plans");
    assert!(
        call.structural_arguments.iter().any(field_range),
        "the call passes the field range: {:#?}",
        call.structural_arguments
    );
    assert!(
        call.claim_transfers().is_empty(),
        "a shared view moves no claim"
    );
}

/// Constant endpoints do not create a second range operation or let missing
/// source evidence fall back to a fixed byte-window loan.
#[test]
fn literal_byte_range_requires_the_shared_descriptor_evidence() {
    let checked = checked(
        r#"
        data Root { buffer: [u8; 8]; seen: u64; }
        machine Root::read(&mut self, bytes: &[u8]) { self.seen = bytes.len; }
        machine Root::send(&mut self) { self.read(self.buffer[1..3]); }
    "#,
    );
    let symbol = machine_named(&checked, "Root::send");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(symbol)
        .unwrap_or_else(|| {
            panic!(
                "send plans: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(symbol)
            )
        });
    let call = plan
        .operations
        .iter()
        .find_map(CheckedUnitEffectOperationPlan::call)
        .expect("the reading call plans");
    let argument = call
        .structural_arguments
        .iter()
        .find(|argument| {
            matches!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            )
        })
        .expect("constant bounds use the shared range descriptor");
    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        expression,
        start: Some(_),
        end: Some(_),
        ..
    } = argument.source
    else {
        panic!("the range retains both authored endpoint facts");
    };
    assert!(matches!(
        argument.path.as_slice(),
        [CheckedUnitStructuralPathSegment::Field(_)]
    ));
    assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
    assert!(call.claim_transfers().is_empty());

    let retains_plan = |facts: &checked_trees::CheckFacts| {
        crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns,
            },
        )
        .for_machine(symbol)
        .is_some()
    };
    for role in [
        checked_trees::CheckedScalarExpressionRole::SubsliceStart {
            site: checked_trees::CheckedSubsliceSite::CallArgument {
                call_ordinal: call.coordinate.call_ordinal,
                argument_ordinal: 0,
            },
        },
        checked_trees::CheckedScalarExpressionRole::SubsliceEnd {
            site: checked_trees::CheckedSubsliceSite::CallArgument {
                call_ordinal: call.coordinate.call_ordinal,
                argument_ordinal: 0,
            },
        },
    ] {
        let mut changed = checked.facts.clone();
        let bindings = &mut changed.values.scalar_expressions.source_bindings;
        let handle = bindings
            .iter()
            .find(|(_, binding)| binding.state == plan.state && binding.role == role)
            .expect("the range endpoint is source-bound")
            .0;
        bindings.get_mut(handle).expression = Default::default();
        assert!(
            !retains_plan(&changed),
            "missing {role:?} cannot become a fixed byte loan"
        );
    }
    let mut changed = checked.facts.clone();
    let mut uses = arena::Arena::new();
    uses.append(checked_trees::CheckedOperatorUseFact {
        expression,
        spelling: language_core::OperatorSpelling::Range,
        status: checked_trees::CheckedOperatorResolutionStatus::Resolved,
        selected_operator_symbol: symbol,
        ..Default::default()
    });
    changed.operators = checked_trees::CheckedOperatorFacts::with_roots(
        uses,
        arena::Arena::new(),
        arena::Arena::new(),
    );
    assert!(
        !retains_plan(&changed),
        "a selected operator cannot become a builtin byte loan"
    );
}

const QUALIFIED: &str = r#"
    domain [u8]::Path
    requires
        no_nul(self);
    data Root { k: u64; stored: i32; }
    machine Root::plain(&mut self, prefix: &[u8]) { self.stored = 70; }
    machine Root::qualified(&mut self, prefix: &[u8] in Path) { self.stored = 70; }
    machine Root::forget(&mut self, path: &[u8] in Path) {
        transition self.k < path.len {
            true -> self.plain(path[0..self.k])
            false -> rejected()
        }
        state rejected(&mut self) { self.stored = 0; }
    }
    machine Root::keep(&mut self, path: &[u8] in Path) {
        transition self.k < path.len {
            true -> self.qualified(path[0..self.k])
            false -> rejected()
        }
        state rejected(&mut self) { self.stored = 0; }
    }
"#;

/// A domain-qualified byte view parameter narrows like any other view into
/// an unqualified formal: its range is the plain subslice of that parameter.
#[test]
fn qualified_view_narrows_into_an_unqualified_formal() {
    let checked = checked(QUALIFIED);
    let arguments = arm_call_arguments(&checked, "Root::forget");
    assert!(
        arguments.iter().any(|argument| matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root: CheckedStorageRoot::Parameter { index: 1 },
                end: Some(_),
                ..
            }
        ) && argument.path.is_empty()),
        "the arm call narrows the qualified parameter: {arguments:#?}"
    );
}

/// A range into a domain-qualified formal has no Terminal lowering: the
/// subslice result carries no qualifications. The plan names that boundary.
#[test]
fn range_into_a_qualified_formal_names_its_boundary() {
    let checked = checked(QUALIFIED);
    let symbol = machine_named(&checked, "Root::keep");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(symbol)
            .is_none(),
        "no plan admits the qualified range"
    );
    let omission = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(symbol)
        .expect("the omission is recorded");
    assert!(
        matches!(
            omission.stage,
            CheckedUnitPlanOmissionStage::LocalConstruction {
                phase: "call operation: structural arguments: range into a qualified view formal",
                ..
            }
        ),
        "the omission names the qualified formal: {omission:?}"
    );
}

/// A range whose bound no fact proves is rejected by checking, before any
/// argument plan exists.
#[test]
fn unproved_field_range_bound_rejects() {
    let diagnostics = crate::tests::front_end::checked_program_result(
        r#"
        boundary trait PortIo {}
        data Root { items: [i32; 5]; seen: u64; }
        machine Root::consume(&mut self, sub: &[i32]) { self.seen = sub.len; }
        machine Root::view(&mut self, lo: u64, hi: u64) {
            transition lo <= hi {
                true -> self.consume(self.items[lo..hi])
                false -> rejected()
            }
            state rejected(&mut self) { self.seen = 0; }
        }
    "#,
    )
    .expect_err("an unproved range end rejects");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .starts_with("cannot prove subslice range")),
        "the unproved bound is reported: {diagnostics:?}"
    );
}

/// `TARGET` names the formal the range supplies.
const TEXT: &str = r#"
    domain [u8]::Text
    requires
        valid_utf8(self);
    data Root { k: u64; stored: i32; }
    machine Root::plain(&mut self, prefix: &[u8]) { self.stored = 70; }
    machine Root::qualified(&mut self, prefix: &[u8] in Text) { self.stored = 70; }
    machine Root::forward(&mut self, text: &[u8] in Text) {
        transition self.k < text.len {
            true -> self.TARGET(text[0..self.k])
            false -> rejected()
        }
        state rejected(&mut self) { self.stored = 0; }
    }
"#;

/// `valid_utf8` is not subslice-preserving: a range can cut a multi-byte
/// scalar. A `Text` view therefore narrows into a plain formal, which forgets
/// the domain, but its range cannot satisfy a `Text` formal: checking rejects
/// the requirement before any argument plan exists.
#[test]
fn non_preserving_domain_narrows_only_into_a_plain_formal() {
    let checked = checked(&TEXT.replace("TARGET", "plain"));
    let arguments = arm_call_arguments(&checked, "Root::forward");
    assert!(
        arguments.iter().any(|argument| matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                root: CheckedStorageRoot::Parameter { index: 1 },
                ..
            }
        )),
        "the plain formal receives the range: {arguments:#?}"
    );
    let diagnostics = crate::tests::front_end::checked_program_result(&format!(
        "boundary trait PortIo {{}}\n{}",
        TEXT.replace("TARGET", "qualified")
    ))
    .expect_err("a cut UTF-8 view is not proved `Text`");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "the domain requirement is reported: {diagnostics:?}"
    );
}
