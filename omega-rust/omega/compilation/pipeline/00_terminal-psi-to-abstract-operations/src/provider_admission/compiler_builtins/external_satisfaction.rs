//! Join selected external realizations to canonical satisfaction and exact owners.

use effects::CompilerIntrinsicExecutionIdentity;
use effects::provider_plan::ProviderPlan;

/// Join the selected external leaf to Psi's exact satisfaction edge. Numeric
/// meaning alone cannot authorize execution; a same-signature provider or a
/// declaration from another package cannot replace the selected realization.
pub(crate) fn selected_execution(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    requirement: &terminal_psi::NominalDeclarationIdentity,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    retained: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<Option<CompilerIntrinsicExecutionIdentity>, &'static str> {
    let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
    else {
        return Err("numeric provider does not select an external compiler intrinsic");
    };
    let mut candidates = external_providers.iter().filter(|candidate| {
        candidate.requirement == *requirement
            && matches_external_selection(candidate, identities, plan, row, machine)
    });
    let candidate = candidates.next();
    if candidates.next().is_some() {
        return Err(
            "selected numeric provider does not rejoin one canonical external satisfaction",
        );
    }
    let Some(candidate) = candidate else {
        reconcile_numeric_meaning(None, retained)?;
        return Ok(retained);
    };
    candidate.validate(identities)?;
    let meaning = candidate
        .numeric_intrinsic
        .ok_or("numeric provider has no canonical meaning")?;
    let execution = CompilerIntrinsicExecutionIdentity::from(meaning);
    // A source-review consumer can still supply its independently retained row.
    // It cannot replace the portable meaning. Native persistence omits it.
    if retained.is_some() {
        reconcile_numeric_meaning(Some(meaning), retained)?;
    }
    Ok(Some(execution))
}

fn matches_external_selection(
    candidate: &terminal_psi::ExternalProviderConformance,
    identities: &terminal_psi::TerminalIdentityTable,
    plan: &ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
    machine: &str,
) -> bool {
    candidate.numeric_intrinsic.is_some()
        && identities.text(candidate.requirement_identity)
            == Some(plan.identities.text(row.requirement_identity))
        && candidate.requirement_package == plan.schema.trait_package_identity
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

/// Rejoin exact selecting owners at native proposal ingress, including private
/// record recovery. Later execution borrows this same immutable catalog and
/// selected plan; it does not need another parallel declaration sidecar.
pub(crate) fn validate_external_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<(), &'static str> {
    for selected in declarations {
        for row in &selected.plan.rows {
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } =
                &row.binding
            else {
                continue;
            };
            for candidate in external_providers.iter().filter(|candidate| {
                matches_external_selection(candidate, identities, &selected.plan, row, machine)
            }) {
                let owner = candidate
                    .provider
                    .as_ref()
                    .and_then(|provider| identities.text(provider.declaration.identity()));
                if owner != selected.provider_declaration.as_deref() {
                    return Err("selected external provider has a different exact declaring owner");
                }
            }
        }
    }
    Ok(())
}

/// Portable meaning never replaces selected-provider custody. In particular,
/// a forged numeric boundary cannot turn an unsupported selected row into an
/// executable provider, and a numeric provider cannot omit its portable meaning.
pub(super) fn reconcile_numeric_meaning(
    meaning: Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
    selected: Option<CompilerIntrinsicExecutionIdentity>,
) -> Result<(), &'static str> {
    use CompilerIntrinsicExecutionIdentity as Execution;
    let selected_numeric = selected.is_some_and(Execution::is_numeric);
    match (meaning, selected, selected_numeric) {
        (Some(meaning), Some(execution), _)
            if meaning.signature().is_some() && Execution::from(meaning) == execution =>
        {
            Ok(())
        }
        (None, _, false) => Ok(()),
        _ => Err("numeric meaning disagrees with selected provider custody"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use language_semantics::numeric_intrinsics::NumericIntrinsic;

    #[test]
    fn numeric_provider_joins_portable_satisfaction_without_a_source_execution_row() {
        use terminal_psi::{
            CheckedMachineDeclaration, ExternalProviderConformance, ExternalProviderOwner,
            NominalDeclarationIdentity, TerminalIdentityTableBuilder,
        };
        let mut identities = TerminalIdentityTableBuilder::default();
        let callable = identities.intern("Realizer::negate(f32)->f32");
        let requirement = identities.intern("F32::negate(f32)->f32");
        let provider = identities.intern("Realizer");
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([31; 32]);
        let meaning = NumericIntrinsic::from_canonical_encoding([3, 0, 0, 0]).unwrap();
        let candidate = ExternalProviderConformance {
            requirement: NominalDeclarationIdentity::new(requirement),
            requirement_identity: requirement,
            requirement_package: package,
            numeric_intrinsic: Some(meaning),
            target: Some("macos_arm64".into()),
            realization: CheckedMachineDeclaration {
                machine: NominalDeclarationIdentity::new(callable),
                state: NominalDeclarationIdentity::new(callable),
                semantic_declaration: callable,
                parameter_types: vec![identities.intern("f32")],
                callable,
                result_type: identities.intern("f32"),
                contract: [22; 32],
                specialization: None,
            },
            realization_package: package,
            provider: Some(ExternalProviderOwner {
                declaration: NominalDeclarationIdentity::new(provider),
                identity: provider,
                package,
            }),
            binding: language_semantics::ExternalBindingIdentity::CompilerIntrinsic,
        };
        let identities = identities.build();
        let mut plan = ProviderPlan {
            target: "macos_arm64".into(),
            provider_type: "Realizer".into(),
            provider_type_package_identity: package,
            origin_package_identity: package,
            ..ProviderPlan::default()
        };
        plan.schema.trait_package_identity = package;
        let row = effects::provider_plan::ProviderPlanRow {
            method: "negate".into(),
            requirement_identity: plan.identities.intern("F32::negate(f32)->f32"),
            requirement_lifetime_partition: Vec::new(),
            admitted_same_stack_bytes: 0,
            binding: effects::provider_plan::ProviderBinding::CompilerIntrinsic {
                machine: "Realizer::negate(f32)->f32".into(),
            },
        };
        let execute = |candidates: &[ExternalProviderConformance]| {
            selected_execution(
                candidates,
                &identities,
                &candidate.requirement,
                &plan,
                &row,
                None,
            )
        };
        assert_eq!(
            execute(std::slice::from_ref(&candidate)).unwrap(),
            Some(meaning.into())
        );
        assert_ne!(execute(&[]), Ok(Some(meaning.into())));
        for mutation in 0..10 {
            let mut altered = candidate.clone();
            match mutation {
                0 => altered.realization.callable = provider,
                1 => altered.realization_package = None,
                2 => altered.provider.as_mut().unwrap().identity = callable,
                3 => altered.provider.as_mut().unwrap().package = None,
                4 => {
                    altered.binding =
                        language_semantics::ExternalBindingIdentity::Syscall { number: 1 }
                }
                5 => altered.provider = None,
                6 => altered.target = Some("linux_x86_64".into()),
                7 => altered.requirement_identity = callable,
                8 => altered.requirement_package = None,
                9 => altered.requirement = NominalDeclarationIdentity::new(callable),
                _ => unreachable!(),
            }
            assert_ne!(
                execute(&[altered]),
                Ok(Some(meaning.into())),
                "mutation {mutation}"
            );
        }
        let mut sibling = candidate.clone();
        sibling.target = Some("linux_x86_64".into());
        assert_eq!(
            execute(&[candidate.clone(), sibling]).unwrap(),
            Some(meaning.into())
        );
        assert!(execute(&[candidate.clone(), candidate.clone()]).is_err());
        let mut selected = plan.clone();
        selected.rows.push(row.clone());
        let declaration = backend_plan::TerminalTargetProviderPlan {
            plan: selected,
            provider_declaration: Some("Realizer".into()),
        };
        assert!(
            validate_external_provider_owners(
                std::slice::from_ref(&candidate),
                &identities,
                std::slice::from_ref(&declaration),
            )
            .is_ok()
        );
        let mut substituted = candidate.clone();
        substituted.provider.as_mut().unwrap().declaration =
            NominalDeclarationIdentity::new(callable);
        assert!(
            validate_external_provider_owners(
                &[substituted],
                &identities,
                std::slice::from_ref(&declaration),
            )
            .is_err()
        );
        let mut missing = declaration;
        missing.provider_declaration = None;
        assert!(
            validate_external_provider_owners(
                std::slice::from_ref(&candidate),
                &identities,
                &[missing],
            )
            .is_err()
        );
        assert!(
            selected_execution(
                std::slice::from_ref(&candidate),
                &identities,
                &candidate.requirement,
                &plan,
                &row,
                Some(CompilerIntrinsicExecutionIdentity::HostedWriteByteI32)
            )
            .is_err()
        );
    }
}
