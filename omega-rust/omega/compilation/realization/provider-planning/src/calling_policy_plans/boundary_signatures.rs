//! Boundary signatures: the value classes, shapes and fields a boundary
//! call carries, the materialized signature, and how signatures are
//! collected from the typed program and its calling-policy machines.

use crate::calling_policy_plans::callback_bindings::validate_fresh_native_parameter_report_identity;
use crate::calling_policy_plans::callback_layout_catalog::BoundaryCallbackLayoutEntry;
use crate::calling_policy_plans::opaque_representations::{
    BoundaryOpaqueRepresentationMovement, BoundaryOpaqueRepresentationMovementRole,
    BoundaryOpaqueRepresentationPathElement, BoundaryOpaqueRepresentationUse,
};
pub use calling_conventions::{
    BoundaryNativeParameterOrigin, BoundaryValueClass, BoundaryValueField, BoundaryValueShape,
};
use calling_conventions::{
    BoundaryValueGraph, CallSignature, CallbackRequirementId, CallingPolicy, NativeCallbackDemand,
    NativeParameterId, StaticMachineBinderId, ValidatedBoundaryEntryPlan,
};
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::boundary_values::{
    BoundaryTypeBinding, BoundaryValuePreparation, substitute_boundary_type_reference,
};
use checked_trees::typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use layout::BoundaryValueLayout;
use representation_planning::OpaqueRepresentationSelection;
use target::NativeTarget;

pub(crate) struct BoundarySignatureInstance<'a> {
    pub(crate) signature: &'a checked_trees::typed_trees::signature::StateSignature,
    pub(crate) requirement_identity: String,
    pub(crate) bindings: Vec<BoundaryTypeBinding>,
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

    /// Source-free public shape input for the backend's ABI classifier.
    pub(crate) fn value_graph(&self) -> BoundaryValueGraph<'_> {
        BoundaryValueGraph {
            shapes: &self.shapes,
            fields: &self.fields,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryNativeParameter {
    pub(crate) identity: NativeParameterId,
    pub(crate) native_ordinal: u32,
    pub(crate) shape: BoundaryNativeParameterShape,
    pub(crate) origin: BoundaryNativeParameterOrigin,
    pub(crate) layout_data_symbol: symbols::SymbolHandle,
}

pub use calling_conventions::BoundaryNativeParameterShape;

pub use language_semantics::callback_declarations::BoundaryDirectCallbackDeclaration as BoundaryDirectCallbackParameter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackBinder {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
    pub static_machine_ordinal: u32,
    pub parameter_symbol: symbols::SymbolHandle,
    pub requirement_trait: symbols::SymbolHandle,
    pub requirement_machine: symbols::SymbolHandle,
    pub requirement_identity: String,
    pub invokes_synchronously: bool,
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
    // Register capacity is not the source value's width or class. The old
    // syscall shortcut replaced every root with integer(8, 8), so an i32
    // formal no longer matched native transport and floats/large aggregates
    // bypassed the syscall evaluator's normal rejection. Classify every root
    // through the same checked graph; the ABI evaluator decides placement.
    Ok(CallSignature {
        parameters: materialized
            .parameters
            .iter()
            .skip(dispatch_only_parameter_count)
            .copied()
            .map(|root| materialized.value_graph().classify(root, policy))
            .collect::<Result<Vec<_>, _>>()?,
        result: materialized
            .result
            .map(|root| materialized.value_graph().classify(root, policy))
            .transpose()?,
    })
}

pub(crate) fn collect_boundary_signatures<'a>(
    typed: &'a TypedTrees,
    definition: &'a checked_trees::typed_trees::trait_definition::TraitDefinition,
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
                .map(|argument| substitute_boundary_type_reference(typed, *argument, &bindings))
                .collect::<Vec<_>>();
            collect_boundary_signatures(typed, parent, &parent_arguments, visited, signatures);
        }
    }
    for signature in typed.trait_machine_signatures(definition) {
        let requirement_identity =
            typed.trait_requirement_callable_identity_text(definition, signature);
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

pub(crate) fn trait_type_bindings(
    typed: &TypedTrees,
    definition: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    arguments: &[TypeReferenceHandle],
) -> Vec<BoundaryTypeBinding> {
    typed
        .trait_type_parameters(definition)
        .iter()
        .zip(arguments.iter().copied())
        .map(|(parameter, actual)| BoundaryTypeBinding {
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.as_str().to_owned(),
            actual,
        })
        .collect()
}

/// Materialize the source owner's already-prepared declaration. Type recipes
/// remain per-parameter work so a refused value stops before later values; the
/// declaration telescope itself is shared across target requests.
pub(crate) fn materialize_prepared_signature(
    services: checked_trees::typed_trees::service_declarations::ServiceCatalog<'_>,
    declaration: &checked_trees::typed_trees::service_declarations::signatures::BoundarySignatureDeclaration,
    bindings: &[BoundaryTypeBinding],
    layout_target: layout::LayoutTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<MaterializedBoundarySignature, String> {
    let prepared = declaration.projection.as_ref().map_err(Clone::clone)?;
    let typed = services.context().typed;
    let telescope = &prepared.telescope;
    let callback_binders = telescope
        .binders
        .iter()
        .zip(&prepared.callback_sources)
        .map(|(binder, source)| BoundaryCallbackBinder {
            binder: binder.binder,
            requirement: binder.requirement,
            static_machine_ordinal: binder.static_machine_ordinal,
            parameter_symbol: source.parameter_symbol,
            requirement_trait: source.requirement_trait,
            requirement_machine: source.requirement_machine,
            requirement_identity: binder.requirement_identity.clone(),
            invokes_synchronously: binder.invokes_synchronously,
        })
        .collect();
    let (representation_types, representation_uses) =
        crate::boundary_value_recipes::representation_inputs(opaque_representation_selections);
    let mut preparation = BoundaryValuePreparation::new(typed, bindings, &representation_types);
    let mut values = BoundaryValueLayout::new(layout_target, &representation_uses);
    let mut parameters = Vec::with_capacity(prepared.parameter_types.len());
    let mut native_parameters = Vec::with_capacity(prepared.parameter_types.len());
    for (formal_ordinal, (type_reference, formal)) in prepared
        .parameter_types
        .iter()
        .copied()
        .zip(&telescope.parameters)
        .enumerate()
    {
        let recipe = preparation.value_type(type_reference);
        let (_, root) = values.value_type(preparation.recipes(), recipe)?;
        parameters.push(root);
        let applied_type = substitute_boundary_type_reference(typed, type_reference, bindings);
        let layout_data_symbol = exact_boundary_layout_root_symbol(typed, applied_type);
        let formal_ordinal = u32::try_from(formal_ordinal)
            .map_err(|_| "boundary signature has too many runtime parameters")?;
        validate_fresh_native_parameter_report_identity(
            &native_parameters,
            formal.identity,
            formal.native_ordinal,
        )?;
        native_parameters.push(BoundaryNativeParameter {
            identity: formal.identity,
            native_ordinal: formal.native_ordinal,
            shape: BoundaryNativeParameterShape::Semantic(root),
            origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal },
            layout_data_symbol,
        });
    }
    let result = if prepared.return_type.is_valid() {
        let recipe = preparation.value_type(prepared.return_type);
        Some(values.value_type(preparation.recipes(), recipe)?.1)
    } else {
        None
    };
    let (shapes, fields, opaque_representations) = values.into_parts();
    Ok(MaterializedBoundarySignature {
        owner_requirement_identity: declaration.requirement_identity.clone(),
        native_target: layout_target.native_target(),
        opaque_representations,
        shapes,
        fields,
        parameters,
        callback_binders,
        callback_demands: Vec::new(),
        callback_layout_catalog: Vec::new(),
        native_parameters,
        direct_callback_parameters: telescope.direct_callbacks.clone(),
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
