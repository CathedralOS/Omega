//! Owned independent-component publication and reference-execution companions.
//!
//! A product retains the actual separately emitted image and canonical provider
//! artifact beside its exact receiver/export selection. This is neither an
//! ExecutableImage nor InstalledCode: filesystem publication and process-lifetime
//! loader/instance admission remain explicit consumers of these inputs.

use std::sync::Arc;
use terminal_psi_to_abstract_operations::{
    AdmittedNativeComponentCall, provider_admission::AdmittedNativeCallableAuthority,
};

pub struct NativeComponentProduct {
    image: image_emission::ComponentSharedImage,
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    selection: terminal_installation::AdmittedComponentProvider,
    application: calling_conventions::BoundaryCallingApplication,
    runtime: crate::TerminalCallableRuntimeProvisions,
    authority: AdmittedNativeCallableAuthority,
    locator: target::NormalizedComponentLocator,
}
impl std::fmt::Debug for NativeComponentProduct {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeComponentProduct")
            .field("selection", &self.selection)
            .field("locator", &self.locator)
            .finish()
    }
}
impl NativeComponentProduct {
    pub fn from_emitted(
        call: &AdmittedNativeComponentCall<'_>,
        image: image_emission::ComponentSharedImage,
        runtime: crate::TerminalCallableRuntimeProvisions,
        authority: AdmittedNativeCallableAuthority,
    ) -> Result<Self, &'static str> {
        let selection = call.selection();
        let application = call.application();
        if selection.provider() != authority.subject()
            || selection.provider() != runtime.identity()
            || selection.candidate() != authority.entry()
            || selection.candidate() != runtime.entry()
            || image.body().entry() != selection.candidate()
            || authority.review().terminal_artifact_identity()
                != *call.component().artifact().manifest().identity().as_bytes()
            || authority.review().target() != application.target
            || runtime.target().native_target() != application.target
            || application.recomputed_commitment().ok() != Some(application.commitment)
            || image.slot_commitment()
                != &target::component_export_slot(
                    selection.provider().vocabulary_marker.get(),
                    selection.provider().program_fingerprint.as_bytes(),
                    &application.commitment,
                )
        {
            return Err(
                "component publication product lost its admitted artifact, entry, target or complete ABI application",
            );
        }
        let demand = image_emission::derive_stack_demand(image.body(), image.body().entry())
            .map_err(|_| "component publication lost its exact physical stack evidence")?;
        if demand.psi() != selection.provider() || demand.target() != application.target {
            return Err("component image body belongs to another artifact or native target");
        }
        let locator = target::NormalizedComponentLocator::for_image(
            runtime.target(),
            image.image().contents(),
            *image.slot_commitment(),
        )?;
        Ok(Self {
            image,
            artifact: Arc::new(call.component().artifact().clone()),
            selection: selection.clone(),
            application: application.clone(),
            runtime,
            authority,
            locator,
        })
    }
    pub const fn image(&self) -> &image_emission::ComponentSharedImage {
        &self.image
    }
    pub const fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        &self.artifact
    }
    pub const fn selection(&self) -> &terminal_installation::AdmittedComponentProvider {
        &self.selection
    }
    pub const fn application(&self) -> &calling_conventions::BoundaryCallingApplication {
        &self.application
    }
    pub const fn runtime(&self) -> &crate::TerminalCallableRuntimeProvisions {
        &self.runtime
    }
    pub const fn locator(&self) -> &target::NormalizedComponentLocator {
        &self.locator
    }
    pub const fn authority(&self) -> &AdmittedNativeCallableAuthority {
        &self.authority
    }
    pub fn verify_contents(&self, contents: &[u8]) -> Result<(), &'static str> {
        if target::NormalizedComponentLocator::for_image(
            self.locator.target(),
            contents,
            *self.locator.slot_commitment(),
        )? != self.locator
        {
            return Err("component file differs from its exact admitted content-addressed image");
        }
        Ok(())
    }
}

/// Exact adjacent component files accepted for one process-lifetime graph.
/// This receipt records publication only. The explicit process runtime still
/// validates the complete root graph immediately before asking the OS loader
/// to create its isolated process; this never grants InstalledCode authority.
#[derive(Debug, Clone)]
pub struct NativeComponentPublicationReceipt {
    product: Arc<NativeComponentProduct>,
    path: std::path::PathBuf,
}
impl NativeComponentPublicationReceipt {
    pub fn verify(
        product: Arc<NativeComponentProduct>,
        path: std::path::PathBuf,
    ) -> Result<Self, String> {
        if path.file_name().and_then(|name| name.to_str())
            != Some(product.locator().image_filename().as_str())
        {
            return Err(
                "component publication path differs from its exact content-addressed filename"
                    .into(),
            );
        }
        let result = Self { product, path };
        result.revalidate()?;
        Ok(result)
    }
    pub const fn product(&self) -> &Arc<NativeComponentProduct> {
        &self.product
    }
    pub const fn path(&self) -> &std::path::PathBuf {
        &self.path
    }
    pub fn revalidate(&self) -> Result<(), String> {
        let contents = std::fs::read(&self.path)
            .map_err(|error| format!("component publication {}: {error}", self.path.display()))?;
        self.product
            .verify_contents(&contents)
            .map_err(str::to_owned)
    }
}
