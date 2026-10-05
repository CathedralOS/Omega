//! Join a selected checked adapter to the generic template a machine
//! specialization was made from.
//!
//! A ProviderPlan row names its checked adapter by normalized callable
//! identity and package, not by symbol. A generic adapter is selected by the
//! authored template, while execution runs one of its specializations, so
//! the join compares the plan's identity with the package-qualified template
//! identity stage 04 recorded before substitution.

/// Whether `specialization` was made from the template that a ProviderPlan's
/// split checked-adapter identity names.
pub fn machine_specialization_matches_template_identity(
    typed: &typed_trees::TypedTrees,
    specialization: &typed_trees::typed_trees::MachineSpecialization,
    machine_identity: &str,
    machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> bool {
    if typed
        .symbols
        .symbol_package_identity(specialization.template)
        != machine_package_identity
    {
        return false;
    }
    let path = typed.symbols.display_path(specialization.template, "::");
    let declaration = match machine_package_identity {
        Some(package) => {
            let mut owner = String::with_capacity(package.digest().len() * 2);
            const HEX: &[u8; 16] = b"0123456789abcdef";
            for byte in package.digest() {
                owner.push(char::from(HEX[usize::from(byte >> 4)]));
                owner.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
            format!("package:{owner}::{path}")
        }
        None => format!("unmanaged::{path}"),
    };
    specialization.normalized_template_identity == format!("{declaration}|{machine_identity}")
}
