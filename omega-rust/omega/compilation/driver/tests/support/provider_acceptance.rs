//! Repository test acceptance of one exact package-owned provider plan.
//!
//! Three sibling sites used to resolve, admit and permission a package-owned
//! service role by hand. Every role now resolves the same way, once: one exact
//! plan (the selected provider plans zipped with their provenance) serving the
//! role's schema with an `exit_process` row and belonging to the accepting
//! owner, then an `AcceptedSemanticBinding` with that plan's digests and the
//! sorted terminal-authority rows of the methods the role accepts.
//!
//! `Console` is a package-owned boundary, so its schema symbol names the owner;
//! `ProcessExit` is toolchain-owned (`omega::language::core::process_exit`) and
//! carries no package identity, so the bound package owns the selected nominal
//! provider type. Both spellings live here so the requirement projection and
//! the binding constructors cannot disagree about which package owns a plan.
//! This is test policy, not evidence that an audit occurred.

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
    checked: &CheckedCompilation,
    provenance: &provider_planning::SelectedProviderReviewProvenance,
    owner: PackageKeyIdentity,
) -> bool {
    provenance
        .provider
        .provider_type
        .is_some_and(|provider| checked.symbols.symbol_package_identity(provider) == Some(owner))
}

/// Ownership through the boundary declaration schema the plan was resolved
/// from: a package-owned service boundary carries the owner there.
pub fn schema_symbol_owner(
    checked: &CheckedCompilation,
    provenance: &provider_planning::SelectedProviderReviewProvenance,
    owner: PackageKeyIdentity,
) -> bool {
    checked
        .typed
        .symbols
        .symbol_package_identity(provenance.provider.schema.symbol())
        == Some(owner)
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
pub fn console_exit_binding(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
    accepts_console_output: bool,
    accepts_console_input: bool,
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let mut accepted = vec!["exit_process"];
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
        AcceptedSemanticBindingRole::ConsoleExitProcessI32,
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

/// Resolve the one exact package-owned plan and admit this role's binding.
fn accept(
    checked: &CheckedCompilation,
    owner: PackageKeyIdentity,
    trait_name: &str,
    binding_role: AcceptedSemanticBindingRole,
    owns: fn(
        &CheckedCompilation,
        &provider_planning::SelectedProviderReviewProvenance,
        PackageKeyIdentity,
    ) -> bool,
    accepted_methods: &[&str],
) -> Result<AcceptedSemanticBinding, Vec<Diagnostic>> {
    let candidates = checked
        .execution()
        .provider_source
        .plans()
        .plans()
        .iter()
        .zip(checked.execution().selected_provider_provenance.as_slice())
        .filter(|(plan, provenance)| {
            plan.schema.identity_text(plan.schema.trait_name) == trait_name
                && plan.rows.iter().any(|row| row.method == "exit_process")
                && owns(checked, provenance, owner)
        })
        .collect::<Vec<_>>();
    let [(plan, provenance)] = candidates.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "repository fixture {trait_name} acceptance resolved {} exact package-owned provider plans instead of one",
            candidates.len()
        ))]);
    };
    let binding = AcceptedSemanticBinding::new(
        binding_role,
        owner,
        checked
            .typed
            .symbols
            .display_path(provenance.provider.schema.symbol(), "::"),
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
