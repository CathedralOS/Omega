//! Independent public subjects, custody, operational ceilings and native ABI.
//!
//! Erased qualifications may differ at the assumed semantic crossing. Runtime
//! carrier identity, borrow topology, multiplicity and materialized geometry
//! therefore have their own checks; neither equal byte size nor the assumption
//! can discharge them. All operational premises here are published declarations.

use super::declarations::{DeclaredRequirement, PublicRequirement};
use super::{AssumedBindingCallingCompatibility, AssumedBindingSubjectCorrespondence};
use crate::ProviderPlanningContext;
use crate::calling_policy_plans::{
    BoundaryCallingPlanRealization, MaterializedBoundarySignature,
    boundary_plan_application_identity, compatibility_call_signature,
};
use calling_conventions::{
    CallingPolicy, ValidatedBoundaryEntryPlan, evaluate_ordinary_boundary_entry_plan,
};
use checked_trees::CheckedTrees;
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::name::Identifier;
use checked_trees::typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use diagnostics::Diagnostic;
use effects::provider_plan::BoundaryCallingPlanCommitment;
use representation_planning::OpaqueRepresentationSelection;
use target::NativeTarget;

pub(super) fn subject_correspondence(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
) -> Result<Vec<AssumedBindingSubjectCorrespondence>, Diagnostic> {
    if required.parameters.len() != supplied.parameters.len()
        || required.result.is_valid() != supplied.result.is_valid()
        || required.lifetimes.len() != supplied.lifetimes.len()
        || !required
            .application
            .has_same_selected_regions(supplied.application)
    {
        return Err(Diagnostic::error(
            "assumed binding must preserve every parameter, result and invocation lifetime position",
        ));
    }
    let mut subjects =
        Vec::with_capacity(required.parameters.len() + usize::from(required.result.is_valid()));
    for (ordinal, (caller, supplier)) in required
        .parameters
        .iter()
        .zip(supplied.parameters)
        .enumerate()
    {
        if caller.is_const != supplier.is_const
            || caller.is_mutable != supplier.is_mutable
            || caller.relevance != supplier.relevance
            || !compatible_carriers(
                program,
                caller.type_reference,
                supplier.type_reference,
                &required.lifetimes,
                &supplied.lifetimes,
            )
            || program
                .facts
                .reference_custody
                .custody(caller.type_reference)
                .binding_multiplicity
                != program
                    .facts
                    .reference_custody
                    .custody(supplier.type_reference)
                    .binding_multiplicity
            || program
                .facts
                .reference_custody
                .custody(caller.type_reference)
                .result_multiplicity
                != program
                    .facts
                    .reference_custody
                    .custody(supplier.type_reference)
                    .result_multiplicity
        {
            return Err(Diagnostic::error(format!(
                "assumed binding parameter {ordinal} changes its runtime carrier, passing mode, lifetime or custody",
            )));
        }
        subjects.push(AssumedBindingSubjectCorrespondence::Parameter {
            ordinal: u32::try_from(ordinal).map_err(|_| {
                Diagnostic::error("assumed binding has too many parameter positions")
            })?,
            required: caller.symbol,
            supplied: supplier.symbol,
        });
    }
    if required.result.is_valid() {
        if !compatible_carriers(
            program,
            required.result,
            supplied.result,
            &required.lifetimes,
            &supplied.lifetimes,
        ) || program
            .facts
            .reference_custody
            .custody(required.result)
            .result_multiplicity
            != program
                .facts
                .reference_custody
                .custody(supplied.result)
                .result_multiplicity
        {
            return Err(Diagnostic::error(
                "assumed binding result changes its runtime carrier, lifetime or custody",
            ));
        }
        subjects.push(AssumedBindingSubjectCorrespondence::Result);
    }
    Ok(subjects)
}

/// Compare the actual runtime carriers while retaining explicit reference
/// access and the positional lifetime partition. Only qualifications erase;
/// nominal ownership, scalar kind, array extent and selector identity do not.
fn compatible_carriers(
    program: &TypedTrees,
    required: TypeReferenceHandle,
    supplied: TypeReferenceHandle,
    required_lifetimes: &[&Identifier],
    supplied_lifetimes: &[&Identifier],
) -> bool {
    if !required.is_valid() || !supplied.is_valid() {
        return required.is_valid() == supplied.is_valid();
    }
    let table = &program.type_reference_table;
    let required = table.unconstrained_type_reference(required);
    let supplied = table.unconstrained_type_reference(supplied);
    match (
        table.type_reference(required),
        table.type_reference(supplied),
    ) {
        (
            TypeReferenceNode::Reference {
                referee: caller,
                access: caller_access,
                lifetime: caller_lifetime,
            },
            TypeReferenceNode::Reference {
                referee: supplier,
                access: supplier_access,
                lifetime: supplier_lifetime,
            },
        ) => {
            caller_access == supplier_access
                && lifetime_matches(
                    caller_lifetime.as_ref(),
                    supplier_lifetime.as_ref(),
                    required_lifetimes,
                    supplied_lifetimes,
                )
                && compatible_carriers(
                    program,
                    *caller,
                    *supplier,
                    required_lifetimes,
                    supplied_lifetimes,
                )
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: caller,
                length: caller_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: supplier,
                length: supplier_length,
            },
        ) => {
            caller_length == supplier_length
                && compatible_carriers(
                    program,
                    *caller,
                    *supplier,
                    required_lifetimes,
                    supplied_lifetimes,
                )
        }
        (
            TypeReferenceNode::Slice {
                element_type: caller,
            },
            TypeReferenceNode::Slice {
                element_type: supplier,
            },
        ) => compatible_carriers(
            program,
            *caller,
            *supplier,
            required_lifetimes,
            supplied_lifetimes,
        ),
        (
            TypeReferenceNode::Generic {
                base_symbol: caller,
                arguments: caller_arguments,
                lifetime_arguments: caller_lifetimes,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: supplier,
                arguments: supplier_arguments,
                lifetime_arguments: supplier_lifetimes,
                ..
            },
        ) => {
            caller.is_valid()
                && caller == supplier
                && caller_lifetimes.len() == supplier_lifetimes.len()
                && caller_lifetimes
                    .iter()
                    .zip(supplier_lifetimes)
                    .all(|(caller, supplier)| {
                        lifetime_matches(
                            Some(caller),
                            Some(supplier),
                            required_lifetimes,
                            supplied_lifetimes,
                        )
                    })
                && type_arguments_match(
                    program,
                    *caller_arguments,
                    *supplier_arguments,
                    required_lifetimes,
                    supplied_lifetimes,
                )
        }
        (
            TypeReferenceNode::DynamicTrait {
                symbol: caller,
                arguments: caller_arguments,
                ..
            },
            TypeReferenceNode::DynamicTrait {
                symbol: supplier,
                arguments: supplier_arguments,
                ..
            },
        ) => {
            caller.is_valid()
                && caller == supplier
                && type_arguments_match(
                    program,
                    *caller_arguments,
                    *supplier_arguments,
                    required_lifetimes,
                    supplied_lifetimes,
                )
        }
        (
            TypeReferenceNode::Named { symbol: caller, .. },
            TypeReferenceNode::Named {
                symbol: supplier, ..
            },
        ) => caller.is_valid() && caller == supplier,
        (TypeReferenceNode::Named { symbol, .. }, _) => program
            .data_definition_by_symbol(*symbol)
            .and_then(|definition| definition.generic_instance)
            .is_some_and(|application| {
                application != required
                    && compatible_carriers(
                        program,
                        application,
                        supplied,
                        required_lifetimes,
                        supplied_lifetimes,
                    )
            }),
        (_, TypeReferenceNode::Named { symbol, .. }) => program
            .data_definition_by_symbol(*symbol)
            .and_then(|definition| definition.generic_instance)
            .is_some_and(|application| {
                application != supplied
                    && compatible_carriers(
                        program,
                        required,
                        application,
                        required_lifetimes,
                        supplied_lifetimes,
                    )
            }),
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        (TypeReferenceNode::ConstExpression(_), TypeReferenceNode::ConstExpression(_)) => {
            program.normalized_type_identity(required) == program.normalized_type_identity(supplied)
        }
        _ => false,
    }
}

fn type_arguments_match(
    program: &TypedTrees,
    required: arena::HandleSpan<TypeReferenceHandle>,
    supplied: arena::HandleSpan<TypeReferenceHandle>,
    required_lifetimes: &[&Identifier],
    supplied_lifetimes: &[&Identifier],
) -> bool {
    let required = program
        .type_reference_table
        .type_reference_handles(required);
    let supplied = program
        .type_reference_table
        .type_reference_handles(supplied);
    required.len() == supplied.len()
        && required.iter().zip(supplied).all(|(caller, supplier)| {
            compatible_carriers(
                program,
                *caller,
                *supplier,
                required_lifetimes,
                supplied_lifetimes,
            )
        })
}

fn lifetime_matches(
    required: Option<&Identifier>,
    supplied: Option<&Identifier>,
    required_lifetimes: &[&Identifier],
    supplied_lifetimes: &[&Identifier],
) -> bool {
    match (required, supplied) {
        (None, None) => true,
        (Some(caller), Some(supplier)) => {
            let caller_position = required_lifetimes
                .iter()
                .rposition(|lifetime| **lifetime == *caller);
            let supplier_position = supplied_lifetimes
                .iter()
                .rposition(|lifetime| **lifetime == *supplier);
            match (caller_position, supplier_position) {
                (Some(caller), Some(supplier)) => caller == supplier,
                // Lifetime constants are not new invocation binders.
                (None, None) => caller == supplier,
                _ => false,
            }
        }
        _ => false,
    }
}

pub(super) fn public_operational_compatibility(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
) -> Result<(), Diagnostic> {
    let caller = required.method();
    let supplier = supplied.method();
    let caller_invocations = required.invocations(program);
    let supplier_invocations = supplied.invocations(program);
    if (supplier.may_suspend && !caller.may_suspend)
        || (supplier.may_block && !caller.may_block)
        || caller.completion != supplier.completion
        || !supplier_invocations
            .iter()
            .all(|invocation| caller_invocations.contains(invocation))
        || (caller.terminates_guarantee
            && (!supplier.terminates_guarantee
                || !supplier.termination_premises.iter().all(|premise| {
                    caller.termination_premises.iter().any(|caller_premise| {
                        caller_premise.content_eq(
                            &required.schema.identities,
                            premise,
                            &supplied.schema.identities,
                        )
                    })
                })))
    {
        return Err(Diagnostic::error(
            "assumed binding supplier exceeds the required public invocation, suspension, blocking or termination contract",
        ));
    }
    // Own-slot reach is renamed only by the exact declaration pair. Other
    // supplier reach remains an independent dependency, never an assumption.
    let caller_reach = required.service_reach(program)?;
    let supplier_reach = supplied.service_reach(program)?;
    if supplier_reach.iter().any(|service| {
        let mapped = if supplied.own_service().is_valid() && *service == supplied.own_service() {
            required.own_service()
        } else {
            *service
        };
        !caller_reach.contains(&mapped)
    }) {
        return Err(Diagnostic::error(
            "assumed binding supplier exceeds the required public service reach",
        ));
    }
    Ok(())
}

pub(super) fn crash_compatibility(
    program: &CheckedTrees,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
) -> Result<(), Diagnostic> {
    let published = |requirement: &DeclaredRequirement<'_>| {
        requirement
            .crash_capsule(program)
            .map(|capsule| capsule.published_buckets())
            .or_else(|| {
                program
                    .facts
                    .contract_plans
                    .for_machine(requirement.symbol())
                    .map(|plan| plan.crash.published())
            })
    };
    let required = published(required).ok_or_else(|| {
        Diagnostic::error("assumed binding lost its required public crash ceiling")
    })?;
    let supplied = published(supplied).ok_or_else(|| {
        Diagnostic::error("assumed binding lost its supplied public crash ceiling")
    })?;
    if supplied.iter().any(|actual| {
        !required.iter().any(|allowed| {
            allowed.cause() == actual.cause()
                && (allowed.is_unconditional()
                    || actual
                        .alternative_guards()
                        .iter()
                        .all(|guard| allowed.alternative_guards().contains(guard)))
        })
    }) {
        return Err(Diagnostic::error(
            "assumed binding supplier exceeds the required public crash ceiling",
        ));
    }
    Ok(())
}

pub(super) fn native_compatibility(
    context: ProviderPlanningContext<'_>,
    required: &DeclaredRequirement<'_>,
    supplied: &DeclaredRequirement<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
    target: NativeTarget,
    opaque_representations: &[OpaqueRepresentationSelection],
) -> Result<
    (
        MaterializedBoundarySignature,
        MaterializedBoundarySignature,
        ValidatedBoundaryEntryPlan,
        AssumedBindingCallingCompatibility,
    ),
    Diagnostic,
> {
    let (caller_signature, caller_plan, caller_commitment) = native_application(
        context,
        required,
        calling_plans,
        target,
        opaque_representations,
    )?;
    let (supplier_signature, supplier_plan, supplier_commitment) = native_application(
        context,
        supplied,
        calling_plans,
        target,
        opaque_representations,
    )?;
    if caller_signature.shapes() != supplier_signature.shapes()
        || caller_signature.fields() != supplier_signature.fields()
        || caller_signature.parameters() != supplier_signature.parameters()
        || caller_signature.result() != supplier_signature.result()
        || caller_plan != supplier_plan
        || caller_signature.opaque_representation_uses().len()
            != supplier_signature.opaque_representation_uses().len()
        || !caller_signature
            .opaque_representation_uses()
            .iter()
            .zip(supplier_signature.opaque_representation_uses())
            .all(|(caller, supplier)| {
                caller.shape_root() == supplier.shape_root()
                    && caller.selected_application_commitment()
                        == supplier.selected_application_commitment()
            })
    {
        return Err(Diagnostic::error(
            "assumed binding must preserve the complete materialized value graph, opaque representation and validated calling plan",
        ));
    }
    Ok((
        caller_signature,
        supplier_signature,
        caller_plan,
        AssumedBindingCallingCompatibility {
            required: caller_commitment,
            supplied: supplier_commitment,
        },
    ))
}

fn native_application(
    context: ProviderPlanningContext<'_>,
    requirement: &DeclaredRequirement<'_>,
    calling_plans: &[BoundaryCallingPlanRealization],
    target: NativeTarget,
    opaque_representations: &[OpaqueRepresentationSelection],
) -> Result<
    (
        MaterializedBoundarySignature,
        ValidatedBoundaryEntryPlan,
        BoundaryCallingPlanCommitment,
    ),
    Diagnostic,
> {
    let matches = calling_plans
        .iter()
        .filter(|application| {
            application.requirement_machine == requirement.symbol()
                && application.boundary_trait == requirement.application.selected_interface()
                && application.boundary_arguments.is_empty()
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [application] if application.materialized_signature().native_target() == target => Ok((
            application.materialized_signature().clone(),
            application.validated_boundary_entry_plan.clone(),
            application.commitment,
        )),
        [] => {
            if matches!(requirement.declaration, PublicRequirement::Trait { .. })
                && crate::calling_policy_plans::calling_policy_owner(
                    context.typed,
                    requirement.application.selected_interface(),
                    requirement.symbol(),
                )
                .map_err(Diagnostic::error)?
                .is_some()
            {
                return Err(Diagnostic::error(
                    "assumed binding requirement is missing its exact authored calling application",
                ));
            }
            let signature = requirement.materialize(context, target, opaque_representations)?;
            let policy = CallingPolicy::native_for_target(target);
            let classified =
                compatibility_call_signature(&signature, policy, 0).map_err(Diagnostic::error)?;
            let plan = evaluate_ordinary_boundary_entry_plan(policy, &classified)
                .map_err(|reason| Diagnostic::error(reason.to_string()))?;
            // Retain the same canonical application content used by ordinary
            // calling realization. Receiving assembly must not rerun policy
            // evaluation or hash a local signature/debug representation.
            let (_, commitment) = boundary_plan_application_identity(&signature, &plan);
            Ok((
                signature,
                plan,
                BoundaryCallingPlanCommitment::from_digest(commitment),
            ))
        }
        _ => Err(Diagnostic::error(
            "assumed binding requirement has ambiguous or differently targeted calling applications",
        )),
    }
}
