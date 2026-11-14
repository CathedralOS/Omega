//! One source storage-description pass over the shared reader's canonical
//! record namespace. This pass parses no body and grants no value, lifetime,
//! access, initialization, disposal or claim authority.

use arena::Handle;
use language_semantics::boundary_values::{BoundaryValueRecipe, BoundaryValueRecipes};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_check_tables::Checked;
use terminal_psi::{
    CallingAbiParameter, CallingAbiStorage, CallingRuntimeFormal, CallingSignatureTelescope,
    StructuralAccess,
};
use typed_trees::boundary_values::{BoundaryTypeBinding, BoundaryValuePreparation};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::{Program, services};
use crate::body::{StorageShape, ValueType};
use crate::calling_publication::PreparedCallingStorage;
use crate::read::calling::SourceCallingPolicies;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum TypePurpose {
    #[default]
    Value,
    StorageDescription,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StorageDescriptionProperties {
    pub(crate) record: StructuralTypeId,
    pub(crate) owns_linear_claim: bool,
    pub(crate) no_code_affine_cleanup: bool,
    pub(crate) descriptor_cleanup: bool,
}

pub(crate) struct PreparedCallingDescriptions {
    pub(crate) signatures: Vec<PreparedCallingStorage>,
    pub(crate) properties: Vec<StorageDescriptionProperties>,
}

impl Program<'_> {
    /// Called before the shared executable-body closure is read. It resets the
    /// source role before any body is visited; cached record lookup rechecks
    /// value-use eligibility, so describing linear/addr storage cannot admit a
    /// runtime value. The same RecordType/RecordSource tables own both roles.
    pub(crate) fn prepare_calling_storage(
        &mut self,
        context: MachineId,
        source: &SourceCallingPolicies<'_>,
    ) -> Checked<PreparedCallingDescriptions> {
        let previous_purpose =
            std::mem::replace(&mut self.type_purpose, TypePurpose::StorageDescription);
        let previous_scope = std::mem::take(&mut self.scope);
        let previous_owner = self.lifetime_owner;
        let first_new_record = self.records.len();
        let result = (|| {
            let mut descriptions = Vec::with_capacity(source.signatures.len());
            for signature in &source.signatures {
                let telescope = signature
                    .telescope
                    .as_ref()
                    .map_err(|reason| services::unavailable(context, reason.clone()))?;
                if !telescope.binders.is_empty() || !telescope.direct_callbacks.is_empty() {
                    return Err(services::unavailable(
                        context,
                        "calling metadata needs its exact published callback catalog",
                    ));
                }
                let mut agreed = None;
                for projection in &signature.projections {
                    let projection = projection
                        .as_ref()
                        .map_err(|reason| services::unavailable(context, reason.clone()))?;
                    if projection.parameters.len() != telescope.parameters.len() {
                        return Err(services::unavailable(
                            context,
                            "calling storage differs from its exact native formal telescope",
                        ));
                    }
                    self.scope.clear();
                    self.lifetime_owner = signature.template;
                    let mut bound = std::collections::HashSet::<
                        symbols::SymbolHandle,
                        symbols::BuildSymbolHasher,
                    >::default();
                    let bindings = projection
                        .substitutions
                        .iter()
                        .map(|(parameter, actual)| {
                            if !bound.insert(*parameter) {
                                return Err(services::unavailable(context,
                                    "calling storage needs its complete repeated-binder application scopes"));
                            }
                            if !self.typed.symbols.symbols().nodes().is_valid(*parameter)
                                || !self
                                    .typed
                                    .type_reference_table
                                    .contains_type_reference(*actual)
                                || self
                                    .typed
                                    .data_type_parameter_by_symbol(*parameter)
                                    .is_none()
                            {
                                return Err(services::unavailable(
                                    context,
                                    "calling storage has a stale source application binding",
                                ));
                            }
                            Ok(BoundaryTypeBinding {
                                parameter_symbol: *parameter,
                                actual: *actual,
                            })
                        })
                        .collect::<Checked<Vec<_>>>()?;
                    // Non-type operands remain in the complete ABI key and
                    // original binding list. Reading one as a type is never a
                    // fabricated scalar value or an erased actual.
                    let mut supplied_types = Vec::new();
                    for binding in &bindings {
                        if self
                            .typed
                            .data_type_parameter_by_symbol(binding.parameter_symbol)
                            .is_some_and(|parameter| {
                                parameter.kind == typed_trees::data::TypeParameterKind::Type
                            })
                        {
                            let value = self.value_type(context, binding.actual)?;
                            supplied_types.push((binding.parameter_symbol, value));
                        }
                    }
                    // Actuals are interpreted in their supplier namespace,
                    // before the simultaneous required frame is installed.
                    self.scope = supplied_types;
                    let typed = self.typed;
                    let mut recipes = BoundaryValuePreparation::new(typed, &bindings, &[]);
                    let mut parameters = Vec::with_capacity(projection.parameters.len());
                    for &reference in &projection.parameters {
                        let recipe = recipes.value_type(reference);
                        validate_recipe(recipes.recipes(), recipe)
                            .map_err(|reason| services::unavailable(context, reason))?;
                        let reference =
                            typed_trees::boundary_values::substitute_boundary_type_reference(
                                typed, reference, &bindings,
                            );
                        let (storage_type, access) = self.metadata_parameter(context, reference)?;
                        parameters.push(CallingAbiParameter {
                            structural_type: storage_type,
                            // Callback-bearing source was refused above; no
                            // retained callback obligation becomes None.
                            callback_backing: None,
                            access,
                        });
                    }
                    let result = if !projection.result.is_valid()
                        || matches!(
                            typed.type_reference_table.type_reference(projection.result),
                            TypeReferenceNode::Unit
                        ) {
                        None
                    } else {
                        let recipe = recipes.value_type(projection.result);
                        validate_recipe(recipes.recipes(), recipe)
                            .map_err(|reason| services::unavailable(context, reason))?;
                        let reference =
                            typed_trees::boundary_values::substitute_boundary_type_reference(
                                typed,
                                projection.result,
                                &bindings,
                            );
                        Some(self.metadata_storage(context, reference)?)
                    };
                    let storage = CallingAbiStorage {
                        parameters,
                        result,
                        telescope: CallingSignatureTelescope {
                            parameters: telescope
                                .parameters
                                .iter()
                                .map(|formal| CallingRuntimeFormal {
                                    identity: formal.identity,
                                    native_ordinal: formal.native_ordinal,
                                })
                                .collect(),
                            binders: Vec::new(),
                            direct_callbacks: Vec::new(),
                        },
                    };
                    if agreed.as_ref().is_some_and(|prior| prior != &storage) {
                        return Err(services::unavailable(
                            context,
                            "equal declaration ABI keys have different complete storage projections",
                        ));
                    }
                    agreed = Some(storage);
                }
                let storage = agreed.ok_or_else(|| {
                    services::unavailable(
                        context,
                        "calling ABI has no exact source storage projection",
                    )
                })?;
                descriptions.push(PreparedCallingStorage {
                    id: signature.id,
                    storage,
                });
            }
            let mut properties: Vec<StorageDescriptionProperties> = Vec::new();
            for (record, declaration) in self.records.iter() {
                if record.get() <= first_new_record as u64 {
                    continue;
                }
                let source = self.record_sources.get(record).ok_or_else(|| {
                    services::unavailable(context, "storage description lost its source row")
                })?;
                let linear = if source.declaration.is_valid() {
                    self.typed
                        .unique_data_definition_by_symbol(source.declaration)
                        .ok_or_else(|| {
                            services::unavailable(
                                context,
                                "storage description has no unique data declaration",
                            )
                        })?
                        .properties
                        .multiplicity
                        == language_semantics::Multiplicity::Linear
                } else {
                    false
                };
                let mut nested = Vec::new();
                match &declaration.shape {
                    StorageShape::Record(fields) => {
                        nested.extend(fields.iter().filter_map(|field| match field.value_type {
                            ValueType::Record(record, _) => Some(record),
                            _ => None,
                        }))
                    }
                    StorageShape::Sum(cases) => {
                        nested.extend(cases.iter().flat_map(|case| &case.fields).filter_map(
                            |field| match field.value_type {
                                ValueType::Record(record, _) => Some(record),
                                _ => None,
                            },
                        ))
                    }
                    StorageShape::FixedArray { element, .. } => nested.push(*element),
                    StorageShape::ByteSequence(_) | StorageShape::PrimitiveScalar { .. } => {}
                }
                let inherited = nested.iter().any(|nested| {
                    properties
                        .iter()
                        .any(|row| row.record == *nested && row.owns_linear_claim)
                });
                // Selected nominal cleanup and carry mechanisms remain refused
                // by the shared record-schema reader, so there is no missing
                // cleanup code disguised as a no-code metadata description.
                properties.push(StorageDescriptionProperties {
                    record,
                    owns_linear_claim: linear || inherited,
                    no_code_affine_cleanup: true,
                    descriptor_cleanup: true,
                });
            }
            Ok(PreparedCallingDescriptions {
                signatures: descriptions,
                properties,
            })
        })();
        self.type_purpose = previous_purpose;
        self.scope = previous_scope;
        self.lifetime_owner = previous_owner;
        result
    }

    fn metadata_storage(
        &mut self,
        context: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<StructuralTypeId> {
        let value = self.value_type(context, reference)?;
        // A layout describes the bare storage carrier. The complete authored
        // qualifications stay on the ABI/source application; no domain
        // introduction is attached to this physical type identity.
        let reference = self.metadata_carrier(reference);
        self.storage_type(context, reference, value)
    }

    fn metadata_carrier(&self, mut reference: TypeReferenceHandle) -> TypeReferenceHandle {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            self.typed.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        reference
    }

    fn metadata_parameter(
        &mut self,
        context: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<(StructuralTypeId, StructuralAccess)> {
        let reference = self.metadata_carrier(reference);
        match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } => {
                let (referee, access) = (*referee, *access);
                let storage = self.metadata_storage(context, referee)?;
                Ok((
                    storage,
                    match access {
                        language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                        language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                        language_core::ReferenceAccess::WriteOnly => {
                            StructuralAccess::WriteOnlyBorrow
                        }
                    },
                ))
            }
            _ => self
                .metadata_storage(context, reference)
                .map(|record| (record, StructuralAccess::Owned)),
        }
    }
}

fn validate_recipe(
    recipes: &BoundaryValueRecipes,
    root: Handle<BoundaryValueRecipe>,
) -> Result<(), String> {
    let mut pending = vec![root];
    let mut seen = std::collections::HashSet::<Handle<BoundaryValueRecipe>>::new();
    while let Some(root) = pending.pop() {
        if !seen.insert(root) {
            continue;
        }
        if !recipes.values.is_valid(root) {
            return Err("boundary storage recipe has a stale coordinate".into());
        }
        match recipes.values.get(root) {
            BoundaryValueRecipe::Absent => return Err("boundary storage recipe is absent".into()),
            BoundaryValueRecipe::Rejected(reason) => return Err(reason.clone()),
            BoundaryValueRecipe::Integer { .. }
            | BoundaryValueRecipe::Float { .. }
            | BoundaryValueRecipe::Reference { .. } => {}
            BoundaryValueRecipe::FixedArray { element, .. } => pending.push(*element),
            BoundaryValueRecipe::Record {
                fields, storage, ..
            } => {
                if storage.is_some() {
                    return Err(
                        "canonical source metadata needs its exact authored layout publication"
                            .into(),
                    );
                }
                let fields = recipes
                    .fields
                    .span(*fields)
                    .ok_or("boundary storage recipe has stale field coordinates")?;
                pending.extend(fields.iter().map(|field| field.value));
            }
            BoundaryValueRecipe::TargetLayouts { .. } => return Err(
                "canonical source metadata needs its exact target layout application publication"
                    .into(),
            ),
            BoundaryValueRecipe::Opaque { .. } => {
                return Err(
                    "canonical source metadata needs its exact selected opaque carrier publication"
                        .into(),
                );
            }
        }
    }
    Ok(())
}
