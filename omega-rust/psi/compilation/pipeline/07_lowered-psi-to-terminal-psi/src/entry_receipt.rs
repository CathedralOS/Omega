//! Establish checked-source correspondence for the exact published entry.
//!
//! Source eligibility and Terminal identity are publication judgments, not
//! product-coordinator policy. A receipt does not grant native root authority.

use checked_trees::CheckedTrees;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{CheckedProgramEntryTerminalReceipt, TerminalMachineResult};

#[derive(Debug)]
pub enum ProgramEntryTerminalReceiptError {
    TerminalEntryMultiplicity(usize),
    NonUnitEntry,
    MissingEntryDeclaration,
    TerminalIdentity(terminal_codec::CodecError),
    ArtifactSemanticIdentityMismatch,
    ContextBridgeMultiplicity(usize),
    ContextBridgeOccurrenceMismatch,
}

impl std::fmt::Display for ProgramEntryTerminalReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProgramEntryTerminalReceiptError {}

/// Source-issued nominal root description. The private identity table and exact
/// declaration survive source preparation; resolution and receipt construction
/// borrow only the published artifact. This is not native entry authority.
pub struct PreparedNominalRoot {
    identities: terminal_psi::TerminalIdentityTable,
    declaration: terminal_psi::NominalDeclarationIdentity,
    source_machine_name: String,
    diagnostic_name: Option<(String, Option<source::SourceSpan>)>,
}

/// Capture the exact source declaration before portable publication.
pub fn prepare_nominal_root(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source_machine_name: &str,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<PreparedNominalRoot, ProgramEntryTerminalReceiptError> {
    let declaration_identity = checked
        .exact_owner_nominal_identity(machine, exact_toolchain_sources)
        .and_then(|identity| checked.type_identity_text(identity))
        .ok_or(ProgramEntryTerminalReceiptError::MissingEntryDeclaration)?;
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let declaration =
        terminal_psi::NominalDeclarationIdentity::new(identities.intern(&declaration_identity));
    Ok(PreparedNominalRoot {
        identities: identities.build(),
        declaration,
        source_machine_name: source_machine_name.to_owned(),
        diagnostic_name: checked.typed.machine(machine).map(|body| {
            (
                body.name.as_str().to_owned(),
                checked.typed.symbols.symbol_source_span(machine),
            )
        }),
    })
}

impl PreparedNominalRoot {
    /// Rejoin the same nominal declaration, never a same-named body or an
    /// equal-looking signature. Matching still establishes no execution right.
    pub fn resolve<'module>(
        &self,
        module: &'module terminal_psi::TerminalModule,
    ) -> Result<&'module terminal_psi::TerminalMachine, ProgramEntryTerminalReceiptError> {
        let entry_matches = module
            .machines
            .iter()
            .filter(|machine| {
                machine.declaration.as_ref().is_some_and(|declaration| {
                    self.declaration.same_identity_as(
                        &self.identities,
                        &declaration.machine,
                        &module.identities,
                    )
                })
            })
            .collect::<Vec<_>>();
        let [entry] = entry_matches.as_slice() else {
            return Err(ProgramEntryTerminalReceiptError::TerminalEntryMultiplicity(
                entry_matches.len(),
            ));
        };
        Ok(entry)
    }

    pub fn diagnostic_name(&self) -> Option<(&str, Option<source::SourceSpan>)> {
        self.diagnostic_name
            .as_ref()
            .map(|(name, span)| (name.as_str(), *span))
    }
}

/// Bind the source-issued description to the sealed artifact's unique Unit
/// entry. The source-signature identity is caller-owned checked custody; this
/// operation neither reopens source nor grants native entry authority.
pub fn prepare_program_entry_receipt(
    declaration: &PreparedNominalRoot,
    source_signature_identity: [u8; 32],
    artifact: &CanonicalTerminalArtifact,
) -> Result<CheckedProgramEntryTerminalReceipt, ProgramEntryTerminalReceiptError> {
    let entry = declaration.resolve(artifact.semantic_module())?;
    if entry.result != TerminalMachineResult::Unit {
        return Err(ProgramEntryTerminalReceiptError::NonUnitEntry);
    }
    let source_declaration = entry
        .declaration
        .as_ref()
        .ok_or(ProgramEntryTerminalReceiptError::MissingEntryDeclaration)?;
    Ok(CheckedProgramEntryTerminalReceipt::new(
        source_signature_identity,
        declaration.source_machine_name.clone(),
        source_declaration.machine.clone(),
        artifact.manifest().semantic(),
        entry.id,
    ))
}

/// Rejoin a prepared receipt to the exact semantics that publication retained.
/// This checks identity correspondence, not independently derived source evidence.
pub fn rejoin_program_entry_receipt(
    receipt: &CheckedProgramEntryTerminalReceipt,
    artifact: &CanonicalTerminalArtifact,
) -> Result<(), ProgramEntryTerminalReceiptError> {
    if artifact.manifest().semantic() != receipt.terminal_psi_identity() {
        return Err(ProgramEntryTerminalReceiptError::ArtifactSemanticIdentityMismatch);
    }
    Ok(())
}
