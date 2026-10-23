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
    description: Vec<u8>,
    artifact: Arc<terminal_codec::CanonicalTerminalArtifact>,
    selection: terminal_installation::AdmittedComponentProvider,
    application: calling_conventions::BoundaryCallingApplication,
    runtime: crate::TerminalCallableRuntimeProvisions,
    authority: AdmittedNativeCallableAuthority,
    locator: target::NormalizedComponentLocator,
    provider_plan: effects::provider_plan::ProviderPlanDigest,
    stack_bytes: u64,
    stack_alignment: u32,
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
        selected_provider_plans: &effects::SelectedProviderPlanFacts,
    ) -> Result<Self, String> {
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
                "component publication product lost its admitted artifact, entry, target or complete ABI application".into(),
            );
        }
        let demand = image
            .stack_demand()
            .map_err(|error| format!("component publication stack evidence: {error:?}"))?;
        if demand.psi() != selection.provider()
            || demand.entry() != selection.candidate()
            || demand.target() != application.target
        {
            return Err("component image body belongs to another artifact or native target".into());
        }
        let locator = target::NormalizedComponentLocator::for_image(
            runtime.target(),
            image.image().contents(),
            *image.slot_commitment(),
        )?;
        let (_, description) = component_description::publish_component_description(
            component_description::ComponentDescriptionFacts {
                artifact: call.component().artifact(),
                selected_provider_plans,
                stack_demand: Some(
                    component_description::component_description::StackDemandFacts {
                        entry: demand.entry(),
                        ceiling_bytes: demand.ceiling_bytes(),
                        stack_alignment: demand.stack_alignment(),
                    },
                ),
                realization_identity: Some(*locator.identity_digest()),
            },
        )
        .map_err(|error| format!("native component description publication: {error}"))?;
        Ok(Self {
            image,
            description,
            artifact: Arc::new(call.component().artifact().clone()),
            selection: selection.clone(),
            application: application.clone(),
            runtime,
            authority,
            locator,
            provider_plan: *call.plan_digest(),
            stack_bytes: demand.ceiling_bytes(),
            stack_alignment: demand.stack_alignment(),
        })
    }
    pub const fn image(&self) -> &image_emission::ComponentSharedImage {
        &self.image
    }
    /// Canonical descriptive bytes for this exact emitted entry and container.
    /// These rows demand stack provision; they do not supply it or authorize entry.
    pub fn description(&self) -> &[u8] {
        &self.description
    }
    pub fn description_filename(&self) -> String {
        format!("{}.component", self.locator.image_filename())
    }
    /// Publication binds metadata to the producing product, including facts a
    /// Terminal-only consumer cannot reconstruct from semantic code alone.
    pub fn verify_description(&self, description: &[u8]) -> Result<(), &'static str> {
        if description != self.description {
            return Err("component description differs from its exact native publication product");
        }
        Ok(())
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
    /// Join a symbolic settlement to this sealed emitted image, including its
    /// body and entry-bridge resource demand. A genuinely admitted smaller
    /// contribution or another plan cannot borrow this product's authority.
    pub fn validate_same_stack(
        &self,
        contribution: &task_plans::AdmittedSameStackContribution,
    ) -> Result<(), &'static str> {
        if contribution.provider_plan_commitment()
            != task_plans::SameStackProviderPlanCommitment::from_digest(
                *self.provider_plan.as_bytes(),
            )
            || contribution.bytes() != self.stack_bytes
            || contribution.alignment() != u64::from(self.stack_alignment)
        {
            return Err(
                "component same-stack contribution differs from its actual image demand or exact provider plan",
            );
        }
        Ok(())
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
            .map_err(str::to_owned)?;
        let description_path = self
            .path
            .with_file_name(self.product.description_filename());
        let description = std::fs::read(&description_path).map_err(|error| {
            format!(
                "component description publication {}: {error}",
                description_path.display()
            )
        })?;
        self.product
            .verify_description(&description)
            .map_err(str::to_owned)
    }
}
