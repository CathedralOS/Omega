//! Preserve exact direct-call foreign candidates beside provider selection.
//! Population grants no selection: Terminal demand later closes each concrete
//! callable against its original evaluated binding and calling application.

use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::{
    ProviderPlanningContext,
    evaluated_via_bindings::{EvaluatedViaBinding, EvaluatedViaBindingTable},
};
use effects::{DirectForeignBinding, DirectForeignRealization};
use std::sync::Arc;

pub(crate) fn retain_direct_foreign_realizations(
    typed: &checked_trees::typed_trees::TypedTrees,
    target: Option<target::TargetProfile>,
    evaluated: &EvaluatedViaBindingTable,
    calling: &[BoundaryCallingPlanRealization],
    opaque: &[representation_planning::OpaqueRepresentationSelection],
) -> Vec<Arc<DirectForeignRealization>> {
    let Some(target) = target else {
        return Vec::new();
    };
    let context = ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut retained = Vec::new();
    let binding_identities = Arc::new(evaluated.identities().clone());
    for row in evaluated.rows() {
        if row.evaluated().target() != target {
            continue;
        }
        let Some(machine) = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.realization_machine())
        else {
            continue;
        };
        let mut clauses = typed
            .machine_trait_conformances(machine)
            .iter()
            .filter(|clause| {
                clause.symbol == row.satisfied_owner()
                    && clause.requirement_symbol == row.requirement()
                    && clause.via_expression == row.via_expression()
            });
        let Some(clause) = clauses.next() else {
            continue;
        };
        if clauses.next().is_some() {
            continue;
        }
        let schema = crate::service_schema::from_typed_requirement(context, row.requirement())
            .or_else(|| {
                typed
                    .machines()
                    .iter()
                    .find(|requirement| requirement.symbol == row.requirement())
                    .and_then(|requirement| {
                        crate::service_schema::from_typed_boundary_requirement(context, requirement)
                    })
            })
            .or_else(|| {
                checked_trees::typed_trees::operator::resolve_satisfied_boundary_operator_for_conformance(
                    typed, machine, clause,
                )
                .and_then(|operator| crate::service_schema::from_typed_operator(context, operator))
            });
        let Some(schema) = schema else {
            continue;
        };
        let [method] = schema.methods.as_slice() else {
            continue;
        };
        let requirement_identity = schema.identity_text(method.requirement_identity).to_owned();
        let Some(overload_identity) = typed.machine_callable_identity_text(machine) else {
            continue;
        };
        let declaration_identity = typed.machine_declaration_callable_identity(machine.symbol);
        if !declaration_identity.is_valid() {
            continue;
        }
        let callable_identity = typed.callable_identity_text(declaration_identity);
        let binding = match row.evaluated() {
            EvaluatedViaBinding::Import(binding) => DirectForeignBinding::Import(binding.clone()),
            EvaluatedViaBinding::Syscall(binding) => DirectForeignBinding::Syscall(binding.clone()),
        };
        let entry = direct_boundary_entry_plan(
            typed,
            target,
            &schema,
            method,
            row.requirement(),
            &binding,
            calling,
            opaque,
        );
        if let Ok(realization) = DirectForeignRealization::from_evaluated(
            callable_identity,
            overload_identity,
            requirement_identity,
            target,
            schema,
            binding,
            entry,
            Arc::clone(&binding_identities),
        ) {
            retained.push(Arc::new(realization));
        }
    }
    retained
}

/// Preserve the one original calling application for any direct native supply.
/// Intrinsic supply shares ordinary native entry policy; kernel entry retains
/// its distinct syscall convention. Explicit source Calling remains exact.
#[allow(clippy::too_many_arguments)]
pub fn direct_boundary_entry_plan(
    typed: &checked_trees::typed_trees::TypedTrees,
    target: target::TargetProfile,
    schema: &effects::provider_plan::ServiceSchema,
    method: &effects::provider_plan::ServiceMethod,
    requirement: symbols::SymbolHandle,
    binding: &DirectForeignBinding,
    calling: &[BoundaryCallingPlanRealization],
    opaque: &[representation_planning::OpaqueRepresentationSelection],
) -> Option<calling_conventions::BoundaryEntryPlan> {
    let requirement_identity = schema.identity_text(method.requirement_identity);
    match (
        method.calling_plan_report_fingerprint,
        method.calling_plan_commitment,
    ) {
        (Some(fingerprint), Some(commitment)) => {
            let mut applications = calling.iter().filter(|application| {
                application.requirement_machine == requirement
                    && application.report_fingerprint == fingerprint
                    && application.commitment == commitment
                    && application
                        .materialized_signature()
                        .owner_requirement_identity()
                        == requirement_identity
                    && application.materialized_signature().native_target()
                        == target.native_target()
                    && typed.traits().iter().any(|owner| {
                        owner.symbol == application.declaring_trait
                            && typed.trait_declaration_path(owner) == method.requirement_owner
                            && typed.symbols.symbol_package_identity(owner.symbol)
                                == method.requirement_owner_package_identity
                    })
            });
            match (applications.next(), applications.next()) {
                (Some(application), None) => Some(application.exact_boundary_entry_plan().clone()),
                _ => None,
            }
        }
        (None, None) => {
            if let DirectForeignBinding::CompilerIntrinsic { execution, .. } = binding {
                // The authenticated catalog supplies its own native shape;
                // public Calling classification deliberately rejects sums.
                return effects::compiler_intrinsic_boundary_entry_plan(
                    *execution,
                    target.native_target(),
                )
                .ok()
                .flatten();
            }
            let policy = match binding {
                DirectForeignBinding::Import(_) => Some(
                    calling_conventions::CallingPolicy::native_for_target(target.native_target()),
                ),
                DirectForeignBinding::Syscall(_) => {
                    calling_conventions::CallingPolicy::native_syscall_for_target(
                        target.native_target(),
                    )
                }
                DirectForeignBinding::CompilerIntrinsic { .. } => unreachable!("handled above"),
            };
            policy.and_then(|policy| {
                crate::calling_policy_plans::evaluate_compatibility_boundary_entry_plan(
                    typed,
                    target.native_target(),
                    schema.identity_text(schema.trait_name),
                    schema.trait_package_identity,
                    &method.name,
                    &requirement_identity,
                    policy,
                    0,
                    opaque,
                )
                .ok()
                .flatten()
            })
        }
        _ => None,
    }
}
