//! Exact structural custody for interfaces derived from complete target families.

use crate::capture::PackageReviewInput;
use crate::capture::semantics::facts::exactly_one;
use diagnostics::Diagnostic;
use typed_trees::machine::Machine;

/// Return every member of a virtual head, with no selected-target projection.
/// Both presence and absence of the derived role must agree with compiler
/// custody, so deleting a supplied roster cannot turn a head into authored code.
pub(in crate::capture) fn members<'a>(
    compilation: &PackageReviewInput<'a>,
    machine: &Machine,
) -> Result<Option<Vec<&'a Machine>>, Vec<Diagnostic>> {
    let families = compilation
        .target_machine_families
        .iter()
        .filter(|family| family.canonical == machine.symbol)
        .collect::<Vec<_>>();
    let originals = compilation
        .custody
        .typed
        .target_machine_families
        .iter()
        .filter(|family| family.canonical == machine.symbol)
        .collect::<Vec<_>>();
    let original_machine = compilation.custody.typed.machine_by_symbol(machine.symbol);
    let derived = !machine.body_is_present && !families.is_empty();
    let originally_derived =
        original_machine.is_some_and(|original| !original.body_is_present) && !originals.is_empty();
    if !derived && !originally_derived {
        return Ok(None);
    }
    let rejected = || {
        vec![Diagnostic::error(format!(
            "reviewed family `{}` does not retain its exact derived interface and complete member roster",
            machine.name,
        ))]
    };
    let [family] = families.as_slice() else {
        return Err(rejected());
    };
    if !derived
        || !originally_derived
        || originals.as_slice() != [*family]
        || family.members.is_empty()
        || !machine.target_family_interface
        || original_machine.is_none_or(|original| !original.target_family_interface)
    {
        return Err(rejected());
    }
    let mut members = Vec::with_capacity(family.members.len());
    for member in &family.members {
        if member.machine == machine.symbol {
            return Err(rejected());
        }
        members.push(exactly_one(
            compilation
                .program
                .machines()
                .iter()
                .filter(|candidate| candidate.symbol == member.machine),
            machine.name.as_str(),
            "family member declaration",
        )?);
    }
    Ok(Some(members))
}
