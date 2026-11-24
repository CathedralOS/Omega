//! Source validity is decided before the rewrite. The artifact binds the
//! selected pass and the evidence to the actual rechecked program.
use ::optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::OperationKind;

fn publish(
    source: &str,
    selections: PsiOptimizationSelections,
) -> Result<crate::checking::RoutePublication, Vec<diagnostics::Diagnostic>> {
    let typed = super::boundary_calls::typed(source);
    let entry = typed.machines().at(0).symbol;
    let prepared = crate::prepare_fixture(typed);
    let input = crate::lowering::lower_for_publication(
        prepared,
        crate::lowering::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            calling: None,
            providers: None,
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Portable,
    )?;
    let (judgment, _) = crate::checking::judge_publication(input, selections)?;
    let (_, program, _) = judgment.into_source_and_program();
    Ok(program.publish().unwrap())
}

#[test]
fn selected_constants_bind_the_rechecked_code_and_its_own_seals() {
    let source = "machine entry() -> bool {
        let known: bool = false;
        !known
    }";
    let selection =
        PsiOptimizationSelections::new([PsiOptimization::SparseConditionalConstantPropagation])
            .unwrap();
    let original = publish(source, Default::default()).unwrap();
    let selected = publish(source, selection.clone()).unwrap();
    let negations = |artifact: &terminal_codec::CanonicalTerminalArtifact| {
        artifact
            .view()
            .module()
            .machines
            .iter()
            .flat_map(|machine| machine.operations())
            .filter(|operation| matches!(operation.kind, OperationKind::BooleanNot { .. }))
            .count()
    };
    assert_eq!(negations(&original.artifact), 1);
    assert_eq!(negations(&selected.artifact), 0);
    let record = selected.artifact.optimization();
    assert_eq!(record.selections(), &selection);
    assert_eq!(record.input_semantic(), original.artifact.view().identity());
    assert_eq!(
        record.output_semantic(),
        selected.artifact.view().identity()
    );
    assert_ne!(record.input_semantic(), record.output_semantic());
    assert_eq!(selected.seals.module, record.output_semantic());
}

#[test]
fn selection_cannot_hide_invalid_source_or_claim_an_unimplemented_pass() {
    let unsupported =
        PsiOptimizationSelections::new([PsiOptimization::StateSpecialization]).unwrap();
    let refusal = |source, selections| {
        publish(source, selections)
            .err()
            .unwrap()
            .into_iter()
            .map(|row| row.message)
            .collect::<Vec<_>>()
    };
    let invalid = "machine entry(value: u64) -> u64 { value + 1u64 }";
    let original = refusal(invalid, Default::default());
    assert_eq!(refusal(invalid, unsupported.clone()), original);
    assert_eq!(
        refusal("machine entry() {}", unsupported),
        [
            "not supported yet: source Psi optimization `StateSpecialization` has no implemented pass"
        ],
    );
}

#[test]
fn boolean_operands_cannot_reach_integer_ordering_in_the_optimizer() {
    let typed = super::boundary_calls::typed(
        "machine entry(left: bool, right: bool) -> bool { left == right }",
    );
    let entry = typed.machines().at(0).symbol;
    let mut lowered = crate::lowering::elaborate_for(
        &typed,
        crate::lowering::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            calling: None,
            providers: None,
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Portable,
    )
    .unwrap();
    let operation = lowered
        .module
        .machines
        .iter_mut()
        .flat_map(|m| &mut m.blocks)
        .flat_map(|b| &mut b.operations)
        .find(|op| matches!(op.kind, OperationKind::BooleanEqual { .. }))
        .unwrap();
    let OperationKind::BooleanEqual { left, right } = operation.kind else {
        unreachable!()
    };
    operation.kind = OperationKind::IntegerLessThan { left, right };
    let super::RouteVerdict::FallBack(refusal) = super::judge(Ok(lowered)).unwrap() else {
        panic!("the original judgment must refuse integer ordering on Boolean operands");
    };
    assert!(
        refusal
            .detail
            .contains("OperandShapeMismatch(IntegerLessThan)"),
        "{refusal:?}"
    );
}
