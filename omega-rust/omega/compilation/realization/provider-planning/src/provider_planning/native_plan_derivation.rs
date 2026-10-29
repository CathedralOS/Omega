//! Derive ordinary provider candidates from admitted Build-native rows.
//! Complete schema coverage and precedence remain the common selector's work.
use super::provenance_replay::{
    DerivedProviderPlan, ProviderPlanProvenance, ProviderSchemaDeclaration,
};
use crate::{AdmittedNativeBinding, ProviderPlanningContext, ProviderRowOrigin};
use effects::provider_plan::{ProviderIdentityTable, ProviderPlan, ProviderPlanRow};

pub(super) fn append(
    context: ProviderPlanningContext<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    target: Option<&str>,
    bindings: &[AdmittedNativeBinding],
    plans: &mut Vec<DerivedProviderPlan>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let typed = context.typed;
    let mut admitted_applications = Vec::new();
    for binding in bindings
        .iter()
        .filter(|binding| target == Some(binding.target.target_name()))
    {
        let Some(binding) = binding.resolved_application(typed) else {
            continue;
        };
        let application = crate::resolve_native_binding_application(typed, &binding)
            .map_err(|reason| {
                vec![diagnostics::Diagnostic::error(reason).with_source_span(binding.source_span)]
            })?
            .key;
        if admitted_applications
            .iter()
            .any(|(requirement, package, previous)| {
                *requirement == binding.requirement
                    && *package == binding.receipt().producer_package()
                    && applications_overlap(previous, &application)
            })
        {
            return Err(vec![diagnostics::Diagnostic::error(
                "Build-native bindings overlap one exact requirement application for one supplier and target",
            ).with_source_span(binding.source_span)]);
        }
        admitted_applications.push((
            binding.requirement,
            binding.receipt().producer_package(),
            application,
        ));
        let Some(provider_binding) = binding.binding() else {
            continue;
        };
        let refuse = |reason| {
            vec![diagnostics::Diagnostic::error(reason).with_source_span(binding.source_span)]
        };
        if !context.contains(binding.requirement) {
            return Err(refuse(
                "Build-native requirement is outside the admitted Product frontier",
            ));
        }
        let schema = crate::service_schema::from_typed_requirement_application(
            context,
            calling_plans,
            binding.requirement,
            binding.trait_scope,
            binding.closed_requirement,
        )
        .ok_or_else(|| {
            refuse("Build-native requirement lost its exact callable application schema")
        })?;
        let [method_schema] = schema.methods.as_slice() else {
            return Err(refuse(
                "Build-native application does not own one callable requirement",
            ));
        };
        let requirement_identity = schema
            .identity_text(method_schema.requirement_identity)
            .to_owned();
        let method = method_schema.name.clone();
        let (declaration, lifetime_partition) =
            if let Some((owner, _)) = typed.trait_machine_signature(binding.requirement) {
                (
                    ProviderSchemaDeclaration::BoundaryTrait(owner.symbol),
                    (0..owner.lifetime_parameters.len())
                        .map(|ordinal| ordinal as u32)
                        .collect(),
                )
            } else {
                (
                    ProviderSchemaDeclaration::BoundaryRequirement(binding.requirement),
                    Vec::new(),
                )
            };
        let package = binding.receipt().producer_package();
        let name = format!(
            "build-native::{}::{}",
            binding.target.target_name(),
            schema.identity_text(schema.trait_name)
        );
        let index = plans
            .iter()
            .position(|candidate| {
                candidate.plan.name == name
                    && candidate.plan.origin_package_identity == package
                    && candidate.provenance.schema == declaration
                    && candidate.provenance.provider_type.is_none()
                    && candidate.plan.schema == schema
            })
            .unwrap_or_else(|| {
                plans.push(DerivedProviderPlan {
                    plan: ProviderPlan {
                        name,
                        provider_type: String::new(),
                        provider_type_package_identity: None,
                        target: binding.target.target_name().to_owned(),
                        schema,
                        identities: ProviderIdentityTable::default(),
                        rows: Vec::new(),
                        origin_package_identity: package,
                        origin_package: String::new(),
                        digest_memo: Default::default(),
                    },
                    provenance: ProviderPlanProvenance {
                        schema: declaration,
                        provider_type: None,
                        row_requirements: Vec::new(),
                        row_realizations: Vec::new(),
                        row_target_machine_origins: Vec::new(),
                        row_placed_accessors: Vec::new(),
                    },
                });
                plans.len() - 1
            });
        let candidate = &mut plans[index];
        if candidate
            .provenance
            .row_requirements
            .contains(&binding.requirement)
        {
            return Err(refuse(
                "Build-native supplier binds one requirement twice in the same target schema",
            ));
        }
        let requirement_identity = candidate.plan.identities.intern(&requirement_identity);
        candidate.plan.rows.push(ProviderPlanRow {
            method,
            requirement_identity,
            requirement_lifetime_partition: lifetime_partition,
            admitted_same_stack_bytes: binding.same_stack_demand.map_or(0, |demand| demand.bytes),
            binding: provider_binding,
        });
        candidate
            .provenance
            .row_requirements
            .push(binding.requirement);
        candidate
            .provenance
            .row_realizations
            .push(ProviderRowOrigin::BuildNative {
                receipt: binding.receipt().identity_digest(),
                table_field: binding.table_field(),
                trait_scope: binding.trait_scope,
                closed_requirement: binding.closed_requirement,
            });
        candidate.provenance.row_target_machine_origins.push(None);
        candidate.provenance.row_placed_accessors.push(None);
    }
    // The native candidate identity commits to every event receipt, including
    // syscall bindings whose physical plan row contains only the number.
    for candidate in plans.iter_mut().filter(|candidate| {
        candidate
            .provenance
            .row_realizations
            .iter()
            .any(|origin| matches!(origin, ProviderRowOrigin::BuildNative { .. }))
    }) {
        let mut hash = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega.build-native-provider.sha256.v2\0",
        );
        for origin in &candidate.provenance.row_realizations {
            let ProviderRowOrigin::BuildNative { receipt, .. } = *origin else {
                return Err(vec![diagnostics::Diagnostic::error(
                    "native candidate mixed incompatible producer classes",
                )]);
            };
            let binding = bindings
                .iter()
                .find(|binding| binding.receipt().identity_digest() == receipt)
                .ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "native candidate lost its admitted event",
                    )]
                })?;
            hash.raw(&binding.receipt().identity_digest());
        }
        let identity = hash.finish();
        use std::fmt::Write;
        candidate.plan.name.push_str("::");
        for byte in identity {
            write!(candidate.plan.name, "{byte:02x}").expect("formatting into String");
        }
    }
    Ok(())
}

/// A missing application axis is an admitted family, not a fresh arbitrary
/// tuple. Families and concrete members cannot silently install overlapping
/// choices; two different complete tuples remain independent coordinates.
fn applications_overlap(
    left: &package_compilation::retained_native_bindings::RetainedNativeApplication,
    right: &package_compilation::retained_native_bindings::RetainedNativeApplication,
) -> bool {
    use package_compilation::retained_native_bindings::RetainedNativeApplication;
    match (left, right) {
        (RetainedNativeApplication::Universal, _) | (_, RetainedNativeApplication::Universal) => {
            true
        }
        (
            RetainedNativeApplication::Applied {
                outer: left_outer,
                local: left_local,
            },
            RetainedNativeApplication::Applied {
                outer: right_outer,
                local: right_local,
            },
        ) => {
            left_outer
                .as_ref()
                .zip(right_outer.as_ref())
                .is_none_or(|(left, right)| left == right)
                && left_local
                    .as_ref()
                    .zip(right_local.as_ref())
                    .is_none_or(|(left, right)| left == right)
        }
    }
}
