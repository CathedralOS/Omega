//! Source identities, quoted bytes and code catalogs for native publication.
//!
//! These rows contain no executable body. Target lowering records them while
//! it owns the current source function; later forms share the sealed roster
//! rather than reopening that function to recover an ordinal or quoted bytes.

mod execution_scope;
pub use execution_scope::{EmittedCallbackOccurrence, EmittedExecutionScope};

use std::sync::Arc;

use ::wire::digest_encoder::DomainDigestEncoder;
pub use native_vocabulary::provenance::PsiProvenance;
use semantic_vocabulary::{MachineId, OperationId, PlaceId};

/// Facts resolved while the admitted module and its target selection are available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentEntryFacts {
    pub terminal: terminal_psi::TerminalPsiIdentity,
    pub profile: Result<target::TargetProfile, Box<str>>,
    pub machine: Result<MachineId, &'static str>,
}

/// Entry and execution facts projected from a checked program. The original
/// provider and interrupt admissions stay with installation planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEntryFacts {
    pub terminal: terminal_psi::TerminalPsiIdentity,
    pub entry: MachineId,
    pub original_entry: Option<terminal_psi::TargetMachineCallable>,
    pub entry_has_body: bool,
    pub deployment: Option<DeploymentEntryFacts>,
    pub identities: terminal_psi::TerminalIdentityTable,
    pub entry_declaration: Option<terminal_psi::CheckedMachineDeclaration>,
    pub native_execution_scope: Option<std::sync::Arc<EmittedExecutionScope>>,
    pub selected_provider_closure: Option<[u8; 32]>,
}

impl NativeEntryFacts {
    /// Check current entry custody without reopening an earlier executable graph.
    pub fn selected_terminal_entry(
        &self,
        identity: terminal_psi::TerminalPsiIdentity,
        entry: MachineId,
        native_target: target::NativeTarget,
    ) -> Result<Option<MachineId>, &'static str> {
        if self.terminal != identity || self.entry != entry {
            return Err("native object entry differs from its admitted executable input");
        }
        let Some(deployment) = &self.deployment else {
            return Ok(None);
        };
        let profile = deployment
            .profile
            .as_ref()
            .map_err(|_| "native object entry has an unknown admitted deployment tag")?;
        if profile.native_realization() != Some(native_target) {
            return Err("native object target differs from its admitted deployment profile");
        }
        deployment.machine.map(Some)
    }

    pub fn admits_hosted_entry(
        &self,
        identity: terminal_psi::TerminalPsiIdentity,
        entry: MachineId,
        profile: target::TargetProfile,
    ) -> bool {
        match &self.deployment {
            Some(deployment) => {
                deployment.terminal == identity
                    && deployment.profile.as_ref() == Ok(&profile)
                    && deployment.machine == Ok(entry)
            }
            None => {
                self.original_entry == Some(terminal_psi::TargetMachineCallable::Machine(entry))
            }
        }
    }

    pub const fn terminal(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal
    }
    pub const fn entry(&self) -> MachineId {
        self.entry
    }
    pub fn has_entry_body(&self, entry: MachineId) -> bool {
        self.entry == entry && self.entry_has_body
    }

    pub const fn original_entry(&self) -> Option<terminal_psi::TargetMachineCallable> {
        self.original_entry
    }
    pub fn identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.identities
    }
    pub fn entry_declaration(&self) -> Option<&terminal_psi::CheckedMachineDeclaration> {
        self.entry_declaration.as_ref()
    }
    pub fn native_execution_scope_for(
        &self,
        terminal: terminal_psi::TerminalPsiIdentity,
        target: target::NativeTarget,
        entry: MachineId,
        selected_provider_closure: [u8; 32],
    ) -> Result<Option<&EmittedExecutionScope>, &'static str> {
        let Some(scope) = self.native_execution_scope.as_deref() else {
            return Ok(None);
        };
        let deployment = self
            .deployment
            .as_ref()
            .ok_or("native execution scope has no target selection custody")?;
        let profile = deployment
            .profile
            .as_ref()
            .map_err(|_| "native execution scope names an unknown target")?;
        if self.terminal != terminal
            || deployment.terminal != terminal
            || profile.native_target() != target
            || self.entry != entry
            || self.selected_provider_closure != Some(selected_provider_closure)
        {
            return Err(
                "native execution scope belongs to another artifact, target, entry or provider selection",
            );
        }
        Ok(Some(scope))
    }
}

pub const NATIVE_PUBLICATION_DOMAIN: &[u8] = b"omega.native-publication-facts.v3\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeByteLiteral {
    pub operation: OperationId,
    pub place: PlaceId,
    pub bytes: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFunctionPublication {
    pub machine: MachineId,
    pub operation_count: u32,
    /// Ordered by site, with one exact original operation ordinal per site.
    /// Several control edges can belong to the same source operation.
    pub sites: Vec<(PsiProvenance, u32)>,
    /// Ordered by operation. Empty literals still have an explicit row.
    pub byte_literals: Vec<NativeByteLiteral>,
    /// The exact evaluated target of each source structural call, by operation.
    pub call_targets: Vec<(OperationId, crate::NativeCallTarget)>,
}

impl NativeFunctionPublication {
    pub fn ordinal(&self, site: PsiProvenance) -> Option<u32> {
        self.sites
            .binary_search_by_key(&site, |row| row.0)
            .ok()
            .map(|position| self.sites[position].1)
    }

    pub fn byte_literal(&self, operation: OperationId) -> Option<&NativeByteLiteral> {
        self.byte_literals
            .binary_search_by_key(&operation, |row| row.operation)
            .ok()
            .map(|position| &self.byte_literals[position])
    }

    pub fn call_target(&self, operation: OperationId) -> Option<crate::NativeCallTarget> {
        self.call_targets
            .binary_search_by_key(&operation, |row| row.0)
            .ok()
            .map(|position| self.call_targets[position].1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePublicationFacts {
    functions: Arc<[NativeFunctionPublication]>,
    code_catalogs: crate::code_catalogs::NativeCodeCatalogs,
    identity: [u8; 32],
}

impl NativePublicationFacts {
    pub fn seal(
        functions: Vec<NativeFunctionPublication>,
        code_catalogs: crate::code_catalogs::NativeCodeCatalogs,
    ) -> Result<Self, &'static str> {
        if functions
            .windows(2)
            .any(|rows| rows[0].machine >= rows[1].machine)
        {
            return Err("publication functions are unordered or repeated");
        }
        for function in &functions {
            if function
                .call_targets
                .windows(2)
                .any(|rows| rows[0].0 >= rows[1].0)
                || function
                    .call_targets
                    .iter()
                    .any(|row| function.ordinal(PsiProvenance::Operation(row.0)).is_none())
            {
                return Err(
                    "publication calls are repeated, unordered or have no source occurrence",
                );
            }
            if function.sites.windows(2).any(|rows| rows[0].0 >= rows[1].0) {
                return Err("publication source sites are unordered or repeated");
            }
            if function
                .sites
                .iter()
                .any(|row| row.1 >= function.operation_count)
            {
                return Err("publication source ordinal exceeds its function");
            }
            if function
                .byte_literals
                .windows(2)
                .any(|rows| rows[0].operation >= rows[1].operation)
            {
                return Err("publication literals are unordered or repeated");
            }
            if function.byte_literals.iter().any(|row| {
                function
                    .ordinal(PsiProvenance::Operation(row.operation))
                    .is_none()
            }) {
                return Err("publication literal has no source occurrence");
            }
        }
        let mut digest = DomainDigestEncoder::new(b"");
        encode(&functions, &code_catalogs, |bytes| digest.raw(bytes));
        Ok(Self {
            functions: functions.into(),
            code_catalogs,
            identity: digest.finish(),
        })
    }

    pub fn functions(&self) -> &[NativeFunctionPublication] {
        &self.functions
    }

    pub const fn code_catalogs(&self) -> &crate::code_catalogs::NativeCodeCatalogs {
        &self.code_catalogs
    }

    pub fn function(&self, machine: MachineId) -> Option<&NativeFunctionPublication> {
        self.functions
            .binary_search_by_key(&machine, |row| row.machine)
            .ok()
            .map(|position| &self.functions[position])
    }

    pub const fn identity_bytes(&self) -> [u8; 32] {
        self.identity
    }

    pub fn encode(&self, bytes: &mut Vec<u8>) {
        encode(&self.functions, &self.code_catalogs, |part| {
            bytes.extend_from_slice(part)
        });
    }
}

impl Default for NativePublicationFacts {
    fn default() -> Self {
        Self::seal(Vec::new(), Default::default()).expect("the empty publication roster is ordered")
    }
}

fn encode(
    functions: &[NativeFunctionPublication],
    code_catalogs: &crate::code_catalogs::NativeCodeCatalogs,
    mut write: impl FnMut(&[u8]),
) {
    write(NATIVE_PUBLICATION_DOMAIN);
    write(&(functions.len() as u64).to_le_bytes());
    for function in functions {
        write(&function.machine.get().to_le_bytes());
        write(&function.operation_count.to_le_bytes());
        write(&(function.sites.len() as u64).to_le_bytes());
        for (site, ordinal) in &function.sites {
            let (tag, identity) = match site {
                PsiProvenance::Operation(operation) => (0, operation.get()),
                PsiProvenance::Edge(edge) => (1, edge.get()),
            };
            write(&[tag]);
            write(&identity.to_le_bytes());
            write(&ordinal.to_le_bytes());
        }
        write(&(function.byte_literals.len() as u64).to_le_bytes());
        for literal in &function.byte_literals {
            write(&literal.operation.get().to_le_bytes());
            write(&literal.place.get().to_le_bytes());
            write(&(literal.bytes.len() as u64).to_le_bytes());
            write(&literal.bytes);
        }
        write(&(function.call_targets.len() as u64).to_le_bytes());
        for (operation, target) in &function.call_targets {
            write(&operation.get().to_le_bytes());
            target.visit_canonical_bytes(&mut write);
        }
    }
    code_catalogs.encode(write);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function() -> NativeFunctionPublication {
        let operation = OperationId::new(1).unwrap();
        NativeFunctionPublication {
            call_targets: Vec::new(),
            machine: MachineId::new(1).unwrap(),
            operation_count: 2,
            sites: vec![
                (PsiProvenance::Operation(operation), 0),
                (
                    PsiProvenance::Edge(semantic_vocabulary::EdgeId::new(1).unwrap()),
                    1,
                ),
            ],
            byte_literals: vec![NativeByteLiteral {
                operation,
                place: PlaceId::new(1).unwrap(),
                bytes: Arc::from(b"kept".as_slice()),
            }],
        }
    }

    #[test]
    fn publication_shares_payloads_and_commits_exact_sites_and_bytes() {
        let row = function();
        let bytes = Arc::clone(&row.byte_literals[0].bytes);
        let facts = NativePublicationFacts::seal(vec![row.clone()], Default::default()).unwrap();
        let copy = facts.clone();
        assert!(Arc::ptr_eq(&facts.functions, &copy.functions));
        assert!(Arc::ptr_eq(
            &bytes,
            &facts.functions()[0].byte_literals[0].bytes
        ));
        let mut encoded = Vec::new();
        facts.encode(&mut encoded);
        assert_eq!(
            facts.identity_bytes(),
            ::wire::digest_encoder::content_identity(&encoded)
        );
        let mut ordinal = row.clone();
        ordinal.sites[0].1 = 1;
        assert_ne!(
            facts.identity_bytes(),
            NativePublicationFacts::seal(vec![ordinal], Default::default())
                .unwrap()
                .identity_bytes()
        );
        let mut literal = row;
        literal.byte_literals[0].bytes = Arc::from(b"other".as_slice());
        assert_ne!(
            facts.identity_bytes(),
            NativePublicationFacts::seal(vec![literal], Default::default())
                .unwrap()
                .identity_bytes()
        );
    }

    #[test]
    fn publication_refuses_duplicate_missing_and_out_of_range_source_rows() {
        let original = function();
        let mut duplicate = original.clone();
        duplicate.sites.push(duplicate.sites[1]);
        let mut missing = original.clone();
        missing.sites.remove(0);
        let mut outside = original.clone();
        outside.sites[0].1 = outside.operation_count;
        let mut literal = original.clone();
        literal.byte_literals.push(literal.byte_literals[0].clone());
        for row in [duplicate, missing, outside, literal] {
            assert!(NativePublicationFacts::seal(vec![row], Default::default()).is_err());
        }
        assert!(
            NativePublicationFacts::seal(vec![original.clone(), original], Default::default())
                .is_err()
        );
    }
}
