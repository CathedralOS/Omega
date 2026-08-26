//! Build-time evaluation of source-authored calling policies.
//!
//! The source vocabulary lives in `omega/language/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.

use omega_calling_conventions::{
    BoundaryEntryPlan, BoundaryPlanResult, CallPlan, CallSignature, CallbackBinderRequirement,
    CallbackMaterialization, CallbackMaterializationContext, CallbackRequirementId, CallingPolicy,
    CallingPolicyRejection, EntryControl, EntryStack, IndirectPointerLocation, LayoutPlanId,
    LayoutSlotId, MachineRegime, MachineRegister, MachineState, MachineStateSet,
    NativeCallbackDemand, NativeParameterId, NativePlace, Preemption, RegisterSet, StatePlan,
    StaticMachineBinderId, SystemVEightbyteClass, ValidatedBoundaryEntryPlan, ValueClass,
    ValueLocation, ValuePlacement, ValueShape, callback_native_parameter_id,
    callback_requirement_id, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};
use omega_target::NativeTarget;
use psi_build_time_evaluation::BuildTimeValue;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

const PARAMETER_CAPACITY: usize = 32;
const VALUE_SHAPE_CAPACITY: usize = 256;
const VALUE_FIELD_CAPACITY: usize = 256;
const LOCATION_CAPACITY: usize = 16;
const REGISTER_CAPACITY: usize = 64;
const CALLBACK_MATERIALIZATION_CAPACITY: usize = 32;
const CALLBACK_FIELD_PATH_CAPACITY: usize = 16;

#[derive(Clone)]
struct TraitTypeBinding {
    parameter_symbol: psi_symbols::SymbolHandle,
    parameter_name: String,
    actual: TypeReferenceHandle,
}

struct BoundarySignatureInstance<'a> {
    signature: &'a psi_typed_trees::signature::StateSignature,
    requirement_identity: String,
    bindings: Vec<TraitTypeBinding>,
}

#[derive(Debug, Clone, Copy)]
enum BoundaryValueClass {
    Integer,
    Float,
    Reference,
    FixedArray { element: u16, length: u16 },
    Record { first_field: u16, field_count: u16 },
}

#[derive(Debug, Clone, Copy)]
struct BoundaryValueShape {
    class: BoundaryValueClass,
    byte_size: u16,
    alignment: u16,
}

#[derive(Debug, Clone, Copy)]
struct BoundaryValueField {
    shape: u16,
    byte_offset: u16,
}

#[derive(Debug, Clone)]
pub(crate) struct MaterializedBoundarySignature {
    shapes: Vec<BoundaryValueShape>,
    fields: Vec<BoundaryValueField>,
    parameters: Vec<u16>,
    callback_binders: Vec<BoundaryCallbackBinder>,
    callback_demands: Vec<NativeCallbackDemand>,
    native_parameters: Vec<BoundaryNativeParameter>,
    result: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundaryNativeParameter {
    identity: NativeParameterId,
    layout_data_symbol: psi_symbols::SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BoundaryCallbackBinder {
    pub(crate) binder: StaticMachineBinderId,
    pub(crate) requirement: CallbackRequirementId,
    pub(crate) static_machine_ordinal: u32,
    pub(crate) parameter_symbol: psi_symbols::SymbolHandle,
    pub(crate) requirement_trait: psi_symbols::SymbolHandle,
    pub(crate) requirement_machine: psi_symbols::SymbolHandle,
}

pub(crate) fn evaluate_compatibility_boundary_entry_plan(
    typed: &TypedTrees,
    trait_name: &str,
    method_name: &str,
    requirement_identity: &str,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
) -> Result<Option<BoundaryEntryPlan>, String> {
    let trait_leaf = trait_name.rsplit("::").next().unwrap_or(trait_name);
    let candidates = typed
        .traits()
        .iter()
        .filter(|definition| definition.name.as_str().rsplit("::").next() == Some(trait_leaf))
        .flat_map(|definition| {
            typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(|signature| signature.name.as_str() == method_name)
                .map(|signature| {
                    (
                        signature,
                        typed
                            .normalized_trait_requirement_overload_identity(definition, signature)
                            .identity(),
                    )
                })
        })
        .collect::<Vec<_>>();
    let Some(candidate_index) = exact_compatibility_overload_index(
        trait_name,
        method_name,
        requirement_identity,
        candidates.iter().map(|(_, identity)| identity.as_str()),
    )?
    else {
        return Ok(None);
    };
    let signature = candidates[candidate_index].0;
    let materialized = call_signature_from_typed(typed, signature, &[], requirement_identity)?;
    let classified =
        compatibility_call_signature(&materialized, policy, dispatch_only_parameter_count)?;
    evaluate_ordinary_boundary_entry_plan(policy, &classified)
        .map(|validated| Some(validated.plan().clone()))
        .map_err(|diagnostic| diagnostic.to_string())
}

fn exact_compatibility_overload_index<'identity>(
    trait_name: &str,
    method_name: &str,
    requirement_identity: &str,
    candidate_identities: impl IntoIterator<Item = &'identity str>,
) -> Result<Option<usize>, String> {
    if requirement_identity.is_empty() {
        return Err(format!(
            "compatibility calling-plan lookup for `{trait_name}::{method_name}` has no exact requirement overload identity"
        ));
    }
    let mut matches = candidate_identities
        .into_iter()
        .enumerate()
        .filter(|(_, identity)| *identity == requirement_identity);
    let Some((candidate_index, _)) = matches.next() else {
        return Ok(None);
    };
    let duplicate_count = matches.count();
    if duplicate_count != 0 {
        return Err(format!(
            "compatibility calling-plan lookup for `{trait_name}::{method_name}` matches {} exact requirement overload rows for identity `{requirement_identity}`",
            duplicate_count + 1,
        ));
    }
    Ok(Some(candidate_index))
}

fn compatibility_call_signature(
    materialized: &MaterializedBoundarySignature,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
) -> Result<CallSignature, String> {
    if dispatch_only_parameter_count > materialized.parameters.len() {
        return Err(format!(
            "compatibility calling plan removes {dispatch_only_parameter_count} dispatch-only parameter(s) from a signature with only {} parameter(s)",
            materialized.parameters.len()
        ));
    }
    let syscall_words = matches!(
        policy,
        CallingPolicy::LinuxSyscallX86_64 | CallingPolicy::LinuxSyscallAarch64
    );
    Ok(CallSignature {
        parameters: materialized
            .parameters
            .iter()
            .skip(dispatch_only_parameter_count)
            .copied()
            .map(|root| {
                if syscall_words {
                    Ok(ValueShape::integer(8, 8))
                } else {
                    classified_boundary_shape(&materialized, root, policy)
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        result: materialized
            .result
            .map(|root| {
                if syscall_words {
                    Ok(ValueShape::integer(8, 8))
                } else {
                    classified_boundary_shape(&materialized, root, policy)
                }
            })
            .transpose()?,
    })
}

fn classified_boundary_shape(
    signature: &MaterializedBoundarySignature,
    root: u16,
    policy: CallingPolicy,
) -> Result<ValueShape, String> {
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    let class = match shape.class {
        BoundaryValueClass::Integer | BoundaryValueClass::Reference => ValueClass::Integer,
        BoundaryValueClass::Float => ValueClass::Float,
        BoundaryValueClass::FixedArray { .. } | BoundaryValueClass::Record { .. } => {
            classify_boundary_aggregate(signature, root, policy)?
        }
    };
    Ok(ValueShape {
        class,
        byte_size: shape.byte_size,
        alignment: shape.alignment,
    })
}

/// Omega-owned realization state for one canonical source boundary contract.
///
/// Typed trees retain only the semantic key and fingerprint. Concrete ABI,
/// register, and stack choices stay beside the native/provider pipeline that
/// consumes them.
#[derive(Debug, Clone)]
pub(crate) struct BoundaryCallingPlanRealization {
    pub(crate) boundary_trait: psi_symbols::SymbolHandle,
    pub(crate) boundary_arguments: Vec<TypeReferenceHandle>,
    pub(crate) requirement_machine: psi_symbols::SymbolHandle,
    pub(crate) fingerprint: u64,
    pub(crate) boundary_entry_plan: BoundaryEntryPlan,
    pub(crate) callback_binders: Vec<BoundaryCallbackBinder>,
    pub(crate) callback_demands: Vec<NativeCallbackDemand>,
    pub(crate) callback_context_closed: bool,
    pub(crate) native_parameters: Vec<BoundaryNativeParameter>,
    pub(crate) materialized_signature: MaterializedBoundarySignature,
    pub(crate) policy_machine: String,
    pub(crate) relationship_span: psi_source::SourceSpan,
}

fn validate_retained_callback_binders(
    realization: &BoundaryCallingPlanRealization,
) -> Result<(), String> {
    for (index, binder) in realization.callback_binders.iter().enumerate() {
        if !binder.parameter_symbol.is_valid()
            || !binder.requirement_trait.is_valid()
            || !binder.requirement_machine.is_valid()
        {
            return Err(
                "evaluated registrar plan retained an invalid nominal callback binder symbol"
                    .to_owned(),
            );
        }
        if realization.callback_binders[..index].iter().any(|prior| {
            prior.binder == binder.binder
                || prior.static_machine_ordinal == binder.static_machine_ordinal
                || prior.parameter_symbol == binder.parameter_symbol
        }) {
            return Err(
                "evaluated registrar plan retained a duplicate nominal callback binder identity"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/// Bind checked nominal callback authority back to the one target-owned plan
/// realization that produced its retained fingerprint. This runs before any
/// backend lowering so a missing, duplicated, or changed placement recipe
/// cannot silently fall back to a convention oracle.
pub(crate) fn validate_nominal_callback_placement_bindings(
    checked: &psi_checked_trees::CheckedTrees,
    realizations: &[BoundaryCallingPlanRealization],
) -> Result<Vec<omega_backend_plan::BoundNominalCallbackPlacement>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut bound = Vec::new();
    for nominal_use in &checked.facts.nominal_machine_uses.uses {
        let matching = realizations
            .iter()
            .filter(|realization| {
                realization.boundary_trait == nominal_use.satisfaction_trait
                    && realization.requirement_machine == nominal_use.satisfaction_requirement
                    && realization.boundary_arguments.is_empty()
            })
            .collect::<Vec<_>>();
        let Some(placement) = nominal_use.callback_placement else {
            if !matching.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` lost its evaluated boundary calling-plan identity",
                    nominal_use.canonical_requirement_overload
                )));
            }
            continue;
        };
        let [realization] = matching.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` resolves to {} target calling-plan realizations; exactly one is required",
                nominal_use.canonical_requirement_overload,
                matching.len()
            )));
            continue;
        };
        let realized_signature = CallSignature {
            parameters: realization
                .boundary_entry_plan
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: realization
                .boundary_entry_plan
                .call
                .result
                .as_ref()
                .map(|placement| placement.shape),
        };
        let replayed = if realization.callback_context_closed {
            let context = CallbackMaterializationContext {
                binders: realization
                    .callback_binders
                    .iter()
                    .map(|binder| CallbackBinderRequirement {
                        binder: binder.binder,
                        requirement: binder.requirement,
                    })
                    .collect(),
                demands: realization.callback_demands.clone(),
            };
            validate_boundary_entry_plan_with_callback_materializations(
                realization.boundary_entry_plan.clone(),
                &realized_signature,
                &context,
            )
        } else {
            omega_calling_conventions::validate_boundary_entry_plan(
                realization.boundary_entry_plan.clone(),
                &realized_signature,
            )
        };
        let validated = match replayed {
            Ok(validated) => validated,
            Err(error) => {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` retained an invalid target calling-plan realization: {error}",
                    nominal_use.canonical_requirement_overload
                )));
                continue;
            }
        };
        let realized_fingerprint = validated.contract_fingerprint();
        if realization.fingerprint != realized_fingerprint
            || placement.boundary_calling_plan_fingerprint != realized_fingerprint
        {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` does not bind its exact evaluated target calling plan",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        }
        bound.push(omega_backend_plan::BoundNominalCallbackPlacement {
            site: nominal_use.site,
            registration_operation: nominal_use.registration_operation,
            static_machine_ordinal: nominal_use.static_machine_ordinal,
            selected_machine: nominal_use.selected_machine,
            selected_entry: nominal_use.selected_entry,
            satisfaction_trait: nominal_use.satisfaction_trait,
            satisfaction_requirement: nominal_use.satisfaction_requirement,
            canonical_requirement_overload: nominal_use.canonical_requirement_overload.clone(),
            boundary_calling_plan_fingerprint: realized_fingerprint,
            boundary_entry_plan: validated.plan().clone(),
        });
    }

    if diagnostics.is_empty() {
        Ok(bound)
    } else {
        Err(diagnostics)
    }
}

/// Discover concrete `Calling<C>` relationships, evaluate `C::plan` once for
/// every method in the boundary service surface, and retain only canonical
/// evaluated identities on the typed program.
pub(crate) fn compute_boundary_calling_plans(
    typed: &mut TypedTrees,
    package_inputs: Option<&crate::pipeline::PackageCompilationInputs>,
) -> Result<Vec<BoundaryCallingPlanRealization>, Vec<Diagnostic>> {
    let Some(calling_policy_trait) = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str().rsplit("::").next() == Some("CallingPolicy"))
    else {
        // Programs that do not import std::calling cannot accidentally opt in
        // merely by having an unrelated local trait named Calling.
        return Ok(Vec::new());
    };
    let calling_policy_symbol = calling_policy_trait.symbol;
    let Some(calling_trait) = typed.traits().iter().find(|definition| {
        definition.name.as_str().rsplit("::").next() == Some("Calling")
            && typed.trait_type_parameters(definition).len() == 1
            && typed.trait_machine_signatures(definition).is_empty()
    }) else {
        return Ok(Vec::new());
    };
    let calling_symbol = calling_trait.symbol;

    let mut pending = Vec::new();
    for boundary in typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        let relationships = typed
            .trait_requirements(boundary)
            .iter()
            .filter(|requirement| requirement.symbol == calling_symbol)
            .collect::<Vec<_>>();
        if relationships.is_empty() {
            continue;
        }
        if relationships.len() != 1 {
            return Err(vec![Diagnostic::error(format!(
                "boundary trait `{}` selects {} Calling<C> policies; exactly one concrete policy may define a boundary contract",
                boundary.name,
                relationships.len()
            ))
            .with_source_span(relationships[0].source_span)]);
        }
        let relationship = relationships[0];
        let relationship_span = relationship.source_span;
        let arguments = typed
            .type_reference_table
            .type_reference_handles(relationship.arguments);
        if arguments.len() != 1 {
            return Err(vec![Diagnostic::error(format!(
                "boundary trait `{}` has a Calling relationship with {} policy arguments; expected one",
                boundary.name,
                arguments.len()
            ))
            .with_source_span(relationship_span)]);
        }
        let instances =
            boundary_policy_instances(typed, boundary, arguments[0]).map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "boundary trait `{}` cannot evaluate Calling<C>: {reason}",
                        boundary.name
                    ))
                    .with_source_span(relationship_span),
                ]
            })?;
        for (boundary_arguments, policy_argument) in instances {
            let policy_type =
                concrete_policy_type_name(typed, policy_argument).map_err(|reason| {
                    vec![
                        Diagnostic::error(format!(
                            "boundary trait `{}` cannot evaluate Calling<C>: {reason}",
                            boundary.name
                        ))
                        .with_source_span(relationship_span),
                    ]
                })?;
            let policy_machine = find_policy_machine(typed, &policy_type).ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "boundary trait `{}` selects `{policy_type}`, but no `{policy_type}::plan` machine exists",
                    boundary.name
                ))
                .with_source_span(relationship_span)]
            })?;
            let satisfies_policy =
                typed
                    .machine_trait_conformances(policy_machine)
                    .iter()
                    .any(|conformance| {
                        conformance.symbol == calling_policy_symbol
                            && conformance.requirement.as_ref().is_some_and(|requirement| {
                                requirement.as_str().rsplit("::").next() == Some("plan")
                            })
                    });
            if !satisfies_policy {
                return Err(vec![
                    Diagnostic::error(format!(
                        "calling-policy machine `{}` must satisfy `CallingPolicy::plan`",
                        policy_machine.name
                    ))
                    .with_source_span(relationship_span),
                ]);
            }

            let mut signatures = Vec::new();
            collect_boundary_signatures(
                typed,
                boundary,
                &boundary_arguments,
                &mut Vec::new(),
                &mut signatures,
            );
            for signature_instance in signatures {
                let signature = signature_instance.signature;
                let boundary_signature = call_signature_from_typed(
                    typed,
                    signature,
                    &signature_instance.bindings,
                    &signature_instance.requirement_identity,
                )
                .map_err(|reason| {
                    vec![
                        Diagnostic::error(format!(
                            "cannot materialize boundary signature `{}::{}` for `{}`: {reason}",
                            boundary.name, signature.name, policy_machine.name
                        ))
                        .with_source_span(relationship_span),
                    ]
                })?;
                pending.push((
                    boundary.symbol,
                    boundary_arguments.clone(),
                    signature.symbol,
                    policy_machine.name.as_str().to_owned(),
                    boundary_signature,
                    relationship_span,
                ));
            }
        }
    }

    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let admission =
        psi_build_time_evaluation::BuildTimeAdmissionPlan::infer_with_selection_authority(
            typed,
            package_inputs.map(|inputs| {
                std::sync::Arc::new(inputs.clone())
                    as std::sync::Arc<dyn psi_build_time_evaluation::BuildTimeSelectionAuthority>
            }),
        );
    let mut evaluated = Vec::with_capacity(pending.len());
    for (
        boundary_trait,
        boundary_arguments,
        requirement_machine,
        policy_machine,
        signature,
        relationship_span,
    ) in pending
    {
        let validated = evaluate_materialized_calling_policy_plan(
            typed,
            &admission,
            &policy_machine,
            &signature,
            Some(psi_build_time_evaluation::BuildTimeInvocationCustody::Source(relationship_span)),
        )
        .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(relationship_span)])?;
        evaluated.push(BoundaryCallingPlanRealization {
            boundary_trait,
            boundary_arguments,
            requirement_machine,
            fingerprint: validated.contract_fingerprint(),
            boundary_entry_plan: validated.plan().clone(),
            callback_binders: signature.callback_binders.clone(),
            callback_demands: signature.callback_demands.clone(),
            callback_context_closed: false,
            native_parameters: signature.native_parameters.clone(),
            materialized_signature: signature,
            policy_machine,
            relationship_span,
        });
    }
    for realization in &evaluated {
        validate_retained_callback_binders(realization)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        typed.record_boundary_calling_plan(
            psi_typed_trees::typed_trees::BoundaryCallingPlanIdentity {
                boundary_trait: realization.boundary_trait,
                boundary_arguments: realization.boundary_arguments.clone(),
                requirement_machine: realization.requirement_machine,
                fingerprint: realization.fingerprint,
            },
        );
    }
    Ok(evaluated)
}

/// Re-evaluate outbound registrar policies only after the checked native
/// layout pipeline has published its authoritative private-demand catalog.
/// The target-neutral typed reports are deliberately not consulted here.
pub(crate) fn close_outbound_callback_materializations(
    checked: &mut psi_checked_trees::CheckedTrees,
    realizations: &mut [BoundaryCallingPlanRealization],
    native_target: NativeTarget,
    package_inputs: Option<&crate::pipeline::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    let layout_plan = omega_layout::build_layout_plan(checked, native_target)
        .map_err(|diagnostic| vec![diagnostic])?;
    let admission =
        psi_build_time_evaluation::BuildTimeAdmissionPlan::infer_with_selection_authority(
            &checked.typed,
            package_inputs.map(|inputs| {
                std::sync::Arc::new(inputs.clone())
                    as std::sync::Arc<dyn psi_build_time_evaluation::BuildTimeSelectionAuthority>
            }),
        );

    for realization in realizations {
        let mut demands = Vec::new();
        for parameter in &realization.native_parameters {
            for demand in layout_plan
                .private_callback_demands
                .iter()
                .filter(|demand| demand.data_symbol == parameter.layout_data_symbol)
            {
                demands.push(demand.native_demand(parameter.identity));
            }
        }
        if demands.is_empty() && realization.callback_binders.is_empty() {
            continue;
        }
        if demands.len() > CALLBACK_MATERIALIZATION_CAPACITY {
            return Err(vec![Diagnostic::error(format!(
                "boundary registrar has {} target-closed private callback demands; calling policies currently support at most {CALLBACK_MATERIALIZATION_CAPACITY}",
                demands.len()
            ))
            .with_source_span(realization.relationship_span)]);
        }
        demands.sort_unstable_by(|left, right| left.destination.cmp(&right.destination));
        if demands
            .windows(2)
            .any(|pair| pair[0].destination == pair[1].destination)
        {
            return Err(vec![
                Diagnostic::error(
                    "boundary registrar target closure repeats one exact private callback demand",
                )
                .with_source_span(realization.relationship_span),
            ]);
        }

        let old_fingerprint = realization.fingerprint;
        let mut signature = realization.materialized_signature.clone();
        signature.callback_demands = demands.clone();
        let validated = evaluate_materialized_calling_policy_plan(
            &checked.typed,
            &admission,
            &realization.policy_machine,
            &signature,
            Some(
                psi_build_time_evaluation::BuildTimeInvocationCustody::Source(
                    realization.relationship_span,
                ),
            ),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
        })?;
        let classified = CallSignature {
            parameters: validated
                .plan()
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: validated
                .plan()
                .call
                .result
                .as_ref()
                .map(|placement| placement.shape),
        };
        let context = callback_materialization_context(&signature);
        let validated = validate_boundary_entry_plan_with_callback_materializations(
            validated.plan().clone(),
            &classified,
            &context,
        )
        .map_err(|diagnostic| {
            vec![
                Diagnostic::error(format!(
                    "target-closed outbound callback materialization is invalid: {diagnostic}"
                ))
                .with_source_span(realization.relationship_span),
            ]
        })?;
        let new_fingerprint = validated.contract_fingerprint();
        realization.fingerprint = new_fingerprint;
        realization.boundary_entry_plan = validated.plan().clone();
        realization.callback_demands = demands;
        realization.callback_context_closed = true;
        realization.materialized_signature = signature;

        checked.typed.record_boundary_calling_plan(
            psi_typed_trees::typed_trees::BoundaryCallingPlanIdentity {
                boundary_trait: realization.boundary_trait,
                boundary_arguments: realization.boundary_arguments.clone(),
                requirement_machine: realization.requirement_machine,
                fingerprint: new_fingerprint,
            },
        );
        for nominal_use in &mut checked.facts.nominal_machine_uses.uses {
            if nominal_use.satisfaction_trait == realization.boundary_trait
                && nominal_use.satisfaction_requirement == realization.requirement_machine
                && nominal_use.callback_placement.is_some_and(|placement| {
                    placement.boundary_calling_plan_fingerprint == old_fingerprint
                })
            {
                nominal_use.callback_placement =
                    Some(psi_checked_trees::CheckedCallbackPlacementIdentity {
                        boundary_calling_plan_fingerprint: new_fingerprint,
                    });
            }
        }
    }
    Ok(())
}

fn boundary_policy_instances(
    typed: &TypedTrees,
    boundary: &psi_typed_trees::trait_definition::TraitDefinition,
    policy_argument: TypeReferenceHandle,
) -> Result<Vec<(Vec<TypeReferenceHandle>, TypeReferenceHandle)>, String> {
    let parameters = typed.trait_type_parameters(boundary);
    if parameters.is_empty() {
        return Ok(vec![(Vec::new(), policy_argument)]);
    }
    let policy_parameter_index = named_parameter_index(typed, policy_argument, parameters);

    let mut instances = Vec::new();
    for conformance in typed
        .conformances()
        .iter()
        .filter(|conformance| names_match(conformance.trait_name.as_str(), boundary.name.as_str()))
    {
        let Some(carrier_name) = conformance.carrier_name() else {
            continue;
        };
        let arguments = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .to_vec();
        if arguments.len() != parameters.len() {
            return Err(format!(
                "conformance `{} satisfies {}` supplies {} argument(s), expected {}",
                carrier_name,
                conformance.trait_name,
                arguments.len(),
                parameters.len()
            ));
        }
        let concrete_policy = policy_parameter_index.map_or(policy_argument, |parameter_index| {
            arguments[parameter_index]
        });
        instances.push((arguments, concrete_policy));
    }

    // A generic declaration is not itself a callable ABI. With no concrete
    // conformance there is no requirement identity to publish yet.
    Ok(instances)
}

fn names_match(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

fn named_parameter_index(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    parameters: &[psi_typed_trees::data::TypeParameter],
) -> Option<usize> {
    loop {
        match typed.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                return parameters.iter().position(|parameter| {
                    (parameter.symbol.is_valid() && parameter.symbol == *symbol)
                        || parameter.name.as_str() == name.as_str()
                });
            }
            _ => return None,
        }
    }
}

fn concrete_policy_type_name(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Result<String, String> {
    loop {
        match typed.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { name, .. } => return Ok(name.as_str().to_owned()),
            other => {
                return Err(format!(
                    "policy argument `{}` is not a concrete data type ({other:?})",
                    typed.display_type_reference(type_reference)
                ));
            }
        }
    }
}

fn find_policy_machine<'a>(
    typed: &'a TypedTrees,
    policy_type: &str,
) -> Option<&'a psi_typed_trees::machine::Machine> {
    let expected = format!("{policy_type}::plan");
    typed.machines().iter().find(|machine| {
        machine.name.as_str() == expected
            || (machine.name.as_str().ends_with("::plan")
                && machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|data| data.as_str() == policy_type))
    })
}

fn collect_boundary_signatures<'a>(
    typed: &'a TypedTrees,
    definition: &'a psi_typed_trees::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
    visited: &mut Vec<psi_symbols::SymbolHandle>,
    signatures: &mut Vec<BoundarySignatureInstance<'a>>,
) {
    if visited.contains(&definition.symbol) {
        return;
    }
    visited.push(definition.symbol);
    let bindings = trait_type_bindings(typed, definition, arguments);
    for requirement in typed.trait_requirements(definition) {
        if let Some(parent) = typed
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == requirement.symbol && candidate.is_boundary)
        {
            let parent_arguments = typed
                .type_reference_table
                .type_reference_handles(requirement.arguments)
                .iter()
                .map(|argument| substituted_type_reference(typed, *argument, &bindings))
                .collect::<Vec<_>>();
            collect_boundary_signatures(typed, parent, &parent_arguments, visited, signatures);
        }
    }
    for signature in typed.trait_machine_signatures(definition) {
        let requirement_identity = typed
            .normalized_trait_requirement_overload_identity(definition, signature)
            .identity();
        if !signatures
            .iter()
            .any(|candidate| candidate.requirement_identity == requirement_identity)
        {
            signatures.push(BoundarySignatureInstance {
                signature,
                requirement_identity,
                bindings: bindings.clone(),
            });
        }
    }
    visited.pop();
}

fn trait_type_bindings(
    typed: &TypedTrees,
    definition: &psi_typed_trees::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
) -> Vec<TraitTypeBinding> {
    typed
        .trait_type_parameters(definition)
        .iter()
        .zip(arguments.iter().copied())
        .map(|(parameter, actual)| TraitTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.as_str().to_owned(),
            actual,
        })
        .collect()
}

fn substituted_type_reference(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
) -> TypeReferenceHandle {
    let TypeReferenceNode::Named { symbol, name } =
        typed.type_reference_table.type_reference(type_reference)
    else {
        return type_reference;
    };
    bindings
        .iter()
        .find(|binding| {
            (binding.parameter_symbol.is_valid() && binding.parameter_symbol == *symbol)
                || binding.parameter_name == name.as_str()
        })
        .map_or(type_reference, |binding| binding.actual)
}

fn call_signature_from_typed(
    typed: &TypedTrees,
    signature: &psi_typed_trees::signature::StateSignature,
    bindings: &[TraitTypeBinding],
    owner_requirement_identity: &str,
) -> Result<MaterializedBoundarySignature, String> {
    let mut shapes = Vec::new();
    let mut fields = Vec::new();
    let mut parameters = Vec::new();
    let runtime_parameters = typed
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let mut native_parameters = Vec::with_capacity(runtime_parameters.len());
    for (ordinal, parameter) in runtime_parameters.iter().enumerate() {
        let (_, root) = value_shape_from_type(
            typed,
            parameter.type_reference,
            bindings,
            &mut Vec::new(),
            &mut shapes,
            &mut fields,
        )?;
        parameters.push(root);
        let type_reference = substituted_type_reference(typed, parameter.type_reference, bindings);
        let layout_data_symbol = exact_boundary_layout_root_symbol(typed, type_reference);
        native_parameters.push(BoundaryNativeParameter {
            identity: callback_native_parameter_id(
                owner_requirement_identity,
                u32::try_from(ordinal)
                    .map_err(|_| "boundary signature has too many runtime parameters")?,
            ),
            layout_data_symbol,
        });
    }
    let result = if signature.return_type.is_valid() {
        let (_, root) = value_shape_from_type(
            typed,
            signature.return_type,
            bindings,
            &mut Vec::new(),
            &mut shapes,
            &mut fields,
        )?;
        Some(root)
    } else {
        None
    };
    let mut callback_binders = Vec::new();
    let mut static_machine_ordinal = 0u32;
    for parameter in typed.state_signature_type_parameters(signature) {
        let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        let ordinal = static_machine_ordinal;
        static_machine_ordinal = static_machine_ordinal
            .checked_add(1)
            .ok_or_else(|| "boundary signature has too many static machine binders".to_owned())?;
        let psi_typed_trees::data::MachineParameterContract::Nominal {
            trait_definition,
            requirement,
        } = contract
        else {
            continue;
        };
        let trait_definition_row = typed
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == *trait_definition)
            .ok_or_else(|| "nominal callback binder lost its declaring trait".to_owned())?;
        let requirement_row = typed
            .trait_machine_signatures(trait_definition_row)
            .iter()
            .find(|candidate| candidate.symbol == *requirement)
            .ok_or_else(|| "nominal callback binder lost its exact requirement".to_owned())?;
        let requirement_identity = typed
            .normalized_trait_requirement_overload_identity(trait_definition_row, requirement_row)
            .identity();
        let binder_identity = callback_plan_identity(
            b"omega.callback-binder.v1",
            &[
                owner_requirement_identity.as_bytes(),
                &ordinal.to_le_bytes(),
                parameter.name.as_str().as_bytes(),
            ],
        );
        callback_binders.push(BoundaryCallbackBinder {
            binder: StaticMachineBinderId::new(binder_identity)
                .expect("callback binder fingerprint is nonzero"),
            requirement: callback_requirement_id(&requirement_identity),
            static_machine_ordinal: ordinal,
            parameter_symbol: parameter.symbol,
            requirement_trait: *trait_definition,
            requirement_machine: *requirement,
        });
    }
    Ok(MaterializedBoundarySignature {
        shapes,
        fields,
        parameters,
        callback_binders,
        callback_demands: Vec::new(),
        native_parameters,
        result,
    })
}

fn exact_boundary_layout_root_symbol(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> psi_symbols::SymbolHandle {
    loop {
        match typed.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => return *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => return *base_symbol,
            TypeReferenceNode::FixedArray { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => return psi_symbols::SymbolHandle::invalid(),
        }
    }
}

fn callback_plan_identity(domain: &[u8], parts: &[&[u8]]) -> u64 {
    let mut identity = 0xcbf2_9ce4_8422_2325u64;
    for bytes in std::iter::once(domain).chain(parts.iter().copied()) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            identity ^= u64::from(byte);
            identity = identity.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    if identity == 0 { 1 } else { identity }
}

fn value_shape_from_type(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
    shapes: &mut Vec<BoundaryValueShape>,
    fields: &mut Vec<BoundaryValueField>,
) -> Result<(ValueShape, u16), String> {
    type_reference = substituted_type_reference(typed, type_reference, bindings);
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        let abi = primitive_value_shape(primitive)?;
        let class = if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
            BoundaryValueClass::Float
        } else {
            BoundaryValueClass::Integer
        };
        let root = push_boundary_shape(
            shapes,
            BoundaryValueShape {
                class,
                byte_size: abi.byte_size,
                alignment: abi.alignment,
            },
        )?;
        return Ok((abi, root));
    }
    match typed.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            value_shape_from_type(typed, *base_type, bindings, visiting, shapes, fields)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            let mut referee = substituted_type_reference(typed, *referee, bindings);
            loop {
                match typed.type_reference_table.type_reference(referee) {
                    TypeReferenceNode::Constrained { base_type, .. } => {
                        referee = substituted_type_reference(typed, *base_type, bindings);
                    }
                    _ => break,
                }
            }
            let is_fat = match typed.type_reference_table.type_reference(referee) {
                TypeReferenceNode::Slice { .. } => true,
                TypeReferenceNode::Named { name, .. } => name.as_str() == "string",
                _ => false,
            };
            let abi = ValueShape::integer(if is_fat { 16 } else { 8 }, 8);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Reference,
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::Slice { .. } => {
            let abi = ValueShape::integer(16, 8);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Reference,
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
        } => {
            let (element, element_root) =
                value_shape_from_type(typed, *element_type, bindings, visiting, shapes, fields)?;
            let size = usize::from(element.byte_size)
                .checked_mul(*length)
                .ok_or_else(|| "fixed-array boundary shape overflows".to_owned())?;
            let abi = ValueShape::integer(
                u16::try_from(size)
                    .map_err(|_| "fixed-array boundary shape exceeds 65535 bytes".to_owned())?,
                element.alignment,
            );
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::FixedArray {
                        element: element_root,
                        length: u16::try_from(*length).map_err(|_| {
                            "fixed-array boundary length exceeds 65535 elements".to_owned()
                        })?,
                    },
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::Named { symbol, name } => plain_data_value_shape(
            typed,
            *symbol,
            name.as_str(),
            bindings,
            visiting,
            shapes,
            fields,
        ),
        TypeReferenceNode::DynamicTrait { .. } => {
            let abi = ValueShape::integer(16, 8);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Reference,
                    byte_size: abi.byte_size,
                    alignment: abi.alignment,
                },
            )?;
            Ok((abi, root))
        }
        TypeReferenceNode::Generic { .. } => Err(format!(
            "generic value type `{}` is not concrete at the Calling<C> relationship",
            typed.display_type_reference(type_reference)
        )),
        TypeReferenceNode::FixedArray { .. } => Err(format!(
            "array type `{}` still has a non-literal length",
            typed.display_type_reference(type_reference)
        )),
        TypeReferenceNode::ConstExpression(_) => {
            Err("a proof-static index expression is not a boundary value shape".to_owned())
        }
        TypeReferenceNode::Unit => Err("unit is not a boundary value shape".to_owned()),
    }
}

/// Derive the exact address-free `Extent` graph for the currently admitted
/// UEFI/Microsoft program-storage source schema. A future SysV/AAPCS source
/// schema must retain and classify its own structural graph rather than
/// passing this fence.
pub(crate) fn selected_program_storage_source_extent_value_layout(
    typed: &TypedTrees,
    slot: omega_target::ProgramEntrySlotDeclaration,
    type_reference: TypeReferenceHandle,
) -> Result<super::ProgramEntrySourceExtentValueLayout, String> {
    if slot.owner != omega_target::TargetProfile::UefiX64
        || slot.schema != omega_target::ProgramEntrySchema::ProgramStorageApplication
        || slot.visible_parameters
            != omega_target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        || slot.semantic_calling_convention
            != Some(omega_target::ProgramEntryCallingConvention::MicrosoftX64)
    {
        return Err(
            "selected-source Extent layout derivation is restricted to the exact UEFI/Microsoft program-storage schema"
                .into(),
        );
    }
    let mut shapes = Vec::new();
    let mut fields = Vec::new();
    let (shape, root) = value_shape_from_type(
        typed,
        type_reference,
        &[],
        &mut Vec::new(),
        &mut shapes,
        &mut fields,
    )?;
    let mut base_type = type_reference;
    while let TypeReferenceNode::Constrained {
        base_type: unconstrained,
        ..
    } = typed.type_reference_table.type_reference(base_type)
    {
        base_type = *unconstrained;
    }
    let TypeReferenceNode::Named {
        symbol: data_symbol,
        ..
    } = typed.type_reference_table.type_reference(base_type)
    else {
        return Err("selected program-storage root is not a named Extent record".into());
    };
    let definition = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *data_symbol)
        .ok_or_else(|| "selected program-storage Extent has no data definition".to_owned())?;
    let [
        psi_typed_trees::data::DataMember::Field(base),
        psi_typed_trees::data::DataMember::Field(length),
    ] = typed.data_members(definition)
    else {
        return Err("selected program-storage Extent must declare exactly two fields".into());
    };
    if base.relevance.is_erased()
        || length.relevance.is_erased()
        || base.name.as_str() != "base"
        || length.name.as_str() != "length"
        || typed.primitive_type_reference(base.type_reference) != Some(PrimitiveType::Addr)
        || typed.primitive_type_reference(length.type_reference) != Some(PrimitiveType::U64)
    {
        return Err(
            "selected program-storage Extent must declare exact `base: addr; length: u64` fields"
                .into(),
        );
    }
    let root = shapes
        .get(usize::from(root))
        .ok_or_else(|| "selected program-storage Extent lost its record shape root".to_owned())?;
    let BoundaryValueClass::Record {
        first_field,
        field_count: 2,
    } = root.class
    else {
        return Err(
            "selected program-storage Extent is not an exact two-field record graph".into(),
        );
    };
    let normalized_fields = fields
        .get(usize::from(first_field)..usize::from(first_field) + 2)
        .ok_or_else(|| "selected program-storage Extent field graph is out of bounds".to_owned())?;
    let [base_field, length_field] = normalized_fields else {
        unreachable!("exact Extent record graph retains two fields")
    };
    let scalar_shape = |field: &BoundaryValueField| -> Result<ValueShape, String> {
        let child = shapes.get(usize::from(field.shape)).ok_or_else(|| {
            "selected program-storage Extent child shape is out of bounds".to_owned()
        })?;
        if !matches!(child.class, BoundaryValueClass::Integer) {
            return Err("selected program-storage Extent field is not an integer scalar".into());
        }
        Ok(ValueShape::integer(child.byte_size, child.alignment))
    };
    super::ProgramEntrySourceExtentValueLayout::from_checked_record(
        *data_symbol,
        base.symbol,
        base_field.byte_offset,
        scalar_shape(base_field)?,
        length.symbol,
        length_field.byte_offset,
        scalar_shape(length_field)?,
        shape,
    )
}

fn push_boundary_shape(
    shapes: &mut Vec<BoundaryValueShape>,
    shape: BoundaryValueShape,
) -> Result<u16, String> {
    if shapes.len() >= VALUE_SHAPE_CAPACITY {
        return Err(format!(
            "boundary signature exceeds the normalized shape capacity of {VALUE_SHAPE_CAPACITY}"
        ));
    }
    let index = u16::try_from(shapes.len()).expect("boundary shape capacity fits u16");
    shapes.push(shape);
    Ok(index)
}

fn primitive_value_shape(primitive: PrimitiveType) -> Result<ValueShape, String> {
    let byte_size = primitive.scalar_byte_size().ok_or_else(|| {
        format!(
            "primitive `{}` has no concrete boundary size",
            primitive.name()
        )
    })?;
    let byte_size = u16::try_from(byte_size).expect("primitive size fits u16");
    Ok(match primitive {
        PrimitiveType::F32 | PrimitiveType::F64 => ValueShape::float(byte_size),
        _ => ValueShape::integer(byte_size, byte_size.min(8).max(1)),
    })
}

fn plain_data_value_shape(
    typed: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
    name: &str,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
    shapes: &mut Vec<BoundaryValueShape>,
    fields: &mut Vec<BoundaryValueField>,
) -> Result<(ValueShape, u16), String> {
    if visiting.contains(&symbol) {
        return Err(format!(
            "recursive by-value data `{name}` has no finite boundary shape"
        ));
    }
    let definition = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
        .ok_or_else(|| format!("named boundary type `{name}` has no data definition"))?;
    let planned_layout = typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_symbol == definition.symbol);
    let runtime_field_symbols = typed
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            psi_typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                Some(field.symbol)
            }
            psi_typed_trees::data::DataMember::Field(_)
            | psi_typed_trees::data::DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    if let Some(layout) = planned_layout
        && (layout.offsets.len() != runtime_field_symbols.len()
            || layout.field_symbols != runtime_field_symbols)
    {
        return Err(format!(
            "plan-laid data `{name}` has {} members but its layout publishes {} offsets",
            runtime_field_symbols.len(),
            layout.offsets.len()
        ));
    }
    visiting.push(symbol);
    let mut size = 0usize;
    let mut alignment = 1usize;
    let mut float_member_size = None;
    let mut float_members = 0usize;
    let mut record_fields = Vec::new();
    for member in typed.data_members(definition) {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            visiting.pop();
            return Err(format!(
                "case data `{name}` is not yet classifiable as a boundary value; pass it by reference"
            ));
        };
        if field.relevance.is_erased() {
            continue;
        }
        let field_index = record_fields.len();
        let (shape, field_root) = if let Some(stored_integer) = planned_layout.and_then(|layout| {
            layout
                .integer_fields
                .iter()
                .find(|integer| integer.field_index == field_index)
        }) {
            let stored_byte_size = stored_integer.stored_width_bits / 8;
            if stored_integer.stored_width_bits == 0
                || stored_integer.stored_width_bits % 8 != 0
                || stored_byte_size > 8
            {
                visiting.pop();
                return Err(format!(
                    "plan-laid data `{name}` field `{}` has invalid stored-integer width {}",
                    field.name, stored_integer.stored_width_bits
                ));
            }
            let shape = ValueShape::integer(stored_byte_size, stored_byte_size);
            let root = push_boundary_shape(
                shapes,
                BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                },
            )?;
            (shape, root)
        } else {
            value_shape_from_type(
                typed,
                field.type_reference,
                bindings,
                visiting,
                shapes,
                fields,
            )?
        };
        let field_alignment = usize::from(shape.alignment);
        let field_offset = planned_layout.map_or_else(
            || align_up(size, field_alignment),
            |layout| layout.offsets[field_index],
        );
        size = size.max(
            field_offset
                .checked_add(usize::from(shape.byte_size))
                .ok_or_else(|| format!("data `{name}` boundary layout overflows"))?,
        );
        record_fields.push(BoundaryValueField {
            shape: field_root,
            byte_offset: u16::try_from(field_offset)
                .map_err(|_| format!("data `{name}` field offset exceeds 65535 bytes"))?,
        });
        alignment = alignment.max(field_alignment);
        match shape.class {
            ValueClass::Float
                if float_members != usize::MAX
                    && float_member_size.is_none_or(|prior| prior == shape.byte_size) =>
            {
                float_member_size = Some(shape.byte_size);
                float_members += 1;
            }
            _ => float_members = usize::MAX,
        }
    }
    visiting.pop();
    if let Some(layout) = planned_layout {
        size = layout.size;
        alignment = layout.align;
    } else {
        size = align_up(size, alignment);
    }
    if size == 0 {
        return Err(format!(
            "zero-sized data `{name}` cannot cross a boundary by value"
        ));
    }
    let size = u16::try_from(size)
        .map_err(|_| format!("data `{name}` boundary shape exceeds 65535 bytes"))?;
    let alignment = u16::try_from(alignment)
        .map_err(|_| format!("data `{name}` boundary alignment exceeds 65535 bytes"))?;
    let abi = if (1..=4).contains(&float_members)
        && float_member_size.is_some_and(|member_size| {
            usize::from(size) == usize::from(member_size) * float_members
                && alignment == member_size
                && record_fields.iter().enumerate().all(|(index, field)| {
                    usize::from(field.byte_offset) == index * usize::from(member_size)
                })
        }) {
        ValueShape::homogeneous_float_aggregate(
            float_member_size.expect("float member count has size"),
            u8::try_from(float_members).expect("HFA member count fits u8"),
        )
    } else {
        ValueShape::integer(size, alignment)
    };
    if fields.len().saturating_add(record_fields.len()) > VALUE_FIELD_CAPACITY {
        return Err(format!(
            "boundary signature exceeds the normalized field capacity of {VALUE_FIELD_CAPACITY}"
        ));
    }
    let first_field = u16::try_from(fields.len()).expect("boundary field capacity fits u16");
    let field_count = u16::try_from(record_fields.len())
        .map_err(|_| format!("data `{name}` has more than 65535 boundary fields"))?;
    fields.extend(record_fields);
    let root = push_boundary_shape(
        shapes,
        BoundaryValueShape {
            class: BoundaryValueClass::Record {
                first_field,
                field_count,
            },
            byte_size: size,
            alignment,
        },
    )?;
    Ok((abi, root))
}

fn align_up(value: usize, alignment: usize) -> usize {
    value
        .checked_add(alignment.saturating_sub(1))
        .map(|value| value / alignment * alignment)
        .unwrap_or(usize::MAX)
}

pub fn evaluate_calling_policy_plan(
    typed: &TypedTrees,
    policy_machine: &str,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    let materialized = materialized_boundary_signature_from_abi(signature)?;
    let admission = psi_build_time_evaluation::BuildTimeAdmissionPlan::infer(typed);
    evaluate_materialized_calling_policy_plan(
        typed,
        &admission,
        policy_machine,
        &materialized,
        None,
    )
}

fn evaluate_materialized_calling_policy_plan(
    typed: &TypedTrees,
    admission: &psi_build_time_evaluation::BuildTimeAdmissionPlan,
    policy_machine: &str,
    signature: &MaterializedBoundarySignature,
    custody: Option<psi_build_time_evaluation::BuildTimeInvocationCustody>,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    if signature.parameters.len() > PARAMETER_CAPACITY {
        return Err(format!(
            "boundary signature has {} parameters; calling policies currently support at most {PARAMETER_CAPACITY}",
            signature.parameters.len()
        ));
    }

    let arguments = vec![build_boundary_signature(signature)];
    let value = match custody {
        Some(custody) => {
            admission.evaluate_machine_for_invocation(typed, policy_machine, arguments, custody)
        }
        None => admission.evaluate_machine(typed, policy_machine, arguments),
    }
    .map_err(|reason| {
        format!("build-time evaluation of calling policy `{policy_machine}` failed: {reason}")
    })?;
    let result = decode_boundary_plan_result(&value).map_err(|reason| {
        format!("calling policy `{policy_machine}` returned an invalid result: {reason}")
    })?;

    validate_materialized_boundary_plan_result(result, signature)
}

pub(crate) fn materialized_boundary_signature_from_abi(
    signature: &CallSignature,
) -> Result<MaterializedBoundarySignature, String> {
    let mut shapes = Vec::new();
    let mut parameters = Vec::new();
    for shape in &signature.parameters {
        parameters.push(push_boundary_shape(
            &mut shapes,
            BoundaryValueShape {
                class: match shape.class {
                    ValueClass::Float => BoundaryValueClass::Float,
                    _ => BoundaryValueClass::Integer,
                },
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            },
        )?);
    }
    let result = signature
        .result
        .map(|shape| {
            push_boundary_shape(
                &mut shapes,
                BoundaryValueShape {
                    class: match shape.class {
                        ValueClass::Float => BoundaryValueClass::Float,
                        _ => BoundaryValueClass::Integer,
                    },
                    byte_size: shape.byte_size,
                    alignment: shape.alignment,
                },
            )
        })
        .transpose()?;
    Ok(MaterializedBoundarySignature {
        shapes,
        fields: Vec::new(),
        parameters,
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        native_parameters: Vec::new(),
        result,
    })
}

fn build_boundary_signature(signature: &MaterializedBoundarySignature) -> BuildTimeValue {
    let mut shapes = Vec::with_capacity(VALUE_SHAPE_CAPACITY);
    for index in 0..VALUE_SHAPE_CAPACITY {
        shapes.push(build_boundary_value_shape(
            signature
                .shapes
                .get(index)
                .copied()
                .unwrap_or(BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: 0,
                    alignment: 0,
                }),
        ));
    }
    let mut fields = Vec::with_capacity(VALUE_FIELD_CAPACITY);
    for index in 0..VALUE_FIELD_CAPACITY {
        fields.push(build_boundary_value_field(
            signature
                .fields
                .get(index)
                .copied()
                .unwrap_or(BoundaryValueField {
                    shape: 0,
                    byte_offset: 0,
                }),
        ));
    }
    let mut parameters = Vec::with_capacity(PARAMETER_CAPACITY);
    for index in 0..PARAMETER_CAPACITY {
        parameters.push(BuildTimeValue::Int(i64::from(
            signature.parameters.get(index).copied().unwrap_or(0),
        )));
    }
    let mut callback_binders = Vec::with_capacity(CALLBACK_MATERIALIZATION_CAPACITY);
    for index in 0..CALLBACK_MATERIALIZATION_CAPACITY {
        let binder = signature.callback_binders.get(index);
        callback_binders.push(BuildTimeValue::Struct {
            type_name: "CallbackBinderIdentity".to_owned(),
            fields: vec![
                (
                    "binder".to_owned(),
                    BuildTimeValue::Int(binder.map_or(0, |row| row.binder.get()) as i64),
                ),
                (
                    "requirement".to_owned(),
                    BuildTimeValue::Int(binder.map_or(0, |row| row.requirement.get()) as i64),
                ),
            ],
        });
    }
    let mut callback_demands = Vec::with_capacity(CALLBACK_MATERIALIZATION_CAPACITY);
    for index in 0..CALLBACK_MATERIALIZATION_CAPACITY {
        let demand = signature.callback_demands.get(index);
        callback_demands.push(BuildTimeValue::Struct {
            type_name: "NativeCallbackDemandIdentity".to_owned(),
            fields: vec![
                (
                    "destination".to_owned(),
                    demand.map_or_else(
                        || {
                            case(
                                "NativePlace::Parameter",
                                vec![("parameter".to_owned(), BuildTimeValue::Int(0))],
                            )
                        },
                        |row| build_native_place(&row.destination),
                    ),
                ),
                (
                    "requirement".to_owned(),
                    BuildTimeValue::Int(demand.map_or(0, |row| row.requirement.get()) as i64),
                ),
            ],
        });
    }
    BuildTimeValue::Struct {
        type_name: "BoundarySignature".to_owned(),
        fields: vec![
            ("shapes".to_owned(), BuildTimeValue::Array(shapes)),
            (
                "shape_count".to_owned(),
                BuildTimeValue::Int(signature.shapes.len() as i64),
            ),
            ("fields".to_owned(), BuildTimeValue::Array(fields)),
            (
                "field_count".to_owned(),
                BuildTimeValue::Int(signature.fields.len() as i64),
            ),
            ("parameters".to_owned(), BuildTimeValue::Array(parameters)),
            (
                "parameter_count".to_owned(),
                BuildTimeValue::Int(signature.parameters.len() as i64),
            ),
            (
                "callback_binders".to_owned(),
                BuildTimeValue::Array(callback_binders),
            ),
            (
                "callback_binder_count".to_owned(),
                BuildTimeValue::Int(signature.callback_binders.len() as i64),
            ),
            (
                "callback_demands".to_owned(),
                BuildTimeValue::Array(callback_demands),
            ),
            (
                "callback_demand_count".to_owned(),
                BuildTimeValue::Int(signature.callback_demands.len() as i64),
            ),
            (
                "has_result".to_owned(),
                BuildTimeValue::Bool(signature.result.is_some()),
            ),
            (
                "result".to_owned(),
                BuildTimeValue::Int(i64::from(signature.result.unwrap_or(0))),
            ),
        ],
    }
}

fn build_native_place(place: &NativePlace) -> BuildTimeValue {
    match place {
        NativePlace::Parameter(parameter) => case(
            "NativePlace::Parameter",
            vec![(
                "parameter".to_owned(),
                BuildTimeValue::Int(parameter.get() as i64),
            )],
        ),
        NativePlace::Field {
            parameter,
            layout,
            field_path,
        } => {
            let mut slots = Vec::with_capacity(CALLBACK_FIELD_PATH_CAPACITY);
            for index in 0..CALLBACK_FIELD_PATH_CAPACITY {
                slots.push(BuildTimeValue::Int(
                    field_path.get(index).map_or(0, |slot| slot.get()) as i64,
                ));
            }
            case(
                "NativePlace::Field",
                vec![
                    (
                        "parameter".to_owned(),
                        BuildTimeValue::Int(parameter.get() as i64),
                    ),
                    (
                        "layout".to_owned(),
                        BuildTimeValue::Int(layout.get() as i64),
                    ),
                    ("field_path".to_owned(), BuildTimeValue::Array(slots)),
                    (
                        "field_path_count".to_owned(),
                        BuildTimeValue::Int(field_path.len() as i64),
                    ),
                ],
            )
        }
    }
}

fn build_boundary_value_shape(shape: BoundaryValueShape) -> BuildTimeValue {
    let class = match shape.class {
        BoundaryValueClass::Integer => case("Integer", vec![]),
        BoundaryValueClass::Float => case("Float", vec![]),
        BoundaryValueClass::Reference => case("Reference", vec![]),
        BoundaryValueClass::FixedArray { element, length } => case(
            "FixedArray",
            vec![
                (
                    "element".to_owned(),
                    BuildTimeValue::Int(i64::from(element)),
                ),
                ("length".to_owned(), BuildTimeValue::Int(i64::from(length))),
            ],
        ),
        BoundaryValueClass::Record {
            first_field,
            field_count,
        } => case(
            "Record",
            vec![
                (
                    "first_field".to_owned(),
                    BuildTimeValue::Int(i64::from(first_field)),
                ),
                (
                    "field_count".to_owned(),
                    BuildTimeValue::Int(i64::from(field_count)),
                ),
            ],
        ),
    };
    BuildTimeValue::Struct {
        type_name: "ValueShape".to_owned(),
        fields: vec![
            ("class".to_owned(), class),
            (
                "byte_size".to_owned(),
                BuildTimeValue::Int(i64::from(shape.byte_size)),
            ),
            (
                "alignment".to_owned(),
                BuildTimeValue::Int(i64::from(shape.alignment)),
            ),
        ],
    }
}

fn build_boundary_value_field(field: BoundaryValueField) -> BuildTimeValue {
    BuildTimeValue::Struct {
        type_name: "ValueField".to_owned(),
        fields: vec![
            (
                "shape".to_owned(),
                BuildTimeValue::Int(i64::from(field.shape)),
            ),
            (
                "byte_offset".to_owned(),
                BuildTimeValue::Int(i64::from(field.byte_offset)),
            ),
        ],
    }
}

fn validate_materialized_boundary_plan_result(
    result: BoundaryPlanResult,
    signature: &MaterializedBoundarySignature,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    let BoundaryPlanResult::Accepted(plan) = result else {
        return validate_boundary_plan_result(result, &CallSignature::default())
            .map_err(|diagnostic| diagnostic.to_string());
    };
    if plan.call.parameters.len() != signature.parameters.len() {
        return Err(invalid_authored_plan(format!(
            "plan places {} parameters for a boundary signature with {} parameters",
            plan.call.parameters.len(),
            signature.parameters.len()
        )));
    }
    if plan.call.result.is_some() != signature.result.is_some() {
        return Err(invalid_authored_plan(
            "plan result presence does not match the boundary signature".to_owned(),
        ));
    }
    for (index, (placement, root)) in plan
        .call
        .parameters
        .iter()
        .zip(signature.parameters.iter().copied())
        .enumerate()
    {
        validate_authored_abi_shape(signature, root, placement.shape, plan.call.policy).map_err(
            |reason| {
                invalid_authored_plan(format!(
                    "parameter {index} classification is invalid: {reason}"
                ))
            },
        )?;
    }
    if let (Some(placement), Some(root)) = (&plan.call.result, signature.result) {
        validate_authored_abi_shape(signature, root, placement.shape, plan.call.policy).map_err(
            |reason| invalid_authored_plan(format!("result classification is invalid: {reason}")),
        )?;
    }
    let classified = CallSignature {
        parameters: plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: plan.call.result.as_ref().map(|placement| placement.shape),
    };
    if signature.callback_demands.is_empty() {
        return validate_boundary_plan_result(BoundaryPlanResult::Accepted(plan), &classified)
            .map_err(|diagnostic| diagnostic.to_string());
    }
    let context = callback_materialization_context(signature);
    validate_boundary_entry_plan_with_callback_materializations(plan, &classified, &context)
        .map_err(|diagnostic| diagnostic.to_string())
}

fn callback_materialization_context(
    signature: &MaterializedBoundarySignature,
) -> CallbackMaterializationContext {
    CallbackMaterializationContext {
        binders: signature
            .callback_binders
            .iter()
            .map(|binder| CallbackBinderRequirement {
                binder: binder.binder,
                requirement: binder.requirement,
            })
            .collect(),
        demands: signature.callback_demands.clone(),
    }
}

fn invalid_authored_plan(reason: String) -> String {
    format!("calling policy accepted an invalid plan: {reason}")
}

fn validate_authored_abi_shape(
    signature: &MaterializedBoundarySignature,
    root: u16,
    abi: ValueShape,
    policy: CallingPolicy,
) -> Result<(), String> {
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    if abi.byte_size != shape.byte_size || abi.alignment != shape.alignment {
        return Err(format!(
            "ABI shape {}/{} does not preserve semantic size/alignment {}/{}",
            abi.byte_size, abi.alignment, shape.byte_size, shape.alignment
        ));
    }
    let expected = match shape.class {
        BoundaryValueClass::Integer | BoundaryValueClass::Reference => ValueClass::Integer,
        BoundaryValueClass::Float => ValueClass::Float,
        BoundaryValueClass::FixedArray { .. } | BoundaryValueClass::Record { .. } => {
            classify_boundary_aggregate(signature, root, policy)?
        }
    };
    if abi.class != expected {
        return Err(format!(
            "policy published ABI class {:?}, but its recursive public shape requires {:?} under {:?}",
            abi.class, expected, policy
        ));
    }
    Ok(())
}

fn classify_boundary_aggregate(
    signature: &MaterializedBoundarySignature,
    root: u16,
    policy: CallingPolicy,
) -> Result<ValueClass, String> {
    if policy == CallingPolicy::MicrosoftX64 {
        return Ok(ValueClass::Integer);
    }
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    let mut leaves = Vec::new();
    collect_boundary_scalar_leaves(signature, root, 0, &mut leaves, 0)?;
    if let Some(members) = homogeneous_float_leaf_shape(shape, &leaves) {
        if policy == CallingPolicy::Aapcs64
            || (policy == CallingPolicy::SystemVAMD64 && members > 1)
        {
            return Ok(ValueClass::HomogeneousFloatAggregate { members });
        }
        if policy == CallingPolicy::SystemVAMD64 && members == 1 {
            return Ok(ValueClass::Float);
        }
    }
    if policy != CallingPolicy::SystemVAMD64 || shape.byte_size > 16 {
        return Ok(ValueClass::Integer);
    }
    let mut classes = [None, None];
    for &(offset, byte_size, is_float) in &leaves {
        let eightbyte = usize::from(offset) / 8;
        let last = usize::from(offset)
            .checked_add(usize::from(byte_size))
            .and_then(|end| end.checked_sub(1))
            .ok_or_else(|| "aggregate contains a zero-width scalar leaf".to_owned())?;
        if eightbyte > 1 || eightbyte != last / 8 {
            return Err(
                "aggregate has a scalar leaf crossing a SysV eightbyte boundary".to_owned(),
            );
        }
        classes[eightbyte] = Some(match classes[eightbyte] {
            Some(existing_is_sse) => existing_is_sse && is_float,
            None => is_float,
        });
    }
    let first = classes[0].ok_or_else(|| "aggregate has no scalar leaves".to_owned())?;
    let second = if shape.byte_size > 8 {
        classes[1].ok_or_else(|| "aggregate leaves do not cover its second eightbyte".to_owned())?
    } else {
        false
    };
    if !first && !second {
        return Ok(ValueClass::Integer);
    }
    if shape.byte_size <= 8 {
        return Err(
            "one-eightbyte non-homogeneous SSE aggregates are not representable in the closed ABI vocabulary"
                .to_owned(),
        );
    }
    Ok(ValueClass::SystemVAggregate {
        first: if first {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
        second: if second {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
    })
}

fn homogeneous_float_leaf_shape(
    aggregate: &BoundaryValueShape,
    leaves: &[(u16, u16, bool)],
) -> Option<u8> {
    let &(first_offset, member_size, true) = leaves.first()? else {
        return None;
    };
    let members = u8::try_from(leaves.len()).ok()?;
    (first_offset == 0
        && (1..=4).contains(&members)
        && matches!(member_size, 4 | 8)
        && leaves
            .iter()
            .enumerate()
            .all(|(index, &(offset, size, is_float))| {
                is_float
                    && size == member_size
                    && usize::from(offset) == index * usize::from(member_size)
            })
        && aggregate.byte_size == member_size * u16::from(members)
        && aggregate.alignment == member_size)
        .then_some(members)
}

fn collect_boundary_scalar_leaves(
    signature: &MaterializedBoundarySignature,
    root: u16,
    base_offset: u16,
    leaves: &mut Vec<(u16, u16, bool)>,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("boundary shape nesting exceeds 32 levels".to_owned());
    }
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    match shape.class {
        BoundaryValueClass::Integer | BoundaryValueClass::Reference => {
            leaves.push((base_offset, shape.byte_size, false));
        }
        BoundaryValueClass::Float => leaves.push((base_offset, shape.byte_size, true)),
        BoundaryValueClass::FixedArray { element, length } => {
            let element_shape = signature
                .shapes
                .get(usize::from(element))
                .ok_or_else(|| format!("array element shape {element} is outside the graph"))?;
            for index in 0..length {
                let offset = element_shape
                    .byte_size
                    .checked_mul(index)
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| "fixed-array leaf offset overflows u16".to_owned())?;
                collect_boundary_scalar_leaves(signature, element, offset, leaves, depth + 1)?;
            }
        }
        BoundaryValueClass::Record {
            first_field,
            field_count,
        } => {
            let start = usize::from(first_field);
            let end = start
                .checked_add(usize::from(field_count))
                .ok_or_else(|| "record field range overflows".to_owned())?;
            let fields = signature
                .fields
                .get(start..end)
                .ok_or_else(|| "record field range is outside the normalized graph".to_owned())?;
            for field in fields {
                let offset = base_offset
                    .checked_add(field.byte_offset)
                    .ok_or_else(|| "record field offset overflows u16".to_owned())?;
                collect_boundary_scalar_leaves(signature, field.shape, offset, leaves, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn case(variant: &str, payload: Vec<(String, BuildTimeValue)>) -> BuildTimeValue {
    BuildTimeValue::Case {
        variant: variant.to_owned(),
        payload,
    }
}

fn decode_boundary_plan_result(value: &BuildTimeValue) -> Result<BoundaryPlanResult, String> {
    let (variant, payload) = case_parts(value, "BoundaryPlanResult")?;
    match variant {
        "Accepted" => Ok(BoundaryPlanResult::Accepted(decode_boundary_entry_plan(
            field(payload, "plan", "Accepted")?,
        )?)),
        "Rejected" => {
            let reason = struct_parts(field(payload, "reason", "Rejected")?, "rejection")?;
            let bytes = text(field(reason, "reason", "CallingPolicyRejection")?, "reason")?;
            let reason = String::from_utf8(bytes.to_vec())
                .map_err(|_| "CallingPolicyRejection.reason is not valid UTF-8".to_owned())?;
            Ok(BoundaryPlanResult::Rejected(CallingPolicyRejection::new(
                reason,
            )))
        }
        other => Err(format!(
            "BoundaryPlanResult case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_boundary_entry_plan(value: &BuildTimeValue) -> Result<BoundaryEntryPlan, String> {
    let fields = struct_parts(value, "BoundaryEntryPlan")?;
    Ok(BoundaryEntryPlan {
        call: decode_call_plan(field(fields, "call", "BoundaryEntryPlan")?)?,
        state: decode_state_plan(field(fields, "state", "BoundaryEntryPlan")?)?,
    })
}

fn decode_call_plan(value: &BuildTimeValue) -> Result<CallPlan, String> {
    let fields = struct_parts(value, "CallPlan")?;
    let parameters = decode_counted_array(
        field(fields, "parameters", "CallPlan")?,
        uint(
            field(fields, "parameter_count", "CallPlan")?,
            "parameter_count",
        )?,
        PARAMETER_CAPACITY,
        "CallPlan.parameters",
        decode_value_placement,
    )?;
    let has_result = bool_value(field(fields, "has_result", "CallPlan")?, "has_result")?;
    let callback_materializations = decode_counted_array(
        field(fields, "callback_materializations", "CallPlan")?,
        uint(
            field(fields, "callback_materialization_count", "CallPlan")?,
            "callback_materialization_count",
        )?,
        CALLBACK_MATERIALIZATION_CAPACITY,
        "CallPlan.callback_materializations",
        decode_callback_materialization,
    )?;
    Ok(CallPlan {
        policy: decode_calling_convention(field(fields, "convention", "CallPlan")?)?,
        parameters,
        result: has_result
            .then(|| decode_value_placement(field(fields, "result", "CallPlan")?))
            .transpose()?,
        callback_materializations,
        ordinary_clobbers: decode_register_set(field(fields, "ordinary_clobbers", "CallPlan")?)?,
        stack_alignment: u16_value(
            field(fields, "stack_alignment", "CallPlan")?,
            "stack_alignment",
        )?,
        shadow_bytes: u16_value(field(fields, "shadow_bytes", "CallPlan")?, "shadow_bytes")?,
        entry_control: decode_entry_control(field(fields, "entry_control", "CallPlan")?)?,
    })
}

fn decode_callback_materialization(
    value: &BuildTimeValue,
) -> Result<CallbackMaterialization, String> {
    let fields = struct_parts(value, "CallbackMaterialization")?;
    let binder = uint(
        field(fields, "binder", "CallbackMaterialization")?,
        "CallbackMaterialization.binder",
    )?;
    Ok(CallbackMaterialization {
        binder: StaticMachineBinderId::new(binder).ok_or_else(|| {
            "CallbackMaterialization.binder must be a nonzero compiler-issued identity".to_owned()
        })?,
        destination: decode_native_place(field(fields, "destination", "CallbackMaterialization")?)?,
    })
}

fn decode_native_place(value: &BuildTimeValue) -> Result<NativePlace, String> {
    let (variant, payload) = case_parts(value, "NativePlace")?;
    let parameter = |payload: &[(String, BuildTimeValue)]| {
        let identity = uint(
            field(payload, "parameter", "NativePlace")?,
            "NativePlace.parameter",
        )?;
        NativeParameterId::new(identity)
            .ok_or_else(|| "NativePlace.parameter must be a nonzero nominal identity".to_owned())
    };
    match variant {
        "Parameter" => Ok(NativePlace::Parameter(parameter(payload)?)),
        "Field" => {
            let layout = uint(
                field(payload, "layout", "NativePlace::Field")?,
                "NativePlace::Field.layout",
            )?;
            let field_path = decode_counted_array(
                field(payload, "field_path", "NativePlace::Field")?,
                uint(
                    field(payload, "field_path_count", "NativePlace::Field")?,
                    "NativePlace::Field.field_path_count",
                )?,
                CALLBACK_FIELD_PATH_CAPACITY,
                "NativePlace::Field.field_path",
                |value| {
                    let identity = uint(value, "NativePlace::Field.field_path slot")?;
                    LayoutSlotId::new(identity).ok_or_else(|| {
                        "NativePlace::Field.field_path contains a zero slot identity".to_owned()
                    })
                },
            )?;
            if field_path.is_empty() {
                return Err("NativePlace::Field.field_path cannot be empty".to_owned());
            }
            Ok(NativePlace::Field {
                parameter: parameter(payload)?,
                layout: LayoutPlanId::new(layout).ok_or_else(|| {
                    "NativePlace::Field.layout must be a nonzero nominal identity".to_owned()
                })?,
                field_path,
            })
        }
        other => Err(format!(
            "NativePlace case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_calling_convention(value: &BuildTimeValue) -> Result<CallingPolicy, String> {
    let (variant, _) = case_parts(value, "CallingConvention")?;
    match variant {
        "MicrosoftX64" => Ok(CallingPolicy::MicrosoftX64),
        "SystemVAMD64" => Ok(CallingPolicy::SystemVAMD64),
        "Aapcs64" => Ok(CallingPolicy::Aapcs64),
        "LinuxSyscallX86_64" => Ok(CallingPolicy::LinuxSyscallX86_64),
        "LinuxSyscallAarch64" => Ok(CallingPolicy::LinuxSyscallAarch64),
        other => Err(format!(
            "CallingConvention case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_value_placement(value: &BuildTimeValue) -> Result<ValuePlacement, String> {
    let fields = struct_parts(value, "ValuePlacement")?;
    Ok(ValuePlacement {
        shape: decode_value_shape(field(fields, "shape", "ValuePlacement")?)?,
        locations: decode_counted_array(
            field(fields, "locations", "ValuePlacement")?,
            uint(
                field(fields, "location_count", "ValuePlacement")?,
                "location_count",
            )?,
            LOCATION_CAPACITY,
            "ValuePlacement.locations",
            decode_value_location,
        )?,
    })
}

fn decode_value_shape(value: &BuildTimeValue) -> Result<ValueShape, String> {
    let fields = struct_parts(value, "AbiValueShape")?;
    let (variant, payload) = case_parts(field(fields, "class", "AbiValueShape")?, "AbiValueClass")?;
    let class = match variant {
        "Integer" => ValueClass::Integer,
        "Float" => ValueClass::Float,
        "HomogeneousFloatAggregate" => ValueClass::HomogeneousFloatAggregate {
            members: u8_value(
                field(payload, "members", "HomogeneousFloatAggregate")?,
                "members",
            )?,
        },
        "SystemVAggregate" => ValueClass::SystemVAggregate {
            first: decode_eightbyte_class(field(payload, "first", "SystemVAggregate")?)?,
            second: decode_eightbyte_class(field(payload, "second", "SystemVAggregate")?)?,
        },
        other => {
            return Err(format!(
                "AbiValueClass case `{other}` is outside the compiler-owned vocabulary"
            ));
        }
    };
    Ok(ValueShape {
        class,
        byte_size: u16_value(field(fields, "byte_size", "AbiValueShape")?, "byte_size")?,
        alignment: u16_value(field(fields, "alignment", "AbiValueShape")?, "alignment")?,
    })
}

fn decode_eightbyte_class(value: &BuildTimeValue) -> Result<SystemVEightbyteClass, String> {
    match case_parts(value, "SystemVEightbyteClass")?.0 {
        "Integer" => Ok(SystemVEightbyteClass::Integer),
        "Sse" => Ok(SystemVEightbyteClass::Sse),
        other => Err(format!(
            "SystemVEightbyteClass case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_value_location(value: &BuildTimeValue) -> Result<ValueLocation, String> {
    let (variant, payload) = case_parts(value, "ValueLocation")?;
    match variant {
        "Register" => Ok(ValueLocation::Register {
            register: decode_register(field(payload, "register", "Register location")?)?,
            value_byte_offset: u16_value(
                field(payload, "value_byte_offset", "Register location")?,
                "value_byte_offset",
            )?,
            byte_size: u16_value(
                field(payload, "byte_size", "Register location")?,
                "byte_size",
            )?,
        }),
        "Stack" => Ok(ValueLocation::Stack {
            stack_byte_offset: u32_value(
                field(payload, "stack_byte_offset", "Stack location")?,
                "stack_byte_offset",
            )?,
            value_byte_offset: u16_value(
                field(payload, "value_byte_offset", "Stack location")?,
                "value_byte_offset",
            )?,
            byte_size: u16_value(field(payload, "byte_size", "Stack location")?, "byte_size")?,
            alignment: u16_value(field(payload, "alignment", "Stack location")?, "alignment")?,
        }),
        "Indirect" => {
            let has_copy =
                bool_value(field(payload, "has_copy", "Indirect location")?, "has_copy")?;
            Ok(ValueLocation::Indirect {
                pointer: decode_indirect_pointer(field(payload, "pointer", "Indirect location")?)?,
                copy_stack_byte_offset: has_copy
                    .then(|| {
                        u32_value(
                            field(payload, "copy_stack_byte_offset", "Indirect location")?,
                            "copy_stack_byte_offset",
                        )
                    })
                    .transpose()?,
                byte_size: u16_value(
                    field(payload, "byte_size", "Indirect location")?,
                    "byte_size",
                )?,
                alignment: u16_value(
                    field(payload, "alignment", "Indirect location")?,
                    "alignment",
                )?,
            })
        }
        other => Err(format!(
            "ValueLocation case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_indirect_pointer(value: &BuildTimeValue) -> Result<IndirectPointerLocation, String> {
    let (variant, payload) = case_parts(value, "IndirectPointerLocation")?;
    match variant {
        "Register" => Ok(IndirectPointerLocation::Register(decode_register(field(
            payload,
            "register",
            "indirect register pointer",
        )?)?)),
        "Stack" => Ok(IndirectPointerLocation::Stack {
            stack_byte_offset: u32_value(
                field(payload, "stack_byte_offset", "indirect stack pointer")?,
                "stack_byte_offset",
            )?,
            alignment: u16_value(
                field(payload, "alignment", "indirect stack pointer")?,
                "alignment",
            )?,
        }),
        other => Err(format!(
            "IndirectPointerLocation case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_register_set(value: &BuildTimeValue) -> Result<RegisterSet, String> {
    let fields = struct_parts(value, "RegisterSet")?;
    Ok(RegisterSet::new(decode_counted_array(
        field(fields, "registers", "RegisterSet")?,
        uint(
            field(fields, "register_count", "RegisterSet")?,
            "register_count",
        )?,
        REGISTER_CAPACITY,
        "RegisterSet.registers",
        decode_register,
    )?))
}

fn decode_register(value: &BuildTimeValue) -> Result<MachineRegister, String> {
    let (variant, payload) = case_parts(value, "MachineRegister")?;
    let indexed = |name: &str| u8_value(field(payload, "index", name)?, "register index");
    match variant {
        "X86Rax" => Ok(MachineRegister::X86Rax),
        "X86Rcx" => Ok(MachineRegister::X86Rcx),
        "X86Rdx" => Ok(MachineRegister::X86Rdx),
        "X86Rbx" => Ok(MachineRegister::X86Rbx),
        "X86Rsp" => Ok(MachineRegister::X86Rsp),
        "X86Rbp" => Ok(MachineRegister::X86Rbp),
        "X86Rsi" => Ok(MachineRegister::X86Rsi),
        "X86Rdi" => Ok(MachineRegister::X86Rdi),
        "X86R8" => Ok(MachineRegister::X86R8),
        "X86R9" => Ok(MachineRegister::X86R9),
        "X86R10" => Ok(MachineRegister::X86R10),
        "X86R11" => Ok(MachineRegister::X86R11),
        "X86R12" => Ok(MachineRegister::X86R12),
        "X86R13" => Ok(MachineRegister::X86R13),
        "X86R14" => Ok(MachineRegister::X86R14),
        "X86R15" => Ok(MachineRegister::X86R15),
        "X86Xmm" => Ok(MachineRegister::X86Xmm(indexed("X86Xmm")?)),
        "Aarch64X" => Ok(MachineRegister::Aarch64X(indexed("Aarch64X")?)),
        "Aarch64V" => Ok(MachineRegister::Aarch64V(indexed("Aarch64V")?)),
        other => Err(format!(
            "MachineRegister case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_entry_control(value: &BuildTimeValue) -> Result<EntryControl, String> {
    let (variant, payload) = case_parts(value, "EntryControl")?;
    match variant {
        "CallReturn" => Ok(EntryControl::CallReturn),
        "SupervisorCall" => Ok(EntryControl::SupervisorCall {
            number_register: decode_register(field(payload, "number_register", "SupervisorCall")?)?,
            immediate: u16_value(field(payload, "immediate", "SupervisorCall")?, "immediate")?,
        }),
        "InterruptReturn" => Ok(EntryControl::InterruptReturn),
        other => Err(format!(
            "EntryControl case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_state_plan(value: &BuildTimeValue) -> Result<StatePlan, String> {
    let fields = struct_parts(value, "StatePlan")?;
    Ok(StatePlan {
        initial_regime: decode_machine_regime(field(fields, "initial_regime", "StatePlan")?)?,
        interrupted_state: decode_machine_state_set(field(
            fields,
            "interrupted_state",
            "StatePlan",
        )?)?,
        saved_state: decode_machine_state_set(field(fields, "saved_state", "StatePlan")?)?,
        restored_state: decode_machine_state_set(field(fields, "restored_state", "StatePlan")?)?,
        permitted_transitive_use: decode_machine_state_set(field(
            fields,
            "permitted_transitive_use",
            "StatePlan",
        )?)?,
        stack: decode_entry_stack(field(fields, "stack", "StatePlan")?)?,
        preemption: decode_preemption(field(fields, "preemption", "StatePlan")?)?,
    })
}

fn decode_machine_regime(value: &BuildTimeValue) -> Result<MachineRegime, String> {
    let (variant, payload) = case_parts(value, "MachineRegime")?;
    match variant {
        "X86Long64" => Ok(MachineRegime::X86Long64),
        "Aarch64A64" => Ok(MachineRegime::Aarch64A64 {
            exception_level: u8_value(
                field(payload, "exception_level", "Aarch64A64")?,
                "exception_level",
            )?,
        }),
        other => Err(format!(
            "MachineRegime case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_machine_state_set(value: &BuildTimeValue) -> Result<MachineStateSet, String> {
    let fields = struct_parts(value, "MachineStateSet")?;
    let candidates = [
        ("general_registers", MachineState::GeneralRegisters),
        ("vector_registers", MachineState::VectorRegisters),
        ("flags", MachineState::Flags),
        ("instruction_pointer", MachineState::InstructionPointer),
        ("stack_pointer", MachineState::StackPointer),
        ("segment_state", MachineState::SegmentState),
        ("control_state", MachineState::ControlState),
        ("debug_state", MachineState::DebugState),
        ("extended_state", MachineState::ExtendedState),
    ];
    let mut states = Vec::new();
    for (name, state) in candidates {
        if bool_value(field(fields, name, "MachineStateSet")?, name)? {
            states.push(state);
        }
    }
    Ok(MachineStateSet::new(states))
}

fn decode_entry_stack(value: &BuildTimeValue) -> Result<EntryStack, String> {
    let (variant, payload) = case_parts(value, "EntryStack")?;
    match variant {
        "Interrupted" => Ok(EntryStack::Interrupted),
        "Dedicated" => Ok(EntryStack::Dedicated {
            class: u16_value(field(payload, "class", "Dedicated")?, "class")?,
        }),
        "ProviderSelected" => Ok(EntryStack::ProviderSelected),
        other => Err(format!(
            "EntryStack case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_preemption(value: &BuildTimeValue) -> Result<Preemption, String> {
    let (variant, payload) = case_parts(value, "Preemption")?;
    match variant {
        "NotApplicable" => Ok(Preemption::NotApplicable),
        "Masked" => Ok(Preemption::Masked),
        "Nestable" => Ok(Preemption::Nestable {
            maximum_depth: u16_value(
                field(payload, "maximum_depth", "Nestable")?,
                "maximum_depth",
            )?,
        }),
        "ProviderDefined" => Ok(Preemption::ProviderDefined),
        other => Err(format!(
            "Preemption case `{other}` is outside the compiler-owned vocabulary"
        )),
    }
}

fn decode_counted_array<T>(
    value: &BuildTimeValue,
    count: u64,
    capacity: usize,
    context: &str,
    decode: impl Fn(&BuildTimeValue) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    if count > capacity as u64 {
        return Err(format!("{context} count {count} is outside 0..={capacity}"));
    }
    let count = count as usize;
    let BuildTimeValue::Array(values) = value else {
        return Err(format!("{context} is not an array"));
    };
    if count > values.len() {
        return Err(format!(
            "{context} count is {count}, but the value carries only {} cells",
            values.len()
        ));
    }
    values[..count].iter().map(decode).collect()
}

fn struct_parts<'a>(
    value: &'a BuildTimeValue,
    context: &str,
) -> Result<&'a [(String, BuildTimeValue)], String> {
    match value {
        BuildTimeValue::Struct { fields, .. } => Ok(fields),
        other => Err(format!("{context} is not a struct: {other:?}")),
    }
}

fn case_parts<'a>(
    value: &'a BuildTimeValue,
    context: &str,
) -> Result<(&'a str, &'a [(String, BuildTimeValue)]), String> {
    match value {
        BuildTimeValue::Case { variant, payload } => Ok((
            variant.rsplit("::").next().unwrap_or(variant),
            payload.as_slice(),
        )),
        other => Err(format!("{context} is not a case value: {other:?}")),
    }
}

fn field<'a>(
    fields: &'a [(String, BuildTimeValue)],
    name: &str,
    context: &str,
) -> Result<&'a BuildTimeValue, String> {
    fields
        .iter()
        .find(|(candidate, _)| candidate == name)
        .map(|(_, value)| value)
        .ok_or_else(|| format!("{context} carries no `{name}` field"))
}

fn uint(value: &BuildTimeValue, context: &str) -> Result<u64, String> {
    match value {
        // Build-time integers retain the declared u64 value's exact bits in
        // the evaluator's i64 carrier.
        BuildTimeValue::Int(value) => Ok(*value as u64),
        other => Err(format!("{context} is not an integer: {other:?}")),
    }
}

fn bool_value(value: &BuildTimeValue, context: &str) -> Result<bool, String> {
    match value {
        BuildTimeValue::Bool(value) => Ok(*value),
        other => Err(format!("{context} is not a bool: {other:?}")),
    }
}

fn text<'a>(value: &'a BuildTimeValue, context: &str) -> Result<&'a [u8], String> {
    match value {
        BuildTimeValue::Text(value) => Ok(value),
        other => Err(format!("{context} is not text: {other:?}")),
    }
}

fn u8_value(value: &BuildTimeValue, context: &str) -> Result<u8, String> {
    let value = uint(value, context)?;
    u8::try_from(value).map_err(|_| format!("{context} {value} is outside u8 range"))
}

fn u16_value(value: &BuildTimeValue, context: &str) -> Result<u16, String> {
    let value = uint(value, context)?;
    u16::try_from(value).map_err(|_| format!("{context} {value} is outside u16 range"))
}

fn u32_value(value: &BuildTimeValue, context: &str) -> Result<u32, String> {
    let value = uint(value, context)?;
    u32::try_from(value).map_err(|_| format!("{context} {value} is outside u32 range"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_native_place_decoder_preserves_nominal_field_path() {
        let direct = case(
            "Parameter",
            vec![("parameter".to_owned(), BuildTimeValue::Int(7))],
        );
        assert_eq!(
            decode_native_place(&direct).expect("direct native place"),
            NativePlace::Parameter(NativeParameterId::new(7).unwrap())
        );

        let field = case(
            "Field",
            vec![
                ("parameter".to_owned(), BuildTimeValue::Int(7)),
                ("layout".to_owned(), BuildTimeValue::Int(11)),
                (
                    "field_path".to_owned(),
                    BuildTimeValue::Array(vec![BuildTimeValue::Int(13), BuildTimeValue::Int(17)]),
                ),
                ("field_path_count".to_owned(), BuildTimeValue::Int(2)),
            ],
        );
        assert_eq!(
            decode_native_place(&field).expect("nested native place"),
            NativePlace::Field {
                parameter: NativeParameterId::new(7).unwrap(),
                layout: LayoutPlanId::new(11).unwrap(),
                field_path: vec![
                    LayoutSlotId::new(13).unwrap(),
                    LayoutSlotId::new(17).unwrap(),
                ],
            }
        );

        let mut empty = field;
        let BuildTimeValue::Case { payload, .. } = &mut empty else {
            unreachable!()
        };
        payload
            .iter_mut()
            .find(|(name, _)| name == "field_path_count")
            .expect("field path count")
            .1 = BuildTimeValue::Int(0);
        assert!(
            decode_native_place(&empty)
                .expect_err("empty native field path")
                .contains("cannot be empty")
        );
    }

    #[test]
    fn boundary_signature_publishes_compiler_issued_callback_catalogs() {
        let binder = BoundaryCallbackBinder {
            binder: StaticMachineBinderId::new(41).unwrap(),
            requirement: CallbackRequirementId::new(43).unwrap(),
            static_machine_ordinal: 2,
            parameter_symbol: psi_symbols::SymbolHandle::from_arena_index(5),
            requirement_trait: psi_symbols::SymbolHandle::from_arena_index(7),
            requirement_machine: psi_symbols::SymbolHandle::from_arena_index(11),
        };
        let demand = NativeCallbackDemand {
            destination: NativePlace::Field {
                parameter: NativeParameterId::new(47).unwrap(),
                layout: LayoutPlanId::new(53).unwrap(),
                field_path: vec![LayoutSlotId::new(59).unwrap()],
            },
            requirement: CallbackRequirementId::new(43).unwrap(),
        };
        let signature = MaterializedBoundarySignature {
            shapes: Vec::new(),
            fields: Vec::new(),
            parameters: Vec::new(),
            callback_binders: vec![binder],
            callback_demands: vec![demand.clone()],
            native_parameters: Vec::new(),
            result: None,
        };
        let value = build_boundary_signature(&signature);
        let fields = struct_parts(&value, "BoundarySignature").expect("signature struct");
        assert_eq!(
            uint(
                field(fields, "callback_binder_count", "BoundarySignature")
                    .expect("callback binder count"),
                "callback binder count",
            )
            .unwrap(),
            1
        );
        let BuildTimeValue::Array(rows) =
            field(fields, "callback_binders", "BoundarySignature").expect("callback binder rows")
        else {
            panic!("callback binder catalog is not an array")
        };
        let row = struct_parts(&rows[0], "CallbackBinderIdentity").expect("binder row");
        assert_eq!(
            uint(field(row, "binder", "binder row").unwrap(), "binder").unwrap(),
            41
        );
        assert_eq!(
            uint(
                field(row, "requirement", "binder row").unwrap(),
                "requirement",
            )
            .unwrap(),
            43
        );
        assert_eq!(
            uint(
                field(fields, "callback_demand_count", "BoundarySignature")
                    .expect("callback demand count"),
                "callback demand count",
            )
            .unwrap(),
            1
        );
        let BuildTimeValue::Array(rows) =
            field(fields, "callback_demands", "BoundarySignature").expect("callback demands")
        else {
            panic!("callback demand catalog is not an array")
        };
        let row = struct_parts(&rows[0], "NativeCallbackDemandIdentity").expect("demand row");
        assert_eq!(
            decode_native_place(field(row, "destination", "demand row").unwrap()).unwrap(),
            demand.destination
        );
        assert_eq!(
            uint(
                field(row, "requirement", "demand row").unwrap(),
                "requirement",
            )
            .unwrap(),
            43
        );
    }

    fn nominal_callback_fixture() -> (
        psi_checked_trees::CheckedTrees,
        Vec<BoundaryCallingPlanRealization>,
    ) {
        let validated = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty ordinary boundary plan");
        let fingerprint = validated.contract_fingerprint();
        let boundary_trait = psi_symbols::SymbolHandle::from_arena_index(1);
        let requirement = psi_symbols::SymbolHandle::from_arena_index(2);
        let nominal_use = psi_checked_trees::CheckedNominalMachineUse {
            site: psi_checked_trees::NominalMachineUseSite::Expression(
                psi_typed_trees::expression::ExpressionHandle::from_arena_index(1),
            ),
            registration_operation: psi_symbols::SymbolHandle::from_arena_index(3),
            static_machine_ordinal: 0,
            selected_machine: psi_symbols::SymbolHandle::from_arena_index(4),
            selected_entry: psi_symbols::SymbolHandle::from_arena_index(5),
            satisfaction_trait: boundary_trait,
            satisfaction_requirement: requirement,
            canonical_requirement_overload: "Handler::call(i32)->i32".to_owned(),
            published_requirement_envelope:
                psi_checked_trees::CheckedMachineContractEnvelopeIdentity {
                    contract_fingerprint: 6,
                },
            selected_actual_envelope: psi_checked_trees::CheckedMachineContractEnvelopeIdentity {
                contract_fingerprint: 7,
            },
            callback_placement: Some(psi_checked_trees::CheckedCallbackPlacementIdentity {
                boundary_calling_plan_fingerprint: fingerprint,
            }),
            refinement: psi_checked_trees::CheckedMachineContractRefinement {
                published_requirement_fingerprint: 6,
                selected_actual_fingerprint: 7,
            },
        };
        let mut checked = psi_checked_trees::CheckedTrees::default();
        checked.facts.nominal_machine_uses =
            psi_checked_trees::NominalMachineUseFacts::try_with_uses([nominal_use])
                .expect("valid nominal callback row");
        let realization = BoundaryCallingPlanRealization {
            boundary_trait,
            boundary_arguments: Vec::new(),
            requirement_machine: requirement,
            fingerprint,
            boundary_entry_plan: validated.plan().clone(),
            callback_binders: Vec::new(),
            callback_demands: Vec::new(),
            callback_context_closed: false,
            native_parameters: Vec::new(),
            materialized_signature: materialized_boundary_signature_from_abi(
                &CallSignature::default(),
            )
            .unwrap(),
            policy_machine: String::new(),
            relationship_span: psi_source::SourceSpan::default(),
        };
        (checked, vec![realization])
    }

    #[test]
    fn nominal_callback_placement_binds_one_exact_evaluated_plan() {
        let (checked, realizations) = nominal_callback_fixture();

        let bound = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect("exact checked and target plan identities should bind");
        let [bound] = bound.as_slice() else {
            panic!("one checked callback must produce one durable target plan");
        };
        let nominal_use = &checked.facts.nominal_machine_uses.uses[0];
        assert_eq!(bound.site, nominal_use.site);
        assert_eq!(
            bound.static_machine_ordinal,
            nominal_use.static_machine_ordinal
        );
        assert_eq!(bound.selected_machine, nominal_use.selected_machine);
        assert_eq!(bound.selected_entry, nominal_use.selected_entry);
        assert_eq!(
            bound.boundary_calling_plan_fingerprint,
            realizations[0].fingerprint
        );
        assert_eq!(
            bound.boundary_entry_plan,
            realizations[0].boundary_entry_plan
        );
    }

    #[test]
    fn nominal_callback_consumer_replays_exact_materialization_context() {
        let (mut checked, mut realizations) = nominal_callback_fixture();
        let realization = &mut realizations[0];
        let binder = StaticMachineBinderId::new(101).unwrap();
        let requirement = CallbackRequirementId::new(103).unwrap();
        let destination = NativePlace::Field {
            parameter: NativeParameterId::new(107).unwrap(),
            layout: LayoutPlanId::new(109).unwrap(),
            field_path: vec![LayoutSlotId::new(113).unwrap()],
        };
        realization.callback_binders = vec![BoundaryCallbackBinder {
            binder,
            requirement,
            static_machine_ordinal: 0,
            parameter_symbol: psi_symbols::SymbolHandle::from_arena_index(13),
            requirement_trait: psi_symbols::SymbolHandle::from_arena_index(17),
            requirement_machine: psi_symbols::SymbolHandle::from_arena_index(19),
        }];
        realization.callback_demands = vec![NativeCallbackDemand {
            destination: destination.clone(),
            requirement,
        }];
        realization.callback_context_closed = true;
        realization
            .boundary_entry_plan
            .call
            .callback_materializations = vec![CallbackMaterialization {
            binder,
            destination,
        }];
        let signature = CallSignature::default();
        let context = CallbackMaterializationContext {
            binders: vec![CallbackBinderRequirement {
                binder,
                requirement,
            }],
            demands: realization.callback_demands.clone(),
        };
        realization.fingerprint = validate_boundary_entry_plan_with_callback_materializations(
            realization.boundary_entry_plan.clone(),
            &signature,
            &context,
        )
        .unwrap()
        .contract_fingerprint();
        checked.facts.nominal_machine_uses.uses[0]
            .callback_placement
            .as_mut()
            .unwrap()
            .boundary_calling_plan_fingerprint = realization.fingerprint;

        validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect("exact retained callback context should replay");

        realizations[0].callback_demands[0].destination =
            NativePlace::Parameter(NativeParameterId::new(127).unwrap());
        let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("demand substitution must fail the later consumer replay");
        assert!(
            diagnostics[0]
                .message
                .contains("does not name a declared private native-place demand"),
            "unexpected diagnostic: {:?}",
            diagnostics[0]
        );
    }

    #[test]
    fn target_neutral_binder_defers_supply_but_target_closed_binder_rejects_missing_supply() {
        let (checked, mut realizations) = nominal_callback_fixture();
        realizations[0].callback_binders = vec![BoundaryCallbackBinder {
            binder: StaticMachineBinderId::new(131).unwrap(),
            requirement: CallbackRequirementId::new(137).unwrap(),
            static_machine_ordinal: 0,
            parameter_symbol: psi_symbols::SymbolHandle::from_arena_index(23),
            requirement_trait: psi_symbols::SymbolHandle::from_arena_index(29),
            requirement_machine: psi_symbols::SymbolHandle::from_arena_index(31),
        }];

        validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect("a no-target checked realization must remain target-neutral");

        realizations[0].callback_context_closed = true;
        let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("target closure must reject a binder without native supply");
        assert!(
            diagnostics[0]
                .message
                .contains("omits a nominal callback binder"),
            "unexpected diagnostic: {:?}",
            diagnostics[0]
        );
    }

    #[test]
    fn private_callback_parameter_mapping_rejects_array_and_slice_wrappers() {
        let mut typed = TypedTrees::default();
        let symbol = psi_symbols::SymbolHandle::from_arena_index(41);
        let named = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol,
            name: psi_typed_trees::name::Identifier::generated("NativeRecord"),
        });
        let reference = typed
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee: named,
                access: psi_language_core::ReferenceAccess::Shared,
                lifetime: None,
            });
        let array = typed
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: named,
                length: psi_typed_trees::types::FixedArrayLength::Literal(2),
            });
        let slice = typed.type_reference_table.insert(TypeReferenceNode::Slice {
            element_type: named,
        });

        assert_eq!(exact_boundary_layout_root_symbol(&typed, named), symbol);
        assert_eq!(exact_boundary_layout_root_symbol(&typed, reference), symbol);
        assert!(!exact_boundary_layout_root_symbol(&typed, array).is_valid());
        assert!(!exact_boundary_layout_root_symbol(&typed, slice).is_valid());
    }

    #[test]
    fn nominal_callback_placement_rejects_missing_or_drifting_plan() {
        let (mut checked, realizations) = nominal_callback_fixture();
        let missing = validate_nominal_callback_placement_bindings(&checked, &[])
            .expect_err("a checked callback cannot lose its target plan");
        assert!(
            missing[0]
                .message
                .contains("0 target calling-plan realizations")
        );

        let duplicated = vec![realizations[0].clone(), realizations[0].clone()];
        let duplicate = validate_nominal_callback_placement_bindings(&checked, &duplicated)
            .expect_err("one callback cannot bind duplicate target plans");
        assert!(
            duplicate[0]
                .message
                .contains("2 target calling-plan realizations")
        );

        checked.facts.nominal_machine_uses.uses[0]
            .callback_placement
            .as_mut()
            .expect("callback placement")
            .boundary_calling_plan_fingerprint ^= 1;
        let drift = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("a changed target plan identity must reject");
        assert!(
            drift[0]
                .message
                .contains("does not bind its exact evaluated target calling plan")
        );

        checked.facts.nominal_machine_uses.uses[0].callback_placement = None;
        let lost = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("a callback plan realization requires a checked join key");
        assert!(
            lost[0]
                .message
                .contains("lost its evaluated boundary calling-plan identity")
        );
    }

    fn four_f32_array_signature() -> MaterializedBoundarySignature {
        MaterializedBoundarySignature {
            shapes: vec![
                BoundaryValueShape {
                    class: BoundaryValueClass::Float,
                    byte_size: 4,
                    alignment: 4,
                },
                BoundaryValueShape {
                    class: BoundaryValueClass::FixedArray {
                        element: 0,
                        length: 4,
                    },
                    byte_size: 16,
                    alignment: 4,
                },
            ],
            fields: Vec::new(),
            parameters: vec![1],
            callback_binders: Vec::new(),
            callback_demands: Vec::new(),
            native_parameters: Vec::new(),
            result: None,
        }
    }

    #[test]
    fn recursive_array_classification_is_policy_specific() {
        let signature = four_f32_array_signature();
        let hfa = ValueShape::homogeneous_float_aggregate(4, 4);
        validate_authored_abi_shape(&signature, 1, hfa, CallingPolicy::Aapcs64)
            .expect("AAPCS64 should classify four f32 elements as an HFA");
        validate_authored_abi_shape(&signature, 1, hfa, CallingPolicy::SystemVAMD64)
            .expect("SysV should classify four f32 elements through its SSE aggregate path");
        validate_authored_abi_shape(
            &signature,
            1,
            ValueShape::integer(16, 4),
            CallingPolicy::MicrosoftX64,
        )
        .expect("Microsoft x64 should classify the fixed array as an indirect aggregate");
    }

    #[test]
    fn compatibility_signature_excludes_dispatch_only_table_parameter() {
        let signature = MaterializedBoundarySignature {
            shapes: vec![
                BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: 8,
                    alignment: 8,
                },
                BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: 4,
                    alignment: 4,
                },
            ],
            fields: Vec::new(),
            parameters: vec![0, 1, 0],
            callback_binders: Vec::new(),
            callback_demands: Vec::new(),
            native_parameters: Vec::new(),
            result: Some(1),
        };
        let wire = compatibility_call_signature(&signature, CallingPolicy::MicrosoftX64, 1)
            .expect("one table pointer should project out of the wire signature");
        assert_eq!(
            wire.parameters,
            [ValueShape::integer(4, 4), ValueShape::integer(8, 8)]
        );
        assert_eq!(wire.result, Some(ValueShape::integer(4, 4)));
        assert!(compatibility_call_signature(&signature, CallingPolicy::MicrosoftX64, 4).is_err());
    }

    #[test]
    fn compatibility_lookup_requires_one_exact_nonempty_overload_identity() {
        let cases: [(&str, &str, &[&str], Result<Option<usize>, &str>); 5] = [
            (
                "empty",
                "",
                &["exact"],
                Err(
                    "compatibility calling-plan lookup for `pkg::Readable::read` has no exact requirement overload identity",
                ),
            ),
            ("absent", "missing", &["first", "second"], Ok(None)),
            ("name-only singleton", "exact", &["lookalike"], Ok(None)),
            (
                "unique same-name overload",
                "exact",
                &["lookalike", "exact", "other"],
                Ok(Some(1)),
            ),
            (
                "duplicate exact",
                "exact",
                &["exact", "lookalike", "exact"],
                Err(
                    "compatibility calling-plan lookup for `pkg::Readable::read` matches 2 exact requirement overload rows for identity `exact`",
                ),
            ),
        ];

        for (case, requirement_identity, candidates, expected) in cases {
            let actual = exact_compatibility_overload_index(
                "pkg::Readable",
                "read",
                requirement_identity,
                candidates.iter().copied(),
            );
            assert_eq!(
                actual
                    .as_ref()
                    .map(|candidate_index| *candidate_index)
                    .map_err(String::as_str),
                expected,
                "case: {case}",
            );
        }
    }

    #[test]
    fn authored_policy_cannot_publish_a_preclassified_shape_for_the_wrong_abi() {
        let signature = four_f32_array_signature();
        let error = validate_authored_abi_shape(
            &signature,
            1,
            ValueShape::integer(16, 4),
            CallingPolicy::Aapcs64,
        )
        .expect_err("AAPCS64 policy must preserve the HFA classification");
        assert!(
            error.contains("requires HomogeneousFloatAggregate"),
            "{error}"
        );
    }

    #[test]
    fn stored_integer_boundary_shape_uses_validated_physical_width() {
        let main = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../../../tests/canaries/pass/layouts/runtime_plan_laid_integer_at_proved_write_exit/main.omg",
        );
        let checked = crate::compile_to_checked(&main, None)
            .expect("stored-integer canary should compile to checked trees");
        let layout = checked
            .typed
            .plan_laid_layouts
            .iter()
            .find(|layout| !layout.integer_fields.is_empty())
            .expect("canary should retain one stored-integer layout");
        let definition = checked
            .typed
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == layout.data_symbol)
            .expect("stored-integer layout should name its synthesized data");

        let mut visiting = Vec::new();
        let mut shapes = Vec::new();
        let mut fields = Vec::new();
        let (abi, root) = plain_data_value_shape(
            &checked.typed,
            definition.symbol,
            definition.name.as_str(),
            &[],
            &mut visiting,
            &mut shapes,
            &mut fields,
        )
        .expect("validated stored integers should be classifiable by value");

        assert_eq!(abi, ValueShape::integer(1, 1));
        let BoundaryValueClass::Record {
            first_field,
            field_count,
        } = shapes[usize::from(root)].class
        else {
            panic!("plan-laid data should remain a record boundary shape")
        };
        assert_eq!(field_count, 1);
        let field = fields[usize::from(first_field)];
        let stored = shapes[usize::from(field.shape)];
        assert!(matches!(stored.class, BoundaryValueClass::Integer));
        assert_eq!(stored.byte_size, 1);
        assert_eq!(stored.alignment, 1);

        let signature = MaterializedBoundarySignature {
            shapes,
            fields,
            parameters: vec![root],
            callback_binders: Vec::new(),
            callback_demands: Vec::new(),
            native_parameters: Vec::new(),
            result: None,
        };
        assert_eq!(
            classified_boundary_shape(&signature, root, CallingPolicy::SystemVAMD64)
                .expect("physical stored-integer aggregate should classify for SysV"),
            ValueShape::integer(1, 1)
        );
    }
}
