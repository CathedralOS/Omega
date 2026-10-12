//! Source admission uses the shared original Product requirement selection.

pub use typed_trees::product_requirements::ProductRequirementSelection;

pub fn resolve_product_requirement(
    input: &dyn crate::CheckingInput,
    argument: &typed_trees::expression::StaticMachineArgument,
    occurrence: source::SourceSpan,
) -> Option<ProductRequirementSelection> {
    typed_trees::product_requirements::resolve_product_requirement(
        input.product_declarations(),
        argument,
        occurrence,
    )
}
