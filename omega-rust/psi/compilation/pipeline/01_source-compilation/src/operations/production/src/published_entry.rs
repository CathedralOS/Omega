//! Issue per-declaration outcomes while Psi owns the authenticated source selections.

pub(super) fn issue(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    selections: Option<&super::TerminalSourceSelections>,
    provenance: terminal_psi::TerminalMachineProvenance,
) -> terminal_psi::PublishedEntryMetadata {
    let module = artifact.semantic_module();
    let mut entry_custody = Vec::new();
    for entry in &module.program_entry_contracts {
        let declaration = entry.schema_declaration;
        if entry_custody.iter().any(|(known, _)| *known == declaration) {
            continue;
        }
        let custody = selections
            .ok_or_else(|| "entry schema has no authenticated source selection custody".to_owned())
            .and_then(|selections| {
                let declaration = module.identities.text(declaration).ok_or_else(|| {
                    "entry schema lost its canonical declaration identity".to_owned()
                })?;
                selections.declaration_custody(
                    artifact.view().into(),
                    declaration,
                    semantic_vocabulary::source_selections::SelectionPurpose::Product,
                )
            });
        entry_custody.push((declaration, custody));
    }

    terminal_psi::PublishedEntryMetadata::new(provenance, entry_custody)
}
