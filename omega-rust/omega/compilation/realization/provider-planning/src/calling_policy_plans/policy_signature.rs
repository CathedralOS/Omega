//! Source-free calling-policy input and exact returned-plan validation.
//!
//! The same canonical geometry enters policy execution and native application
//! publication. Source declaration correspondence stays with preparation and
//! package review; it is neither retained nor consulted by this protocol.

use super::build_time_decoding::{
    CALLBACK_FIELD_PATH_CAPACITY, CALLBACK_MATERIALIZATION_CAPACITY, PARAMETER_CAPACITY,
    VALUE_FIELD_CAPACITY, VALUE_SHAPE_CAPACITY, case,
};
use build_time_evaluation::BuildTimeValue;
use calling_conventions::{
    BoundaryCallbackBinding, BoundaryNativeParameterGeometry, BoundaryNativeParameterOrigin,
    BoundaryNativeParameterShape, BoundaryPlanResult, BoundarySignatureGeometry,
    BoundaryValueClass, BoundaryValueField, BoundaryValueGraph, BoundaryValueShape, CallSignature,
    CallbackMaterializationContext, NativePlace, ValidatedBoundaryEntryPlan, ValueShape,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};

/// Target geometry and callback obligations supplied to one policy activation.
/// This is descriptive input, not provider selection or callback authority.
pub struct CallingPolicySignature {
    pub target: target::NativeTarget,
    pub geometry: BoundarySignatureGeometry,
    pub callbacks: CallbackMaterializationContext,
}

impl CallingPolicySignature {
    pub(crate) fn native_parameters(&self) -> Result<&[BoundaryNativeParameterGeometry], String> {
        self.geometry
            .native_parameters
            .span(self.geometry.native_parameter_rows)
            .ok_or_else(|| "calling policy signature has stale native parameter storage".into())
    }

    pub(crate) fn callback_binders(&self) -> Result<&[BoundaryCallbackBinding], String> {
        self.geometry
            .callback_bindings
            .span(self.geometry.callback_binding_rows)
            .ok_or_else(|| "calling policy signature has stale callback binder storage".into())
    }

    fn value_graph(&self) -> BoundaryValueGraph<'_> {
        BoundaryValueGraph {
            shapes: &self.geometry.shapes,
            fields: &self.geometry.fields,
        }
    }
}

macro_rules! row {
    ($a:expr => $b:expr) => {
        ($a, $b)
    };
}

/// The zero-valued case an absent optional boundary field decodes to.
fn zero_case(variant: &str, field: &str) -> BuildTimeValue {
    case(variant, vec![(field.to_owned(), BuildTimeValue::Int(0))])
}

pub(crate) fn encode_calling_policy_signature(
    signature: &CallingPolicySignature,
) -> Result<BuildTimeValue, String> {
    let native_formals = signature.native_parameters()?;
    let binder_rows = signature.callback_binders()?;
    let demands = &signature.callbacks.demands;
    // The fixed arrays and their explicit counts are one vocabulary contract.
    // Refuse excess input before padding; truncating the arrays while retaining
    // a larger count would violate the policy entry's checked invariants.
    for (label, count, capacity) in [
        (
            "parameters",
            signature.geometry.parameters.len(),
            PARAMETER_CAPACITY,
        ),
        (
            "shapes",
            signature.geometry.shapes.len(),
            VALUE_SHAPE_CAPACITY,
        ),
        (
            "fields",
            signature.geometry.fields.len(),
            VALUE_FIELD_CAPACITY,
        ),
        (
            "native parameters",
            native_formals.len(),
            PARAMETER_CAPACITY,
        ),
        (
            "callback binders",
            binder_rows.len(),
            CALLBACK_MATERIALIZATION_CAPACITY,
        ),
        (
            "callback demands",
            demands.len(),
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
                .geometry
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
                .geometry
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
            signature
                .geometry
                .parameters
                .get(index)
                .copied()
                .unwrap_or(0),
        )));
    }
    let mut callback_binders = Vec::with_capacity(CALLBACK_MATERIALIZATION_CAPACITY);
    for index in 0..CALLBACK_MATERIALIZATION_CAPACITY {
        let binder = binder_rows.get(index);
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
        let parameter = native_formals.get(index);
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
        let demand = demands.get(index);
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
            row!("shape_count".to_owned() => BuildTimeValue::Int(signature.geometry.shapes.len() as i64)),
            ("fields".to_owned(), BuildTimeValue::Array(fields)),
            row!("field_count".to_owned() => BuildTimeValue::Int(signature.geometry.fields.len() as i64)),
            ("parameters".to_owned(), BuildTimeValue::Array(parameters)),
            (
                "parameter_count".to_owned(),
                BuildTimeValue::Int(signature.geometry.parameters.len() as i64),
            ),
            row!("native_parameters".to_owned() => BuildTimeValue::Array(native_parameters)),
            (
                "native_parameter_count".to_owned(),
                BuildTimeValue::Int(native_formals.len() as i64),
            ),
            row!("callback_binders".to_owned() => BuildTimeValue::Array(callback_binders)),
            (
                "callback_binder_count".to_owned(),
                BuildTimeValue::Int(binder_rows.len() as i64),
            ),
            row!("callback_demands".to_owned() => BuildTimeValue::Array(callback_demands)),
            (
                "callback_demand_count".to_owned(),
                BuildTimeValue::Int(demands.len() as i64),
            ),
            row!("has_result".to_owned() => BuildTimeValue::Bool(signature.geometry.result.is_some())),
            (
                "result".to_owned(),
                BuildTimeValue::Int(i64::from(signature.geometry.result.unwrap_or(0))),
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

pub(crate) fn validate_calling_policy_result(
    result: BoundaryPlanResult,
    signature: &CallingPolicySignature,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    let native_formals = signature.native_parameters()?;
    let demands = &signature.callbacks.demands;
    let BoundaryPlanResult::Accepted(plan) = result else {
        return validate_boundary_plan_result(result, &CallSignature::default())
            .map_err(|diagnostic| diagnostic.to_string());
    };
    if plan.call.parameters.len() != native_formals.len() {
        return Err(invalid_authored_plan(format!(
            "plan places {} parameters for a boundary signature with {} native parameters",
            plan.call.parameters.len(),
            native_formals.len()
        )));
    }
    if plan.call.result.is_some() != signature.geometry.result.is_some() {
        return Err(invalid_authored_plan(
            "plan result presence does not match the boundary signature".to_owned(),
        ));
    }
    for (index, (placement, native_parameter)) in plan
        .call
        .parameters
        .iter()
        .zip(native_formals.iter())
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
    if let (Some(placement), Some(root)) = (&plan.call.result, signature.geometry.result) {
        signature
            .value_graph()
            .validate_abi_shape(root, placement.shape, plan.call.policy)
            .map_err(|reason| {
                invalid_authored_plan(format!("result classification is invalid: {reason}"))
            })?;
    }
    let classified = CallSignature::of_placements(&plan.call.parameters, plan.call.result.as_ref());
    if demands.is_empty() {
        return validate_boundary_plan_result(BoundaryPlanResult::Accepted(plan), &classified)
            .map_err(|diagnostic| diagnostic.to_string());
    }
    let context = &signature.callbacks;
    validate_boundary_entry_plan_with_callback_materializations(plan, &classified, context)
        .map_err(|diagnostic| diagnostic.to_string())
}

fn invalid_authored_plan(reason: String) -> String {
    format!("calling policy accepted an invalid plan: {reason}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::{Arena, HandleSpan};
    use calling_conventions::{
        CallbackBinderRequirement, CallbackMaterialization, CallbackRequirementId, CallingPolicy,
        NativeCallbackDemand, NativeParameterId, StaticMachineBinderId,
        evaluate_ordinary_boundary_entry_plan,
    };

    #[test]
    fn canonical_input_keeps_native_only_arguments_and_callback_obligations() {
        let binder = StaticMachineBinderId::new(11).unwrap();
        let requirement = CallbackRequirementId::new(12).unwrap();
        let semantic = NativeParameterId::new(13).unwrap();
        let callback = NativeParameterId::new(14).unwrap();
        let mut native_parameters = Arena::new();
        let native_parameter_rows = native_parameters.insert_many([
            BoundaryNativeParameterGeometry {
                identity: semantic,
                native_ordinal: 0,
                shape: BoundaryNativeParameterShape::Semantic(0),
                origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 0 },
            },
            BoundaryNativeParameterGeometry {
                identity: callback,
                native_ordinal: 1,
                shape: BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size: 8,
                    alignment: 8,
                },
                origin: BoundaryNativeParameterOrigin::PrivateCallback {
                    binder,
                    requirement,
                },
            },
        ]);
        let mut callback_bindings = Arena::new();
        let callback_binding_rows = callback_bindings.insert_many([BoundaryCallbackBinding {
            binder,
            requirement,
            static_machine_ordinal: 2,
            requirement_identity: "exact callback requirement".into(),
            invokes_synchronously: true,
        }]);
        let destination = NativePlace::Parameter(callback);
        let signature = CallingPolicySignature {
            target: target::TargetProfile::MacosArm64.native_target(),
            geometry: BoundarySignatureGeometry {
                shapes: vec![BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: 8,
                    alignment: 8,
                }],
                fields: Vec::new(),
                parameters: vec![0],
                result: None,
                native_parameters,
                native_parameter_rows,
                callback_layouts: Arena::new(),
                callback_layout_rows: HandleSpan::empty(),
                callback_bindings,
                callback_binding_rows,
            },
            callbacks: CallbackMaterializationContext {
                binders: vec![CallbackBinderRequirement {
                    binder,
                    requirement,
                }],
                demands: vec![NativeCallbackDemand {
                    destination: destination.clone(),
                    requirement,
                }],
            },
        };
        let BuildTimeValue::Struct { fields, .. } =
            encode_calling_policy_signature(&signature).expect("canonical input")
        else {
            panic!("signature input");
        };
        for (name, expected) in [
            ("parameter_count", 1),
            ("native_parameter_count", 2),
            ("callback_binder_count", 1),
            ("callback_demand_count", 1),
        ] {
            assert!(
                fields.iter().any(|(field, value)| {
                    field == name
                        && matches!(value, BuildTimeValue::Int(value) if *value == expected)
                }),
                "{name}"
            );
        }
        let word = ValueShape::integer(8, 8);
        let classified = CallSignature {
            parameters: vec![word, word],
            result: None,
        };
        let mut plan = evaluate_ordinary_boundary_entry_plan(CallingPolicy::Aapcs64, &classified)
            .expect("ordinary target placements")
            .plan()
            .clone();
        plan.call
            .callback_materializations
            .push(CallbackMaterialization {
                binder,
                destination,
            });
        validate_calling_policy_result(BoundaryPlanResult::Accepted(plan.clone()), &signature)
            .expect("native-only callback remains placed");

        let mut omitted = plan.clone();
        omitted.call.callback_materializations.clear();
        assert!(
            validate_calling_policy_result(BoundaryPlanResult::Accepted(omitted), &signature)
                .is_err(),
            "lost callback demand"
        );
        plan.call.parameters[1].shape = ValueShape::integer(4, 4);
        assert!(
            validate_calling_policy_result(BoundaryPlanResult::Accepted(plan), &signature)
                .unwrap_err()
                .contains("target function-pointer ABI shape")
        );
    }
}
