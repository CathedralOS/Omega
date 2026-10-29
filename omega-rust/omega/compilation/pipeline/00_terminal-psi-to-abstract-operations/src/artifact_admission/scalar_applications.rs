//! Bind completed scalar checking to this native deployment without reopening
//! source or executing code. Full provider custody is distinct from the local
//! native work domain, which may omit separately compiled component providers.

use crate::native_execution_scope::NativeExecutionScope;
use terminal_installation::{
    AdmittedScalarApplications, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
};

/// Completed checking and the whole selected provider companion it consumed.
/// Neither values alone nor the native scope's local adapter subset can replace
/// this deployment binding.
pub struct NativeScalarApplications<'request> {
    pub completed: &'request AdmittedScalarApplications,
    pub provider_declarations: &'request [backend_plan::TerminalTargetProviderPlan],
}

impl<'request> NativeScalarApplications<'request> {
    pub(super) fn bind(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        targets: &AdmittedTargetMachineSelections,
        layouts: &AdmittedTargetLayouts,
        selection: &super::NativeExecutionSelection<'_>,
        scope: &NativeExecutionScope,
    ) -> Result<&'request AdmittedScalarApplications, String> {
        let completed = self.completed;
        let primary = completed.primary();
        if completed.terminal() != artifact.view().identity()
            || primary.targets() != targets
            || primary.layouts() != layouts
        {
            return Err(
                "native scalar closure belongs to another artifact, target or layout selection"
                    .into(),
            );
        }
        for inputs in std::iter::once(primary).chain(completed.supplemental()) {
            let Some(model) = inputs.address_model() else {
                continue;
            };
            let profile =
                target::TargetProfile::from_canonical_target_name(inputs.targets().target())
                    .map_err(|error| format!("scalar closure address-model target: {error:?}"))?;
            let expected = target::TargetSemantics::close_address_model(profile)
                .ok_or("scalar closure requires an unavailable address model")?;
            if model.target != profile.target_name()
                || model.carrier != expected.carrier()
                || model.semantics_version != expected.semantics_version()
                || model.commitment != expected.commitment()
            {
                return Err(format!(
                    "scalar closure for {} has a different target address model",
                    inputs.targets().target(),
                ));
            }
        }
        let providers = backend_plan::admit_selected_checked_providers(
            artifact.view(),
            profile.target_name(),
            selection.plans,
            self.provider_declarations,
        )?;
        if primary.providers() != &providers {
            return Err(
                "native scalar closure differs from exact provider installation custody".into(),
            );
        }
        let full_adapters = crate::provider_admission::retain_selected_provider_adapters(
            selection.plans,
            self.provider_declarations,
        )?;
        if selection
            .providers
            .iter()
            .any(|adapter| !full_adapters.contains(adapter))
        {
            return Err("native local provider differs from the completed scalar selection".into());
        }
        // Ordinary native calls may reach a generated entry even when no
        // ReadScalarApplication operation demands its final value. The entry's
        // own condition debt must already have completed under this admission.
        let module = artifact.semantic_module();
        let catalog = &module.scalar_applications;
        for (position, declaration) in catalog.declarations().iter().enumerate() {
            let terminal_psi::ScalarApplication::Evaluate { entry, .. } = declaration else {
                continue;
            };
            let machine = match entry {
                terminal_psi::TargetMachineCallable::Machine(machine) => *machine,
                terminal_psi::TargetMachineCallable::Boundary(_) => {
                    let Ok(member) = targets.member(*entry) else {
                        continue;
                    };
                    let terminal_psi::TargetMachineCallable::Machine(machine) = member.callable()
                    else {
                        continue;
                    };
                    machine
                }
            };
            if !scope.contains_machine(machine) {
                continue;
            }
            let application = u64::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .and_then(semantic_vocabulary::ScalarApplicationId::new)
                .ok_or("native scalar application coordinate exceeds its catalog")?;
            if catalog.conditions.contains_subject(
                terminal_psi::ScalarConditionSubject::Application(application),
            ) && completed.values().value(application).is_none()
            {
                return Err(format!(
                    "native execution invokes unchecked scalar entry {application}"
                ));
            }
        }
        Ok(completed)
    }
}
