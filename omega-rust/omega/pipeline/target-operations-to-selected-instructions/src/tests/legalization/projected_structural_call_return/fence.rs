use super::*;
use crate::tests::fixtures::structural_call::structural_call_fixture;
use target_operations::TargetOperation;

#[test]
fn unrelated_target_parameter_roster_is_not_silently_ignored() {
    let (source, mut target, unit) = structural_call_fixture();
    let TargetOperation::UnitBody(body) = &mut target.functions[0].operation else {
        unreachable!()
    };
    body.parameters[0].projected_qualifications = vec![terminal_psi::StructuralPathQualification {
        path: vec![terminal_psi::StructuralPathSegment::Field("base".into())],
        domain: semantic_vocabulary::StructuralDomainId::new(1).unwrap(),
    }];
    assert!(legalize_target_operations(&target, &source, &unit).is_err());
}

#[test]
fn selection_requires_the_exact_projected_closure() {
    let (source, target, unit) = projected_fixture(NativeTarget::windows_x64());
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
    let (physical, catalog, constraints) =
        crate::tests::fixtures::microsoft_environment::microsoft_selection_environment();
    let selected = crate::select_instructions(&legalized, &constraints, &physical, &catalog)
        .expect("exact projected closure has bounded selection authority");
    assert_eq!(selected.plan().projected_structural_call_returns.len(), 1);
}
