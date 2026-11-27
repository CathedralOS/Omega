//! Nominate hosted services from the selected provider and its actual implementation.
//! An implementation can delegate across a checked call or a selected boundary.
use super::{declarations::unavailable, input::PolicySource, semantic_bindings::checked_service};
use diagnostics::Diagnostic;
use native_intrinsics::CompilerIntrinsicExecutionIdentity as Intrinsic;
use build_providers::acceptance::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole as Role,
    ServiceTerminalAuthorityPermission,
};
use build_providers::plans::{ProviderBinding, ProviderPlan};
use review_evidence::record::SemanticBindingReviewCandidate;
use semantic_vocabulary::{
    PackageKeyIdentity, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use terminal_semantics::callable_values::{CallableBoundaryImplementation, reached_boundary_calls};

pub(super) fn nominations(
    input: &PolicySource<'_>,
    selected: &compiler::SelectedProviderResult,
) -> Result<Vec<SemanticBindingReviewCandidate>, Vec<Diagnostic>> {
    let plans = selected.plans().plans();
    if plans.is_empty() {
        return Ok(Vec::new());
    }
    if selected.realization().row_executions().len() != plans.len() {
        return Err(unavailable("the complete provider execution roster"));
    }
    let module = input.accepted().module();
    let rows = target_provider_admission::selected_checked_invocation_rows(plans, module)
        .map_err(|e| unavailable(&e))?;
    let implementations: Vec<_> = rows
        .iter()
        .flat_map(|row| {
            row.candidates
                .iter()
                .map(|candidate| CallableBoundaryImplementation {
                    boundary: candidate.conformance.boundary,
                    machine: candidate.conformance.candidate,
                })
        })
        .collect();
    let mut nominations = Vec::new();
    for (plan, executions) in plans.iter().zip(selected.realization().row_executions()) {
        let Some((supplier, role, name)) = hosted_supplier(plan) else {
            continue;
        };
        if executions.len() != plan.rows.len() {
            return Err(unavailable(
                "the hosted provider's complete selected execution roster",
            ));
        }
        let mut permissions = Vec::new();
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        for (row, execution) in plan.rows.iter().zip(executions) {
            let method = plan
                .schema
                .declaration
                .methods
                .iter()
                .find(|method| plan.schema.row_binds_method(row, method, &plan.identities))
                .ok_or_else(|| unavailable("a selected hosted requirement's exact schema row"))?;
            let expected = match (role, method.name.as_str()) {
                (Role::ProcessExitExitProcessI32, "exit_process") => (
                    Intrinsic::HostedExitProcessI32,
                    TerminalAuthorityClass::ProcessTermination,
                ),
                (Role::ConsoleIo, "read_byte") => (
                    Intrinsic::HostedReadByte,
                    TerminalAuthorityClass::ProcessInput,
                ),
                (Role::ConsoleIo, "write_byte") => (
                    Intrinsic::HostedWriteByteI32,
                    TerminalAuthorityClass::ProcessOutput,
                ),
                _ => continue,
            };
            let mut reached = execution.as_ref() == Some(&expected.0);
            if !reached && matches!(row.binding, ProviderBinding::CheckedAdapter { .. }) {
                let candidates = rows
                    .iter()
                    .find(|candidate| {
                        std::ptr::eq(candidate.plan, plan) && std::ptr::eq(candidate.row, row)
                    })
                    .ok_or_else(|| unavailable("a hosted provider's checked adapter bodies"))?;
                for candidate in &candidates.candidates {
                    let boundaries = reached_boundary_calls(
                        module,
                        candidate.conformance.candidate,
                        &implementations,
                    )
                    .map_err(|e| unavailable(&e))?;
                    for boundary in boundaries {
                        let decl = module
                            .boundary_machine(boundary)
                            .ok_or_else(|| unavailable("a reached hosted boundary"))?;
                        if let Some(source) = &decl.declaration {
                            let target = module
                                .identities
                                .text(source.source.callable)
                                .ok_or_else(|| unavailable("the reached callable identity"))?;
                            reached |= selected.realization().direct_boundary_leaves().iter().any(|leaf|leaf.callable_identity==target &&
                                matches!(&leaf.execution,direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(Some(execution)) if execution==&expected.0));
                            if let terminal_psi::CheckedBoundaryExecution::HostedPrimitive {
                                operation:
                                    language_core::sealed_intrinsics::HostedOperation::ProcessExit,
                                ..
                            } = source.execution
                            {
                                reached |= expected.0 == Intrinsic::HostedExitProcessI32;
                            }
                        }
                    }
                }
            }
            if reached {
                permissions.push(ServiceTerminalAuthorityPermission::new(
                    plan.schema.identity_digest(),
                    identities.intern(plan.schema.identity_text(method.requirement_identity)),
                    TerminalAuthorityDisposition::from_classes([expected.1]),
                ));
            }
        }
        if permissions.is_empty() {
            continue;
        }
        if nominations
            .iter()
            .any(|prior: &SemanticBindingReviewCandidate| prior.binding().role() == role)
        {
            return Err(unavailable("an ambiguous selected hosted provider"));
        }
        let binding = AcceptedSemanticBinding::new(
            role,
            supplier,
            name,
            plan.schema.identity_digest(),
            plan.identity_digest(),
        )
        .map_err(unavailable)?
        .with_terminal_authority_permissions(permissions, &identities.build())
        .map_err(unavailable)?;
        let (_, schema) = checked_service(input, selected, &binding)?;
        nominations.push(SemanticBindingReviewCandidate::new(binding, schema));
    }
    Ok(nominations)
}

/// Console names the receiver of the per-method physical permissions. Its
/// requirements can reach different core services.
/// The plan and its provider must have the same supplier. The consumer whose Product
/// selected this plan is recorded independently by the package review occurrence.
fn hosted_supplier(plan: &ProviderPlan) -> Option<(PackageKeyIdentity, Role, &str)> {
    let supplier = plan.provider_type_package_identity?;
    if plan.origin_package_identity != Some(supplier) {
        return None;
    }
    let name = plan
        .schema
        .identity_text(plan.schema.declaration.trait_name);
    let role = match (name, plan.schema.declaration.trait_package_identity) {
        ("ProcessExit", None) => Role::ProcessExitExitProcessI32,
        ("Console", Some(declaration)) if declaration == supplier => Role::ConsoleIo,
        _ => return None,
    };
    Some((supplier, role, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_hosted_provider_keeps_its_supplier_in_a_different_consumers_product() {
        let consumer = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let supplier = PackageKeyIdentity::from_digest([2; 32]).unwrap();
        let mut plan = ProviderPlan {
            provider_type_package_identity: Some(supplier),
            origin_package_identity: Some(supplier),
            ..Default::default()
        };
        let schema = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        schema.trait_package_identity = Some(supplier);
        schema.trait_name = schema.identities.intern("Console");
        assert_eq!(
            hosted_supplier(&plan),
            Some((supplier, Role::ConsoleIo, "Console"))
        );

        plan.origin_package_identity = Some(consumer);
        assert!(hosted_supplier(&plan).is_none());
        plan.origin_package_identity = Some(supplier);
        std::sync::Arc::make_mut(&mut plan.schema.declaration).trait_package_identity =
            Some(consumer);
        assert!(hosted_supplier(&plan).is_none());

        let schema = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        schema.trait_package_identity = None;
        for service in ["StandardInput", "StandardOutput"] {
            let schema = std::sync::Arc::make_mut(&mut plan.schema.declaration);
            schema.trait_name = schema.identities.intern(service);
            assert!(hosted_supplier(&plan).is_none());
        }
        let schema = std::sync::Arc::make_mut(&mut plan.schema.declaration);
        schema.trait_name = schema.identities.intern("ProcessExit");
        assert_eq!(
            hosted_supplier(&plan),
            Some((supplier, Role::ProcessExitExitProcessI32, "ProcessExit"))
        );
        std::sync::Arc::make_mut(&mut plan.schema.declaration).trait_package_identity =
            Some(supplier);
        assert!(hosted_supplier(&plan).is_none());
    }
}
