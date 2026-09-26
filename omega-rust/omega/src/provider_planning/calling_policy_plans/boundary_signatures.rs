//! Boundary signatures: the value classes, shapes and fields a boundary
//! call carries, the materialized signature, and how signatures are
//! collected from the typed program and its calling-policy machines.

use crate::provider_planning::calling_policy_plans::build_time_decoding::PARAMETER_CAPACITY;
use crate::provider_planning::calling_policy_plans::callback_bindings::{
    callback_plan_report_fingerprint, validate_fresh_native_parameter_report_identity,
};
use crate::provider_planning::calling_policy_plans::callback_layout_catalog::BoundaryCallbackLayoutEntry;
use crate::provider_planning::calling_policy_plans::opaque_representations::{
    BoundaryOpaqueRepresentationMovement, BoundaryOpaqueRepresentationMovementRole,
    BoundaryOpaqueRepresentationPathElement, BoundaryOpaqueRepresentationUse,
};
use crate::provider_planning::calling_policy_plans::value_shapes::{
    classify_boundary_aggregate, opaque_representation_value_shape, plain_data_value_shape,
    value_shape_from_type,
};
use crate::representation_planning::OpaqueRepresentationSelection;
use abstract_operations_to_target_operations::calling_conventions::{
    CallSignature, CallbackRequirementId, CallingPolicy, NativeCallbackDemand, NativeParameterId,
    StaticMachineBinderId, ValidatedBoundaryEntryPlan, ValueClass, ValueShape,
    callback_requirement_id, nominal_callback_native_parameter_id,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    TypeReferenceHandle, TypeReferenceNode,
};
use target::NativeTarget;

#[derive(Clone)]
pub(crate) struct TraitTypeBinding {
    parameter_symbol: symbols::SymbolHandle,
    parameter_name: String,
    actual: TypeReferenceHandle,
}

pub(crate) struct BoundarySignatureInstance<'a> {
    pub(crate) signature:
        &'a symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateSignature,
    pub(crate) requirement_identity: String,
    pub(crate) bindings: Vec<TraitTypeBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryValueClass {
    Integer,
    Float,
    Reference,
    FixedArray { element: u16, length: u16 },
    Record { first_field: u16, field_count: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryValueShape {
    pub(crate) class: BoundaryValueClass,
    pub(crate) byte_size: u16,
    pub(crate) alignment: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryValueField {
    pub(crate) shape: u16,
    pub(crate) byte_offset: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedBoundarySignature {
    pub(crate) owner_requirement_identity: String,
    pub(crate) native_target: NativeTarget,
    pub(crate) opaque_representations: Vec<BoundaryOpaqueRepresentationUse>,
    pub(crate) shapes: Vec<BoundaryValueShape>,
    pub(crate) fields: Vec<BoundaryValueField>,
    pub(crate) parameters: Vec<u16>,
    pub(crate) callback_binders: Vec<BoundaryCallbackBinder>,
    pub(crate) callback_demands: Vec<NativeCallbackDemand>,
    pub(crate) callback_layout_catalog: Vec<BoundaryCallbackLayoutEntry>,
    pub(crate) native_parameters: Vec<BoundaryNativeParameter>,
    pub(crate) direct_callback_parameters: Vec<BoundaryDirectCallbackParameter>,
    pub(crate) result: Option<u16>,
}

impl MaterializedBoundarySignature {
    /// Named layout-owned destinations in native-demand order. Direct callback
    /// parameters have no layout field and therefore contribute no entry here.
    /// Typed slot applications survive without retaining a complete LayoutPlan.
    pub fn callback_layout_catalog(&self) -> &[BoundaryCallbackLayoutEntry] {
        &self.callback_layout_catalog
    }

    pub fn owner_requirement_identity(&self) -> &str {
        &self.owner_requirement_identity
    }

    pub const fn native_target(&self) -> NativeTarget {
        self.native_target
    }

    /// Compiler-derived opaque representations actually used by value while
    /// materializing this exact boundary signature. An unused build selection
    /// is deliberately absent.
    pub fn opaque_representation_uses(&self) -> &[BoundaryOpaqueRepresentationUse] {
        &self.opaque_representations
    }

    pub fn shapes(&self) -> &[BoundaryValueShape] {
        &self.shapes
    }

    pub fn fields(&self) -> &[BoundaryValueField] {
        &self.fields
    }

    pub fn parameters(&self) -> &[u16] {
        &self.parameters
    }

    pub const fn result(&self) -> Option<u16> {
        self.result
    }

    /// Rejoin one exact opaque shape node to the top-level semantic occurrence
    /// and target placement that carries it. This consumes a replay-validated
    /// plan; equal-looking layouts or an aggregate plan digest are not enough.
    pub fn opaque_representation_movement(
        &self,
        representation: &BoundaryOpaqueRepresentationUse,
        validated: &ValidatedBoundaryEntryPlan,
    ) -> Result<BoundaryOpaqueRepresentationMovement, String> {
        if self
            .opaque_representations
            .iter()
            .filter(|candidate| *candidate == representation)
            .count()
            != 1
        {
            return Err(
                "opaque representation use is not one exact materialized-signature occurrence"
                    .to_owned(),
            );
        }
        let mut carrying_parameters = Vec::new();
        for (formal_ordinal, root) in self.parameters.iter().copied().enumerate() {
            if let Some(path) =
                boundary_shape_path(&self.shapes, &self.fields, root, representation.shape_root)?
            {
                carrying_parameters.push((formal_ordinal, root, path));
            }
        }
        let result_path = match self.result {
            Some(root) => {
                boundary_shape_path(&self.shapes, &self.fields, root, representation.shape_root)?
            }
            None => None,
        };
        if carrying_parameters.len() + usize::from(result_path.is_some()) != 1 {
            return Err(format!(
                "opaque representation shape node belongs to {} top-level boundary occurrences; expected one",
                carrying_parameters.len() + usize::from(result_path.is_some()),
            ));
        }
        if let Some((formal_ordinal, root, path)) = carrying_parameters.into_iter().next() {
            let formal_ordinal = u32::try_from(formal_ordinal)
                .map_err(|_| "opaque representation formal ordinal exceeds u32".to_owned())?;
            let matching = self
                .native_parameters
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.origin,
                        BoundaryNativeParameterOrigin::SemanticFormal {
                            formal_ordinal: candidate,
                        } if candidate == formal_ordinal
                    ) && matches!(
                        parameter.shape,
                        BoundaryNativeParameterShape::Semantic(candidate) if candidate == root
                    )
                })
                .collect::<Vec<_>>();
            let [native] = matching.as_slice() else {
                return Err(format!(
                    "opaque representation formal parameter maps to {} native parameters; expected one",
                    matching.len(),
                ));
            };
            let placement = validated
                .plan()
                .call
                .parameters
                .get(
                    usize::try_from(native.native_ordinal).map_err(|_| {
                        "opaque representation native ordinal exceeds usize".to_owned()
                    })?,
                )
                .ok_or_else(|| {
                    "opaque representation native parameter has no validated placement".to_owned()
                })?
                .clone();
            return Ok(BoundaryOpaqueRepresentationMovement {
                role: BoundaryOpaqueRepresentationMovementRole::Parameter {
                    formal_ordinal,
                    native_ordinal: native.native_ordinal,
                },
                path,
                placement,
            });
        }
        let placement =
            validated.plan().call.result.clone().ok_or_else(|| {
                "opaque representation result has no validated placement".to_owned()
            })?;
        Ok(BoundaryOpaqueRepresentationMovement {
            role: BoundaryOpaqueRepresentationMovementRole::Result,
            path: result_path.expect("one validated result occurrence has a path"),
            placement,
        })
    }
}

pub(crate) fn boundary_shape_path(
    shapes: &[BoundaryValueShape],
    fields: &[BoundaryValueField],
    root: u16,
    target: u16,
) -> Result<Option<Vec<BoundaryOpaqueRepresentationPathElement>>, String> {
    let shape = shapes
        .get(usize::from(root))
        .ok_or_else(|| "boundary shape graph contains an out-of-range node".to_owned())?;
    if root == target {
        return Ok(Some(Vec::new()));
    }
    match shape.class {
        BoundaryValueClass::Integer | BoundaryValueClass::Float | BoundaryValueClass::Reference => {
            Ok(None)
        }
        BoundaryValueClass::FixedArray { element, .. } => {
            if element >= root {
                return Err("boundary fixed-array shape graph is not acyclic postorder".to_owned());
            }
            let Some(mut path) = boundary_shape_path(shapes, fields, element, target)? else {
                return Ok(None);
            };
            path.insert(
                0,
                BoundaryOpaqueRepresentationPathElement::FixedArrayElement,
            );
            Ok(Some(path))
        }
        BoundaryValueClass::Record {
            first_field,
            field_count,
        } => {
            let start = usize::from(first_field);
            let end = start
                .checked_add(usize::from(field_count))
                .ok_or_else(|| "boundary record field range overflowed".to_owned())?;
            let children = fields
                .get(start..end)
                .ok_or_else(|| "boundary record shape has an out-of-range field span".to_owned())?;
            let mut found = None;
            for (ordinal, child) in children.iter().enumerate() {
                if child.shape >= root {
                    return Err("boundary record shape graph is not acyclic postorder".to_owned());
                }
                if let Some(mut path) = boundary_shape_path(shapes, fields, child.shape, target)? {
                    path.insert(
                        0,
                        BoundaryOpaqueRepresentationPathElement::RecordField {
                            ordinal: u16::try_from(ordinal).map_err(|_| {
                                "boundary record field ordinal exceeds u16".to_owned()
                            })?,
                        },
                    );
                    if found.replace(path).is_some() {
                        return Err(
                            "opaque representation shape node is reachable through multiple record fields"
                                .to_owned(),
                        );
                    }
                }
            }
            Ok(found)
        }
    }
}

impl BoundaryValueShape {
    pub const fn class(self) -> BoundaryValueClass {
        self.class
    }

    pub const fn byte_size(self) -> u16 {
        self.byte_size
    }

    pub const fn alignment(self) -> u16 {
        self.alignment
    }
}

impl BoundaryValueField {
    pub const fn shape(self) -> u16 {
        self.shape
    }

    pub const fn byte_offset(self) -> u16 {
        self.byte_offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryNativeParameter {
    pub(crate) identity: NativeParameterId,
    pub(crate) native_ordinal: u32,
    pub(crate) shape: BoundaryNativeParameterShape,
    pub(crate) origin: BoundaryNativeParameterOrigin,
    pub(crate) layout_data_symbol: symbols::SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryNativeParameterShape {
    Semantic(u16),
    TargetFunctionPointer { byte_size: u16, alignment: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryNativeParameterOrigin {
    SemanticFormal {
        formal_ordinal: u32,
    },
    PrivateCallback {
        binder: StaticMachineBinderId,
        requirement: CallbackRequirementId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryDirectCallbackParameter {
    pub(crate) name: String,
    pub(crate) identity: NativeParameterId,
    pub(crate) native_ordinal: u32,
    pub(crate) binder: StaticMachineBinderId,
    pub(crate) requirement: CallbackRequirementId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryCallbackBinder {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
    pub static_machine_ordinal: u32,
    pub parameter_symbol: symbols::SymbolHandle,
    pub requirement_trait: symbols::SymbolHandle,
    pub requirement_machine: symbols::SymbolHandle,
}

/// Materialize the ABI of one explicit top-level boundary requirement.
///
/// Unlike a trait requirement, this declaration's `self` is the semantic
/// carrier operated on by the selected satisfier. Conformance checking maps it
/// to the satisfier's first explicit parameter, so it remains in the foreign
/// signature rather than being erased as a provider receiver.
pub(crate) fn call_signature_from_top_level_requirement(
    typed: &TypedTrees,
    requirement: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    entry: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    owner_requirement_identity: &str,
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<MaterializedBoundarySignature, String> {
    if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement
        || requirement.body_is_present
        || !requirement.lifetime_parameters.is_empty()
        || !typed.machine_type_parameters(requirement).is_empty()
    {
        return Err(
            "compatibility calling plans require one nongeneric bodyless top-level boundary requirement"
                .to_owned(),
        );
    }

    let runtime_parameters = typed.state_parameters(entry);
    if runtime_parameters.len() > PARAMETER_CAPACITY {
        return Err(format!(
            "top-level boundary requirement has {} native parameters; calling policies currently support at most {PARAMETER_CAPACITY}",
            runtime_parameters.len(),
        ));
    }

    let mut shapes = Vec::new();
    let mut fields = Vec::new();
    let mut opaque_representations = Vec::new();
    let mut parameters = Vec::with_capacity(runtime_parameters.len());
    let mut native_parameters = Vec::with_capacity(runtime_parameters.len());
    for (formal_ordinal, parameter) in runtime_parameters.iter().enumerate() {
        let (_, root) = if parameter.is_self {
            let attachment = typed
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == requirement.attached_data_symbol)
                .ok_or_else(|| {
                    "top-level boundary requirement `self` lost its exact attached data declaration"
                        .to_owned()
                })?;
            if attachment.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                opaque_representation_value_shape(
                    typed,
                    attachment.symbol,
                    attachment.name.as_str(),
                    &[],
                    &mut Vec::new(),
                    &mut shapes,
                    &mut fields,
                    native_target,
                    opaque_representation_selections,
                    &mut opaque_representations,
                )
            } else {
                plain_data_value_shape(
                    typed,
                    attachment.symbol,
                    attachment.name.as_str(),
                    &[],
                    &mut Vec::new(),
                    &mut shapes,
                    &mut fields,
                    native_target,
                    opaque_representation_selections,
                    &mut opaque_representations,
                )
            }
        } else {
            value_shape_from_type(
                typed,
                parameter.type_reference,
                &[],
                &mut Vec::new(),
                &mut shapes,
                &mut fields,
                native_target,
                opaque_representation_selections,
                &mut opaque_representations,
            )
        }?;
        parameters.push(root);
        let native_ordinal = u32::try_from(formal_ordinal)
            .map_err(|_| "top-level boundary requirement has too many runtime parameters")?;
        let identity = nominal_callback_native_parameter_id(
            owner_requirement_identity,
            parameter.name.as_str(),
        );
        validate_fresh_native_parameter_report_identity(
            &native_parameters,
            identity,
            native_ordinal,
        )?;
        native_parameters.push(BoundaryNativeParameter {
            identity,
            native_ordinal,
            shape: BoundaryNativeParameterShape::Semantic(root),
            origin: BoundaryNativeParameterOrigin::SemanticFormal {
                formal_ordinal: native_ordinal,
            },
            layout_data_symbol: if parameter.is_self {
                requirement.attached_data_symbol
            } else {
                exact_boundary_layout_root_symbol(typed, parameter.type_reference)
            },
        });
    }

    let result = if entry.return_type.is_valid() {
        let (_, root) = value_shape_from_type(
            typed,
            entry.return_type,
            &[],
            &mut Vec::new(),
            &mut shapes,
            &mut fields,
            native_target,
            opaque_representation_selections,
            &mut opaque_representations,
        )?;
        Some(root)
    } else {
        None
    };

    Ok(MaterializedBoundarySignature {
        owner_requirement_identity: owner_requirement_identity.to_owned(),
        native_target,
        opaque_representations,
        shapes,
        fields,
        parameters,
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        callback_layout_catalog: Vec::new(),
        native_parameters,
        direct_callback_parameters: Vec::new(),
        result,
    })
}

pub(crate) fn exact_compatibility_overload_index<'identity>(
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

pub(crate) fn compatibility_call_signature(
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
                    classified_boundary_shape(materialized, root, policy)
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        result: materialized
            .result
            .map(|root| {
                if syscall_words {
                    Ok(ValueShape::integer(8, 8))
                } else {
                    classified_boundary_shape(materialized, root, policy)
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

pub(crate) fn boundary_policy_instances(
    typed: &TypedTrees,
    boundary: &symbol_resolved_trees_to_typed_trees::typed_trees::trait_definition::TraitDefinition,
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
    parameters: &[symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameter],
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

pub(crate) fn concrete_policy_type_name(
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

pub(crate) fn find_policy_machine<'a>(
    typed: &'a TypedTrees,
    policy_type: &str,
) -> Option<&'a symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine> {
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

pub(crate) fn collect_boundary_signatures<'a>(
    typed: &'a TypedTrees,
    definition: &'a symbol_resolved_trees_to_typed_trees::typed_trees::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
    visited: &mut Vec<symbols::SymbolHandle>,
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
    definition: &symbol_resolved_trees_to_typed_trees::typed_trees::trait_definition::TraitDefinition,
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

pub(crate) fn substituted_type_reference(
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

pub(crate) fn call_signature_from_typed(
    typed: &TypedTrees,
    signature: &symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateSignature,
    bindings: &[TraitTypeBinding],
    owner_requirement_identity: &str,
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<MaterializedBoundarySignature, String> {
    let mut shapes = Vec::new();
    let mut fields = Vec::new();
    let mut opaque_representations = Vec::new();
    let mut parameters = Vec::new();
    let mut callback_binders = Vec::new();
    let mut static_machine_ordinal = 0u32;
    for parameter in typed.state_signature_type_parameters(signature) {
        let symbol_resolved_trees_to_typed_trees::typed_trees::data::TypeParameterKind::Machine {
            contract,
        } = &parameter.kind
        else {
            continue;
        };
        let ordinal = static_machine_ordinal;
        static_machine_ordinal = static_machine_ordinal
            .checked_add(1)
            .ok_or_else(|| "boundary signature has too many static machine binders".to_owned())?;
        let symbol_resolved_trees_to_typed_trees::typed_trees::data::MachineParameterContract::Nominal {
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
        let binder_report_fingerprint = callback_plan_report_fingerprint(
            b"omega.callback-binder.v1",
            &[
                owner_requirement_identity.as_bytes(),
                &ordinal.to_le_bytes(),
                parameter.name.as_str().as_bytes(),
            ],
        );
        callback_binders.push(BoundaryCallbackBinder {
            binder: StaticMachineBinderId::new(binder_report_fingerprint)
                .expect("callback binder fingerprint is nonzero"),
            requirement: callback_requirement_id(&requirement_identity),
            static_machine_ordinal: ordinal,
            parameter_symbol: parameter.symbol,
            requirement_trait: *trait_definition,
            requirement_machine: *requirement,
        });
    }

    let runtime_parameters = typed
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let total_native_parameters = runtime_parameters
        .len()
        .checked_add(signature.native_callback_parameters.len())
        .ok_or_else(|| "boundary native parameter telescope length overflow".to_owned())?;
    if total_native_parameters > PARAMETER_CAPACITY {
        return Err(format!(
            "boundary signature has {total_native_parameters} native parameters; calling policies currently support at most {PARAMETER_CAPACITY}"
        ));
    }
    let mut direct_callback_parameters = Vec::new();
    for direct in &signature.native_callback_parameters {
        let native_ordinal = usize::try_from(direct.native_ordinal)
            .map_err(|_| "native callback parameter ordinal exceeds usize".to_owned())?;
        if native_ordinal >= total_native_parameters {
            return Err(format!(
                "native callback parameter `{}` has telescope position {}, outside {} declared native parameters",
                direct.name, direct.native_ordinal, total_native_parameters,
            ));
        }
        if direct_callback_parameters
            .iter()
            .any(|prior: &BoundaryDirectCallbackParameter| {
                prior.native_ordinal == direct.native_ordinal || prior.name == direct.name.as_str()
            })
            || runtime_parameters
                .iter()
                .any(|runtime| runtime.name.as_str() == direct.name.as_str())
        {
            return Err(format!(
                "native callback parameter `{}` repeats a native telescope position or declared parameter name",
                direct.name,
            ));
        }
        let Some(machine_parameter) = typed
            .state_signature_type_parameters(signature)
            .iter()
            .find(|parameter| parameter.name.as_str() == direct.binder.as_str())
        else {
            return Err(format!(
                "native callback parameter `{}` names unknown machine binder `{}`",
                direct.name, direct.binder,
            ));
        };
        let Some(binder) = callback_binders
            .iter()
            .find(|binder| binder.parameter_symbol == machine_parameter.symbol)
        else {
            return Err(format!(
                "native callback parameter `{}` binder `{}` does not have one exact nominal callback requirement",
                direct.name, direct.binder,
            ));
        };
        if direct_callback_parameters
            .iter()
            .any(|prior: &BoundaryDirectCallbackParameter| prior.binder == binder.binder)
        {
            return Err(format!(
                "native callback binder `{}` is assigned to more than one declared native callback parameter",
                direct.binder,
            ));
        }
        direct_callback_parameters.push(BoundaryDirectCallbackParameter {
            name: direct.name.as_str().to_owned(),
            identity: nominal_callback_native_parameter_id(
                owner_requirement_identity,
                direct.name.as_str(),
            ),
            native_ordinal: direct.native_ordinal,
            binder: binder.binder,
            requirement: binder.requirement,
        });
    }
    direct_callback_parameters.sort_unstable_by_key(|parameter| parameter.native_ordinal);

    let mut native_parameters = Vec::with_capacity(runtime_parameters.len());
    let mut next_native_ordinal = 0u32;
    for (formal_ordinal, parameter) in runtime_parameters.iter().enumerate() {
        while direct_callback_parameters
            .iter()
            .any(|direct| direct.native_ordinal == next_native_ordinal)
        {
            next_native_ordinal = next_native_ordinal
                .checked_add(1)
                .ok_or_else(|| "boundary native parameter ordinal overflow".to_owned())?;
        }
        let (_, root) = value_shape_from_type(
            typed,
            parameter.type_reference,
            bindings,
            &mut Vec::new(),
            &mut shapes,
            &mut fields,
            native_target,
            opaque_representation_selections,
            &mut opaque_representations,
        )?;
        parameters.push(root);
        let type_reference = substituted_type_reference(typed, parameter.type_reference, bindings);
        let layout_data_symbol = exact_boundary_layout_root_symbol(typed, type_reference);
        let formal_ordinal = u32::try_from(formal_ordinal)
            .map_err(|_| "boundary signature has too many runtime parameters")?;
        let identity = nominal_callback_native_parameter_id(
            owner_requirement_identity,
            parameter.name.as_str(),
        );
        validate_fresh_native_parameter_report_identity(
            &native_parameters,
            identity,
            next_native_ordinal,
        )?;
        native_parameters.push(BoundaryNativeParameter {
            identity,
            native_ordinal: next_native_ordinal,
            shape: BoundaryNativeParameterShape::Semantic(root),
            origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal },
            layout_data_symbol,
        });
        next_native_ordinal = next_native_ordinal
            .checked_add(1)
            .ok_or_else(|| "boundary native parameter ordinal overflow".to_owned())?;
    }
    let result = if signature.return_type.is_valid() {
        let (_, root) = value_shape_from_type(
            typed,
            signature.return_type,
            bindings,
            &mut Vec::new(),
            &mut shapes,
            &mut fields,
            native_target,
            opaque_representation_selections,
            &mut opaque_representations,
        )?;
        Some(root)
    } else {
        None
    };
    Ok(MaterializedBoundarySignature {
        owner_requirement_identity: owner_requirement_identity.to_owned(),
        native_target,
        opaque_representations,
        shapes,
        fields,
        parameters,
        callback_binders,
        callback_demands: Vec::new(),
        callback_layout_catalog: Vec::new(),
        native_parameters,
        direct_callback_parameters,
        result,
    })
}

pub(crate) fn exact_boundary_layout_root_symbol(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> symbols::SymbolHandle {
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
            | TypeReferenceNode::Unit => return symbols::SymbolHandle::invalid(),
        }
    }
}
