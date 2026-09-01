//! Build-time evaluation of source-authored calling policies.
//!
//! The source vocabulary lives in `source/library/std/calling.omg`; this
//! module is the compiler-owned decoder that keeps that open policy surface
//! behind the closed normalized-plan validator.

mod opaque_representations;

pub use opaque_representations::{
    BoundaryOpaqueRepresentationMovement, BoundaryOpaqueRepresentationMovementRole,
    BoundaryOpaqueRepresentationPathElement, BoundaryOpaqueRepresentationUse,
};

use omega_calling_conventions::{
    BoundaryEntryPlan, BoundaryPlanResult, CallPlan, CallSignature, CallbackBinderRequirement,
    CallbackMaterialization, CallbackMaterializationContext, CallbackRequirementId, CallingPolicy,
    CallingPolicyRejection, EntryControl, EntryStack, IndirectPointerLocation, LayoutPlanId,
    LayoutSlotId, MachineRegime, MachineRegister, MachineState, MachineStateSet,
    NativeCallbackDemand, NativeParameterId, NativePlace, Preemption, RegisterSet, StatePlan,
    StaticMachineBinderId, SystemVEightbyteClass, ValidatedBoundaryEntryPlan, ValueClass,
    ValueLocation, ValuePlacement, ValueShape, callback_requirement_id,
    evaluate_ordinary_boundary_entry_plan, nominal_callback_native_parameter_id,
    validate_boundary_entry_plan_with_callback_materializations, validate_boundary_plan_result,
};
use omega_representation_planning::{OpaqueRepresentationSelection, selection_for_opaque};
use omega_target::NativeTarget;
use psi_build_time_evaluation::BuildTimeValue;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use sha2::{Digest, Sha256};

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
    class: BoundaryValueClass,
    byte_size: u16,
    alignment: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryValueField {
    shape: u16,
    byte_offset: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedBoundarySignature {
    owner_requirement_identity: String,
    native_target: NativeTarget,
    opaque_representations: Vec<BoundaryOpaqueRepresentationUse>,
    shapes: Vec<BoundaryValueShape>,
    fields: Vec<BoundaryValueField>,
    parameters: Vec<u16>,
    callback_binders: Vec<BoundaryCallbackBinder>,
    callback_demands: Vec<NativeCallbackDemand>,
    native_parameters: Vec<BoundaryNativeParameter>,
    direct_callback_parameters: Vec<BoundaryDirectCallbackParameter>,
    result: Option<u16>,
}

impl MaterializedBoundarySignature {
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

fn boundary_shape_path(
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
    identity: NativeParameterId,
    native_ordinal: u32,
    shape: BoundaryNativeParameterShape,
    origin: BoundaryNativeParameterOrigin,
    layout_data_symbol: psi_symbols::SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundaryNativeParameterShape {
    Semantic(u16),
    TargetFunctionPointer { byte_size: u16, alignment: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundaryNativeParameterOrigin {
    SemanticFormal {
        formal_ordinal: u32,
    },
    PrivateCallback {
        binder: StaticMachineBinderId,
        requirement: CallbackRequirementId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundaryDirectCallbackParameter {
    name: String,
    identity: NativeParameterId,
    native_ordinal: u32,
    binder: StaticMachineBinderId,
    requirement: CallbackRequirementId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryCallbackBinder {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
    pub static_machine_ordinal: u32,
    pub parameter_symbol: psi_symbols::SymbolHandle,
    pub requirement_trait: psi_symbols::SymbolHandle,
    pub requirement_machine: psi_symbols::SymbolHandle,
}

pub fn evaluate_compatibility_boundary_entry_plan(
    typed: &TypedTrees,
    native_target: NativeTarget,
    trait_name: &str,
    method_name: &str,
    requirement_identity: &str,
    policy: CallingPolicy,
    dispatch_only_parameter_count: usize,
) -> Result<Option<BoundaryEntryPlan>, String> {
    let trait_leaf = trait_name.rsplit("::").next().unwrap_or(trait_name);
    let trait_candidates = typed
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
    let top_level_candidates = typed
        .machines()
        .iter()
        .filter(|requirement| {
            requirement.supply_mode
                == psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                && requirement.name.as_str() == trait_name
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
                        .normalized_machine_overload_identity(requirement)
                        .map(|identity| identity.identity())
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
            &[],
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
            &[],
        )?
    };
    let classified =
        compatibility_call_signature(&materialized, policy, dispatch_only_parameter_count)?;
    evaluate_ordinary_boundary_entry_plan(policy, &classified)
        .map(|validated| Some(validated.plan().clone()))
        .map_err(|diagnostic| diagnostic.to_string())
}

/// Materialize the ABI of one explicit top-level boundary requirement.
///
/// Unlike a trait requirement, this declaration's `self` is the semantic
/// carrier operated on by the selected satisfier. Conformance checking maps it
/// to the satisfier's first explicit parameter, so it remains in the foreign
/// signature rather than being erased as a provider receiver.
fn call_signature_from_top_level_requirement(
    typed: &TypedTrees,
    requirement: &psi_typed_trees::machine::Machine,
    entry: &psi_typed_trees::state::State,
    owner_requirement_identity: &str,
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
) -> Result<MaterializedBoundarySignature, String> {
    if requirement.supply_mode != psi_language_semantics::MachineSupplyMode::TopLevelRequirement
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
            if attachment.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
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
        native_parameters,
        direct_callback_parameters: Vec::new(),
        result,
    })
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingPlanRealization {
    pub boundary_trait: psi_symbols::SymbolHandle,
    pub boundary_arguments: Vec<TypeReferenceHandle>,
    pub requirement_machine: psi_symbols::SymbolHandle,
    /// Compact compatibility/report coordinate for the complete target-closed
    /// application. Exact plan and materialized-signature custody below remain
    /// the authority for realization joins.
    pub report_fingerprint: u64,
    pub commitment: psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment,
    pub boundary_entry_plan: BoundaryEntryPlan,
    /// Crate-sealed exact evidence minted with the validated realization.
    /// The public fingerprint remains a compact compatibility/report
    /// coordinate and cannot authorize replacement of this plan.
    pub(crate) exact_boundary_entry_plan: BoundaryEntryPlan,
    pub callback_binders: Vec<BoundaryCallbackBinder>,
    pub callback_demands: Vec<NativeCallbackDemand>,
    pub callback_context_closed: bool,
    pub native_parameters: Vec<BoundaryNativeParameter>,
    pub materialized_signature: MaterializedBoundarySignature,
    pub policy_machine: String,
    pub relationship_span: psi_source::SourceSpan,
}

impl BoundaryCallingPlanRealization {
    pub const fn exact_boundary_entry_plan(&self) -> &BoundaryEntryPlan {
        &self.exact_boundary_entry_plan
    }

    pub fn replayed_validated_plan(
        &self,
    ) -> Result<ValidatedBoundaryEntryPlan, omega_calling_conventions::PlanDiagnostic> {
        let signature = CallSignature {
            parameters: self
                .boundary_entry_plan
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: self
                .boundary_entry_plan
                .call
                .result
                .as_ref()
                .map(|placement| placement.shape),
        };
        if self.callback_context_closed {
            let context = CallbackMaterializationContext {
                binders: self
                    .callback_binders
                    .iter()
                    .map(|binder| CallbackBinderRequirement {
                        binder: binder.binder,
                        requirement: binder.requirement,
                    })
                    .collect(),
                demands: self.callback_demands.clone(),
            };
            validate_boundary_entry_plan_with_callback_materializations(
                self.boundary_entry_plan.clone(),
                &signature,
                &context,
            )
        } else {
            omega_calling_conventions::validate_boundary_entry_plan(
                self.boundary_entry_plan.clone(),
                &signature,
            )
        }
    }

    pub fn replayed_validated_application(
        &self,
    ) -> Result<
        (
            ValidatedBoundaryEntryPlan,
            u64,
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment,
        ),
        omega_calling_conventions::PlanDiagnostic,
    > {
        if self
            .materialized_signature
            .opaque_representations
            .iter()
            .any(|use_| {
                !use_.opaque.is_valid()
                    || !use_.conformance.is_valid()
                    || !use_.carrier.is_valid()
                    || usize::from(use_.shape_root)
                        >= self.materialized_signature.shapes.len()
                    || use_.representation_schema_version
                    != omega_representation_planning::OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
                    || use_.selected_application_commitment
                        != use_.rederived_selected_application_commitment()
            })
        {
            return Err(omega_calling_conventions::PlanDiagnostic(
                "boundary calling plan retained stale opaque-representation application custody"
                    .to_owned(),
            ));
        }
        let validated = self.replayed_validated_plan()?;
        for representation in &self.materialized_signature.opaque_representations {
            self.materialized_signature
                .opaque_representation_movement(representation, &validated)
                .map_err(|reason| {
                    omega_calling_conventions::PlanDiagnostic(format!(
                        "boundary calling plan cannot rejoin opaque-representation movement: {reason}"
                    ))
                })?;
        }
        let (report_fingerprint, commitment) =
            boundary_plan_application_identity(&self.materialized_signature, &validated);
        Ok((
            validated,
            report_fingerprint,
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(commitment),
        ))
    }
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

fn exact_callback_requirement_catalog(
    typed: &TypedTrees,
    binders: &[BoundaryCallbackBinder],
) -> Result<Vec<(CallbackRequirementId, String)>, String> {
    let mut catalog = Vec::with_capacity(binders.len());
    for binder in binders {
        let trait_definition = typed
            .traits()
            .iter()
            .find(|candidate| candidate.symbol == binder.requirement_trait)
            .ok_or_else(|| {
                "evaluated callback binder lost its exact requirement trait".to_owned()
            })?;
        let requirement = typed
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|candidate| candidate.symbol == binder.requirement_machine)
            .ok_or_else(|| {
                "evaluated callback binder lost its exact requirement machine".to_owned()
            })?;
        let exact = typed
            .normalized_trait_requirement_overload_identity(trait_definition, requirement)
            .identity();
        let report = callback_requirement_id(&exact);
        if report != binder.requirement {
            return Err(
                "evaluated callback binder requirement report drifted from its exact requirement"
                    .to_owned(),
            );
        }
        validate_callback_requirement_report_collision(&catalog, report, &exact)?;
        catalog.push((report, exact));
    }
    Ok(catalog)
}

fn validate_callback_requirement_report_collision(
    exact_catalog: &[(CallbackRequirementId, String)],
    report: CallbackRequirementId,
    exact: &str,
) -> Result<(), String> {
    if exact_catalog
        .iter()
        .any(|(prior_report, prior_exact)| *prior_report == report && prior_exact != exact)
    {
        return Err(
            "distinct exact callback requirements collide on one compact report identity"
                .to_owned(),
        );
    }
    Ok(())
}

/// Bind checked nominal callback authority back to the one target-owned plan
/// realization that produced its retained fingerprint. This runs before any
/// backend lowering so a missing, duplicated, or changed placement recipe
/// cannot silently fall back to a convention oracle.
pub fn validate_nominal_callback_placement_bindings(
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
        let (validated, realized_report_fingerprint, realized_commitment) = match realization
            .replayed_validated_application()
        {
            Ok(application) => application,
            Err(error) => {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` retained an invalid target calling-plan realization: {error}",
                    nominal_use.canonical_requirement_overload
                )));
                continue;
            }
        };
        if realization.report_fingerprint != realized_report_fingerprint
            || realization.commitment != realized_commitment
            || realization.exact_boundary_entry_plan() != validated.plan()
            || placement.boundary_calling_plan_report_fingerprint != realized_report_fingerprint
            || placement.boundary_calling_plan_commitment != realized_commitment
        {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` does not bind its exact evaluated target calling plan",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        }
        let Some(resource_envelope) = checked
            .facts
            .contract_plans
            .resource_envelope(nominal_use.selected_machine, nominal_use.selected_entry)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` is missing its exact checked entry resource envelope",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        };
        let Some(actual_contract) = checked
            .facts
            .contract_plans
            .for_machine(nominal_use.selected_machine)
        else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` is missing its canonical selected contract",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        };
        let Some(published_contract) = checked.facts.contract_plans.crash_capsule(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` is missing its canonical published requirement contract",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        };
        if nominal_use
            .published_requirement_envelope
            .contract_report_fingerprint
            != published_contract.target_contract_report_fingerprint()
            || nominal_use
                .published_requirement_envelope
                .contract_commitment
                != published_contract.target_contract_commitment()
        {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` does not bind its exact published requirement contract",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        }
        if placement.resource_receipt.contract_report_fingerprint()
            != nominal_use
                .selected_actual_envelope
                .contract_report_fingerprint
            || placement.resource_receipt.contract_commitment()
                != nominal_use.selected_actual_envelope.contract_commitment
            || nominal_use.selected_actual_envelope.contract_commitment
                != actual_contract.commitment
            || placement
                .resource_receipt
                .validate_against(resource_envelope)
                .is_err()
        {
            diagnostics.push(Diagnostic::error(format!(
                "nominal callback use for `{}` does not bind its exact checked entry resource receipt",
                nominal_use.canonical_requirement_overload
            )));
            continue;
        }
        let private_materialization = match bound_private_callback_materialization(
            nominal_use,
            realizations,
        ) {
            Ok(materialization) => materialization,
            Err(reason) => {
                diagnostics.push(Diagnostic::error(format!(
                    "nominal callback use for `{}` could not retain its exact private materialization: {reason}",
                    nominal_use.canonical_requirement_overload
                )));
                continue;
            }
        };
        bound.push(omega_backend_plan::BoundNominalCallbackPlacement {
            site: nominal_use.site,
            registration_operation: nominal_use.registration_operation,
            static_machine_ordinal: nominal_use.static_machine_ordinal,
            selected_machine: nominal_use.selected_machine,
            selected_entry: nominal_use.selected_entry,
            satisfaction_trait: nominal_use.satisfaction_trait,
            satisfaction_requirement: nominal_use.satisfaction_requirement,
            canonical_requirement_overload: nominal_use.canonical_requirement_overload.clone(),
            boundary_calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
            resource_receipt: placement.resource_receipt,
            boundary_entry_plan: validated.plan().clone(),
            private_materialization,
        });
    }

    if diagnostics.is_empty() {
        Ok(bound)
    } else {
        Err(diagnostics)
    }
}

fn bound_private_callback_materialization(
    nominal_use: &psi_checked_trees::CheckedNominalMachineUse,
    realizations: &[BoundaryCallingPlanRealization],
) -> Result<Option<omega_backend_plan::BoundCallbackPrivateMaterialization>, String> {
    let matching_registrars = realizations
        .iter()
        .filter(|realization| {
            realization.requirement_machine == nominal_use.registration_operation
                && realization.callback_binders.iter().any(|binder| {
                    binder.static_machine_ordinal == nominal_use.static_machine_ordinal
                        && binder.requirement_trait == nominal_use.satisfaction_trait
                        && binder.requirement_machine == nominal_use.satisfaction_requirement
                })
        })
        .collect::<Vec<_>>();
    let [registrar] = matching_registrars.as_slice() else {
        return Err(format!(
            "registration operation and exact satisfaction row resolve to {} outbound registrar realizations; exactly one is required",
            matching_registrars.len()
        ));
    };
    validate_retained_callback_binders(registrar)?;

    let matching_binders = registrar
        .callback_binders
        .iter()
        .filter(|binder| {
            binder.static_machine_ordinal == nominal_use.static_machine_ordinal
                && binder.requirement_trait == nominal_use.satisfaction_trait
                && binder.requirement_machine == nominal_use.satisfaction_requirement
        })
        .collect::<Vec<_>>();
    let [binder] = matching_binders.as_slice() else {
        return Err(format!(
            "static-machine ordinal {} and exact satisfaction row resolve to {} binder identities in the selected registrar realization; exactly one is required",
            nominal_use.static_machine_ordinal,
            matching_binders.len()
        ));
    };
    if !registrar.callback_context_closed {
        if !registrar
            .boundary_entry_plan
            .call
            .callback_materializations
            .is_empty()
        {
            return Err(
                "an unclosed target context retained private callback materialization rows"
                    .to_owned(),
            );
        }
        let (validated, report_fingerprint, commitment) = registrar
            .replayed_validated_application()
            .map_err(|error| error.to_string())?;
        if validated.plan() != &registrar.boundary_entry_plan
            || validated.plan() != registrar.exact_boundary_entry_plan()
            || report_fingerprint != registrar.report_fingerprint
            || commitment != registrar.commitment
        {
            return Err(
                "the exact outbound registrar realization drifted from its retained fingerprint"
                    .to_owned(),
            );
        }
        return Ok(None);
    }

    let context = CallbackMaterializationContext {
        binders: registrar
            .callback_binders
            .iter()
            .map(|binder| CallbackBinderRequirement {
                binder: binder.binder,
                requirement: binder.requirement,
            })
            .collect(),
        demands: registrar.callback_demands.clone(),
    };
    let (validated, application_report_fingerprint, commitment) = registrar
        .replayed_validated_application()
        .map_err(|error| error.to_string())?;
    if validated.plan() != &registrar.boundary_entry_plan
        || validated.plan() != registrar.exact_boundary_entry_plan()
        || application_report_fingerprint != registrar.report_fingerprint
        || commitment != registrar.commitment
    {
        return Err(
            "the exact outbound registrar realization drifted from its retained fingerprint"
                .to_owned(),
        );
    }
    let matching_rows = registrar
        .boundary_entry_plan
        .call
        .callback_materializations
        .iter()
        .filter(|row| row.binder == binder.binder)
        .collect::<Vec<_>>();
    let [row] = matching_rows.as_slice() else {
        return Err(format!(
            "exact binder identity resolves to {} private materialization rows; exactly one is required",
            matching_rows.len()
        ));
    };
    let matching_demands = registrar
        .callback_demands
        .iter()
        .filter(|demand| {
            demand.destination == row.destination && demand.requirement == binder.requirement
        })
        .collect::<Vec<_>>();
    let [demand] = matching_demands.as_slice() else {
        return Err(format!(
            "exact binder destination and callback requirement resolve to {} private demands; exactly one is required",
            matching_demands.len()
        ));
    };
    let direct_registrar_parameter_application = match row.destination {
        NativePlace::Parameter(parameter) => {
            let matching_parameters = registrar
                .native_parameters
                .iter()
                .filter(|candidate| candidate.identity == parameter)
                .collect::<Vec<_>>();
            let [native_parameter] = matching_parameters.as_slice() else {
                return Err(format!(
                    "direct callback destination resolves to {} target-closed native parameter rows; exactly one is required",
                    matching_parameters.len(),
                ));
            };
            if native_parameter.origin
                != (BoundaryNativeParameterOrigin::PrivateCallback {
                    binder: binder.binder,
                    requirement: binder.requirement,
                })
            {
                return Err(
                    "direct callback native parameter is not owned by its exact binder and requirement"
                        .to_owned(),
                );
            }
            let BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            } = native_parameter.shape
            else {
                return Err(
                    "direct callback native parameter does not retain a target function-pointer shape"
                        .to_owned(),
                );
            };
            let shape = ValueShape::integer(byte_size, alignment);
            let placement = usize::try_from(native_parameter.native_ordinal)
                .ok()
                .and_then(|ordinal| validated.plan().call.parameters.get(ordinal))
                .ok_or_else(|| {
                    "direct callback native parameter ordinal is absent from the exact registrar plan"
                        .to_owned()
                })?;
            if placement.shape != shape {
                return Err(
                    "direct callback native parameter shape drifted from its exact registrar placement"
                        .to_owned(),
                );
            }
            Some(omega_calling_conventions::NativeParameterApplication {
                parameter,
                native_ordinal: native_parameter.native_ordinal,
                shape,
                placement: placement.clone(),
            })
        }
        NativePlace::Field { .. } => None,
    };
    let application_commitment = commitment.as_bytes();
    Ok(Some(
        omega_backend_plan::BoundCallbackPrivateMaterialization {
            binder: binder.binder,
            destination: row.destination.clone(),
            requirement: demand.requirement,
            registrar_boundary_entry_plan: validated.plan().clone(),
            registrar_calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
            registrar_application_report_fingerprint: application_report_fingerprint,
            registrar_application_commitment: application_commitment,
            direct_registrar_parameter_application,
            context,
        },
    ))
}

/// Discover concrete `Calling<C>` relationships, evaluate `C::plan` once for
/// every method in the boundary service surface, and retain only canonical
/// evaluated identities on the typed program.
pub fn compute_boundary_calling_plans(
    typed: &mut TypedTrees,
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&omega_package_compilation::PackageCompilationInputs>,
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
        let (application_report_fingerprint, application_commitment) =
            boundary_plan_application_identity(&signature, &validated);
        evaluated.push(BoundaryCallingPlanRealization {
            boundary_trait,
            boundary_arguments,
            requirement_machine,
            report_fingerprint: application_report_fingerprint,
            commitment: psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
                application_commitment,
            ),
            boundary_entry_plan: validated.plan().clone(),
            exact_boundary_entry_plan: validated.plan().clone(),
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
                report_fingerprint: realization.report_fingerprint,
                commitment: realization.commitment,
            },
        );
    }
    Ok(evaluated)
}

/// Re-evaluate outbound registrar policies only after the checked native
/// layout pipeline has published its authoritative private-demand catalog.
/// The target-neutral typed reports are deliberately not consulted here.
pub fn close_outbound_callback_materializations(
    checked: &mut psi_checked_trees::CheckedTrees,
    realizations: &mut [BoundaryCallingPlanRealization],
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    package_inputs: Option<&omega_package_compilation::PackageCompilationInputs>,
) -> Result<(), Vec<Diagnostic>> {
    let layout_plan =
        omega_layout::build_layout_plan(checked, native_target, opaque_representation_selections)
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
        let mut signature = realization.materialized_signature.clone();
        let mut demands =
            close_direct_callback_parameters(&mut signature, native_target).map_err(|reason| {
                vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
            })?;
        let binder_requirement_catalog =
            exact_callback_requirement_catalog(&checked.typed, &realization.callback_binders)
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                })?;
        let mut demand_requirement_catalog = Vec::new();
        for parameter in &signature.native_parameters {
            for demand in layout_plan
                .private_callback_demands
                .iter()
                .filter(|demand| demand.data_symbol == parameter.layout_data_symbol)
            {
                validate_callback_requirement_report_collision(
                    &binder_requirement_catalog,
                    demand.requirement,
                    &demand.callback_requirement_identity,
                )
                .and_then(|()| {
                    validate_callback_requirement_report_collision(
                        &demand_requirement_catalog,
                        demand.requirement,
                        &demand.callback_requirement_identity,
                    )
                })
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                })?;
                demand_requirement_catalog.push((
                    demand.requirement,
                    demand.callback_requirement_identity.to_string(),
                ));
                demands.push(demand.native_demand(parameter.identity));
            }
            for path in layout_plan
                .two_hop_private_callback_paths
                .iter()
                .filter(|path| path.root_layout.data_symbol == parameter.layout_data_symbol)
            {
                validate_callback_requirement_report_collision(
                    &binder_requirement_catalog,
                    path.terminal_demand.requirement,
                    &path.terminal_demand.callback_requirement_identity,
                )
                .and_then(|()| {
                    validate_callback_requirement_report_collision(
                        &demand_requirement_catalog,
                        path.terminal_demand.requirement,
                        &path.terminal_demand.callback_requirement_identity,
                    )
                })
                .map_err(|reason| {
                    vec![Diagnostic::error(reason).with_source_span(realization.relationship_span)]
                })?;
                demand_requirement_catalog.push((
                    path.terminal_demand.requirement,
                    path.terminal_demand
                        .callback_requirement_identity
                        .to_string(),
                ));
                demands.push(path.native_demand(parameter.identity));
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

        let old_report_fingerprint = realization.report_fingerprint;
        let old_commitment = realization.commitment;
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
        let (new_fingerprint, new_commitment) =
            boundary_plan_application_identity(&signature, &validated);
        realization.report_fingerprint = new_fingerprint;
        realization.commitment =
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
                new_commitment,
            );
        realization.boundary_entry_plan = validated.plan().clone();
        realization.exact_boundary_entry_plan = validated.plan().clone();
        realization.callback_demands = demands;
        realization.native_parameters = signature.native_parameters.clone();
        realization.callback_context_closed = true;
        realization.materialized_signature = signature;

        checked.typed.record_boundary_calling_plan(
            psi_typed_trees::typed_trees::BoundaryCallingPlanIdentity {
                boundary_trait: realization.boundary_trait,
                boundary_arguments: realization.boundary_arguments.clone(),
                requirement_machine: realization.requirement_machine,
                report_fingerprint: new_fingerprint,
                commitment: realization.commitment,
            },
        );
        reconcile_closed_callback_plan_identity(
            &mut checked.facts.nominal_machine_uses.uses,
            realization.boundary_trait,
            realization.requirement_machine,
            old_report_fingerprint,
            old_commitment,
            new_fingerprint,
            realization.commitment,
        );
    }
    Ok(())
}

fn close_direct_callback_parameters(
    signature: &mut MaterializedBoundarySignature,
    target: NativeTarget,
) -> Result<Vec<NativeCallbackDemand>, String> {
    signature.native_parameters.retain(|parameter| {
        matches!(
            parameter.origin,
            BoundaryNativeParameterOrigin::SemanticFormal { .. }
        )
    });
    let byte_size = u16::try_from(target.pointer_size)
        .map_err(|_| "target function-pointer size exceeds boundary shape range".to_owned())?;
    let alignment = u16::try_from(target.pointer_alignment)
        .map_err(|_| "target function-pointer alignment exceeds boundary shape range".to_owned())?;
    if byte_size == 0 || alignment == 0 || !alignment.is_power_of_two() {
        return Err(format!(
            "target function-pointer geometry {}/{} is invalid for a direct callback parameter",
            target.pointer_size, target.pointer_alignment,
        ));
    }

    let mut demands = Vec::with_capacity(signature.direct_callback_parameters.len());
    for direct in &signature.direct_callback_parameters {
        validate_fresh_native_parameter_report_identity(
            &signature.native_parameters,
            direct.identity,
            direct.native_ordinal,
        )?;
        signature.native_parameters.push(BoundaryNativeParameter {
            identity: direct.identity,
            native_ordinal: direct.native_ordinal,
            shape: BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            },
            origin: BoundaryNativeParameterOrigin::PrivateCallback {
                binder: direct.binder,
                requirement: direct.requirement,
            },
            layout_data_symbol: psi_symbols::SymbolHandle::invalid(),
        });
        demands.push(NativeCallbackDemand {
            destination: NativePlace::Parameter(direct.identity),
            requirement: direct.requirement,
        });
    }
    signature
        .native_parameters
        .sort_unstable_by_key(|parameter| parameter.native_ordinal);
    for (expected, parameter) in signature.native_parameters.iter().enumerate() {
        if usize::try_from(parameter.native_ordinal).ok() != Some(expected) {
            return Err(format!(
                "native parameter telescope has position {} where contiguous position {expected} is required",
                parameter.native_ordinal,
            ));
        }
        if matches!(
            parameter.shape,
            BoundaryNativeParameterShape::TargetFunctionPointer { .. }
        ) && (byte_size != 8 || alignment != 8)
        {
            // The current source-level closed vocabulary has only admitted
            // 64-bit native targets. Keep unsupported future pointer geometry
            // fail-closed until its scalar ABI carrier is present.
            return Err(format!(
                "direct callback parameters currently require an admitted 8/8 target function-pointer shape, got {byte_size}/{alignment}"
            ));
        }
    }
    Ok(demands)
}

fn reconcile_closed_callback_plan_identity(
    nominal_uses: &mut [psi_checked_trees::CheckedNominalMachineUse],
    boundary_trait: psi_symbols::SymbolHandle,
    requirement_machine: psi_symbols::SymbolHandle,
    old_report_fingerprint: u64,
    old_commitment: psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment,
    new_report_fingerprint: u64,
    new_commitment: psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment,
) {
    for nominal_use in nominal_uses {
        if nominal_use.satisfaction_trait == boundary_trait
            && nominal_use.satisfaction_requirement == requirement_machine
            && nominal_use.callback_placement.is_some_and(|placement| {
                placement.boundary_calling_plan_report_fingerprint == old_report_fingerprint
                    && placement.boundary_calling_plan_commitment == old_commitment
            })
        {
            let resource_receipt = nominal_use
                .callback_placement
                .expect("matched callback placement")
                .resource_receipt;
            nominal_use.callback_placement =
                Some(psi_checked_trees::CheckedCallbackPlacementIdentity {
                    boundary_calling_plan_report_fingerprint: new_report_fingerprint,
                    boundary_calling_plan_commitment: new_commitment,
                    resource_receipt,
                });
        }
    }
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
        native_parameters,
        direct_callback_parameters,
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

fn validate_fresh_native_parameter_report_identity(
    prior: &[BoundaryNativeParameter],
    identity: NativeParameterId,
    ordinal: u32,
) -> Result<(), String> {
    if prior.iter().any(|parameter| parameter.identity == identity) {
        return Err(format!(
            "boundary runtime parameter {ordinal} collides with an earlier exact parameter on one compact native-parameter report identity"
        ));
    }
    Ok(())
}

/// Compact callback-binder catalog discriminator. The exact binder ordinal,
/// parameter symbol, and requirement symbols remain beside it and collisions
/// reject before a calling policy is evaluated.
fn callback_plan_report_fingerprint(domain: &[u8], parts: &[&[u8]]) -> u64 {
    let mut report_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for bytes in std::iter::once(domain).chain(parts.iter().copied()) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            report_fingerprint ^= u64::from(byte);
            report_fingerprint = report_fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    if report_fingerprint == 0 {
        1
    } else {
        report_fingerprint
    }
}

fn value_shape_from_type(
    typed: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
    shapes: &mut Vec<BoundaryValueShape>,
    fields: &mut Vec<BoundaryValueField>,
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    opaque_representations: &mut Vec<BoundaryOpaqueRepresentationUse>,
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
        TypeReferenceNode::Constrained { base_type, .. } => value_shape_from_type(
            typed,
            *base_type,
            bindings,
            visiting,
            shapes,
            fields,
            native_target,
            opaque_representation_selections,
            opaque_representations,
        ),
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
            let pointer_size = u16::try_from(native_target.pointer_size)
                .map_err(|_| "target pointer size exceeds boundary shape range".to_owned())?;
            let pointer_alignment = u16::try_from(native_target.pointer_alignment)
                .map_err(|_| "target pointer alignment exceeds boundary shape range".to_owned())?;
            let byte_size = if is_fat {
                pointer_size
                    .checked_mul(2)
                    .ok_or_else(|| "fat reference boundary shape overflows".to_owned())?
            } else {
                pointer_size
            };
            let abi = ValueShape::integer(byte_size, pointer_alignment);
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
            let byte_size = u16::try_from(native_target.pointer_size)
                .ok()
                .and_then(|size| size.checked_mul(2))
                .ok_or_else(|| {
                    "target slice-reference size exceeds boundary shape range".to_owned()
                })?;
            let alignment = u16::try_from(native_target.pointer_alignment)
                .map_err(|_| "target pointer alignment exceeds boundary shape range".to_owned())?;
            let abi = ValueShape::integer(byte_size, alignment);
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
            let (element, element_root) = value_shape_from_type(
                typed,
                *element_type,
                bindings,
                visiting,
                shapes,
                fields,
                native_target,
                opaque_representation_selections,
                opaque_representations,
            )?;
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
        TypeReferenceNode::Named { symbol, name } => {
            let definition = typed
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol);
            if definition.is_some_and(|definition| {
                definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque
            }) {
                opaque_representation_value_shape(
                    typed,
                    *symbol,
                    name.as_str(),
                    bindings,
                    visiting,
                    shapes,
                    fields,
                    native_target,
                    opaque_representation_selections,
                    opaque_representations,
                )
            } else {
                plain_data_value_shape(
                    typed,
                    *symbol,
                    name.as_str(),
                    bindings,
                    visiting,
                    shapes,
                    fields,
                    native_target,
                    opaque_representation_selections,
                    opaque_representations,
                )
            }
        }
        TypeReferenceNode::DynamicTrait { .. } => {
            let byte_size = u16::try_from(native_target.pointer_size)
                .ok()
                .and_then(|size| size.checked_mul(2))
                .ok_or_else(|| {
                    "target dynamic-trait reference size exceeds boundary shape range".to_owned()
                })?;
            let alignment = u16::try_from(native_target.pointer_alignment)
                .map_err(|_| "target pointer alignment exceeds boundary shape range".to_owned())?;
            let abi = ValueShape::integer(byte_size, alignment);
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

fn opaque_representation_value_shape(
    typed: &TypedTrees,
    opaque: psi_symbols::SymbolHandle,
    opaque_name: &str,
    bindings: &[TraitTypeBinding],
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
    shapes: &mut Vec<BoundaryValueShape>,
    fields: &mut Vec<BoundaryValueField>,
    native_target: NativeTarget,
    selections: &[OpaqueRepresentationSelection],
    uses: &mut Vec<BoundaryOpaqueRepresentationUse>,
) -> Result<(ValueShape, u16), String> {
    if visiting.contains(&opaque) {
        return Err(format!(
            "opaque representation for `{opaque_name}` recursively contains its own semantic declaration"
        ));
    }
    let selection = selection_for_opaque(selections, opaque).ok_or_else(|| {
        format!(
            "boundary-opaque data `{opaque_name}` crosses this boundary by value, but the authoritative build selects no exact `OpaqueRepresentation<{opaque_name}>` conformance"
        )
    })?;
    let application = selection.application();
    let carrier = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == selection.carrier())
        .ok_or_else(|| {
            format!(
                "selected opaque representation carrier for `{opaque_name}` disappeared before shape closure"
            )
        })?;
    visiting.push(opaque);
    let shape = plain_data_value_shape(
        typed,
        carrier.symbol,
        carrier.name.as_str(),
        bindings,
        visiting,
        shapes,
        fields,
        native_target,
        selections,
        uses,
    );
    visiting.pop();
    let (abi, shape_root) = shape.map_err(|reason| {
        format!(
            "selected carrier `{}` cannot represent boundary-opaque `{opaque_name}` by value: {reason}",
            carrier.name,
        )
    })?;
    let use_identity = BoundaryOpaqueRepresentationUse {
        opaque,
        conformance: application.declaration,
        carrier: selection.carrier(),
        shape_root,
        application_report_fingerprint: application.report_fingerprint,
        conformance_application_commitment: application.commitment.as_bytes(),
        representation_schema_version: selection.schema_version(),
        origin: selection.origin(),
        lifecycle: selection.lifecycle(),
        copy_disposition: selection.copy_disposition(),
        selected_application_commitment: selection.selected_application_commitment(),
    };
    if !uses.contains(&use_identity) {
        uses.push(use_identity);
    }
    Ok((abi, shape_root))
}

/// Derive the exact address-free `Extent` graph for the currently admitted
/// UEFI/Microsoft program-storage source schema. A future SysV/AAPCS source
/// schema must retain and classify its own structural graph rather than
/// passing this fence.
pub fn selected_program_storage_source_extent_value_layout(
    typed: &TypedTrees,
    slot: omega_target::ProgramEntrySlotDeclaration,
    type_reference: TypeReferenceHandle,
) -> Result<omega_program_entry_plan::ProgramEntrySourceExtentValueLayout, String> {
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
    let mut opaque_representations = Vec::new();
    let (shape, root) = value_shape_from_type(
        typed,
        type_reference,
        &[],
        &mut Vec::new(),
        &mut shapes,
        &mut fields,
        slot.owner.native_target(),
        &[],
        &mut opaque_representations,
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
    omega_program_entry_plan::ProgramEntrySourceExtentValueLayout::from_checked_record(
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
    native_target: NativeTarget,
    opaque_representation_selections: &[OpaqueRepresentationSelection],
    opaque_representations: &mut Vec<BoundaryOpaqueRepresentationUse>,
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
                native_target,
                opaque_representation_selections,
                opaque_representations,
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

pub fn materialized_boundary_signature_from_abi(
    signature: &CallSignature,
) -> Result<MaterializedBoundarySignature, String> {
    let mut shapes = Vec::new();
    let mut parameters = Vec::new();
    let mut native_parameters = Vec::new();
    for (ordinal, shape) in signature.parameters.iter().enumerate() {
        let root = push_boundary_shape(
            &mut shapes,
            BoundaryValueShape {
                class: match shape.class {
                    ValueClass::Float => BoundaryValueClass::Float,
                    _ => BoundaryValueClass::Integer,
                },
                byte_size: shape.byte_size,
                alignment: shape.alignment,
            },
        )?;
        parameters.push(root);
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| "synthetic ABI signature has too many parameters")?;
        native_parameters.push(BoundaryNativeParameter {
            identity: nominal_callback_native_parameter_id(
                "omega.synthetic-abi-signature",
                &format!("parameter-{ordinal}"),
            ),
            native_ordinal: ordinal,
            shape: BoundaryNativeParameterShape::Semantic(root),
            origin: BoundaryNativeParameterOrigin::SemanticFormal {
                formal_ordinal: ordinal,
            },
            layout_data_symbol: psi_symbols::SymbolHandle::invalid(),
        });
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
        owner_requirement_identity: "omega.synthetic-abi-signature".to_owned(),
        native_target: NativeTarget::host(),
        opaque_representations: Vec::new(),
        shapes,
        fields: Vec::new(),
        parameters,
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        native_parameters,
        direct_callback_parameters: Vec::new(),
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

fn validate_materialized_boundary_plan_result(
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
            BoundaryNativeParameterShape::Semantic(root) => {
                validate_authored_abi_shape(signature, root, placement.shape, plan.call.policy)
            }
            BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            } => {
                let expected = ValueShape::integer(byte_size, alignment);
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

fn boundary_plan_application_identity(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> (u64, [u8; 32]) {
    let mut strong = Sha256::new();
    strong.update(b"omega.boundary-plan-application.v4");
    strong.update((signature.owner_requirement_identity.len() as u64).to_le_bytes());
    strong.update(signature.owner_requirement_identity.as_bytes());
    strong.update([match signature.native_target.architecture {
        omega_target::Architecture::Aarch64 => 1,
        omega_target::Architecture::X86_64 => 2,
    }]);
    strong.update([match signature.native_target.object_format {
        omega_target::ObjectFormat::Elf => 1,
        omega_target::ObjectFormat::MachO => 2,
        omega_target::ObjectFormat::Coff => 3,
    }]);
    strong.update((signature.native_target.pointer_size as u64).to_le_bytes());
    strong.update((signature.native_target.pointer_alignment as u64).to_le_bytes());
    strong.update((signature.opaque_representations.len() as u64).to_le_bytes());
    for representation in &signature.opaque_representations {
        strong.update(representation.shape_root.to_le_bytes());
        strong.update(representation.application_report_fingerprint.to_le_bytes());
        strong.update(representation.conformance_application_commitment);
        strong.update(representation.representation_schema_version.to_le_bytes());
        strong.update([match representation.origin {
            omega_representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance => 1,
        }]);
        strong.update([match representation.lifecycle {
            omega_representation_planning::OpaqueRepresentationLifecycleDisposition::Inert => 1,
        }]);
        strong.update([match representation.copy_disposition {
            omega_representation_planning::OpaqueRepresentationCopyDisposition::PlacementOnly => 1,
            omega_representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => 2,
        }]);
        strong.update(representation.selected_application_commitment);
    }
    strong.update((signature.shapes.len() as u64).to_le_bytes());
    for shape in &signature.shapes {
        match shape.class {
            BoundaryValueClass::Integer => strong.update([1]),
            BoundaryValueClass::Float => strong.update([2]),
            BoundaryValueClass::Reference => strong.update([3]),
            BoundaryValueClass::FixedArray { element, length } => {
                strong.update([4]);
                strong.update(element.to_le_bytes());
                strong.update(length.to_le_bytes());
            }
            BoundaryValueClass::Record {
                first_field,
                field_count,
            } => {
                strong.update([5]);
                strong.update(first_field.to_le_bytes());
                strong.update(field_count.to_le_bytes());
            }
        }
        strong.update(shape.byte_size.to_le_bytes());
        strong.update(shape.alignment.to_le_bytes());
    }
    strong.update((signature.fields.len() as u64).to_le_bytes());
    for field in &signature.fields {
        strong.update(field.shape.to_le_bytes());
        strong.update(field.byte_offset.to_le_bytes());
    }
    strong.update((signature.parameters.len() as u64).to_le_bytes());
    for parameter in &signature.parameters {
        strong.update(parameter.to_le_bytes());
    }
    match signature.result {
        Some(result) => {
            strong.update([1]);
            strong.update(result.to_le_bytes());
        }
        None => strong.update([0]),
    }
    strong.update((signature.native_parameters.len() as u64).to_le_bytes());
    for parameter in &signature.native_parameters {
        strong.update(parameter.identity.get().to_le_bytes());
        strong.update(parameter.native_ordinal.to_le_bytes());
        match parameter.origin {
            BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } => {
                strong.update([1]);
                strong.update(formal_ordinal.to_le_bytes());
            }
            BoundaryNativeParameterOrigin::PrivateCallback {
                binder,
                requirement,
            } => {
                strong.update([2]);
                strong.update(binder.get().to_le_bytes());
                strong.update(requirement.get().to_le_bytes());
            }
        }
        match parameter.shape {
            BoundaryNativeParameterShape::Semantic(root) => {
                strong.update([1]);
                strong.update(root.to_le_bytes());
            }
            BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            } => {
                strong.update([2]);
                strong.update(byte_size.to_le_bytes());
                strong.update(alignment.to_le_bytes());
            }
        }
    }
    strong.update(validated.contract_commitment_digest());
    let commitment: [u8; 32] = strong.finalize().into();
    let report = callback_plan_report_fingerprint(
        b"omega.boundary-plan-application-report.v3",
        &[&commitment],
    );
    (report, commitment)
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

    fn test_opaque_representation_use(shape_root: u16) -> BoundaryOpaqueRepresentationUse {
        let conformance_application_commitment = [0x41; 32];
        let lifecycle =
            omega_representation_planning::OpaqueRepresentationLifecycleDisposition::Inert;
        let copy_disposition =
            omega_representation_planning::OpaqueRepresentationCopyDisposition::PlacementOnly;
        let origin =
            omega_representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance;
        BoundaryOpaqueRepresentationUse {
            opaque: psi_symbols::SymbolHandle::from_arena_index(1),
            conformance: psi_symbols::SymbolHandle::from_arena_index(2),
            carrier: psi_symbols::SymbolHandle::from_arena_index(3),
            shape_root,
            application_report_fingerprint: 17,
            conformance_application_commitment,
            representation_schema_version:
                omega_representation_planning::OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
            origin,
            lifecycle,
            copy_disposition,
            selected_application_commitment:
                omega_representation_planning::selected_application_commitment(
                    conformance_application_commitment,
                    lifecycle,
                    copy_disposition,
                    origin,
                ),
        }
    }

    #[test]
    fn opaque_shape_path_rejects_one_marker_reused_by_multiple_record_fields() {
        let shapes = vec![
            BoundaryValueShape {
                class: BoundaryValueClass::Integer,
                byte_size: 8,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::Record {
                    first_field: 0,
                    field_count: 2,
                },
                byte_size: 16,
                alignment: 8,
            },
        ];
        let fields = vec![
            BoundaryValueField {
                shape: 0,
                byte_offset: 0,
            },
            BoundaryValueField {
                shape: 0,
                byte_offset: 8,
            },
        ];

        let error = boundary_shape_path(&shapes, &fields, 1, 0)
            .expect_err("one opaque shape marker cannot name two record occurrences");
        assert!(error.contains("multiple record fields"), "{error}");
    }

    #[test]
    fn opaque_shape_path_retains_fixed_array_and_record_coordinates() {
        let shapes = vec![
            BoundaryValueShape {
                class: BoundaryValueClass::Integer,
                byte_size: 8,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::Record {
                    first_field: 0,
                    field_count: 1,
                },
                byte_size: 8,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::FixedArray {
                    element: 1,
                    length: 4,
                },
                byte_size: 32,
                alignment: 8,
            },
        ];
        let fields = vec![BoundaryValueField {
            shape: 0,
            byte_offset: 0,
        }];

        assert_eq!(
            boundary_shape_path(&shapes, &fields, 2, 0)
                .expect("closed postorder shape graph")
                .expect("target shape is reachable"),
            vec![
                BoundaryOpaqueRepresentationPathElement::FixedArrayElement,
                BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 0 },
            ]
        );
    }

    #[test]
    fn boundary_plan_application_commitment_binds_opaque_shape_root() {
        let shape = ValueShape::integer(8, 8);
        let call_signature = CallSignature {
            parameters: vec![shape, shape],
            result: None,
        };
        let validated = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(NativeTarget::host()),
            &call_signature,
        )
        .expect("ordinary two-parameter plan");
        let mut signature = materialized_boundary_signature_from_abi(&call_signature)
            .expect("materialized two-parameter signature");
        signature
            .opaque_representations
            .push(test_opaque_representation_use(0));

        let first = boundary_plan_application_identity(&signature, &validated);
        signature.opaque_representations[0].shape_root = 1;
        let second = boundary_plan_application_identity(&signature, &validated);
        assert_ne!(first, second);
    }

    #[test]
    fn opaque_movement_rejoins_an_exact_result_placement() {
        let shape = ValueShape::integer(8, 8);
        let call_signature = CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        };
        let validated = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(NativeTarget::host()),
            &call_signature,
        )
        .expect("ordinary parameter-and-result plan");
        let mut signature = materialized_boundary_signature_from_abi(&call_signature)
            .expect("materialized parameter-and-result signature");
        let result_root = signature.result.expect("materialized result root");
        signature
            .opaque_representations
            .push(test_opaque_representation_use(result_root));

        let movement = signature
            .opaque_representation_movement(&signature.opaque_representations[0], &validated)
            .expect("result marker rejoins one exact validated placement");
        assert_eq!(
            movement.role(),
            BoundaryOpaqueRepresentationMovementRole::Result
        );
        assert!(movement.path().is_empty());
        assert_eq!(
            movement.placement(),
            validated.plan().call.result.as_ref().unwrap()
        );
    }

    fn legacy_boundary_plan_application_v2_identity(
        signature: &MaterializedBoundarySignature,
        validated: &ValidatedBoundaryEntryPlan,
    ) -> (u64, [u8; 32]) {
        let mut strong = Sha256::new();
        strong.update(b"omega.boundary-plan-application.v2");
        strong.update((signature.owner_requirement_identity.len() as u64).to_le_bytes());
        strong.update(signature.owner_requirement_identity.as_bytes());
        strong.update((signature.native_parameters.len() as u64).to_le_bytes());
        for parameter in &signature.native_parameters {
            strong.update(parameter.identity.get().to_le_bytes());
            strong.update(parameter.native_ordinal.to_le_bytes());
            match parameter.origin {
                BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } => {
                    strong.update([1]);
                    strong.update(formal_ordinal.to_le_bytes());
                }
                BoundaryNativeParameterOrigin::PrivateCallback {
                    binder,
                    requirement,
                } => {
                    strong.update([2]);
                    strong.update(binder.get().to_le_bytes());
                    strong.update(requirement.get().to_le_bytes());
                }
            }
            match parameter.shape {
                BoundaryNativeParameterShape::Semantic(root) => {
                    strong.update([1]);
                    strong.update(root.to_le_bytes());
                }
                BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                } => {
                    strong.update([2]);
                    strong.update(byte_size.to_le_bytes());
                    strong.update(alignment.to_le_bytes());
                }
            }
        }
        strong.update(validated.contract_commitment_digest());
        let commitment: [u8; 32] = strong.finalize().into();
        let report = callback_plan_report_fingerprint(
            b"omega.boundary-plan-application-report.v2",
            &[&commitment],
        );
        (report, commitment)
    }

    #[test]
    fn compact_native_parameter_collision_rejects_within_exact_registrar_catalog() {
        let identity = NativeParameterId::new(0x51).expect("nonzero report identity");
        let prior = [BoundaryNativeParameter {
            identity,
            native_ordinal: 0,
            shape: BoundaryNativeParameterShape::Semantic(0),
            origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 0 },
            layout_data_symbol: psi_symbols::SymbolHandle::from_arena_index(3),
        }];

        let error = validate_fresh_native_parameter_report_identity(&prior, identity, 1)
            .expect_err("a second exact parameter cannot reuse a compact report identity");
        assert!(error.contains("collides with an earlier exact parameter"));
    }

    #[test]
    fn boundary_application_v3_commits_nominal_telescope_order() {
        let call_signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
            result: None,
        };
        let validated =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &call_signature)
                .expect("Microsoft x64 ordinary plan");
        let signature = materialized_boundary_signature_from_abi(&call_signature).unwrap();
        let first = boundary_plan_application_identity(&signature, &validated);

        let mut reordered = signature.clone();
        reordered.native_parameters.swap(0, 1);
        let second = boundary_plan_application_identity(&reordered, &validated);
        assert_ne!(
            first, second,
            "equally shaped nominal parameters cannot reorder under one application identity"
        );

        let mut renamed = signature;
        renamed.native_parameters[0].identity = nominal_callback_native_parameter_id(
            "omega.synthetic-abi-signature",
            "renamed-parameter",
        );
        assert_ne!(
            first,
            boundary_plan_application_identity(&renamed, &validated),
            "a changed declaration-owned native parameter identity must reissue the application"
        );
    }

    #[test]
    fn materialized_plan_cannot_create_an_undeclared_native_parameter() {
        let call_signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let validated =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &call_signature)
                .expect("Microsoft x64 ordinary plan");
        let signature = materialized_boundary_signature_from_abi(&call_signature).unwrap();
        let mut invented = validated.plan().clone();
        invented
            .call
            .parameters
            .push(invented.call.parameters[0].clone());

        let error = validate_materialized_boundary_plan_result(
            BoundaryPlanResult::Accepted(invented),
            &signature,
        )
        .expect_err("a calling policy cannot add a parameter to the declared telescope");
        assert!(
            error.contains(
                "plan places 2 parameters for a boundary signature with 1 native parameters"
            ),
            "unexpected diagnostic: {error}",
        );
    }

    #[test]
    fn compact_callback_requirement_collision_rejects_exact_requirement_substitution() {
        let report = CallbackRequirementId::new(0x61).expect("nonzero report identity");
        let catalog = [(report, "package::First::call#exact".to_owned())];

        let error = validate_callback_requirement_report_collision(
            &catalog,
            report,
            "package::Second::call#exact",
        )
        .expect_err("compact equality cannot substitute another exact callback requirement");
        assert!(error.contains("distinct exact callback requirements collide"));

        validate_callback_requirement_report_collision(
            &catalog,
            report,
            "package::First::call#exact",
        )
        .expect("the same exact requirement may reuse its catalog report coordinate");
    }

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
            owner_requirement_identity: "test::Registrar::register#exact".to_owned(),
            native_target: NativeTarget::host(),
            opaque_representations: Vec::new(),
            shapes: Vec::new(),
            fields: Vec::new(),
            parameters: Vec::new(),
            callback_binders: vec![binder],
            callback_demands: vec![demand.clone()],
            native_parameters: Vec::new(),
            direct_callback_parameters: Vec::new(),
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
        let materialized_signature =
            materialized_boundary_signature_from_abi(&CallSignature::default()).unwrap();
        let (report_fingerprint, application_commitment) =
            boundary_plan_application_identity(&materialized_signature, &validated);
        let commitment = psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
            application_commitment,
        );
        let boundary_trait = psi_symbols::SymbolHandle::from_arena_index(1);
        let requirement = psi_symbols::SymbolHandle::from_arena_index(2);
        let registration_operation = psi_symbols::SymbolHandle::from_arena_index(3);
        let selected_machine = psi_symbols::SymbolHandle::from_arena_index(4);
        let selected_entry = psi_symbols::SymbolHandle::from_arena_index(5);
        let selected_actual_fingerprint = 7;
        let resource_envelope =
            psi_checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
                selected_machine,
                selected_entry,
                selected_actual_fingerprint,
                psi_checked_trees::MachineContractCommitment::from_digest([8; 32]),
            );
        let resource_receipt =
            psi_checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
                &resource_envelope,
            )
            .expect("canonical callback resource receipt");
        let nominal_use = psi_checked_trees::CheckedNominalMachineUse {
            site: psi_checked_trees::NominalMachineUseSite::Expression(
                psi_typed_trees::expression::ExpressionHandle::from_arena_index(1),
            ),
            registration_operation,
            static_machine_ordinal: 0,
            selected_machine,
            selected_entry,
            satisfaction_trait: boundary_trait,
            satisfaction_requirement: requirement,
            canonical_requirement_overload: "Handler::call(i32)->i32".to_owned(),
            published_requirement_envelope:
                psi_checked_trees::CheckedMachineContractEnvelopeIdentity {
                    contract_report_fingerprint: 6,
                    contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                        [6; 32],
                    ),
                },
            selected_actual_envelope: psi_checked_trees::CheckedMachineContractEnvelopeIdentity {
                contract_report_fingerprint: selected_actual_fingerprint,
                contract_commitment: psi_checked_trees::MachineContractCommitment::from_digest(
                    [8; 32],
                ),
            },
            callback_placement: Some(psi_checked_trees::CheckedCallbackPlacementIdentity {
                boundary_calling_plan_report_fingerprint: report_fingerprint,
                boundary_calling_plan_commitment: commitment,
                resource_receipt,
            }),
            refinement: psi_checked_trees::CheckedMachineContractRefinement {
                published_requirement_report_fingerprint: 6,
                published_requirement_commitment:
                    psi_checked_trees::MachineContractCommitment::from_digest([6; 32]),
                selected_actual_report_fingerprint: selected_actual_fingerprint,
                selected_actual_commitment:
                    psi_checked_trees::MachineContractCommitment::from_digest([8; 32]),
            },
        };
        let mut checked = psi_checked_trees::CheckedTrees::default();
        checked.facts.contract_plans.machines = vec![psi_checked_trees::MachineContractPlan {
            machine: selected_machine,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: selected_actual_fingerprint,
            commitment: psi_checked_trees::MachineContractCommitment::from_digest([8; 32]),
        }];
        checked.facts.contract_plans.crash_capsules = vec![
            psi_checked_trees::CrashContractCapsule::new_with_commitment(
                boundary_trait,
                requirement,
                6,
                psi_checked_trees::MachineContractCommitment::from_digest([6; 32]),
                Vec::new(),
            ),
        ];
        checked.facts.contract_plans.realized_envelopes = vec![
            psi_checked_trees::RealizedMachineContractEnvelope {
                machine: selected_machine,
                contract_report_fingerprint: selected_actual_fingerprint,
                contract_commitment:
                    psi_checked_trees::MachineContractCommitment::from_digest([8; 32]),
                effective_service_reach: Vec::new(),
                concrete_service_reach: Vec::new(),
                unresolved_installation_reaches: Vec::new(),
                effective_synchronous_invocations: Vec::new(),
                checked_may_suspend: false,
                checked_may_block: false,
                checked_termination:
                    psi_language_semantics::TerminationGuarantee::NoGuarantee,
                checked_crash: psi_checked_trees::CrashPlan::default(),
                mutation: Vec::new(),
                capabilities: Vec::new(),
                resources:
                    psi_checked_trees::CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                        selected_machine,
                        selected_actual_fingerprint,
                        psi_checked_trees::MachineContractCommitment::from_digest([8; 32]),
                        [selected_entry],
                    ),
            },
        ];
        checked.facts.nominal_machine_uses =
            psi_checked_trees::NominalMachineUseFacts::try_with_uses([nominal_use])
                .expect("valid nominal callback row");
        let inbound_realization = BoundaryCallingPlanRealization {
            boundary_trait,
            boundary_arguments: Vec::new(),
            requirement_machine: requirement,
            report_fingerprint,
            commitment,
            boundary_entry_plan: validated.plan().clone(),
            exact_boundary_entry_plan: validated.plan().clone(),
            callback_binders: Vec::new(),
            callback_demands: Vec::new(),
            callback_context_closed: false,
            native_parameters: Vec::new(),
            materialized_signature: materialized_signature.clone(),
            policy_machine: String::new(),
            relationship_span: psi_source::SourceSpan::default(),
        };
        let registrar_realization = BoundaryCallingPlanRealization {
            boundary_trait: psi_symbols::SymbolHandle::from_arena_index(9),
            boundary_arguments: Vec::new(),
            requirement_machine: registration_operation,
            report_fingerprint,
            commitment,
            boundary_entry_plan: validated.plan().clone(),
            exact_boundary_entry_plan: validated.plan().clone(),
            callback_binders: vec![BoundaryCallbackBinder {
                binder: StaticMachineBinderId::new(31).unwrap(),
                requirement: CallbackRequirementId::new(37).unwrap(),
                static_machine_ordinal: 0,
                parameter_symbol: psi_symbols::SymbolHandle::from_arena_index(11),
                requirement_trait: boundary_trait,
                requirement_machine: requirement,
            }],
            callback_demands: Vec::new(),
            callback_context_closed: false,
            native_parameters: Vec::new(),
            materialized_signature,
            policy_machine: String::new(),
            relationship_span: psi_source::SourceSpan::default(),
        };
        (checked, vec![inbound_realization, registrar_realization])
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
            bound.resource_receipt,
            nominal_use
                .callback_placement
                .expect("checked callback placement")
                .resource_receipt
        );
        assert_eq!(
            bound.boundary_calling_plan_report_fingerprint,
            realizations[0]
                .replayed_validated_plan()
                .unwrap()
                .contract_report_fingerprint()
        );
        assert_eq!(
            bound.boundary_entry_plan,
            realizations[0].boundary_entry_plan
        );
    }

    #[test]
    fn nominal_callback_placement_rejects_self_consistent_legacy_v2_evidence() {
        let (mut checked, mut realizations) = nominal_callback_fixture();
        let validated = realizations[0]
            .replayed_validated_plan()
            .expect("fixture plan replays");
        let (legacy_report, legacy_digest) = legacy_boundary_plan_application_v2_identity(
            &realizations[0].materialized_signature,
            &validated,
        );
        let legacy_commitment =
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(legacy_digest);
        realizations[0].report_fingerprint = legacy_report;
        realizations[0].commitment = legacy_commitment;
        let placement = checked.facts.nominal_machine_uses.uses[0]
            .callback_placement
            .as_mut()
            .expect("fixture callback placement");
        placement.boundary_calling_plan_report_fingerprint = legacy_report;
        placement.boundary_calling_plan_commitment = legacy_commitment;

        let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("retired application-v2 evidence must not be reinterpreted as v3");
        assert!(
            diagnostics[0]
                .message
                .contains("does not bind its exact evaluated target calling plan"),
            "unexpected diagnostic: {:?}",
            diagnostics[0],
        );
    }

    #[test]
    fn nominal_callback_placement_rejoins_the_current_entry_resource_envelope() {
        let (mut checked, realizations) = nominal_callback_fixture();
        checked.facts.contract_plans.realized_envelopes.clear();
        let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("a callback cannot lose its current resource envelope");
        assert!(
            diagnostics[0]
                .message
                .contains("missing its exact checked entry")
        );

        let (mut checked, realizations) = nominal_callback_fixture();
        let nominal_use = &mut checked.facts.nominal_machine_uses.uses[0];
        let foreign = psi_checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
            nominal_use.selected_machine,
            psi_symbols::SymbolHandle::from_arena_index(17),
            nominal_use
                .selected_actual_envelope
                .contract_report_fingerprint,
            nominal_use.selected_actual_envelope.contract_commitment,
        );
        nominal_use
            .callback_placement
            .as_mut()
            .expect("callback placement")
            .resource_receipt =
            psi_checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(&foreign)
                .expect("canonical foreign entry receipt");
        let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("a callback cannot substitute another entry resource receipt");
        assert!(
            diagnostics[0]
                .message
                .contains("does not bind its exact checked entry resource receipt")
        );
    }

    #[test]
    fn nominal_callback_placement_rejects_compact_equal_resource_contract_substitution() {
        let (mut checked, realizations) = nominal_callback_fixture();
        let nominal_use = &mut checked.facts.nominal_machine_uses.uses[0];
        let substituted = psi_checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
            nominal_use.selected_machine,
            nominal_use.selected_entry,
            nominal_use
                .selected_actual_envelope
                .contract_report_fingerprint,
            psi_checked_trees::MachineContractCommitment::from_digest([0x77; 32]),
        );
        nominal_use
            .callback_placement
            .as_mut()
            .expect("callback placement")
            .resource_receipt =
            psi_checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
                &substituted,
            )
            .expect("independently canonical compact-equal resource receipt");

        let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("compact-equal resource contract substitution must reject");
        assert!(
            diagnostics[0]
                .message
                .contains("does not bind its exact checked entry resource receipt")
        );
    }

    #[test]
    fn nominal_callback_consumer_replays_exact_materialization_context() {
        let (checked, mut realizations) = nominal_callback_fixture();
        let nominal_use = &checked.facts.nominal_machine_uses.uses[0];
        let satisfaction_trait = nominal_use.satisfaction_trait;
        let satisfaction_requirement = nominal_use.satisfaction_requirement;
        let realization = &mut realizations[1];
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
            requirement_trait: satisfaction_trait,
            requirement_machine: satisfaction_requirement,
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
            destination: destination.clone(),
        }];
        let signature = CallSignature::default();
        let context = CallbackMaterializationContext {
            binders: vec![CallbackBinderRequirement {
                binder,
                requirement,
            }],
            demands: realization.callback_demands.clone(),
        };
        let validated = validate_boundary_entry_plan_with_callback_materializations(
            realization.boundary_entry_plan.clone(),
            &signature,
            &context,
        )
        .unwrap();
        realization.materialized_signature.callback_binders = realization.callback_binders.clone();
        realization.materialized_signature.callback_demands = realization.callback_demands.clone();
        let (report_fingerprint, commitment) =
            boundary_plan_application_identity(&realization.materialized_signature, &validated);
        realization.report_fingerprint = report_fingerprint;
        realization.commitment =
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(commitment);
        realization.exact_boundary_entry_plan = realization.boundary_entry_plan.clone();
        let bound = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect("exact retained callback context should replay");
        let retained = bound[0]
            .private_materialization
            .as_ref()
            .expect("target-closed callback use retains its exact private row");
        assert_eq!(retained.binder, binder);
        assert_eq!(retained.destination, destination);
        assert_eq!(retained.requirement, requirement);
        assert_eq!(retained.context, context);
        assert!(
            retained.direct_registrar_parameter_application.is_none(),
            "a field destination must not acquire a direct native parameter application",
        );
        let mut direct_row_on_field = bound[0].clone();
        let shape = ValueShape::integer(8, 8);
        direct_row_on_field
            .private_materialization
            .as_mut()
            .unwrap()
            .direct_registrar_parameter_application =
            Some(omega_calling_conventions::NativeParameterApplication {
                parameter: NativeParameterId::new(107).unwrap(),
                native_ordinal: 0,
                shape,
                placement: ValuePlacement {
                    shape,
                    locations: Vec::new(),
                },
            });
        assert!(
            omega_backend_plan::validate_bound_nominal_callback_placement(&direct_row_on_field)
                .is_err(),
            "a field destination must reject a substituted direct application row",
        );
        assert_eq!(
            retained.registrar_boundary_entry_plan,
            realizations[1].boundary_entry_plan
        );
        assert_eq!(
            retained.registrar_calling_plan_report_fingerprint,
            validated.contract_report_fingerprint()
        );

        let mut ordinal_drift = checked.clone();
        ordinal_drift.facts.nominal_machine_uses.uses[0].static_machine_ordinal = 1;
        let diagnostics =
            validate_nominal_callback_placement_bindings(&ordinal_drift, &realizations)
                .expect_err("a different static-machine ordinal cannot select the binder row");
        assert!(
            diagnostics[0]
                .message
                .contains("exact private materialization")
                && diagnostics[0]
                    .message
                    .contains("outbound registrar realizations"),
            "unexpected diagnostic: {:?}",
            diagnostics[0]
        );

        let mut operation_drift = checked.clone();
        operation_drift.facts.nominal_machine_uses.uses[0].registration_operation =
            psi_symbols::SymbolHandle::from_arena_index(149);
        let diagnostics =
            validate_nominal_callback_placement_bindings(&operation_drift, &realizations)
                .expect_err("a substituted registrar operation cannot select the outbound plan");
        assert!(
            diagnostics[0]
                .message
                .contains("outbound registrar realizations"),
            "unexpected diagnostic: {:?}",
            diagnostics[0]
        );

        let diagnostics =
            validate_nominal_callback_placement_bindings(&checked, &realizations[..1])
                .expect_err("the callback use cannot lose its outbound registrar realization");
        assert!(
            diagnostics[0]
                .message
                .contains("0 outbound registrar realizations"),
            "unexpected diagnostic: {:?}",
            diagnostics[0]
        );
        let duplicate_registrar = vec![
            realizations[0].clone(),
            realizations[1].clone(),
            realizations[1].clone(),
        ];
        let diagnostics =
            validate_nominal_callback_placement_bindings(&checked, &duplicate_registrar)
                .expect_err("the callback use cannot select duplicate outbound realizations");
        assert!(
            diagnostics[0]
                .message
                .contains("2 outbound registrar realizations"),
            "unexpected diagnostic: {:?}",
            diagnostics[0]
        );

        realizations[1].callback_demands[0].destination =
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
        validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect("a no-target checked realization must remain target-neutral");

        realizations[1].callback_context_closed = true;
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
            .boundary_calling_plan_report_fingerprint ^= 1;
        let drift = validate_nominal_callback_placement_bindings(&checked, &realizations)
            .expect_err("a changed target plan identity must reject");
        assert!(
            drift[0]
                .message
                .contains("does not bind its exact evaluated target calling plan")
        );

        let (mut compact_equal_checked, compact_equal_realizations) = nominal_callback_fixture();
        compact_equal_checked.facts.nominal_machine_uses.uses[0]
            .callback_placement
            .as_mut()
            .expect("callback placement")
            .boundary_calling_plan_commitment =
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0x5a; 32]);
        let substitution = validate_nominal_callback_placement_bindings(
            &compact_equal_checked,
            &compact_equal_realizations,
        )
        .expect_err("a compact-equal strong-plan substitution must reject");
        assert!(
            substitution[0]
                .message
                .contains("does not bind its exact evaluated target calling plan")
        );

        let (mut published_substitution, published_realizations) = nominal_callback_fixture();
        let substituted_contract =
            psi_checked_trees::MachineContractCommitment::from_digest([0x6a; 32]);
        published_substitution.facts.nominal_machine_uses.uses[0]
            .published_requirement_envelope
            .contract_commitment = substituted_contract;
        published_substitution.facts.nominal_machine_uses.uses[0]
            .refinement
            .published_requirement_commitment = substituted_contract;
        let substitution = validate_nominal_callback_placement_bindings(
            &published_substitution,
            &published_realizations,
        )
        .expect_err("a compact-equal published requirement substitution must reject");
        assert!(
            substitution[0]
                .message
                .contains("does not bind its exact published requirement contract")
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

    #[test]
    fn callback_closure_reconciliation_requires_the_preclosure_strong_commitment() {
        let (mut checked, realizations) = nominal_callback_fixture();
        let realization = &realizations[0];
        let original = checked.facts.nominal_machine_uses.uses[0]
            .callback_placement
            .expect("callback placement");
        let substituted_old =
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0x5a; 32]);
        let closed =
            psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0x6b; 32]);

        reconcile_closed_callback_plan_identity(
            &mut checked.facts.nominal_machine_uses.uses,
            realization.boundary_trait,
            realization.requirement_machine,
            realization.report_fingerprint,
            substituted_old,
            0xfeed,
            closed,
        );
        assert_eq!(
            checked.facts.nominal_machine_uses.uses[0].callback_placement,
            Some(original),
            "compact equality without the pre-closure commitment must not rewrite custody"
        );

        reconcile_closed_callback_plan_identity(
            &mut checked.facts.nominal_machine_uses.uses,
            realization.boundary_trait,
            realization.requirement_machine,
            realization.report_fingerprint,
            realization.commitment,
            0xfeed,
            closed,
        );
        let reconciled = checked.facts.nominal_machine_uses.uses[0]
            .callback_placement
            .expect("reconciled callback placement");
        assert_eq!(reconciled.boundary_calling_plan_report_fingerprint, 0xfeed);
        assert_eq!(reconciled.boundary_calling_plan_commitment, closed);
        assert_eq!(reconciled.resource_receipt, original.resource_receipt);
    }

    fn four_f32_array_signature() -> MaterializedBoundarySignature {
        MaterializedBoundarySignature {
            owner_requirement_identity: "test::Array::call#exact".to_owned(),
            native_target: NativeTarget::host(),
            opaque_representations: Vec::new(),
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
            direct_callback_parameters: Vec::new(),
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
            owner_requirement_identity: "test::Compatibility::call#exact".to_owned(),
            native_target: NativeTarget::host(),
            opaque_representations: Vec::new(),
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
            direct_callback_parameters: Vec::new(),
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
}
