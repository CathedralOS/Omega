//! Project direct boundary dependencies from the same derived satisfaction rows.
//!
//! The original row owns its evaluated locator, complete requirement contract
//! and calling policy. Projection changes the callable demand coordinate, never
//! aliases it to a selected wrapper. Native execution scope later chooses which
//! candidates to activate; merely retaining a declaration grants no authority.

use crate::DerivedProviderPlan;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;

/// Source custody is discarded only after intrinsic execution is closed.
/// The projected native plan never replaces the original satisfied schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectBoundaryCandidate {
    pub leaf: backend_plan::DirectBoundaryLeaf,
    pub source: DerivedProviderPlan,
}

pub(crate) fn derive(
    typed: &checked_trees::typed_trees::TypedTrees,
    candidates: &[DerivedProviderPlan],
    target: Option<&str>,
    native_target: target::NativeTarget,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    opaque: &[representation_planning::OpaqueRepresentationSelection],
) -> Result<Vec<DirectBoundaryCandidate>, Vec<Diagnostic>> {
    let mut leaves = Vec::<DirectBoundaryCandidate>::new();
    for candidate in candidates {
        for (position, row) in candidate.plan.rows.iter().enumerate() {
            if !matches!(
                row.binding,
                ProviderBinding::CompilerIntrinsic { .. }
                    | ProviderBinding::Import { .. }
                    | ProviderBinding::Syscall { .. }
                    | ProviderBinding::VtableField { .. }
                    | ProviderBinding::TableFunction { .. }
            ) {
                continue;
            }
            let Some(machine) = candidate
                .provenance
                .row_realizations
                .get(position)
                .and_then(|symbol| typed.machine_by_symbol(*symbol))
            else {
                continue;
            };
            if machine.body_is_present {
                continue;
            }
            let Some(identity) = typed.machine_callable_identity_text(machine) else {
                return Err(vec![Diagnostic::error(
                    "boundary leaf has no exact callable identity",
                )]);
            };
            let requirement = candidate
                .plan
                .identities
                .text(row.requirement_identity)
                .to_owned();
            let mut original = candidate.clone();
            original.plan.rows = vec![row.clone()];
            original.plan.schema.methods.retain(|method| {
                candidate
                    .plan
                    .schema
                    .identity_text(method.requirement_identity)
                    == requirement
            });
            original.provenance.row_requirements =
                vec![candidate.provenance.row_requirements[position]];
            original.provenance.row_realizations = vec![machine.symbol];
            original.provenance.row_target_machine_origins =
                vec![candidate.provenance.row_target_machine_origins[position].clone()];
            original.provenance.row_placed_accessors =
                vec![candidate.provenance.row_placed_accessors[position].clone()];
            let mut plan = original.plan.clone();
            let satisfied_schema = plan.schema.clone();
            // A callable dependency has one method contract, not a selected
            // whole-trait receiver map. Retain that original schema beside the
            // projection; its instantiated method claims and ABI stay intact.
            plan.schema.receiver_applications.clear();
            plan.name = format!("direct-boundary:{}:{identity}", plan.target);
            plan.provider_type = typed.symbols.display_path(machine.symbol, "::");
            plan.provider_type_package_identity =
                typed.symbols.symbol_package_identity(machine.symbol);
            plan.schema.trait_name = plan.schema.identities.intern(&identity);
            plan.schema.trait_package_identity =
                typed.symbols.symbol_package_identity(machine.symbol);
            plan.rows[0].requirement_identity = plan.identities.intern(&identity);
            let [method] = plan.schema.methods.as_mut_slice() else {
                return Err(vec![Diagnostic::error(
                    "boundary leaf does not bind one exact requirement contract",
                )]);
            };
            method.requirement_identity = plan.schema.identities.intern(&identity);
            let execution = if matches!(row.binding, ProviderBinding::CompilerIntrinsic { .. }) {
                backend_plan::DirectBoundaryExecution::Intrinsic(None)
            } else {
                // Only foreign leaves consume ABI extraction. Intrinsics close
                // against their original semantic schema during admission.
                let mut external_binding =
                    super::external_binding_rows::extract_native_external_binding_rows(
                        target,
                        native_target,
                        std::slice::from_ref(&original.plan),
                        std::slice::from_ref(&original),
                        calling_plans,
                        typed,
                        opaque,
                    )
                    .and_then(|mut rows| {
                        if rows.len() != 1 {
                            return Err(vec![Diagnostic::error(
                                "foreign leaf does not retain one normalized external binding",
                            )]);
                        }
                        rows.pop().ok_or_else(|| {
                            vec![Diagnostic::error(
                                "foreign leaf lost its normalized external binding",
                            )]
                        })
                    });
                if let Ok(binding) = &mut external_binding {
                    binding.trait_name = identity.clone();
                    binding.requirement_identity = identity.clone();
                    // The physical row belongs to the projected callable plan,
                    // not the original whole-provider namespace. Original contract
                    // and ABI provenance remain in the satisfied schema and row.
                    binding.table_type = plan.provider_type.clone();
                }
                backend_plan::DirectBoundaryExecution::Foreign(external_binding)
            };
            let leaf = backend_plan::DirectBoundaryLeaf {
                declaration_identity: typed
                    .normalized_hermetic_symbol_identity(machine.symbol)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
                callable_identity: identity,
                satisfied_schema,
                plan,
                satisfied_requirement_identity: requirement,
                execution,
            };
            // Intrinsic shape closure consumes the complete original service
            // schema, not the projected one-method callable contract.
            original.plan.schema = candidate.plan.schema.clone();
            let retained = DirectBoundaryCandidate {
                leaf,
                source: original,
            };
            if !leaves.contains(&retained) {
                leaves.push(retained);
            }
        }
    }
    Ok(leaves)
}
