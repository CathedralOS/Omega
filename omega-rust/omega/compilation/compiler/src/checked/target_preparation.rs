//! Target preparation retains the accepted program's real invocation root.
//! Provider choices are already canonical Psi rows; settlement never reopens
//! source declarations or treats retained provider bodies as separate entries.

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use semantic_vocabulary::MachineId;

pub(crate) struct PreparedProductTarget {
    pub(crate) profile: Option<target::TargetProfile>,
    pub(crate) entry: Option<MachineId>,
}

impl PreparedProductTarget {
    pub(crate) fn capture(
        source: &CheckedProductSource,
        profile: Option<target::TargetProfile>,
        _package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<Self, Vec<Diagnostic>> {
        source
            .terminal_publication
            .require_source_owner(&source.program)
            .map_err(|error| vec![error])?;
        let accepted = source.terminal_publication.accepted()?;
        let module = accepted.module();
        let target = profile.unwrap_or_else(target::TargetProfile::host);
        let entry = if module.entry.is_none() && module.program_entry_contracts.is_empty() {
            // A reusable source artifact has no executable nomination. It
            // acquires provider demand only when a consumer supplies one.
            None
        } else {
            let contract = module
                .program_entry_contract(target.target_name())
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            module.machine(contract.entry).ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected target entry has no exact executable machine",
                )]
            })?;
            Some(contract.entry)
        };
        Ok(Self { profile, entry })
    }
}
