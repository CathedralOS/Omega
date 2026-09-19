use super::associate;
use assembled_syntax_to_checked_compilation::{CheckedCompileRequest, compile_to_checked};

/// The selected built-in integer comparison lane: the crash_routes fixture's
/// `left == right` use selects the `== Comparison::equal` boundary operator
/// through a compiler-intrinsic provider and retains the exact occurrence.
#[test]
fn integer_comparison_association_rejoins_the_exact_selected_use() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass/operators/crash_routes/main.omg");
    let checked = compile_to_checked(CheckedCompileRequest::new(&root, Some("linux_x86_64")))
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "may_crash")
        .expect("selected integer comparison reaches Terminal");
    let occurrences = &lowered.selected_integer_comparison_occurrences;
    assert_eq!(occurrences.len(), 1);
    let occurrence = occurrences[0];
    assert_eq!(
        occurrence.comparison,
        lowered_psi::LoweredSelectedIntegerComparisonOperation::Equal
    );
    assert_eq!(
        occurrence.operand_order,
        lowered_psi::LoweredSelectedIntegerComparisonOperandOrder::Authored
    );
    assert!(!occurrence.negated);

    let proposals = associate(
        &checked,
        &lowered.semantic_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        occurrences,
    )
    .expect("exact integer comparison association");
    let [proposal] = proposals.as_slice() else {
        panic!("one proposal");
    };
    assert_eq!(
        proposal.authored_operation(),
        Some(effects::CompilerPrimitiveIntegerComparisonOperation::Equal)
    );
    assert_eq!(
        proposal.execution_identity(),
        Some(
            effects::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison {
                operation: effects::CompilerPrimitiveIntegerComparisonOperation::Equal,
                integer_type: effects::CompilerNumericType::I32,
            }
        )
    );
}

/// Drifted custody must fail closed: a substituted occurrence, a wrong
/// emission triple, or a roster that no longer covers every emitted integer
/// comparison is not consumable.
#[test]
fn integer_comparison_association_rejects_incomplete_or_substituted_occurrences() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass/operators/crash_routes/main.omg");
    let checked = compile_to_checked(CheckedCompileRequest::new(&root, Some("linux_x86_64")))
        .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "may_crash")
        .expect("selected integer comparison reaches Terminal");
    let occurrences = &lowered.selected_integer_comparison_occurrences;
    assert_eq!(occurrences.len(), 1);
    let check = |rows: &[lowered_psi::LoweredSelectedIntegerComparisonOccurrence]| {
        associate(
            &checked,
            &lowered.semantic_module,
            checked.selected_provider_plans(),
            checked.selected_provider_provenance(),
            rows,
        )
    };
    assert!(check(&[]).is_err(), "missing occurrence");
    let mut changed = occurrences.clone();
    changed[0].provider_plan_commitment = Default::default();
    assert!(check(&changed).is_err(), "missing exact provider");
    let mut changed = occurrences.clone();
    changed[0].provider_plan_report_fingerprint ^= 1;
    assert!(check(&changed).is_err(), "wrong provider");
    let mut changed = occurrences.clone();
    changed[0].comparison = lowered_psi::LoweredSelectedIntegerComparisonOperation::LessThan;
    assert!(check(&changed).is_err(), "wrong emitted operation");
    let mut changed = occurrences.clone();
    changed[0].operand_order = lowered_psi::LoweredSelectedIntegerComparisonOperandOrder::Swapped;
    assert!(check(&changed).is_err(), "swapped operand order");
    let mut changed = occurrences.clone();
    changed[0].negated = true;
    assert!(check(&changed).is_err(), "wrong negation");
    let mut changed = occurrences.clone();
    changed[0].integer_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64)
            .expect("i64 is a valid fixed integer type");
    assert!(check(&changed).is_err(), "wrong integer type");
    let mut changed = occurrences.clone();
    changed[0].operator_use = Default::default();
    assert!(check(&changed).is_err(), "missing selected use");

    let mut changed_module = lowered.semantic_module.clone();
    let operation = changed_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == occurrences[0].terminal_operation)
        .expect("comparison operation");
    let (left, right) = match operation.kind {
        terminal_psi::OperationKind::IntegerEqual { left, right } => (left, right),
        _ => panic!("comparison operation changed kind"),
    };
    operation.kind = terminal_psi::OperationKind::IntegerLessThan { left, right };
    assert!(
        associate(
            &checked,
            &changed_module,
            checked.selected_provider_plans(),
            checked.selected_provider_provenance(),
            occurrences
        )
        .is_err(),
        "Terminal operation drift"
    );
}
