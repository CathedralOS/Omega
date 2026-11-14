/*
We join checked external provider declarations from canonical Terminal Psi with
selected provider plans without consulting source declarations. Display names
serve only as consistency checks; exact overload identities, package ownership,
binding kinds, and deployment targets establish the requirement association and
reconcile portable numeric meanings with selected compiler intrinsics.
*/

use crate::CompilerIntrinsicExecutionIdentity;
use crate::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use terminal_psi::{ExternalProviderConformance, TerminalIdentityTable};

pub struct RequirementPlanIdentity<'a> {
    pub overload: &'a str,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub schema: &'a str,
    pub method: &'a str,
    pub owner: &'a str,
}

impl RequirementPlanIdentity<'_> {
    pub fn matches(
        &self,
        plan: &ProviderPlan,
        method: &crate::provider_plan::ServiceMethod,
        row: &ProviderPlanRow,
    ) -> bool {
        plan.schema.declaration.trait_package_identity == self.package
            && method.requirement_owner_package_identity == self.package
            && plan.schema.identity_text(method.requirement_identity) == self.overload
            && plan.identities.text(row.requirement_identity) == self.overload
            && plan.schema.row_binds_method(row, method, &plan.identities)
            && plan
                .schema
                .identity_text(plan.schema.declaration.trait_name)
                == self.schema
            && method.name == self.method
            && method.requirement_owner == self.owner
    }
}

pub fn matches_requirement_schema(
    candidate: &ExternalProviderConformance,
    identities: &TerminalIdentityTable,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> bool {
    let Some(schema) = &candidate.requirement_schema else {
        return false;
    };
    let ([method], [selected_row]) = (
        plan.schema.declaration.methods.as_slice(),
        plan.rows.as_slice(),
    ) else {
        return false;
    };
    let (Some(overload), Some(schema), Some(name), Some(owner)) = (
        identities.text(candidate.requirement_identity),
        identities.text(schema.identity),
        identities.text(schema.method),
        identities.text(schema.owner),
    ) else {
        return false;
    };
    selected_row == row
        && RequirementPlanIdentity {
            overload,
            package: candidate.requirement_package,
            schema,
            method: name,
            owner,
        }
        .matches(plan, method, row)
}

pub fn selected_numeric_provider<'a>(
    providers: &'a [ExternalProviderConformance],
    identities: &TerminalIdentityTable,
    requirement: &str,
    realization: Option<&str>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<Option<&'a ExternalProviderConformance>, String> {
    let ProviderBinding::CompilerIntrinsic { machine } = &row.binding else {
        return Err("numeric provider does not select an external compiler intrinsic".into());
    };
    let mut candidates = providers.iter().filter(|candidate| {
        identities.text(candidate.requirement.identity()) == Some(requirement)
            && realization.is_none_or(|realization| {
                identities.text(candidate.realization.machine.identity()) == Some(realization)
            })
            && matches_external_selection(candidate, identities, plan, row, machine)
    });
    let candidate = candidates.next();
    if let (Some(first), Some(second)) = (candidate, candidates.next()) {
        return Err(format!(
            "selected numeric provider does not rejoin one canonical external satisfaction: \
             plan `{}`, target `{}`, requirement `{requirement}`, selected realization {realization:?}; \
             first realization {:?}, semantic declaration {:?}, target {:?}, provider {:?}, record {first:?}; \
             second realization {:?}, semantic declaration {:?}, target {:?}, provider {:?}, record {second:?}",
            plan.name,
            plan.target,
            identities.text(first.realization.machine.identity()),
            identities.text(first.realization.semantic_declaration),
            first.target,
            first
                .provider
                .as_ref()
                .and_then(|provider| identities.text(provider.declaration.identity())),
            identities.text(second.realization.machine.identity()),
            identities.text(second.realization.semantic_declaration),
            second.target,
            second
                .provider
                .as_ref()
                .and_then(|provider| identities.text(provider.declaration.identity())),
        ));
    }
    if let Some(candidate) = candidate {
        candidate.validate(identities)?;
    }
    Ok(candidate)
}

pub fn matches_external_selection(
    candidate: &ExternalProviderConformance,
    identities: &TerminalIdentityTable,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    machine: &str,
) -> bool {
    candidate.numeric_intrinsic.is_some()
        && matches_requirement_schema(candidate, identities, plan, row)
        && identities.text(candidate.requirement_identity)
            == Some(plan.identities.text(row.requirement_identity))
        && candidate.requirement_package == plan.schema.declaration.trait_package_identity
        && candidate
            .target
            .as_ref()
            .is_none_or(|target| *target == plan.target)
        && candidate.binding == language_semantics::ExternalBindingIdentity::CompilerIntrinsic
        && identities.text(candidate.realization.callable) == Some(machine)
        && candidate.realization_package == plan.origin_package_identity
        && match &candidate.provider {
            Some(provider) => {
                identities.text(provider.identity) == Some(plan.provider_type.as_str())
                    && provider.package == plan.provider_type_package_identity
            }
            None => plan.provider_type.is_empty() && plan.provider_type_package_identity.is_none(),
        }
}

pub fn reconcile_numeric_meaning(
    meaning: Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
    selected: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<(), &'static str> {
    use CompilerIntrinsicExecutionIdentity as Execution;
    match (
        meaning,
        selected,
        selected.is_some_and(Execution::is_numeric),
    ) {
        (Some(meaning), Some(execution), _)
            if meaning.signature().is_some() && Execution::from(meaning) == execution =>
        {
            Ok(())
        }
        (None, _, false) => Ok(()),
        _ => Err("numeric meaning disagrees with selected provider custody"),
    }
}

/// Project an admitted native declaration's meaning without reopening source.
/// Unsupported targets refuse here; the numeric declaration path is unchanged.
pub fn canonical_external_execution(
    declaration: &ExternalProviderConformance,
    target: &str,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, String> {
    if declaration
        .target
        .as_deref()
        .is_some_and(|declared| declared != target)
    {
        return Err("native intrinsic declaration belongs to another target".into());
    }
    if (declaration.sealed_intrinsic.is_some() || declaration.numeric_intrinsic.is_some())
        && declaration.binding != language_semantics::ExternalBindingIdentity::CompilerIntrinsic
    {
        return Err("canonical intrinsic meaning has another native binding mechanism".into());
    }
    if declaration.sealed_intrinsic.is_some() && declaration.numeric_intrinsic.is_some() {
        return Err("native intrinsic has two different canonical meanings".into());
    }
    if let Some(numeric) = declaration.numeric_intrinsic {
        return Ok(Some(numeric.into()));
    }
    declaration
        .sealed_intrinsic
        .map(|meaning| sealed_native_execution(meaning, target))
        .transpose()
}

pub fn sealed_native_execution(
    meaning: terminal_psi::TerminalSealedNativeIntrinsic,
    target: &str,
) -> Result<CompilerIntrinsicExecutionIdentity, String> {
    use CompilerIntrinsicExecutionIdentity as Execution;
    use language_core::sealed_intrinsics::{FixedImageOperation, HostedOperation};
    use terminal_psi::TerminalSealedNativeIntrinsic;
    Ok(match meaning {
        TerminalSealedNativeIntrinsic::Hosted(operation) => {
            if target::HostedIntrinsicBundle::from_target_name(target).is_none() {
                return Err("hosted intrinsic has no closed realization for this target".into());
            }
            match operation {
                HostedOperation::ConsoleExitProcess | HostedOperation::ProcessExit => {
                    Execution::HostedExitProcessI32
                }
                HostedOperation::ConsoleReadByte => Execution::HostedReadByte,
                HostedOperation::ConsoleWriteByte => Execution::HostedWriteByteI32,
            }
        }
        TerminalSealedNativeIntrinsic::FixedImage(operation) => {
            if target != "uefi_x86_64" {
                return Err("fixed-image intrinsic belongs to another target".into());
            }
            match operation {
                FixedImageOperation::ProgramStorageEntry => Execution::ProgramStorageEntry,
                FixedImageOperation::ProgramImageRetainedSuffixStart => {
                    Execution::ProgramImageRetainedSuffixStart
                }
            }
        }
    })
}

/// A physical selection must keep the meaning carried by its exact checked row.
pub fn reconcile_sealed_execution(
    expected: CompilerIntrinsicExecutionIdentity,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<CompilerIntrinsicExecutionIdentity, &'static str> {
    if retained.is_some_and(|retained| retained != expected) {
        return Err("retained compiler execution differs from the admitted sealed native meaning");
    }
    Ok(expected)
}

pub fn selected_sealed_native_provider<'a>(
    providers: &'a [ExternalProviderConformance],
    identities: &TerminalIdentityTable,
    required_or_realized_declaration: &str,
    realization: Option<&str>,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
) -> Result<Option<&'a ExternalProviderConformance>, String> {
    let ProviderBinding::CompilerIntrinsic { machine } = &row.binding else {
        return Err("selected intrinsic row has another binding mechanism".into());
    };
    let mut rows = providers.iter().filter(|candidate| {
        candidate.sealed_intrinsic.is_some()
            && candidate.binding == language_semantics::ExternalBindingIdentity::CompilerIntrinsic
            && identities.text(candidate.realization.callable) == Some(machine.as_str())
            && (identities.text(candidate.requirement.identity())
                == Some(required_or_realized_declaration)
                || identities.text(candidate.realization.machine.identity())
                    == Some(required_or_realized_declaration))
            && realization.is_none_or(|realization| {
                identities.text(candidate.realization.machine.identity()) == Some(realization)
            })
            && candidate.realization_package == plan.origin_package_identity
            && candidate
                .target
                .as_deref()
                .is_none_or(|target| target == plan.target)
            && match &candidate.provider {
                Some(provider) => {
                    identities.text(provider.identity) == Some(plan.provider_type.as_str())
                        && provider.package == plan.provider_type_package_identity
                }
                None => {
                    plan.provider_type.is_empty() && plan.provider_type_package_identity.is_none()
                }
            }
    });
    let row = rows.next();
    if rows.next().is_some() {
        return Err("selected sealed intrinsic has competing checked external declarations".into());
    }
    if let Some(row) = row {
        row.validate(identities)?;
    }
    Ok(row)
}

#[cfg(test)]
mod sealed_execution_tests {
    use super::{reconcile_sealed_execution, sealed_native_execution};
    use crate::CompilerIntrinsicExecutionIdentity as Execution;
    use language_core::sealed_intrinsics::{FixedImageOperation, HostedOperation};
    use terminal_psi::TerminalSealedNativeIntrinsic as Meaning;

    #[test]
    fn selected_sealed_execution_preserves_exact_meaning_and_target() {
        let exit = sealed_native_execution(
            Meaning::Hosted(HostedOperation::ConsoleExitProcess),
            "macos_arm64",
        )
        .unwrap();
        assert_eq!(exit, Execution::HostedExitProcessI32);
        assert_eq!(reconcile_sealed_execution(exit, Some(exit)).unwrap(), exit);
        assert!(reconcile_sealed_execution(exit, Some(Execution::HostedWriteByteI32)).is_err());
        assert_eq!(reconcile_sealed_execution(exit, None).unwrap(), exit);
        assert!(
            sealed_native_execution(
                Meaning::Hosted(HostedOperation::ConsoleExitProcess),
                "uefi_x86_64"
            )
            .is_err()
        );
        assert!(
            sealed_native_execution(
                Meaning::FixedImage(FixedImageOperation::ProgramStorageEntry),
                "macos_arm64"
            )
            .is_err()
        );
        assert_eq!(
            sealed_native_execution(
                Meaning::FixedImage(FixedImageOperation::ProgramStorageEntry),
                "uefi_x86_64"
            )
            .unwrap(),
            Execution::ProgramStorageEntry
        );
    }
}
