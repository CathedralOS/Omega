//! Build-time evaluation of source-authored calling policies.
//!
//! The source vocabulary lives in `omega/language/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.

use omega_calling_conventions::{
    BoundaryEntryPlan, BoundaryPlanResult, CallPlan, CallSignature, CallingPolicy,
    CallingPolicyRejection, EntryControl, EntryStack, IndirectPointerLocation, MachineRegime,
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StatePlan,
    SystemVEightbyteClass, ValidatedBoundaryEntryPlan, ValueClass, ValueLocation, ValuePlacement,
    ValueShape, validate_boundary_plan_result,
};
use omega_core::diagnostics::Diagnostic;
use omega_interpreter::BuildTimeValue;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

const PARAMETER_CAPACITY: usize = 32;
const VALUE_SHAPE_CAPACITY: usize = 256;
const VALUE_FIELD_CAPACITY: usize = 256;
const LOCATION_CAPACITY: usize = 16;
const REGISTER_CAPACITY: usize = 64;

#[derive(Clone)]
struct TraitTypeBinding {
    parameter_symbol: omega_core::symbols::SymbolHandle,
    parameter_name: String,
    actual: TypeReferenceHandle,
}

struct BoundarySignatureInstance<'a> {
    signature: &'a omega_typed_trees::signature::StateSignature,
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
struct MaterializedBoundarySignature {
    shapes: Vec<BoundaryValueShape>,
    fields: Vec<BoundaryValueField>,
    parameters: Vec<u16>,
    result: Option<u16>,
}

/// Discover concrete `Calling<C>` relationships, evaluate `C::plan` once for
/// every method in the boundary service surface, and retain only canonical
/// evaluated identities on the typed program.
pub(crate) fn compute_boundary_calling_plans(
    typed: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let Some(calling_policy_trait) = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str().rsplit("::").next() == Some("CallingPolicy"))
    else {
        // Programs that do not import std::calling cannot accidentally opt in
        // merely by having an unrelated local trait named Calling.
        return Ok(());
    };
    let calling_policy_symbol = calling_policy_trait.symbol;
    let Some(calling_trait) = typed.traits().iter().find(|definition| {
        definition.name.as_str().rsplit("::").next() == Some("Calling")
            && typed.trait_type_parameters(definition).len() == 1
            && typed.trait_machine_signatures(definition).is_empty()
    }) else {
        return Ok(());
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
                let boundary_signature =
                    call_signature_from_typed(typed, signature, &signature_instance.bindings)
                        .map_err(|reason| {
                            vec![Diagnostic::error(format!(
                        "cannot materialize boundary signature `{}::{}` for `{}`: {reason}",
                        boundary.name, signature.name, policy_machine.name
                    ))
                    .with_source_span(relationship_span)]
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
        return Ok(());
    }
    let admission = super::build_time_admission::BuildTimeAdmissionPlan::infer(typed);
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
        )
        .map_err(|reason| vec![Diagnostic::error(reason).with_source_span(relationship_span)])?;
        evaluated.push(
            omega_typed_trees::typed_trees::BoundaryCallingPlanIdentity {
                boundary_trait,
                boundary_arguments,
                requirement_machine,
                fingerprint: validated.contract_fingerprint(),
                boundary_entry_plan: validated.plan().clone(),
            },
        );
    }
    for identity in evaluated {
        typed.record_boundary_calling_plan(identity);
    }
    Ok(())
}

fn boundary_policy_instances(
    typed: &TypedTrees,
    boundary: &omega_typed_trees::trait_definition::TraitDefinition,
    policy_argument: TypeReferenceHandle,
) -> Result<Vec<(Vec<TypeReferenceHandle>, TypeReferenceHandle)>, String> {
    let parameters = typed.trait_type_parameters(boundary);
    if parameters.is_empty() {
        return Ok(vec![(Vec::new(), policy_argument)]);
    }
    let policy_parameter_index = named_parameter_index(typed, policy_argument, parameters);

    let mut instances = Vec::new();
    for conformance in typed
        .data_conformances()
        .iter()
        .filter(|conformance| names_match(conformance.trait_name.as_str(), boundary.name.as_str()))
    {
        let arguments = typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .to_vec();
        if arguments.len() != parameters.len() {
            return Err(format!(
                "conformance `{} satisfies {}` supplies {} argument(s), expected {}",
                conformance.type_name,
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
    parameters: &[omega_typed_trees::data::TypeParameter],
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
) -> Option<&'a omega_typed_trees::machine::Machine> {
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
    definition: &'a omega_typed_trees::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
    visited: &mut Vec<omega_core::symbols::SymbolHandle>,
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
        if !signatures
            .iter()
            .any(|candidate| candidate.signature.name == signature.name)
        {
            signatures.push(BoundarySignatureInstance {
                signature,
                bindings: bindings.clone(),
            });
        }
    }
    visited.pop();
}

fn trait_type_bindings(
    typed: &TypedTrees,
    definition: &omega_typed_trees::trait_definition::TraitDefinition,
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
    signature: &omega_typed_trees::signature::StateSignature,
    bindings: &[TraitTypeBinding],
) -> Result<MaterializedBoundarySignature, String> {
    let mut shapes = Vec::new();
    let mut fields = Vec::new();
    let mut parameters = Vec::new();
    for parameter in typed
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
    {
        let (_, root) = value_shape_from_type(
            typed,
            parameter.type_reference,
            bindings,
            &mut Vec::new(),
            &mut shapes,
            &mut fields,
        )?;
        parameters.push(root);
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
    Ok(MaterializedBoundarySignature {
        shapes,
        fields,
        parameters,
        result,
    })
}

fn value_shape_from_type(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<omega_core::symbols::SymbolHandle>,
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
            length: omega_typed_trees::types::FixedArrayLength::Literal(length),
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
    symbol: omega_core::symbols::SymbolHandle,
    name: &str,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<omega_core::symbols::SymbolHandle>,
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
        .find(|layout| layout.data_name == definition.name.as_str());
    if let Some(layout) = planned_layout
        && layout.offsets.len() != typed.data_members(definition).len()
    {
        return Err(format!(
            "plan-laid data `{name}` has {} members but its layout publishes {} offsets",
            typed.data_members(definition).len(),
            layout.offsets.len()
        ));
    }
    visiting.push(symbol);
    let mut size = 0usize;
    let mut alignment = 1usize;
    let mut float_member_size = None;
    let mut float_members = 0usize;
    let mut record_fields = Vec::new();
    for (field_index, member) in typed.data_members(definition).iter().enumerate() {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            visiting.pop();
            return Err(format!(
                "case data `{name}` is not yet classifiable as a boundary value; pass it by reference"
            ));
        };
        let (shape, field_root) = value_shape_from_type(
            typed,
            field.type_reference,
            bindings,
            visiting,
            shapes,
            fields,
        )?;
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
    let admission = super::build_time_admission::BuildTimeAdmissionPlan::infer(typed);
    evaluate_materialized_calling_policy_plan(typed, &admission, policy_machine, &materialized)
}

fn evaluate_materialized_calling_policy_plan(
    typed: &TypedTrees,
    admission: &super::build_time_admission::BuildTimeAdmissionPlan,
    policy_machine: &str,
    signature: &MaterializedBoundarySignature,
) -> Result<ValidatedBoundaryEntryPlan, String> {
    if signature.parameters.len() > PARAMETER_CAPACITY {
        return Err(format!(
            "boundary signature has {} parameters; calling policies currently support at most {PARAMETER_CAPACITY}",
            signature.parameters.len()
        ));
    }

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == policy_machine)
        .ok_or_else(|| format!("no calling-policy machine named `{policy_machine}` exists"))?;
    admission.require_service_and_operational_floor(typed, machine)?;

    let value = omega_interpreter::evaluate_build_time_machine(
        typed,
        policy_machine,
        vec![build_boundary_signature(signature)],
    )
    .map_err(|reason| {
        format!("build-time evaluation of calling policy `{policy_machine}` failed: {reason}")
    })?;
    let result = decode_boundary_plan_result(&value).map_err(|reason| {
        format!("calling policy `{policy_machine}` returned an invalid result: {reason}")
    })?;

    validate_materialized_boundary_plan_result(result, signature)
}

fn materialized_boundary_signature_from_abi(
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
    validate_boundary_plan_result(BoundaryPlanResult::Accepted(plan), &classified)
        .map_err(|diagnostic| diagnostic.to_string())
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
    Ok(CallPlan {
        policy: decode_calling_convention(field(fields, "convention", "CallPlan")?)?,
        parameters,
        result: has_result
            .then(|| decode_value_placement(field(fields, "result", "CallPlan")?))
            .transpose()?,
        ordinary_clobbers: decode_register_set(field(fields, "ordinary_clobbers", "CallPlan")?)?,
        stack_alignment: u16_value(
            field(fields, "stack_alignment", "CallPlan")?,
            "stack_alignment",
        )?,
        shadow_bytes: u16_value(field(fields, "shadow_bytes", "CallPlan")?, "shadow_bytes")?,
        entry_control: decode_entry_control(field(fields, "entry_control", "CallPlan")?)?,
    })
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
}
