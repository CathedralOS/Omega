//! Target preparation retains the accepted program's real invocation root.
//! Provider choices are already canonical Psi rows; settlement never reopens
//! source declarations or treats retained provider bodies as separate entries.

use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use products::CheckedProductSource;
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
        let entry = match profile {
            Some(profile)
                if !(source.terminal_publication.roster()?.entries.is_empty()
                    && module.program_entry_contracts.is_empty()) =>
            {
                let contract = module
                    .program_entry_contract(profile.target_name())
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                module.machine(contract.entry).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected target entry has no exact executable machine",
                    )]
                })?;
                Some(contract.entry)
            }
            // A reusable source artifact has no executable nomination. It
            // acquires provider demand only when a consumer supplies one.
            _ => None,
        };
        Ok(Self { profile, entry })
    }
}
