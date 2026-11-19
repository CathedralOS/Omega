//! Issue per-declaration outcomes while Psi owns the authenticated source selections.

pub(super) fn issue(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    owners: Option<&Result<psi_authorization::selection_questions::ProductSchemaCustody, String>>,
    provenance: terminal_psi::TerminalMachineProvenance,
    has_bound_applications: bool,
) -> terminal_psi::PublishedEntryMetadata {
    let module = artifact.semantic_module();
    let mut entry_custody = Vec::new();
    for entry in &module.interfaces.program_entry_contracts {
        let declaration = entry.schema_declaration;
        if entry_custody.iter().any(|(known, _)| *known == declaration) {
            continue;
        }
        let custody = match owners {
            None => Err("entry schema has no authenticated source selection custody".into()),
            Some(_) if module.identities.text(declaration).is_none() => {
                Err("entry schema lost its canonical declaration identity".into())
            }
            Some(Err(reason)) => Err(reason.clone()),
            Some(Ok(owners)) => owners.entry_requester(artifact.view().into(), declaration),
        };
        entry_custody.push((declaration, custody));
    }

    let mut calling: Vec<_> = module
        .interfaces.calling_policy_requests
        .iter()
        .filter_map(|request| request.declaration.as_ref().ok().copied())
        .collect();
    calling.sort_unstable();
    calling.dedup();
    let calling_custody = match owners {
        None => Err("provider policies lost their source selection custody".into()),
        Some(Err(reason)) => Err(reason.clone()),
        Some(Ok(owners)) => Ok(calling
            .into_iter()
            .map(|id| (id, owners.requester(artifact.view().into(), id)))
            .collect()),
    };
    terminal_psi::PublishedEntryMetadata::new(
        provenance,
        has_bound_applications,
        entry_custody,
        calling_custody,
    )
}
