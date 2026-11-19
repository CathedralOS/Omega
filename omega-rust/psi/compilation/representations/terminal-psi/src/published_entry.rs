//! Declaration owners and machine names issued with a portable program.

use semantic_vocabulary::compiler_evaluation::DeclarationCustody;

type DeclarationOwner = (
    crate::TerminalIdentityIndex,
    Result<DeclarationCustody, String>,
);

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct PublishedEntryMetadata {
    provenance: crate::TerminalMachineProvenance,
    has_bound_applications: bool,
    entry_custody: Vec<DeclarationOwner>,
    calling_custody: Result<Vec<DeclarationOwner>, String>,
}

impl PublishedEntryMetadata {
    pub fn new(
        provenance: crate::TerminalMachineProvenance,
        has_bound_applications: bool,
        mut entry_custody: Vec<DeclarationOwner>,
        mut calling_custody: Result<Vec<DeclarationOwner>, String>,
    ) -> Self {
        entry_custody.sort_unstable_by_key(|row| row.0);
        if let Ok(rows) = &mut calling_custody {
            rows.sort_unstable_by_key(|row| row.0);
        }
        Self {
            provenance,
            has_bound_applications,
            entry_custody,
            calling_custody,
        }
    }

    pub fn validate_declarations(&self, module: &crate::TerminalModule) -> bool {
        fn matches(
            rows: &[DeclarationOwner],
            requested: impl Iterator<Item = crate::TerminalIdentityIndex>,
        ) -> bool {
            let mut expected: Vec<_> = requested.collect();
            expected.sort_unstable();
            expected.dedup();
            rows.iter().map(|row| row.0).eq(expected)
        }
        matches(
            &self.entry_custody,
            module
                .program_entry_contracts
                .iter()
                .map(|entry| entry.schema_declaration),
        ) && self.calling_custody.as_ref().map_or(true, |rows| {
            matches(
                rows,
                module
                    .calling_policy_requests
                    .iter()
                    .filter_map(|request| request.declaration.as_ref().ok().copied()),
            )
        })
    }

    pub fn require_calling_custody(&self) -> Result<(), String> {
        self.calling_custody
            .as_ref()
            .map(|_| ())
            .map_err(Clone::clone)
    }

    pub fn calling_schema_custody(
        &self,
        declaration: crate::TerminalIdentityIndex,
    ) -> Result<DeclarationCustody, String> {
        Self::owner(
            self.calling_custody.as_ref().map_err(Clone::clone)?,
            declaration,
        )
    }

    pub fn has_bound_applications(&self) -> bool {
        self.has_bound_applications
    }

    pub fn machine_provenance(&self) -> &crate::TerminalMachineProvenance {
        &self.provenance
    }
    pub fn entry_schema_custody(
        &self,
        declaration: crate::TerminalIdentityIndex,
    ) -> Result<DeclarationCustody, String> {
        Self::owner(&self.entry_custody, declaration)
    }

    fn owner(
        rows: &[DeclarationOwner],
        declaration: crate::TerminalIdentityIndex,
    ) -> Result<DeclarationCustody, String> {
        let index = rows
            .binary_search_by_key(&declaration, |row| row.0)
            .map_err(|_| "selected declaration has no authenticated source custody".to_owned())?;
        rows[index].1.clone()
    }
}
