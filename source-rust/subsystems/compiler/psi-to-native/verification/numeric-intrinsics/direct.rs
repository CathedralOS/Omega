//! Admit compiler-owned float declarations at the same target table used for native lowering.
use crate::AdmittedTerminalIntrinsics;
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, IeeeFloatFormat};
use target::{CanonicalFloatRealization, FloatIsaAdmission, TargetProfile};

pub fn direct_numeric_target(
    module: &terminal_psi::TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    target: &str,
) -> Result<target::CanonicalFloatRealization, Vec<Diagnostic>> {
    let fail = |reason: &str| vec![Diagnostic::error(reason)];
    terminal_semantics::numeric_primitives::validate_boundary(module, boundary).map_err(fail)?;
    let profile = target::TargetProfile::from_canonical_target_name(target)
        .map_err(|_| fail("numeric primitive has no recognized target"))?;
    let requirement = module
        .identities
        .text(boundary.identity)
        .filter(|identity| !identity.is_empty())
        .ok_or_else(|| fail("numeric primitive lost its exact declaration identity"))?;
    let entry = profile
        .canonical_float_realization(requirement)
        .ok_or_else(|| fail("numeric primitive has no canonical target realization entry"))?;
    if let target::CanonicalFloatRealization::Unsupported { constraint } = entry {
        return Err(vec![Diagnostic::error(format!(
            "canonical float operation `{requirement}` has no realization on target `{target}`: {constraint}"
        ))]);
    }
    Ok(entry)
}

impl AdmittedTerminalIntrinsics {
    /// Close the initializer's own reachable numeric calls, including those absent
    /// from the runtime entry's reach. Ordinary boundary providers remain separate.
    pub fn with_direct_numeric(
        mut self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
        demanded: &[BoundaryMachineId],
        profile: TargetProfile,
        x86_fma: Option<target::AdmittedX86ScalarFmaProvider>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let fail = |message: &str| vec![Diagnostic::error(message)];
        self.operations_for(&artifact.custody_identity())
            .map_err(fail)?;
        let module = artifact.module();
        for &slot in demanded {
            let boundary = module
                .boundary_machine(slot)
                .ok_or_else(|| fail("constant evaluation lost a demanded boundary"))?;
            if !boundary.declaration.as_ref().is_some_and(|declaration| {
                declaration.execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            }) {
                continue;
            }
            let entry = direct_numeric_target(module, boundary, profile.target_name())?;
            let operation =
                crate::signature::numeric_boundary_realization(module, boundary).map_err(fail)?;
            let mut operation =
                crate::signature::float_arithmetic_signature(operation, boundary, module)
                    .map_err(|reason| fail(&reason))?;
            if let CanonicalFloatRealization::CatalogIntrinsic {
                isa: FloatIsaAdmission::X86AvxFma3,
                ..
            } = entry
            {
                operation.x86_scalar_fma =
                    Some(deployed_fma(profile, operation.format, x86_fma).map_err(fail)?);
            }
            if let Some((_, selected)) = self.operations.iter_mut().find(|(id, _)| *id == slot) {
                if selected.format != operation.format || selected.operation != operation.operation
                {
                    return Err(fail(
                        "selected numeric provider disagrees with its compiler-owned declaration",
                    ));
                }
                *selected = operation;
            } else {
                self.operations.push((slot, operation));
            }
        }
        Ok(self)
    }
}

fn deployed_fma(
    profile: TargetProfile,
    format: IeeeFloatFormat,
    deployment: Option<target::AdmittedX86ScalarFmaProvider>,
) -> Result<target::AdmittedX86ScalarFmaProvider, &'static str> {
    let required = target::X86FeatureRequirement::scalar_fma(profile)
        .ok_or("constant FMA needs an x86 deployment profile")?;
    let slot = match format {
        IeeeFloatFormat::Binary32 => target::X86ScalarFmaSlot::Binary32,
        IeeeFloatFormat::Binary64 => target::X86ScalarFmaSlot::Binary64,
    };
    deployment
        .filter(|provider| provider.profile() == profile && provider.admits(required, slot))
        .ok_or("constant FMA requires the selected deployment to admit AVX and FMA3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_fma_requires_its_exact_deployment_features() {
        let linux = TargetProfile::LinuxX64;
        let provider = target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
            linux,
            &[
                target::X86TargetFeature::Avx,
                target::X86TargetFeature::Fma3,
            ],
        )
        .unwrap();
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            assert_eq!(deployed_fma(linux, format, Some(provider)), Ok(provider));
            assert!(deployed_fma(linux, format, None).is_err());
            assert!(deployed_fma(TargetProfile::WindowsX64, format, Some(provider)).is_err());
        }
    }
}
