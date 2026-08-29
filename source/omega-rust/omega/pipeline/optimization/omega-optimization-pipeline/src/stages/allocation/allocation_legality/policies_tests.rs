use super::*;

#[test]
fn active_resident_two_view_policy_reports_the_exact_missing_required_view() {
    let mut model = omega_isa_x86_64::x86_64_physical_register_model();
    model.views.retain(|view| view.name != "rcx");

    assert_eq!(
            active_resident_immediate_u64_multi_use_rematerialization_v1_views(
                omega_target::Architecture::X86_64,
                &model,
            ),
            Err(
                OptimizedAllocationLegalityCustodyError::MissingRequiredActiveResidentRematerializationView(
                    "rcx",
                ),
            )
        );
}
