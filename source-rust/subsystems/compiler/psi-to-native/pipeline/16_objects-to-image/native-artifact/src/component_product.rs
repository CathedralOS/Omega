/*
We retain separately emitted component shared images alongside their canonical
Terminal provider artifact, admitted receiver/export selection, ABI calling
application, stack demand, and content-addressed `NormalizedComponentLocator`.
`NativeComponentProduct` and `NativeComponentPublicationReceipt` verify that
published `.component` description files and adjacent shared images match the
admitted compilation outputs without granting `InstalledCode` authority.
*/

use component_description::component_description::StackDemandFacts;
use provider_admission::provider_admission::AdmittedNativeCallableAuthority;
use std::path::PathBuf;
use std::sync::Arc;
use verified_optimization_input::AdmittedNativeComponentCall;

pub struct NativeComponentProduct {
    image: image_emission::ComponentSharedImage,
    description: Vec<u8>,
    runtime: crate::TerminalCallableRuntimeProvisions,
    runtime_record: Arc<published_runtime_input::RuntimeComponent>,
}
impl std::fmt::Debug for NativeComponentProduct {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeComponentProduct")
            .field("selection", self.selection())
            .field("locator", self.locator())
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
        if image.body().psi() != selection.provider()
            || image.body().entry() != selection.candidate()
            || image.body().target() != call.application().target
        {
            return Err("component image body belongs to another artifact or native target".into());
        }
        let demand = image
            .stack_demand()
            .map_err(|error| format!("component publication stack evidence: {error:?}"))?;
        let runtime_record = published_runtime_input::RuntimeComponent::admit(
            call,
            runtime
                .clone()
                .map_adapters(|adapter| Ok(Arc::new(adapter.runtime_identity()))),
            authority,
            &demand,
            image.image().contents(),
            *image.slot_commitment(),
        )?;
        let (_, description) = component_description::publish_component_description(
            component_description::ComponentDescriptionFacts {
                artifact: call.component().artifact(),
                selected_provider_plans,
                stack_demand: Some(StackDemandFacts {
                    entry: selection.candidate(),
                    ceiling_bytes: runtime_record.stack_bytes(),
                    stack_alignment: runtime_record.stack_alignment(),
                }),
                realization_identity: Some(*runtime_record.locator().identity_digest()),
            },
        )
        .map_err(|error| format!("native component description publication: {error}"))?;
        Ok(Self {
            image,
            description,
            runtime,
            runtime_record: Arc::new(runtime_record),
        })
    }
    pub const fn image(&self) -> &image_emission::ComponentSharedImage {
        &self.image
    }
    pub fn description(&self) -> &[u8] {
        &self.description
    }
    pub fn description_filename(&self) -> String {
        format!("{}.component", self.locator().image_filename())
    }
    pub fn verify_description(&self, description: &[u8]) -> Result<(), &'static str> {
        if description != self.description {
            return Err("component description differs from its exact native publication product");
        }
        Ok(())
    }
    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        self.runtime_record.artifact()
    }
    pub fn selection(&self) -> &terminal_installation::AdmittedComponentProvider {
        self.runtime_record.selection()
    }
    pub fn application(&self) -> &calling_conventions::BoundaryCallingApplication {
        self.runtime_record.application()
    }
    pub const fn runtime(&self) -> &crate::TerminalCallableRuntimeProvisions {
        &self.runtime
    }
    pub fn locator(&self) -> &target::NormalizedComponentLocator {
        self.runtime_record.locator()
    }
    pub fn authority(&self) -> &AdmittedNativeCallableAuthority {
        self.runtime_record.authority()
    }
    pub(crate) fn runtime_record(&self) -> &Arc<published_runtime_input::RuntimeComponent> {
        &self.runtime_record
    }
    pub fn validate_same_stack(
        &self,
        contribution: &provider_stack_contribution::AdmittedSameStackContribution,
    ) -> Result<(), &'static str> {
        self.runtime_record.validate_same_stack(contribution)
    }

    pub fn verify_contents(&self, contents: &[u8]) -> Result<(), &'static str> {
        if target::NormalizedComponentLocator::for_image(
            self.locator().target(),
            contents,
            *self.locator().slot_commitment(),
        )? != *self.locator()
        {
            return Err("component file differs from its exact admitted content-addressed image");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NativeComponentPublicationReceipt {
    product: Arc<NativeComponentProduct>,
    path: PathBuf,
}
impl NativeComponentPublicationReceipt {
    pub fn verify(product: Arc<NativeComponentProduct>, path: PathBuf) -> Result<Self, String> {
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
    pub const fn path(&self) -> &PathBuf {
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
