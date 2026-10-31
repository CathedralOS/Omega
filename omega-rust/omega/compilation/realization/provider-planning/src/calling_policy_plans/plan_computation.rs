//! Computing calling plans: evaluating the authored policy at build time,
//! materializing the boundary signature from the ABI result, and validating
//! the plan the policy produced.

use crate::calling_policy_plans::boundary_signatures::{
    compatibility_call_signature, exact_compatibility_overload_index,
    materialize_prepared_signature,
};
use crate::calling_policy_plans::build_time_decoding::{
    CALLBACK_FIELD_PATH_CAPACITY, CALLBACK_MATERIALIZATION_CAPACITY, PARAMETER_CAPACITY,
    VALUE_FIELD_CAPACITY, VALUE_SHAPE_CAPACITY, case,
};
use crate::calling_policy_plans::callback_bindings::{
    callback_materialization_context, validate_retained_callback_binders,
};
use crate::calling_policy_plans::{
    BoundaryCallingPlanRealization, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
use build_time_evaluation::{BuildTimeProgram, BuildTimeValue};
use calling_conventions::callback_nominal_report_fingerprint;
use calling_conventions::{
    BoundaryPlanResult, CallSignature, CallingPolicy, NativePlace, ValidatedBoundaryEntryPlan,
    ValueShape, evaluate_ordinary_boundary_entry_plan,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};
use diagnostics::Diagnostic;
use representation_planning::OpaqueRepresentationSelection;

macro_rules! row {
    ($a:expr => $b:expr) => {
        ($a, $b)
    };
}

pub(crate) fn evaluate_default_calling_application(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    layout_target: layout::LayoutTarget,
    trait_name: &str,
    trait_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    method_name: &str,
    requirement_identity: &str,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<Option<calling_conventions::BoundaryCallingApplication>, String> {
    let candidates = services
        .signatures()
        .declarations()
        .filter(|declaration| {
            declaration.default_eligible
                && declaration.path == trait_name
                && declaration.package == trait_package_identity
                && declaration.method == method_name
        })
        .collect::<Vec<_>>();
    let Some(candidate_index) = exact_compatibility_overload_index(
        trait_name,
        method_name,
        requirement_identity,
        candidates
            .iter()
            .map(|declaration| declaration.requirement_identity.as_str()),
    )?
    else {
        return Ok(None);
    };
    let declaration = candidates.get(candidate_index).ok_or_else(|| {
        "compatibility boundary selection index is outside its exact candidate catalog".to_owned()
    })?;
    let materialized = materialize_prepared_signature(
        services,
        declaration,
        &[],
        layout_target,
        opaque_representation_selections,
    )?;
    let classified =
        compatibility_call_signature(&materialized, policy, dispatch_only_parameter_count)?;
    let validated = evaluate_ordinary_boundary_entry_plan(policy, &classified)
        .map_err(|diagnostic| diagnostic.to_string())?;
    for representation in materialized.opaque_representation_uses() {
        materialized.opaque_representation_movement(representation, &validated)?;
    }
    publish_materialized_application(&materialized, &validated).map(Some)
}

/// Discover concrete `Calling<Policy>` relationships, evaluate the selected implementation row once for
/// every method in the boundary service surface, and retain only canonical
/// evaluated applications in the realization output. Source trees are borrowed.
pub fn prepare_boundary_calling_plans<'checked>(
    checked: &'checked checked_trees::CheckedTrees,
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    target_profile: target::TargetProfile,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
) -> Result<PreparedBoundaryCallingPlans<'checked>, Vec<Diagnostic>> {
    if !std::ptr::eq(&*checked.typed, services.context().typed)
        || services.context().scope != source::DependencyScope::Product
    {
        return Err(vec![Diagnostic::error(
            "calling plans require the exact Product source owner's prepared signature catalog",
        )]);
    }
    // Provider selection consumes the source judgment. Retain it through
    // policy admission rather than rechecking its internal call premises.
    let typed = &checked.typed;
    let signatures = services.signatures();
    let calling = signatures.calling();
    let mut pending = Vec::new();
    for request in calling.requests() {
        for instance in calling.instances(request)? {
            let policy = instance.policy.as_ref().map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "boundary trait `{}` cannot evaluate Calling<Policy>: {reason}",
                        request.name
                    ))
                    .with_source_span(request.span),
                ]
            })?;
            let policy = typed.machine(*policy).ok_or_else(|| {
                vec![
                    Diagnostic::error("prepared calling policy lost its exact machine")
                        .with_source_span(request.span),
                ]
            })?;
            // Psi retains the exact closed named map. Only deployment body
            // selection remains here; a family head is not executable code.
            let policy_machine = typed.machine_for_target(policy, target_profile.target_name())
                .ok_or_else(|| vec![
                    Diagnostic::error(format!(
                        "boundary trait `{}` cannot evaluate Calling<Policy>: calling policy realization `{}` has no body for target `{}`",
                        request.name, policy.name, target_profile.target_name(),
                    )).with_source_span(request.span),
                ])?;
            for application in calling.signatures(instance) {
                let declaration = signatures.applied_declaration(application).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "calling plan lost its exact prepared signature declaration",
                    )]
                })?;
                let boundary_signature = materialize_prepared_signature(
                    services,
                    declaration,
                    calling.bindings(application),
                    layout::LayoutTarget::for_profile(target_profile)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?,
                    opaque_representation_selections,
                )
                .map_err(|reason| {
                    vec![
                        Diagnostic::error(format!(
                            "cannot materialize boundary signature `{}::{}` for `{}`: {reason}",
                            request.name, declaration.method, policy_machine.name,
                        ))
                        .with_source_span(request.span),
                    ]
                })?;
                pending.push(PreparedCallingPolicy {
                    boundary_trait: request.boundary,
                    boundary_arguments: calling.arguments(instance).to_vec(),
                    argument_owner: instance.argument_owner,
                    requirement_machine: declaration.symbol,
                    policy_machine: policy_machine.name.as_str().to_owned(),
                    policy_symbol: policy_machine.symbol,
                    signature: boundary_signature,
                    relationship_span: request.span,
                    admission: Ok(()),
                });
            }
        }
    }

    if pending.is_empty() {
        return Ok(PreparedBoundaryCallingPlans {
            checked,
            pending,
            target: target_profile,
        });
    }
    // Policy helpers use ordinary target families too. Select their exact
    // retained bodies in Omega, then give admission and evaluation one shared
    // identity mapping; Psi must neither guess the host nor pick a sibling.
    let machine_bodies = typed
        .target_machine_families
        .iter()
        .filter_map(|family| {
            typed
                .target_family_member(family.canonical, target_profile.target_name())
                .map(|body| build_time_evaluation::SelectedBuildTimeMachineBody {
                    family: family.canonical,
                    body: body.symbol,
                })
        })
        .collect::<Vec<_>>();
    let admission = build_time_evaluation::BuildTimeAdmissionPlan::infer(
        typed,
        package_inputs.map(|inputs| {
            std::sync::Arc::new(inputs.clone())
                as std::sync::Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
    )
    .with_selected_machine_bodies(typed, &machine_bodies)
    .map_err(|reason| vec![Diagnostic::error(reason)])?;
    for policy in &mut pending {
        policy.admission = admission
            .admitted_program_machine_closure_symbols(
                BuildTimeProgram::Checked(checked),
                policy.policy_symbol,
                build_time_evaluation::BuildTimeInvocationCustody::Source(policy.relationship_span),
            )
            .map(|_closure| ());
    }
    Ok(PreparedBoundaryCallingPlans {
        checked,
        pending,
        target: target_profile,
    })
}

/// Source admission and signature preparation precede code publication.
/// Activation consumes only this owned preparation and retained policy code.
pub struct PreparedBoundaryCallingPlans<'checked> {
    checked: &'checked checked_trees::CheckedTrees,
    pending: Vec<PreparedCallingPolicy>,
    target: target::TargetProfile,
}

struct PreparedCallingPolicy {
    boundary_trait: symbols::SymbolHandle,
    boundary_arguments: Vec<checked_trees::typed_trees::types::TypeReferenceHandle>,
    argument_owner: symbols::SymbolHandle,
    requirement_machine: symbols::SymbolHandle,
    policy_machine: String,
    policy_symbol: symbols::SymbolHandle,
    signature: MaterializedBoundarySignature,
    relationship_span: source::SourceSpan,
    admission: Result<(), String>,
}

impl PreparedBoundaryCallingPlans<'_> {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn evaluate(
        self,
        code: Option<&super::CallingPolicyCode>,
    ) -> Result<Vec<BoundaryCallingPlanRealization>, Vec<Diagnostic>> {
        let mut evaluated = Vec::with_capacity(self.pending.len());
        for PreparedCallingPolicy {
            boundary_trait,
            boundary_arguments,
            argument_owner,
            requirement_machine,
            policy_machine,
            policy_symbol,
            signature,
            relationship_span,
            admission,
        } in self.pending
        {
            admission.map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(relationship_span)]
            })?;
            let code = code.ok_or_else(|| {
                vec![Diagnostic::error(
                    "calling policies have no published Terminal code",
                )]
            })?;
            code.require_owner(self.checked)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            let validated = code
                .evaluate(policy_symbol, self.target, &signature)
                .map_err(|reason| {
                    vec![
                        Diagnostic::error(format!("calling policy `{policy_machine}`: {reason}"))
                            .with_source_span(relationship_span),
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
}

/// The zero-valued case an absent optional boundary field decodes to.
fn zero_case(variant: &str, field: &str) -> BuildTimeValue {
    case(variant, vec![(field.to_owned(), BuildTimeValue::Int(0))])
}

pub(crate) fn build_boundary_signature(
    signature: &MaterializedBoundarySignature,
) -> Result<BuildTimeValue, String> {
    // The fixed arrays and their explicit counts are one vocabulary contract.
    // Refuse excess input before padding; truncating the arrays while retaining
    // a larger count would violate the policy entry's checked invariants.
    for (label, count, capacity) in [
        ("parameters", signature.parameters.len(), PARAMETER_CAPACITY),
        ("shapes", signature.shapes.len(), VALUE_SHAPE_CAPACITY),
        ("fields", signature.fields.len(), VALUE_FIELD_CAPACITY),
        (
            "native parameters",
            signature.native_parameters.len(),
            PARAMETER_CAPACITY,
        ),
        (
            "callback binders",
            signature.callback_binders.len(),
            CALLBACK_MATERIALIZATION_CAPACITY,
        ),
        (
            "callback demands",
            signature.callback_demands.len(),
            CALLBACK_MATERIALIZATION_CAPACITY,
        ),
    ] {
        if count > capacity {
            return Err(format!(
                "boundary signature has {count} {label}; calling policies currently support at most {capacity}"
            ));
        }
    }
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
                        || zero_case("NativeParameterOrigin::SemanticFormal", "formal_ordinal"),
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
                        || zero_case("NativeParameterShape::Semantic", "root"),
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
                        || zero_case("NativePlace::Parameter", "parameter"),
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
    Ok(BuildTimeValue::Struct {
        type_name: "BoundarySignature".to_owned(),
        fields: vec![
            ("shapes".to_owned(), BuildTimeValue::Array(shapes)),
            row!("shape_count".to_owned() => BuildTimeValue::Int(signature.shapes.len() as i64)),
            ("fields".to_owned(), BuildTimeValue::Array(fields)),
            row!("field_count".to_owned() => BuildTimeValue::Int(signature.fields.len() as i64)),
            ("parameters".to_owned(), BuildTimeValue::Array(parameters)),
            (
                "parameter_count".to_owned(),
                BuildTimeValue::Int(signature.parameters.len() as i64),
            ),
            row!("native_parameters".to_owned() => BuildTimeValue::Array(native_parameters)),
            (
                "native_parameter_count".to_owned(),
                BuildTimeValue::Int(signature.native_parameters.len() as i64),
            ),
            row!("callback_binders".to_owned() => BuildTimeValue::Array(callback_binders)),
            (
                "callback_binder_count".to_owned(),
                BuildTimeValue::Int(signature.callback_binders.len() as i64),
            ),
            row!("callback_demands".to_owned() => BuildTimeValue::Array(callback_demands)),
            (
                "callback_demand_count".to_owned(),
                BuildTimeValue::Int(signature.callback_demands.len() as i64),
            ),
            row!("has_result".to_owned() => BuildTimeValue::Bool(signature.result.is_some())),
            (
                "result".to_owned(),
                BuildTimeValue::Int(i64::from(signature.result.unwrap_or(0))),
            ),
        ],
    })
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
                    row!("parameter".to_owned() => BuildTimeValue::Int(parameter.get() as i64)),
                    row!("layout".to_owned() => BuildTimeValue::Int(layout.get() as i64)),
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
                row!("element".to_owned() => BuildTimeValue::Int(i64::from(element))),
                ("length".to_owned(), BuildTimeValue::Int(i64::from(length))),
            ],
        ),
        BoundaryValueClass::Record {
            first_field,
            field_count,
        } => case(
            "Record",
            vec![
                row!("first_field".to_owned() => BuildTimeValue::Int(i64::from(first_field))),
                row!("field_count".to_owned() => BuildTimeValue::Int(i64::from(field_count))),
            ],
        ),
    };
    BuildTimeValue::Struct {
        type_name: "ValueShape".to_owned(),
        fields: vec![
            ("class".to_owned(), class),
            row!("byte_size".to_owned() => BuildTimeValue::Int(i64::from(shape.byte_size))),
            row!("alignment".to_owned() => BuildTimeValue::Int(i64::from(shape.alignment))),
        ],
    }
}

fn build_boundary_value_field(field: BoundaryValueField) -> BuildTimeValue {
    BuildTimeValue::Struct {
        type_name: "ValueField".to_owned(),
        fields: vec![
            row!("shape".to_owned() => BuildTimeValue::Int(i64::from(field.shape))),
            row!("byte_offset".to_owned() => BuildTimeValue::Int(i64::from(field.byte_offset))),
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
    let application = publish_materialized_application(signature, validated)?;
    Ok((application.report_fingerprint, application.commitment))
}

/// Both authored and default policies publish the complete source-free C shape
/// together with its validated plan. Returning only a bare plan discarded the
/// opaque representation graph before the runtime could admit a typed host.
fn publish_materialized_application(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> Result<calling_conventions::BoundaryCallingApplication, String> {
    let mut application = super::callback_bindings::source_free_application(signature, validated)?;
    application.commitment = application.recomputed_commitment()?;
    application.report_fingerprint = callback_nominal_report_fingerprint(
        b"omega.boundary-plan-application-report.v3",
        &[&application.commitment],
    );
    Ok(application)
}

fn invalid_authored_plan(reason: String) -> String {
    format!("calling policy accepted an invalid plan: {reason}")
}
