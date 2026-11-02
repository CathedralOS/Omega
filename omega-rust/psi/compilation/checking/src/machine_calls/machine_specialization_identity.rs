//! Join a selected checked adapter to the generic template a machine
//! specialization was made from.
//!
//! A ProviderPlan row names its checked adapter by normalized callable
//! identity and package, not by symbol. A generic adapter is selected by the
//! authored template, while execution runs one of its specializations, so
//! the join reads the template identity stage 05 recorded before
//! substitution: `<declaration>|<callable identity>`. The declaration half is
//! derived from the template symbol alone and contains no `|`, so the join
//! compares the template's package and the recorded callable half.

/// Whether `specialization` was made from the template that a ProviderPlan's
/// split checked-adapter identity names.
pub fn machine_specialization_matches_template_identity(
    typed: &dyn crate::CheckingInput,
    specialization: &typed_trees::typed_trees::MachineSpecialization,
    machine_identity: &str,
    machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> bool {
    typed_trees::machine_specialization_matches_template_identity(
        typed.typed_program(),
        specialization,
        machine_identity,
        machine_package_identity,
    )
}
