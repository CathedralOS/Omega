//! Computing calling plans: evaluating the authored policy at build time,
//! materializing the boundary signature from the ABI result, and validating
//! the plan the policy produced.

use crate::calling_policy_plans::boundary_signatures::{
    call_signature_from_top_level_requirement, call_signature_from_typed,
    collect_boundary_signatures, compatibility_call_signature, exact_compatibility_overload_index,
};
use crate::calling_policy_plans::build_time_decoding::{
    CALLBACK_FIELD_PATH_CAPACITY, CALLBACK_MATERIALIZATION_CAPACITY, PARAMETER_CAPACITY,
    VALUE_FIELD_CAPACITY, VALUE_SHAPE_CAPACITY, case, decode_boundary_plan_result,
};
use crate::calling_policy_plans::callback_bindings::{
    callback_materialization_context, validate_retained_callback_binders,
};
use crate::calling_policy_plans::policy_implementations::{
    BoundaryPolicyInstance, boundary_policy_instances, selected_policy_machine,
};
use crate::calling_policy_plans::{
    BoundaryCallingPlanRealization, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
use build_time_evaluation::{BuildTimeProgram, BuildTimeValue};
use calling_conventions::callback_nominal_report_fingerprint;
use calling_conventions::{
    BoundaryEntryPlan, BoundaryPlanResult, CallSignature, CallingPolicy, NativePlace,
    ValidatedBoundaryEntryPlan, ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use representation_planning::OpaqueRepresentationSelection;
use target::NativeTarget;

pub(crate) fn evaluate_compatibility_boundary_entry_plan(
    typed: &TypedTrees,
    native_target: NativeTarget,
    trait_name: &str,
    trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    method_name: &str,
    requirement_identity: &str,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<Option<BoundaryEntryPlan>, String> {
    let trait_candidates = typed
        .traits()
        .iter()
        .filter(|definition| {
            typed.trait_declaration_path(definition) == trait_name
                && crate::service_schema::is_product_declaration(typed, definition.symbol)
                && typed.symbols.symbol_package_identity(definition.symbol)
                    == trait_package_identity
        })
        .flat_map(|definition| {
            typed
                .trait_machine_signatures(definition)
                .iter()
                .filter(|signature| signature.name.as_str() == method_name)
                .map(|signature| {
                    (
                        signature,
                        typed.trait_requirement_callable_identity_text(definition, signature),
                    )
                })
        })
        .collect::<Vec<_>>();
    let top_level_candidates = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
                && crate::service_schema::is_product_declaration(typed, requirement.symbol)
                && typed.symbols.display_path(requirement.symbol, "::") == trait_name
                && typed.symbols.symbol_package_identity(requirement.symbol)
                    == trait_package_identity
        })
        .filter_map(|requirement| {
            let [entry] = typed.machine_states(requirement) else {
                return None;
            };
            (entry.name.as_str() == method_name).then(|| {
                (
                    requirement,
                    entry,
                    typed
                        .machine_callable_identity_text(requirement)
                        .unwrap_or_default(),
                )
            })
        })
        .collect::<Vec<_>>();
    let candidate_identities = trait_candidates
        .iter()
        .map(|(_, identity)| identity.as_str())
        .chain(
            top_level_candidates
                .iter()
                .map(|(_, _, identity)| identity.as_str()),
        )
        .collect::<Vec<_>>();
    let Some(candidate_index) = exact_compatibility_overload_index(
        trait_name,
        method_name,
        requirement_identity,
        candidate_identities,
    )?
    else {
        return Ok(None);
    };
    let materialized = if let Some((signature, _)) = trait_candidates.get(candidate_index) {
        call_signature_from_typed(
            typed,
            signature,
            &[],
            requirement_identity,
            native_target,
            opaque_representation_selections,
        )?
    } else {
        let top_level_index = candidate_index - trait_candidates.len();
        let (requirement, entry, _) =
            top_level_candidates.get(top_level_index).ok_or_else(|| {
                "compatibility boundary selection index is outside its exact candidate catalog"
                    .to_owned()
            })?;
        call_signature_from_top_level_requirement(
            typed,
            requirement,
            entry,
            requirement_identity,
            native_target,
            opaque_representation_selections,
        )?
    };
    let classified =
        compatibility_call_signature(&materialized, policy, dispatch_only_parameter_count)?;
    evaluate_ordinary_boundary_entry_plan(policy, &classified)
        .map(|validated| Some(validated.plan().clone()))
        .map_err(|diagnostic| diagnostic.to_string())
}

/// Discover concrete `Calling<Policy>` relationships, evaluate the selected implementation row once for
/// every method in the boundary service surface, and retain only canonical
/// evaluated applications in the realization output. Source trees are borrowed.
pub fn compute_boundary_calling_plans(
    checked: &checked_trees::CheckedTrees,
    target_profile: target::TargetProfile,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<Vec<BoundaryCallingPlanRealization>, Vec<Diagnostic>> {
    // Provider selection consumes the source judgment. Retain it through
    // policy admission rather than rechecking its internal call premises.
    let native_target = target_profile.native_target();
    let typed = &checked.typed;
    let vocabularies =
        checked_trees::typed_trees::calling_vocabulary::standard_calling_vocabularies(typed);
    if vocabularies.is_empty() {
        // Programs that do not import std::calling cannot accidentally opt in
        // merely by having an unrelated local trait named Calling.
        return Ok(Vec::new());
    }

    let mut pending = Vec::new();
    for boundary in typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        let relationships = typed
            .trait_requirements(boundary)
            .iter()
            .filter(|requirement| {
                vocabularies
                    .iter()
                    .any(|(calling, _)| requirement.symbol == calling.symbol)
            })
            .collect::<Vec<_>>();
        if relationships.is_empty() {
            continue;
        }
        if relationships.len() != 1 {
            return Err(vec![Diagnostic::error(format!(
                "boundary trait `{}` selects {} Calling<Policy> policies; exactly one concrete policy may define a boundary contract",
                boundary.name,
                relationships.len()
            ))
            .with_source_span(relationships[0].source_span)]);
        }
        let relationship = relationships[0];
        let relationship_span = relationship.source_span;
        let (_, calling_policy_trait) = vocabularies
            .iter()
            .find(|(calling, _)| calling.symbol == relationship.symbol)
            .copied()
            .expect("filtered above to the vocabulary owning this Calling symbol");
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
                        "boundary trait `{}` cannot evaluate Calling<Policy>: {reason}",
                        boundary.name
                    ))
                    .with_source_span(relationship_span),
                ]
            })?;
        for BoundaryPolicyInstance {
            arguments: boundary_arguments,
            argument_owner,
            policy_argument,
        } in instances
        {
            let policy_machine = selected_policy_machine(
                typed,
                boundary,
                &boundary_arguments,
                policy_argument,
                calling_policy_trait,
                target_profile,
            )
            .map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "boundary trait `{}` cannot evaluate Calling<Policy>: {reason}",
                        boundary.name
                    ))
                    .with_source_span(relationship_span),
                ]
            })?;
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
                    native_target,
                    opaque_representation_selections,
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
                    argument_owner,
                    signature.symbol,
                    policy_machine.name.as_str().to_owned(),
                    policy_machine.symbol,
                    boundary_signature,
                    relationship_span,
                ));
            }
        }
    }

    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let admission = build_time_evaluation::BuildTimeAdmissionPlan::infer(
        typed,
        package_inputs.map(|inputs| {
            std::sync::Arc::new(inputs.clone())
                as std::sync::Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
    );
    let mut evaluated = Vec::with_capacity(pending.len());
    for (
        boundary_trait,
        boundary_arguments,
        argument_owner,
        requirement_machine,
        policy_machine,
        policy_symbol,
        signature,
        relationship_span,
    ) in pending
    {
        let validated = evaluate_selected_materialized_calling_policy_plan(
            BuildTimeProgram::Checked(checked),
            &admission,
            &policy_machine,
            policy_symbol,
            &signature,
            Some(build_time_evaluation::BuildTimeInvocationCustody::Source(
                relationship_span,
            )),
        )
        .map_err(|rejection| {
            vec![
                Diagnostic::error(rejection.reason)
                    .with_source_span(rejection.source_span.unwrap_or(relationship_span)),
            ]
        })?;
        let (application_report_fingerprint, application_commitment) =
            boundary_plan_application_identity(&signature, &validated)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        evaluated.push(BoundaryCallingPlanRealization {
            boundary_trait,
            boundary_arguments,
            argument_owner,
            requirement_machine,
            report_fingerprint: application_report_fingerprint,
            commitment: effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                application_commitment,
            ),
            boundary_entry_plan: validated.plan().clone(),
            validated_boundary_entry_plan: validated,
            callback_binders: signature.callback_binders.clone(),
            synchronous_callback_binders: super::callback_bindings::synchronous_callback_binders(
                typed,
                boundary_trait,
                requirement_machine,
                &signature.callback_binders,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(relationship_span)]
            })?,
            callback_demands: signature.callback_demands.clone(),
            callback_context_closed: false,
            native_parameters: signature.native_parameters.clone(),
            materialized_signature: signature,
            policy_machine,
            policy_symbol,
            relationship_span,
        });
    }
    for realization in &evaluated {
        validate_retained_callback_binders(realization)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
    }
    Ok(evaluated)
}

/// One refused calling-policy evaluation. `source_span` locates the authored
/// occurrence build-time admission rejected, when it named one; the caller
/// otherwise reports the `Calling<Policy>` relationship it was evaluating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallingPolicyRejection {
    pub(crate) reason: String,
    pub(crate) source_span: Option<source::SourceSpan>,
}

impl From<String> for CallingPolicyRejection {
    fn from(reason: String) -> Self {
        Self {
            reason,
            source_span: None,
        }
    }
}

pub(crate) fn evaluate_materialized_calling_policy_plan(
    typed: &TypedTrees,
    admission: &build_time_evaluation::BuildTimeAdmissionPlan,
    policy_machine: &str,
    policy_symbol: symbols::SymbolHandle,
    signature: &MaterializedBoundarySignature,
    custody: Option<build_time_evaluation::BuildTimeInvocationCustody>,
) -> Result<ValidatedBoundaryEntryPlan, CallingPolicyRejection> {
    evaluate_selected_materialized_calling_policy_plan(
        BuildTimeProgram::Typed(typed),
        admission,
        policy_machine,
        policy_symbol,
        signature,
        custody,
    )
}

fn evaluate_selected_materialized_calling_policy_plan(
    program: BuildTimeProgram<'_>,
    admission: &build_time_evaluation::BuildTimeAdmissionPlan,
    policy_machine: &str,
    policy_symbol: symbols::SymbolHandle,
    signature: &MaterializedBoundarySignature,
    custody: Option<build_time_evaluation::BuildTimeInvocationCustody>,
) -> Result<ValidatedBoundaryEntryPlan, CallingPolicyRejection> {
    if signature.parameters.len() > PARAMETER_CAPACITY {
        return Err(format!(
            "boundary signature has {} parameters; calling policies currently support at most {PARAMETER_CAPACITY}",
            signature.parameters.len()
        )
        .into());
    }

    let arguments = vec![build_boundary_signature(signature)];
    let value = admission
        .evaluate_machine_symbol_for_invocation_measured(
            program,
            policy_symbol,
            arguments,
            custody.unwrap_or(build_time_evaluation::BuildTimeInvocationCustody::Symbol(
                policy_symbol,
            )),
        )
        .map(|evaluation| evaluation.into_value())
        .map_err(CallingPolicyRejection::from)?;
    let result = decode_boundary_plan_result(&value).map_err(|reason| {
        format!("calling policy `{policy_machine}` returned an invalid result: {reason}")
    })?;

    validate_materialized_boundary_plan_result(result, signature)
        .map_err(CallingPolicyRejection::from)
}

pub(crate) fn build_boundary_signature(
    signature: &MaterializedBoundarySignature,
) -> BuildTimeValue {
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
    let mut native_parameters = Vec::with_capacity(PARAMETER_CAPACITY);
    for index in 0..PARAMETER_CAPACITY {
        let parameter = signature.native_parameters.get(index);
        native_parameters.push(BuildTimeValue::Struct {
            type_name: "NativeParameterIdentity".to_owned(),
            fields: vec![
                (
                    "identity".to_owned(),
                    BuildTimeValue::Int(parameter.map_or(0, |row| row.identity.get()) as i64),
                ),
                (
                    "native_ordinal".to_owned(),
                    BuildTimeValue::Int(
                        parameter.map_or(0, |row| u64::from(row.native_ordinal)) as i64
                    ),
                ),
                (
                    "origin".to_owned(),
                    parameter.map_or_else(
                        || {
                            case(
                                "NativeParameterOrigin::SemanticFormal",
                                vec![("formal_ordinal".to_owned(), BuildTimeValue::Int(0))],
                            )
                        },
                        |row| match row.origin {
                            BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } => {
                                case(
                                    "NativeParameterOrigin::SemanticFormal",
                                    vec![(
                                        "formal_ordinal".to_owned(),
                                        BuildTimeValue::Int(i64::from(formal_ordinal)),
                                    )],
                                )
                            }
                            BoundaryNativeParameterOrigin::PrivateCallback {
                                binder,
                                requirement,
                            } => case(
                                "NativeParameterOrigin::PrivateCallback",
                                vec![
                                    (
                                        "binder".to_owned(),
                                        BuildTimeValue::Int(binder.get() as i64),
                                    ),
                                    (
                                        "requirement".to_owned(),
                                        BuildTimeValue::Int(requirement.get() as i64),
                                    ),
                                ],
                            ),
                        },
                    ),
                ),
                (
                    "shape".to_owned(),
                    parameter.map_or_else(
                        || {
                            case(
                                "NativeParameterShape::Semantic",
                                vec![("root".to_owned(), BuildTimeValue::Int(0))],
                            )
                        },
                        |row| match row.shape {
                            BoundaryNativeParameterShape::Semantic(root) => case(
                                "NativeParameterShape::Semantic",
                                vec![("root".to_owned(), BuildTimeValue::Int(i64::from(root)))],
                            ),
                            BoundaryNativeParameterShape::TargetFunctionPointer {
                                byte_size,
                                alignment,
                            } => case(
                                "NativeParameterShape::TargetFunctionPointer",
                                vec![
                                    (
                                        "byte_size".to_owned(),
                                        BuildTimeValue::Int(i64::from(byte_size)),
                                    ),
                                    (
                                        "alignment".to_owned(),
                                        BuildTimeValue::Int(i64::from(alignment)),
                                    ),
                                ],
                            ),
                        },
                    ),
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
                "native_parameters".to_owned(),
                BuildTimeValue::Array(native_parameters),
            ),
            (
                "native_parameter_count".to_owned(),
                BuildTimeValue::Int(signature.native_parameters.len() as i64),
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

pub(crate) fn validate_materialized_boundary_plan_result(
    result: BoundaryPlanResult,
    signature: &MaterializedBoundarySignature,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    let BoundaryPlanResult::Accepted(plan) = result else {
        return validate_boundary_plan_result(result, &CallSignature::default())
            .map_err(|diagnostic| diagnostic.to_string());
    };
    if plan.call.parameters.len() != signature.native_parameters.len() {
        return Err(invalid_authored_plan(format!(
            "plan places {} parameters for a boundary signature with {} native parameters",
            plan.call.parameters.len(),
            signature.native_parameters.len()
        )));
    }
    if plan.call.result.is_some() != signature.result.is_some() {
        return Err(invalid_authored_plan(
            "plan result presence does not match the boundary signature".to_owned(),
        ));
    }
    for (index, (placement, native_parameter)) in plan
        .call
        .parameters
        .iter()
        .zip(signature.native_parameters.iter())
        .enumerate()
    {
        let validation = match native_parameter.shape {
            BoundaryNativeParameterShape::Semantic(root) => signature
                .value_graph()
                .validate_abi_shape(root, placement.shape, plan.call.policy),
            BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            } => {
                let expected = ValueShape::integer(u32::from(byte_size), alignment);
                if placement.shape == expected {
                    Ok(())
                } else {
                    Err(format!(
                        "direct callback parameter requires target function-pointer ABI shape {:?}, got {:?}",
                        expected, placement.shape,
                    ))
                }
            }
        };
        validation.map_err(|reason| {
            invalid_authored_plan(format!(
                "parameter {index} classification is invalid: {reason}"
            ))
        })?;
    }
    if let (Some(placement), Some(root)) = (&plan.call.result, signature.result) {
        signature
            .value_graph()
            .validate_abi_shape(root, placement.shape, plan.call.policy)
            .map_err(|reason| {
                invalid_authored_plan(format!("result classification is invalid: {reason}"))
            })?;
    }
    let classified = CallSignature::of_placements(&plan.call.parameters, plan.call.result.as_ref());
    if signature.callback_demands.is_empty() {
        return validate_boundary_plan_result(BoundaryPlanResult::Accepted(plan), &classified)
            .map_err(|diagnostic| diagnostic.to_string());
    }
    let context = callback_materialization_context(signature);
    validate_boundary_entry_plan_with_callback_materializations(plan, &classified, &context)
        .map_err(|diagnostic| diagnostic.to_string())
}

pub(crate) fn boundary_plan_application_identity(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> Result<(u64, [u8; 32]), String> {
    let application = super::callback_bindings::source_free_application(signature, validated)?;
    let commitment = application.recomputed_commitment()?;
    let report = callback_nominal_report_fingerprint(
        b"omega.boundary-plan-application-report.v3",
        &[&commitment],
    );
    Ok((report, commitment))
}

fn invalid_authored_plan(reason: String) -> String {
    format!("calling policy accepted an invalid plan: {reason}")
}
