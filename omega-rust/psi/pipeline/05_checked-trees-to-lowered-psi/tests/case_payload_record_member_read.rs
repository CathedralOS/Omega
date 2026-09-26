//! A case arm reads a bound record payload's member field: `Shape::Boxed
//! { inner } -> emit(inner.x)`. The nested read
//! `[Case(Boxed), Field(inner), Field(x)]` is planned inside the selecting
//! edge's staged block as a `StructuralCaseLeafCopy` of the record member
//! followed by an `IntegerStructuralField` read on the completed copy —
//! where a plain payload destructure binds scalar members as block
//! parameters, a structural member must first be copied out of the sum
//! before its leaf field can be observed.

use lowered_psi_to_terminal_psi::terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

const SOURCE: &str = r#"
    boundary trait Sink { machine record(value: i32); }
    data Inner { x: i32; }
    data Shape {
        case Circle(r: i32);
        case Boxed(inner: Inner);
    }
    machine apply(s: Shape) reaches Sink {
        transition s {
            Shape::Circle { r } -> emit(r)
            Shape::Boxed { inner } -> emit(inner.x)
        }
        state emit(v: i32) { Sink::record(v); }
    }
"#;

fn produce(
    checked: &typed_trees_to_checked_trees::checked_trees::CheckedTrees,
) -> terminal_psi::TerminalModule {
    let artifact =
        lowered_psi_to_terminal_psi::terminal_production::TerminalProductionRequest::new(
            checked,
            TerminalMachineSelection::Name("apply"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("record-payload member read lowers")
        .into_artifact();
    terminal_codec::decode_module(artifact.semantic_bytes()).unwrap()
}

#[test]
fn a_record_payload_member_field_reads_through_a_staged_leaf_copy() {
    let checked = crate::front_end::checked_program(SOURCE);
    let module = produce(&checked);
    let operations: Vec<&terminal_psi::OperationKind> = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .map(|operation| &operation.kind)
        .collect();
    // The member copy names the payload through the case-qualified path; the
    // leaf read observes the completed copy with no further path.
    let copies: Vec<&semantic_vocabulary::CanonicalStructuralPathSegment> = operations
        .iter()
        .filter_map(|kind| match kind {
            terminal_psi::OperationKind::StructuralCaseLeafCopy { path, .. } => {
                path.iter().find(|segment| {
                    matches!(
                        segment,
                        semantic_vocabulary::CanonicalStructuralPathSegment::Case(_)
                    )
                })
            }
            _ => None,
        })
        .collect();
    assert_eq!(copies.len(), 1, "one case-qualified member copy staged");
    assert!(
        operations.iter().any(|kind| matches!(
            kind,
            terminal_psi::OperationKind::IntegerStructuralField { path, .. }
                if path.is_empty()
        )),
        "the leaf field reads off the completed copy"
    );
}

#[test]
fn verifier_rejects_a_member_copy_taken_under_another_case() {
    // The copy's case knowledge is the verifier's own tag dispatch: naming
    // the `Boxed` member on the `Circle` successor's staged block must not
    // verify — dominance over the staged block is what licenses the copy.
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        checked_trees_to_lowered_psi::TerminalMachineSelection::Name("apply"),
    )
    .expect("record-payload read lowers");
    let verify = |module: &terminal_psi::TerminalModule| {
        terminal_verifier::verify_module(
            module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .map(|_| ())
    };
    verify(&lowered.semantic_module).expect("the exact staged copy verifies");
    // A case the dispatch selects that differs from the copy's: the staged
    // block naming that case cannot observe the `Boxed` member.
    let other = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .find_map(|block| match &block.terminator {
            terminal_psi::Terminator::StructuralCase { cases, .. } => cases.first(),
            _ => None,
        })
        .map(|case| case.case)
        .expect("the sum dispatch names a case");
    let mut changed = lowered.semantic_module.clone();
    let path = changed
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            terminal_psi::OperationKind::StructuralCaseLeafCopy { path, .. } => Some(path),
            _ => None,
        })
        .expect("the staged member copy");
    let segment = path
        .iter_mut()
        .find(|segment| {
            matches!(
                segment,
                semantic_vocabulary::CanonicalStructuralPathSegment::Case(_)
            )
        })
        .expect("the copy names a case");
    let semantic_vocabulary::CanonicalStructuralPathSegment::Case(copy_case) = *segment else {
        unreachable!("the segment is a case");
    };
    assert_ne!(
        copy_case, other,
        "the dispatch's first case differs from the copy's"
    );
    *segment = semantic_vocabulary::CanonicalStructuralPathSegment::Case(other);
    assert!(
        verify(&changed).is_err(),
        "a member copy named under another case has no case knowledge"
    );
}
