//! Runtime plans with their published component products and receipts.
use std::sync::Arc;
use target::NormalizedImportLocator;
use terminal_installation::{AdmittedScalarApplications, CheckedPrivateEntryRamRecipe};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::TerminalPsiIdentity;
use backend_plan::TerminalTargetProviderProposal;

pub use native_runtime_provisions::TerminalRuntimeHostProvision;
pub type TerminalRuntimeHostProvisions<A = Arc<image_emission::NativeForeignAdapterImage>> =
    native_runtime_provisions::TerminalRuntimeHostProvisions<A>;
pub type TerminalCallableRuntimeProvisions<A = Arc<image_emission::NativeForeignAdapterImage>> =
    native_runtime_provisions::TerminalCallableRuntimeProvisions<A>;

#[derive(Debug, Clone)]
pub struct TerminalRuntimeProvisions<A = Arc<image_emission::NativeForeignAdapterImage>> {
    plan: native_runtime_provisions::NativeRuntimePlan<A>,
    components: Vec<Arc<crate::NativeComponentProduct>>,
    component_publications: Vec<crate::NativeComponentPublicationReceipt>,
}

impl<A> TerminalRuntimeProvisions<A> {
    pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
        self.plan.scalar_applications()
    }

    pub fn private_entry_ram(&self) -> &[CheckedPrivateEntryRamRecipe] {
        self.plan.private_entry_ram()
    }

    pub const fn host_provisions(&self) -> &TerminalRuntimeHostProvisions<A> {
        self.plan.host_provisions()
    }

    pub fn matches_host_requirement(
        &self,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        binding: &target_operations::NormalizedForeignCallBinding,
        identities: &terminal_psi::TerminalIdentityTable,
    ) -> bool {
        self.plan.matches_host_requirement(declaration, binding, identities)
    }

    pub fn host_calling_application(
        &self,
        boundary: BoundaryMachineId,
    ) -> Option<&calling_conventions::BoundaryCallingApplication> {
        self.plan.host_calling_application(boundary)
    }

    pub const fn identity(&self) -> TerminalPsiIdentity {
        self.plan.identity()
    }

    pub fn target_provider_proposals(&self) -> &[TerminalTargetProviderProposal] {
        self.plan.target_provider_proposals()
    }

    pub fn borrowed_storage_layouts(&self) -> &[runtime_abi::NativeBorrowedStorageLayout] {
        self.plan.borrowed_storage_layouts()
    }

    pub const fn entry(&self) -> MachineId {
        self.plan.entry()
    }

    pub const fn source_signature_identity(&self) -> [u8; 32] {
        self.plan.source_signature_identity()
    }

    pub const fn target_slot(&self) -> target::ProgramEntrySlotDeclaration {
        self.plan.target_slot()
    }

    pub const fn selection_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        self.plan.selection_identities()
    }

    pub const fn installation(&self) -> Option<&terminal_installation::AdmittedProviderSelections> {
        self.plan.installation()
    }

    pub fn host(&self) -> &[TerminalRuntimeHostProvision] {
        self.plan.host()
    }

    pub fn from_plan(plan: native_runtime_provisions::NativeRuntimePlan<A>) -> Self {
        Self { plan, components: Vec::new(), component_publications: Vec::new() }
    }
    pub fn map_adapters<B>(self, transform: impl FnMut(A) -> Result<B, String>) -> TerminalRuntimeProvisions<B> {
        TerminalRuntimeProvisions {
            plan: self.plan.map_adapters(transform),
            components: self.components,
            component_publications: self.component_publications,
        }
    }
    pub fn with_private_entry_ram(mut self, recipes: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>) -> Self {
        self.plan = self.plan.with_private_entry_ram(recipes);
        self
    }
    pub fn with_target_provider_proposals(mut self, proposals: Vec<backend_plan::TerminalTargetProviderProposal>) -> Self {
        self.plan = self.plan.with_target_provider_proposals(proposals);
        self
    }
    pub fn with_borrowed_storage_layouts(mut self, layouts: runtime_abi::NativeBorrowedStorageLayouts) -> Self {
        self.plan = self.plan.with_borrowed_storage_layouts(layouts);
        self
    }

    pub fn record_component_publications(
        &mut self,
        receipts: Vec<crate::NativeComponentPublicationReceipt>,
    ) -> Result<(), &'static str> {
        if !self.component_publications.is_empty() || receipts.len() != self.components.len() {
            return Err(
                "component publication must cover the current immutable product roster exactly once",
            );
        }
        for (receipt, component) in receipts.iter().zip(&self.components) {
            if receipt.product().locator() != component.locator()
                || receipt.product().selection() != component.selection()
                || receipt.product().application() != component.application()
                || receipt.product().artifact().manifest().identity()
                    != component.artifact().manifest().identity()
                || receipt.product().authority().review().identity()
                    != component.authority().review().identity()
            {
                return Err(
                    "component publication receipt belongs to another exact producing image or historical input graph",
                );
            }
        }
        self.component_publications = receipts;
        Ok(())
    }

    pub fn validate_component_process_inputs(&self, root: &std::path::Path) -> Result<(), String> {
        self.validate_component_product_roster()
            .map_err(str::to_owned)?;
        if self.components.is_empty() {
            return Ok(());
        }
        if self.component_publications.len() != self.components.len() {
            return Err(
                "component process has no complete exact publication receipt roster".into(),
            );
        }
        let directory = root
            .parent()
            .ok_or("component process root has no publication directory")?;
        for receipt in &self.component_publications {
            if receipt.path().parent() != Some(directory) {
                return Err("component process dependency is outside the selected root's adjacent immutable publication graph".into());
            }
            receipt.revalidate()?;
        }
        Ok(())
    }
    pub fn component_products(&self) -> &[Arc<crate::NativeComponentProduct>] {
        &self.components
    }
    pub fn with_component_products(
        mut self,
        components: Vec<Arc<crate::NativeComponentProduct>>,
    ) -> Result<Self, &'static str> {
        self.validate_component_products(&components)?;
        self.components = components;
        Ok(self)
    }

    pub fn validate_root_object(
        object: &image_emission::ObjectArtifact,
        runtime: Option<&Self>,
    ) -> Result<(), &'static str> {
        match runtime {
            Some(runtime) => runtime.validate_component_root_object(object),
            None if object
                .object()
                .layout
                .normalized_imports
                .iter()
                .any(|import| matches!(import.locator, NormalizedImportLocator::Component(_))) =>
            {
                Err(
                    "component native product has no complete retained runtime image/resource roster",
                )
            }
            None => Ok(()),
        }
    }

    pub fn validate_component_root_object(
        &self,
        object: &image_emission::ObjectArtifact,
    ) -> Result<(), &'static str> {
        if installation_evidence::ObjectEvidence::psi(object) != self.plan.identity()
            || object.target() != self.plan.target_slot().owner.native_target()
        {
            return Err("component runtime provisions name another actual root artifact or target");
        }
        self.validate_component_product_roster()?;
        for import in &object.object().layout.normalized_imports {
            let NormalizedImportLocator::Component(locator) = &import.locator else {
                continue;
            };
            let mut products = self
                .components
                .iter()
                .filter(|product| product.locator() == locator);
            if products.next().is_none() || products.next().is_some() {
                return Err(
                    "actual component root import has no unique exact retained image/resource product",
                );
            }
        }
        for call in object.foreign_calls() {
            if !matches!(
                &call.binding.mechanism,
                target_operations::NormalizedForeignCallMechanism::Component { .. }
            ) {
                continue;
            }
            if !self.host().iter().any(|provision| {
                matches!(&provision.realization,
                    target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding)
                        if binding == &call.binding)
            }) {
                return Err(
                    "actual component root call differs from its complete runtime ABI, provider or same-stack settlement",
                );
            }
        }
        Ok(())
    }

    pub fn validate_component_product_roster(&self) -> Result<(), &'static str> {
        self.validate_component_products(&self.components)
    }
    fn validate_component_products(
        &self,
        components: &[Arc<crate::NativeComponentProduct>],
    ) -> Result<(), &'static str> {
        let mut expected = 0;
        for provision in self.host() {
            let target_operations::BoundarySettlementRealization::NormalizedForeignCall(binding) =
                &provision.realization
            else {
                continue;
            };
            let target_operations::NormalizedForeignCallMechanism::Component {
                locator,
                provider,
                candidate,
                application_commitment,
                same_stack_contribution,
            } = &binding.mechanism
            else {
                continue;
            };
            expected += 1;
            let mut matches = components.iter().filter(|component| {
                component.selection().receiver()
                    == &terminal_codec::TerminalExecutionIdentity::portable(self.plan.identity())
                    && component.selection().receiving_boundary() == provision.boundary
                    && component.selection().provider() == *provider
                    && component.selection().candidate() == *candidate
                    && component.locator() == locator
                    && component.application().commitment == *application_commitment
                    && component.application().plan.plan() == &binding.boundary_entry_plan
            });
            let component = matches.next().ok_or(
                "component runtime roster does not exactly cover an admitted independent native call",
            )?;
            if matches.next().is_some() {
                return Err(
                    "component runtime roster ambiguously duplicates an admitted independent native call",
                );
            }
            component.validate_same_stack(same_stack_contribution)?;
        }
        if expected != components.len() {
            return Err("component runtime roster contains an unconsumed or duplicated image");
        }
        Ok(())
    }
}
