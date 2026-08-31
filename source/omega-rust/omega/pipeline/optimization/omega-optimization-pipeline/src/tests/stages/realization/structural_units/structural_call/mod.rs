//! Optimizer module role: stage group. This scenario follows one structural
//! Unit call through the four custody boundaries that own its realization.

mod effects_allocation;
mod encoding_layout;
mod legalization_selection;
mod realization_publication;

#[test]
fn structural_unit_call_reaches_post_allocation_machine_custody() {
    let selected = legalization_selection::lower_and_select_structural_call();
    let homes = effects_allocation::analyze_and_allocate_structural_call(selected);
    encoding_layout::verify_structural_call_encoding_and_layout(&homes);
    realization_publication::realize_and_publish_structural_call(homes);
}
