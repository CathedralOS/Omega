//! Publish a named machine and select an independently published implementation by its exact identity.
use super::project;
use psi_provider_selections::{
    AdmittedComponentProvider, ProviderInstallationSelection, admit_component_provider,
};
use std::{fs, path::Path};
use terminal_codec::CanonicalTerminalArtifact;

pub fn inspect(directory: &Path, entry: &str) -> CanonicalTerminalArtifact {
    let output = project::omega(
        directory,
        &[
            "psi",
            "inspect",
            "--machine",
            entry,
            "--target",
            "linux_x86_64",
            "--output",
            "program.psi",
            "build.omg",
        ],
    );
    let (ok, diagnostic) = project::status(&output);
    assert!(ok, "{diagnostic}");
    CanonicalTerminalArtifact::from_bytes(&fs::read(directory.join("program.psi")).unwrap())
        .unwrap()
}

pub fn admit(
    receiver: &CanonicalTerminalArtifact,
    supplier: &CanonicalTerminalArtifact,
    candidate: &terminal_psi::ProviderCandidateConformance,
) -> AdmittedComponentProvider {
    let root = receiver.semantic_module();
    let identities = &supplier.semantic_module().identities;
    let slot = root
        .interfaces
        .boundary_machines
        .iter()
        .find(|slot| {
            root.identities.text(slot.identity) == identities.text(candidate.requirement_identity)
        })
        .unwrap();
    let selected = ProviderInstallationSelection {
        boundary: candidate.boundary,
        provider_identity: identities
            .text(candidate.provider_identity)
            .unwrap()
            .to_owned(),
        provider_declaration: identities
            .text(candidate.provider_declaration.identity())
            .unwrap()
            .to_owned(),
        candidate: candidate.candidate,
    };
    admit_component_provider(receiver.view(), slot.id, supplier.view(), &selected).unwrap()
}
