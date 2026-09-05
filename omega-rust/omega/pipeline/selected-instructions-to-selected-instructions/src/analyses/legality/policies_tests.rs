use super::*;

#[test]
fn active_resident_two_view_policy_reports_the_exact_missing_required_view() {
    let mut model = isa_x86_64::x86_64_physical_register_model();
    model.views.retain(|view| view.name != "rcx");

    assert_eq!(
            active_resident_immediate_u64_multi_use_rematerialization_v1_views(
                target::Architecture::X86_64,
                &model,
            ),
            Err(
                OptimizedAllocationLegalityCustodyError::MissingRequiredActiveResidentRematerializationView(
                    "rcx",
                ),
            )
        );
}
