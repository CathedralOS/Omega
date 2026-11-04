//! Completed default choices retain their actual supplier and Build observation.
//!
//! Package and captured toolchain sources keep distinct custody. This value
//! transports executed choices; it grants neither private nameability nor
//! Build execution authority. Product source still rejoins exact declarations.

use super::{PackageBuildHandoff, PackageCompilationInputs};
use crate::{BuildObservationSummary, CapturedLocalSource};
use build_time_evaluation::provider_declarations::{ProviderSelection, RetainedProviderSelection};
use diagnostics::Diagnostic;
use source::{SourceFile, SourceOrigin, SourceResolutionStratum};
use std::path::{Component, PathBuf};
use std::sync::Arc;
use symbols::RetainedDeclaration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedProviderDefaults {
    supplier: ProviderDefaultSupplier,
    observation: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderDefaultSupplier {
    Package(PackageBuildHandoff),
    CapturedToolchain {
        capture: CapturedLocalSource,
        selections: Arc<[RetainedProviderSelection]>,
    },
}

/// The receiving invocation's admitted source, not authority inferred from rows.
#[derive(Clone, Copy)]
pub enum ProviderDefaultsAdmission<'source> {
    Package(&'source PackageCompilationInputs),
    CapturedToolchain(&'source CapturedLocalSource),
}

impl CompletedProviderDefaults {
    pub(crate) fn from_package(bundle: &PackageBuildHandoff) -> Result<Option<Self>, &'static str> {
        if bundle.provider_selections().is_empty() {
            return Ok(None);
        }
        let observation = bundle
            .selection_observation()
            .ok_or("package defaults have no completed Build observation")?;
        Ok(Some(Self {
            supplier: ProviderDefaultSupplier::Package(bundle.clone()),
            observation,
        }))
    }

    /// Capture actual completed choices from the checked core Build. Its
    /// complete input inventory must match the original execution observation.
    /// A retained row's source spelling cannot establish toolchain custody.
    pub fn from_captured_toolchain(
        capture: CapturedLocalSource,
        typed: &typed_trees::TypedTrees,
        selections: &[ProviderSelection],
        observation: &BuildObservationSummary,
    ) -> Result<Self, Vec<Diagnostic>> {
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        if capture.canonical_live_root() != crate::source::source_imports::bundled_core_root()? {
            return Err(refuse(
                "toolchain defaults do not use the captured core root".into(),
            ));
        }
        if observation.activation().root_package_identity().is_some()
            || observation.activation().root_role().is_some()
        {
            return Err(refuse(
                "toolchain defaults cannot replace a package Build supplier".into(),
            ));
        }
        let input = crate::source::source_snapshot::retained_source_input(capture.tree())
            .map_err(refuse)?;
        let metadata = input.canonical_source_metadata();
        let inventory = observation.captured_source_inventory().ok_or_else(|| {
            refuse("toolchain defaults have no completed captured-source inventory".into())
        })?;
        let identity = inventory.source_metadata_identity();
        if identity.policy_version() != metadata.policy_version()
            || identity.source_content_commitment() != *metadata.source_content_commitment()
            || inventory.entry_count() != input.entry_count()
            || inventory.file_bytes() != input.file_bytes()
            || observation.canonical_source_metadata_identity() != Some(identity)
        {
            return Err(refuse(
                "toolchain defaults differ from their original Build capture".into(),
            ));
        }
        let selections = selections
            .iter()
            .map(|selection| RetainedProviderSelection::capture(typed, selection))
            .collect::<Result<Vec<_>, _>>()
            .map_err(refuse)?;
        if let Some(first) = selections.first() {
            for selection in &selections {
                if selection.origin.event_ordinal().is_none()
                    || selection.origin.activation() != first.origin.activation()
                {
                    return Err(refuse(
                        "toolchain defaults mix incomplete or different Build activations".into(),
                    ));
                }
                for anchor in [
                    &selection.subject,
                    &selection.provider,
                    selection.origin.activation(),
                    selection.origin.author(),
                ] {
                    admit_toolchain_source(&capture, &capture, anchor.source())?;
                }
                for application in [
                    &selection.subject_application,
                    &selection.provider_application,
                ]
                .into_iter()
                .flatten()
                {
                    for span in application.declaration_spans() {
                        let file = application.sources.get(span.source_id).ok_or_else(|| {
                            refuse(
                                "toolchain default application has a foreign source coordinate"
                                    .into(),
                            )
                        })?;
                        admit_toolchain_source(&capture, &capture, file)?;
                    }
                }
            }
        }
        for file in typed.symbols.source_files() {
            if crate::source::source_assembly::is_build_prelude_source(file)
                && file.package_root == capture.canonical_live_root()
            {
                continue;
            }
            // A helper read outside core would not belong to the observation's
            // captured inventory, even if none of its declarations was selected.
            admit_toolchain_source(&capture, &capture, file)?;
        }

        Ok(Self {
            supplier: ProviderDefaultSupplier::CapturedToolchain {
                capture,
                selections: selections.into(),
            },
            observation: observation.identity().digest(),
        })
    }

    pub fn selections(&self) -> &[RetainedProviderSelection] {
        match &self.supplier {
            ProviderDefaultSupplier::Package(bundle) => bundle.provider_selections(),
            ProviderDefaultSupplier::CapturedToolchain { selections, .. } => selections,
        }
    }

    pub const fn observation(&self) -> &[u8; 32] {
        &self.observation
    }

    pub fn toolchain_capture(&self) -> Option<&CapturedLocalSource> {
        match &self.supplier {
            ProviderDefaultSupplier::Package(_) => None,
            ProviderDefaultSupplier::CapturedToolchain { capture, .. } => Some(capture),
        }
    }

    pub fn source_seeds(
        &self,
        admission: ProviderDefaultsAdmission<'_>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        self.admit_activation(admission)?;
        let mut paths = Vec::new();
        for selection in self.selections() {
            for anchor in [&selection.subject, &selection.provider] {
                let source = self.admit_source(admission, anchor.source())?;
                if source.resolution_stratum == SourceResolutionStratum::Base {
                    paths.push(source.path);
                }
            }
            for application in [
                &selection.subject_application,
                &selection.provider_application,
            ]
            .into_iter()
            .flatten()
            {
                for span in application.declaration_spans() {
                    let file = application.sources.get(span.source_id).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "retained application has a foreign source coordinate",
                        )]
                    })?;
                    let source = self.admit_source(admission, file)?;
                    if source.resolution_stratum == SourceResolutionStratum::Base {
                        paths.push(source.path);
                    }
                }
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub fn rejoin(
        &self,
        source: &mut psi_compiler::SourceContinuation,
        admission: ProviderDefaultsAdmission<'_>,
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        self.admit_activation(admission)?;
        if let ProviderDefaultsAdmission::CapturedToolchain(current) = admission {
            let ProviderDefaultSupplier::CapturedToolchain { capture, .. } = &self.supplier else {
                return Err(wrong_supplier());
            };
            // This supplier owns the core portion only. Other independently
            // admitted toolchain contract roots keep their own source custody.
            for file in source
                .typed()
                .symbols
                .source_files()
                .filter(|file| file.path.starts_with(current.canonical_live_root()))
            {
                admit_toolchain_source(current, current, file)?;
            }
            if capture.tree() != current.tree() {
                return Err(vec![Diagnostic::error(
                    "receiving core differs from the completed default Build capture",
                )]);
            }
        }
        let mut selections = Vec::new();
        for selection in self.selections() {
            let mut selected = selection.clone();
            selected.subject = self.admit_declaration(admission, &selected.subject)?;
            selected.provider = self.admit_declaration(admission, &selected.provider)?;
            for application in [
                &mut selected.subject_application,
                &mut selected.provider_application,
            ]
            .into_iter()
            .flatten()
            {
                let needed = application
                    .declaration_spans()
                    .map(|span| span.source_id)
                    .collect::<Vec<_>>();
                let files = application
                    .sources
                    .files()
                    .map(|file| {
                        if needed.contains(&file.source_id) {
                            self.admit_source(admission, file)
                        } else {
                            Ok(file.clone())
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                application.sources = Arc::new(source::SourceMap::from_files(files));
            }
            selections.push(
                source
                    .rejoin_product_provider_selection(&selected)
                    .map_err(|error| vec![error])?,
            );
        }
        Ok(selections)
    }

    fn admit_activation(
        &self,
        admission: ProviderDefaultsAdmission<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        match (&self.supplier, admission) {
            (
                ProviderDefaultSupplier::Package(bundle),
                ProviderDefaultsAdmission::Package(inputs),
            ) => inputs.admit_provider_activation(bundle),
            (
                ProviderDefaultSupplier::CapturedToolchain {
                    capture,
                    selections,
                },
                ProviderDefaultsAdmission::CapturedToolchain(current),
            ) => {
                if current.canonical_live_root()
                    != crate::source::source_imports::bundled_core_root()?
                    || capture.tree() != current.tree()
                {
                    return Err(vec![Diagnostic::error(
                        "toolchain defaults do not match the receiving closed core capture",
                    )]);
                }
                if let Some(first) = selections.first() {
                    admit_toolchain_source(capture, current, first.origin.activation().source())?;
                }
                Ok(())
            }
            _ => Err(wrong_supplier()),
        }
    }

    fn admit_declaration(
        &self,
        admission: ProviderDefaultsAdmission<'_>,
        anchor: &RetainedDeclaration,
    ) -> Result<RetainedDeclaration, Vec<Diagnostic>> {
        let source = self.admit_source(admission, anchor.source())?;
        RetainedDeclaration::from_retained_parts(source, anchor.span(), anchor.kind())
            .map_err(|reason| vec![Diagnostic::error(reason)])
    }

    fn admit_source(
        &self,
        admission: ProviderDefaultsAdmission<'_>,
        original: &SourceFile,
    ) -> Result<SourceFile, Vec<Diagnostic>> {
        match (&self.supplier, admission) {
            (
                ProviderDefaultSupplier::Package(bundle),
                ProviderDefaultsAdmission::Package(inputs),
            ) => inputs.admit_retained_product_source(bundle, original),
            (
                ProviderDefaultSupplier::CapturedToolchain { capture, .. },
                ProviderDefaultsAdmission::CapturedToolchain(current),
            ) => admit_toolchain_source(capture, current, original),
            _ => Err(wrong_supplier()),
        }
    }
}

fn admit_toolchain_source(
    original_capture: &CapturedLocalSource,
    current_capture: &CapturedLocalSource,
    original: &SourceFile,
) -> Result<SourceFile, Vec<Diagnostic>> {
    let refuse = |reason: &str| vec![Diagnostic::error(reason)];
    if original.origin != SourceOrigin::Toolchain
        || original.package_identity.is_some()
        || original.resolution_stratum != SourceResolutionStratum::Base
        || original.package_root != original_capture.canonical_live_root()
    {
        return Err(refuse(
            "toolchain defaults require original, nongenerated core source custody",
        ));
    }
    let relative = original
        .path
        .strip_prefix(original_capture.canonical_live_root())
        .map_err(|_| refuse("toolchain default declaration is outside its original core root"))?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(refuse(
            "toolchain default declaration has a noncanonical source path",
        ));
    }
    let captured = original_capture
        .tree()
        .read_to_string(relative)
        .map_err(|_| refuse("toolchain default declaration is absent from its closed capture"))?;
    if captured != original.source {
        return Err(refuse(
            "toolchain default declaration differs from its captured source",
        ));
    }
    let mut source = original.clone();
    source.path = current_capture.canonical_live_root().join(relative);
    source.package_root = current_capture.canonical_live_root().to_path_buf();
    Ok(source)
}

fn wrong_supplier() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "completed provider defaults were given a different supplier kind",
    )]
}
