//! Disclose each retained runtime relationship without interpreting its trust.
//!
//! The ordinary summary shows the same compact risk meaning as the canonical
//! row. Hiding that meaning behind `--details` would make a dependency's assumed
//! crossing look like ordinary provider selection. Strings describe the exact
//! committed relationship; rendering supplies neither consent nor proof. Debug
//! quoting preserves boundaries when a retained description contains newlines.

use package_evidence::record::PackagePolicyAssumedBindingContracts;
use std::fmt::{self, Write};

use super::Output;

pub(super) fn render(
    output: &mut Output,
    contracts: &PackagePolicyAssumedBindingContracts,
) -> fmt::Result {
    writeln!(
        output,
        "  assumed-binding-contracts {}",
        contracts.relationships().len()
    )?;
    for contract in contracts.relationships() {
        writeln!(
            output,
            "    Extremely high risk - assumed binding contract {}",
            contract.relationship.digest()
        )?;
        writeln!(output, "    scope runtime-only")?;
        for (label, description) in [
            ("required contract", &contract.required_contract),
            ("supplied contract", &contract.supplied_contract),
            ("selected implementation", &contract.selected_implementation),
            ("assumption difference", &contract.assumption_difference),
            ("authority introduced", &contract.authority_introduced),
            ("root scope", &contract.root_scope),
            ("dependency path", &contract.dependency_path),
        ] {
            writeln!(output, "    {label}: {description:?}")?;
        }
        writeln!(
            output,
            "    demonstrated contradictions {}",
            contract.demonstrated_contradictions.len()
        )?;
        for contradiction in &contract.demonstrated_contradictions {
            writeln!(output, "      {contradiction:?}")?;
        }
    }
    Ok(())
}
