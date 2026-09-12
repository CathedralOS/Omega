use super::*;
use crate::CheckedCompileRequest;
use semantic_vocabulary::IeeeFloatComparisonOperation;

#[test]
fn comparison_association_rejects_incomplete_or_substituted_occurrences() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass/expressions/match_float_patterns/main.omg");
    let checked =
        crate::compile_to_checked(CheckedCompileRequest::new(&root, Some("linux_x86_64")))
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "choose")
        .expect("selected Match comparison reaches Terminal");
    let occurrences = &lowered.selected_ieee_float_comparison_occurrences;
    assert_eq!(occurrences.len(), 2);
    let check = |rows: &[lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence]| {
        associate(
            &checked,
            &lowered.semantic_module,
            checked.selected_provider_plans(),
            checked.selected_provider_provenance(),
            rows,
        )
    };
    let proposals = check(occurrences).expect("exact comparison associations");
    assert_eq!(proposals.len(), 2);
    assert!(check(&occurrences[..1]).is_err(), "missing occurrence");
    let mut changed = occurrences.clone();
    changed[1] = changed[0];
    assert!(check(&changed).is_err(), "duplicate occurrence");
    let mut changed = occurrences.clone();
    changed[0].provider_plan_commitment = Default::default();
    assert!(check(&changed).is_err(), "missing exact provider");
    let mut changed = occurrences.clone();
    changed[0].provider_plan_report_fingerprint ^= 1;
    assert!(check(&changed).is_err(), "wrong provider");
    let mut changed = occurrences.clone();
    changed[0].comparison = IeeeFloatComparisonOperation::NotEqual;
    assert!(check(&changed).is_err(), "wrong operation");
    let mut changed = occurrences.clone();
    changed[0].format = semantic_vocabulary::IeeeFloatFormat::Binary64;
    assert!(check(&changed).is_err(), "wrong format");
    let mut changed = occurrences.clone();
    changed[0].application_site = changed[1].application_site;
    assert!(
        check(&changed).is_err(),
        "another authored arm cannot donate custody"
    );
    let mut changed = occurrences.clone();
    changed[0].operator_use = Default::default();
    assert!(check(&changed).is_err(), "missing selected use");

    let mut changed_module = lowered.semantic_module.clone();
    let comparison = changed_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.id == occurrences[0].terminal_operation)
        .expect("comparison operation");
    let terminal_psi::OperationKind::IeeeFloatCompare { comparison, .. } = &mut comparison.kind
    else {
        panic!("comparison operation changed kind");
    };
    *comparison = IeeeFloatComparisonOperation::Greater;
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

    let mut changed_proposals = proposals.clone();
    changed_proposals[0].provider_plan_index = usize::MAX;
    assert!(
        compilation_report::TerminalIeeeFloatComparisonOccurrenceProposal::validate_roster(
            &lowered.semantic_module,
            checked.selected_provider_plans().plans(),
            &changed_proposals,
        )
        .is_err(),
        "retained proposal cannot name another provider slot"
    );
}
