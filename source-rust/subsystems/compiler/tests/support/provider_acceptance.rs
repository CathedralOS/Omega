//! Test-only acceptance of one exact package-owned provider plan.
//!
//! Every package-owned service role is resolved, admitted and given permission
//! the same way, here: find the one exact settled provider plan that serves the
//! role's complete requested method set and belongs to the accepting owner,
//! then build an `AcceptedSemanticBinding` from that plan's digests and the
//! sorted terminal-authority rows of the methods the role accepts.
//!
//! `Console` is a package-owned boundary, so the package the plan comes from
//! is the owner. `ProcessExit` is toolchain-owned
//! (`omega::language::core::process_exit`) and has no package identity, so
//! the bound package owns the selected nominal provider type. Both owners are
//! read from the settled plan itself. Both cases live here so the requirement
//! projection and the binding constructors cannot disagree about which package
//! owns a plan. This is test policy, not evidence that an audit occurred.
//! A direct native exit instead retains its completed Build producer and
//! locator receipt; it acquires no invented nominal provider.

use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use semantic_vocabulary::PackageKeyIdentity;
use terminal_psi::TerminalIdentityTableBuilder;

/// Ownership through the provider-type nominal, the coordinate a
/// package-authored provider type carries its package in.
pub fn provider_type_owner(
    plan: &provider_plans::ProviderPlan,
    owner: PackageKeyIdentity,
) -> bool {
    plan.provider_type_package_identity == Some(owner)
}

/// Ownership through the package the plan originates from: a package-owned
/// service boundary and its provider carry the owner there.
pub fn schema_symbol_owner(
    plan: &provider_plans::ProviderPlan,
    owner: PackageKeyIdentity,
) -> bool {
    plan.origin_package_identity == Some(owner)
}

/// The authority classes one accepted method's terminal event requires; an
/// accepted name absent here is a harness defect, never a silent grant.
fn accepted_classes(method: &str) -> &'static [TerminalAuthorityClass] {
    match method {
        "exit_process" => &[TerminalAuthorityClass::ProcessTermination],
        "write" | "write_byte" | "write_line" => &[TerminalAuthorityClass::ProcessOutput],
        "read_byte" | "read_line" => &[TerminalAuthorityClass::ProcessInput],
        accepted => unreachable!("`{accepted}` is accepted without an authority class row"),
    }
}

/// Acceptance of a package-owned `Console` provider: the operations the
/// program resolves decide which terminal-authority classes it admits.
pub fn console_io_binding(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
    accepts_console_output: bool,
    accepts_console_input: bool,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let mut accepted = Vec::new();
    if accepts_console_output {
        accepted.extend_from_slice(&["write", "write_byte", "write_line"]);
    }
    if accepts_console_input {
        accepted.extend_from_slice(&["read_byte", "read_line"]);
    }
    accept(
        checked,
        owner,
        "Console",
        AcceptedSemanticBindingRole::ConsoleIo,
        schema_symbol_owner,
        &accepted,
    )
}

/// Acceptance of the exact canonical `ProcessExit` provider.
pub fn process_exit_binding(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    accept(
        checked,
        owner,
        "ProcessExit",
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
        provider_type_owner,
        &["exit_process"],
    )
}

/// A direct native supply has no nominal provider. Its completed Build receipt
/// owns the binding; the canonical checked completion identifies the operation.
pub fn direct_process_exit_binding(
    checked: &CheckedCompilation,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<Option<AcceptedSemanticBinding>, Vec<Diagnostic>> {
    let accepted = checked.terminal_publication().accepted()?;
    let module = accepted.module();
    let declaration = module.boundary_machine(boundary).ok_or_else(|| {
        vec![Diagnostic::error(
            "direct fixture consent lost its boundary",
        )]
    })?;
    if declaration.completion
        != semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        )
    {
        return Ok(None);
    }
    let requirement = module
        .identities
        .text(declaration.identity)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "direct fixture consent lost its requirement identity",
            )]
        })?;
    let realization = checked.execution().realization();
    let target = checked.execution().target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "direct ProcessExit consent requires its selected target",
        )]
    })?;
    let mut leaves = realization
        .direct_boundary_leaves()
        .iter()
        .filter(|leaf| leaf.satisfied_requirement_identity == requirement);
    let (Some(leaf), None) = (leaves.next(), leaves.next()) else {
        return Err(vec![Diagnostic::error(
            "direct ProcessExit consent requires one exact native leaf",
        )]);
    };
    direct_process_exit_leaf_binding(
        module,
        boundary,
        target.target_name(),
        leaf,
        realization.evaluated_native_bindings(),
        realization.evaluated_binding_identities(),
    )
    .map(Some)
}

/// Keep the validation on one actual leaf available to hostile-input controls.
pub fn direct_process_exit_leaf_binding(
    module: &terminal_psi::TerminalModule,
    boundary: semantic_vocabulary::BoundaryMachineId,
    target: &str,
    leaf: &backend_plan::DirectBoundaryLeaf,
    evaluated: &[provider_plans::EvaluatedNativeBinding],
    evaluated_identities: &terminal_psi::TerminalIdentityTable,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let refused = |reason: String| {
        vec![Diagnostic::error(format!(
            "direct ProcessExit fixture consent: {reason}"
        ))]
    };
    let declaration = module
        .boundary_machine(boundary)
        .ok_or_else(|| refused("missing checked boundary".into()))?;
    if leaf.plan.target != target {
        return Err(refused("native leaf names another target".into()));
    }
    if declaration.completion
        != semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        )
        || module.identities.text(declaration.identity)
            != Some(leaf.satisfied_requirement_identity.as_str())
    {
        return Err(refused(
            "different canonical completion or requirement".into(),
        ));
    }
    let schema =
        backend_plan::DirectBoundaryLeaf::declared_schema(module, boundary).map_err(refused)?;
    if leaf.satisfied_schema.declaration.as_ref() != schema.surface.as_ref() {
        return Err(refused("different complete checked service schema".into()));
    }
    leaf.validate_plain_execution(module, evaluated, evaluated_identities)
        .map_err(refused)?;
    let [row] = leaf.plan.rows.as_slice() else {
        return Err(refused("native leaf must retain one method".into()));
    };
    let requirement = leaf.plan.identities.text(row.requirement_identity);
    let mut receipts = evaluated.iter().filter(|binding| {
        binding.target().target_name() == leaf.plan.target
            && binding.receipt().build_origin().is_some_and(|origin| {
                evaluated_identities.text(origin.requirement_identity) == Some(requirement)
            })
    });
    let (Some(receipt), None) = (receipts.next(), receipts.next()) else {
        return Err(refused(
            "native leaf has no unique completed Build receipt".into(),
        ));
    };
    let owner = receipt
        .receipt()
        .producer_package()
        .ok_or_else(|| refused("Build receipt has no producer package".into()))?;
    let mut identities = TerminalIdentityTableBuilder::default();
    let permission = ServiceTerminalAuthorityPermission::new(
        leaf.plan.schema.identity_digest(),
        identities.intern(requirement),
        TerminalAuthorityDisposition::from_classes(vec![
            TerminalAuthorityClass::ProcessTermination,
        ]),
    );
    AcceptedSemanticBinding::new(
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
        owner,
        leaf.plan
            .schema
            .identity_text(leaf.plan.schema.declaration.trait_name),
        leaf.plan.schema.identity_digest(),
        leaf.plan.identity_digest(),
    )
    .and_then(|binding| {
        binding.with_terminal_authority_permissions(vec![permission], &identities.build())
    })
    .map_err(|reason| refused(reason.into()))
}

/// Resolve the one exact package-owned plan and admit this role's binding.
fn accept(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
    trait_name: &str,
    binding_role: AcceptedSemanticBindingRole,
    owns: fn(&provider_plans::ProviderPlan, PackageKeyIdentity) -> bool,
    accepted_methods: &[&str],
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let candidates = checked
        .execution()
        .plans()
        .plans()
        .iter()
        .filter(|plan| {
            plan.schema
                .identity_text(plan.schema.declaration.trait_name)
                == trait_name
                && accepted_methods
                    .iter()
                    .all(|method| plan.rows.iter().any(|row| row.method == *method))
                && owns(plan, owner)
        })
        .collect::<Vec<_>>();
    let [plan] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "repository fixture {trait_name} acceptance resolved {} exact package-owned provider plans instead of one",
            candidates.len()
        ))]);
    };
    let binding = AcceptedSemanticBinding::new(
        binding_role,
        owner,
        plan.schema
            .identity_text(plan.schema.declaration.trait_name),
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "cannot construct repository fixture {trait_name} binding: {error}"
        ))]
    })?;
    let mut permission_identities = TerminalIdentityTableBuilder::default();
    let mut permissions = plan
        .schema
        .declaration
        .methods
        .iter()
        .filter(|method| accepted_methods.contains(&method.name.as_str()))
        .map(|method| {
            ServiceTerminalAuthorityPermission::new(
                plan.schema.identity_digest(),
                permission_identities
                    .intern(plan.schema.identity_text(method.requirement_identity)),
                TerminalAuthorityDisposition::from_classes(
                    accepted_classes(method.name.as_str()).to_vec(),
                ),
            )
        })
        .collect::<Vec<_>>();
    permissions.sort_by(|left, right| {
        permission_identities
            .text(left.requirement_identity())
            .cmp(&permission_identities.text(right.requirement_identity()))
    });
    binding
        .with_terminal_authority_permissions(permissions, &permission_identities.build())
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot attach repository fixture {trait_name} terminal permissions: {error}"
            ))]
        })
}
