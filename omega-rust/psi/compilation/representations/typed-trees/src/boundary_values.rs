//! Close the source meaning of boundary types before native layout.
//!
//! This preparation borrows source only while producing recipes. Unsupported
//! values remain deferred refusal nodes: inventorying a dormant declaration
//! must not reject it, and target materialization retains traversal order.

use crate::TypedTrees;
use crate::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use arena::Handle;
use language_semantics::boundary_values::{
    BoundaryFieldRecipe, BoundaryRecordStorage, BoundaryTargetRecipe, BoundaryValueRecipe,
    BoundaryValueRecipes,
};

#[derive(Clone)]
pub struct BoundaryTypeBinding {
    pub parameter_symbol: symbols::SymbolHandle,
    pub parameter_name: String,
    pub actual: TypeReferenceHandle,
}

/// A checked Build selection's source correspondence. This does not select or
/// authorize a representation; the caller retains and validates that evidence.
pub struct BoundaryRepresentationType {
    pub opaque: symbols::SymbolHandle,
    pub carrier: TypeReferenceHandle,
}

pub struct BoundaryValuePreparation<'input> {
    typed: &'input TypedTrees,
    bindings: &'input [BoundaryTypeBinding],
    representations: &'input [BoundaryRepresentationType],
    visiting: Vec<DataApplicationKey>,
    // Share exact closed source recipes, not native occurrences. Repeated
    // record fields must not expand exponentially before the ABI bound runs.
    closed_values: Vec<(
        crate::type_identity::TypeIdentityId,
        Handle<BoundaryValueRecipe>,
    )>,
    recipes: BoundaryValueRecipes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DataApplicationKey {
    Nominal(symbols::SymbolHandle),
    Applied(crate::type_identity::TypeIdentityId),
}

impl<'input> BoundaryValuePreparation<'input> {
    pub fn new(
        typed: &'input TypedTrees,
        bindings: &'input [BoundaryTypeBinding],
        representations: &'input [BoundaryRepresentationType],
    ) -> Self {
        Self {
            typed,
            bindings,
            representations,
            visiting: Vec::new(),
            closed_values: Vec::new(),
            recipes: BoundaryValueRecipes::default(),
        }
    }

    pub fn finish(self) -> BoundaryValueRecipes {
        self.recipes
    }

    pub fn recipes(&self) -> &BoundaryValueRecipes {
        &self.recipes
    }

    pub fn value_type(
        &mut self,
        type_reference: TypeReferenceHandle,
    ) -> Handle<BoundaryValueRecipe> {
        let type_reference =
            substitute_boundary_type_reference(self.typed, type_reference, self.bindings);
        let identity = self.typed.normalized_type_identity(type_reference);
        if identity.is_valid()
            && let Some((_, recipe)) = self
                .closed_values
                .iter()
                .find(|(candidate, _)| *candidate == identity)
        {
            return *recipe;
        }
        let recipe = self.type_recipe(type_reference);
        let recipe = self.recipes.values.insert(recipe);
        if identity.is_valid() {
            self.closed_values.push((identity, recipe));
        }
        recipe
    }

    pub fn data(
        &mut self,
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> Handle<BoundaryValueRecipe> {
        let recipe = self.data_recipe(symbol, name);
        self.recipes.values.insert(recipe)
    }

    fn type_recipe(&mut self, type_reference: TypeReferenceHandle) -> BoundaryValueRecipe {
        let typed = self.typed;
        let type_reference =
            substitute_boundary_type_reference(typed, type_reference, self.bindings);
        if let Some(primitive) = typed.primitive_type_reference(type_reference) {
            return match primitive
                .scalar_byte_size()
                .and_then(|size| u16::try_from(size).ok())
            {
                Some(byte_size) if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) => {
                    BoundaryValueRecipe::Float { byte_size }
                }
                Some(byte_size) => BoundaryValueRecipe::Integer { byte_size },
                None => BoundaryValueRecipe::Rejected(format!(
                    "primitive `{}` has no concrete boundary size",
                    primitive.name()
                )),
            };
        }
        match typed.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => self.type_recipe(*base_type),
            TypeReferenceNode::Reference { referee, .. } => {
                let mut referee =
                    substitute_boundary_type_reference(typed, *referee, self.bindings);
                while let TypeReferenceNode::Constrained { base_type, .. } =
                    typed.type_reference_table.type_reference(referee)
                {
                    referee = substitute_boundary_type_reference(typed, *base_type, self.bindings);
                }
                let descriptor = match typed.type_reference_table.type_reference(referee) {
                    TypeReferenceNode::Slice { .. } | TypeReferenceNode::DynamicTrait { .. } => {
                        true
                    }
                    TypeReferenceNode::Named { name, .. } => name.as_str() == "string",
                    _ => false,
                };
                BoundaryValueRecipe::Reference { descriptor }
            }
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::DynamicTrait { .. } => {
                BoundaryValueRecipe::Reference { descriptor: true }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: crate::types::FixedArrayLength::Literal(length),
            } => {
                let element = self.value_type(*element_type);
                BoundaryValueRecipe::FixedArray {
                    element,
                    length: *length as u64,
                }
            }
            TypeReferenceNode::Named { symbol, name } => self.data_recipe(*symbol, name.as_str()),
            TypeReferenceNode::Generic { .. } => match typed.closed_data_definition(type_reference)
            {
                Some(definition) => self.record(
                    definition,
                    DataApplicationKey::Applied(typed.normalized_type_identity(type_reference)),
                ),
                None => BoundaryValueRecipe::Rejected(format!(
                    "generic value type `{}` is not concrete at the Calling<C> relationship",
                    typed.display_type_reference(type_reference)
                )),
            },
            TypeReferenceNode::FixedArray { .. } => BoundaryValueRecipe::Rejected(format!(
                "array type `{}` still has a non-literal length",
                typed.display_type_reference(type_reference)
            )),
            TypeReferenceNode::ConstExpression(_) => BoundaryValueRecipe::Rejected(
                "a proof-static index expression is not a boundary value shape".into(),
            ),
            TypeReferenceNode::Unit => {
                BoundaryValueRecipe::Rejected("unit is not a boundary value shape".into())
            }
        }
    }

    fn data_recipe(&mut self, symbol: symbols::SymbolHandle, name: &str) -> BoundaryValueRecipe {
        let Some(definition) = self.typed.data_definition_by_symbol(symbol) else {
            return BoundaryValueRecipe::Rejected(format!(
                "named boundary type `{name}` has no data definition"
            ));
        };
        if definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque {
            return self.record(definition, DataApplicationKey::Nominal(symbol));
        }
        if self.visiting.contains(&DataApplicationKey::Nominal(symbol)) {
            return BoundaryValueRecipe::Rejected(format!(
                "opaque representation for `{name}` recursively contains its own semantic declaration"
            ));
        }
        let Some((selection_ordinal, representation)) = self
            .representations
            .iter()
            .enumerate()
            .find(|(_, representation)| representation.opaque == symbol)
        else {
            return BoundaryValueRecipe::Rejected(format!(
                "boundary-opaque data `{name}` crosses this boundary by value, but the authoritative build selects no exact `OpaqueRepresentation<{name}, Representation>` implementation"
            ));
        };
        let Some(carrier) = self.typed.closed_data_definition(representation.carrier) else {
            return BoundaryValueRecipe::Rejected(format!(
                "selected opaque representation carrier for `{name}` disappeared before shape closure"
            ));
        };
        let carrier_name = carrier.name.as_str().to_owned();
        self.visiting.push(DataApplicationKey::Nominal(symbol));
        let carrier = self.value_type(representation.carrier);
        self.visiting.pop();
        BoundaryValueRecipe::Opaque {
            name: name.to_owned(),
            carrier_name,
            selection_ordinal,
            carrier,
        }
    }

    fn record(
        &mut self,
        definition: &'input crate::data::DataDefinition,
        key: DataApplicationKey,
    ) -> BoundaryValueRecipe {
        let typed = self.typed;
        let name = definition.name.as_str();
        if self.visiting.contains(&key) {
            return BoundaryValueRecipe::Rejected(format!(
                "recursive by-value data `{name}` has no finite boundary shape"
            ));
        }
        let application = typed.plan_laid_layout_for_data(definition);
        if application.is_none()
            && definition
                .generic_instance
                .is_some_and(|reference| typed.layout_storage_arguments(reference).is_some())
        {
            return BoundaryValueRecipe::Rejected(format!(
                "boundary data `{name}` requires its exact admitted fixed representation"
            ));
        }
        self.visiting.push(key);
        let recipe = match application {
            Some(application)
                if matches!(
                    application.realizations,
                    crate::PlanLaidRealizations::TargetInputs(_)
                ) =>
            {
                let mut alternatives = Vec::new();
                for row in application.target_layouts(typed) {
                    let recipe =
                        self.record_geometry(definition, Some(application), Some(&row.geometry));
                    let value = self.recipes.values.insert(recipe);
                    alternatives.push(BoundaryTargetRecipe {
                        target: row.target.as_str().to_owned(),
                        value,
                    });
                }
                BoundaryValueRecipe::TargetLayouts {
                    name: name.to_owned(),
                    alternatives: self.recipes.targets.insert_many(alternatives),
                }
            }
            _ => self.record_geometry(
                definition,
                application,
                application.and_then(crate::PlanLaidLayout::invariant_geometry),
            ),
        };
        self.visiting.pop();
        recipe
    }

    fn record_geometry(
        &mut self,
        definition: &'input crate::data::DataDefinition,
        application: Option<&'input crate::PlanLaidLayout>,
        geometry: Option<&'input crate::PlanLaidGeometry>,
    ) -> BoundaryValueRecipe {
        let typed = self.typed;
        let name = definition.name.as_str();
        let runtime_fields = typed
            .live_data_fields(typed.data_members(definition))
            .map(|field| field.symbol)
            .collect::<Vec<_>>();
        if let Some((application, geometry)) = application.zip(geometry)
            && (geometry.offsets.len() != runtime_fields.len()
                || application.field_symbols != runtime_fields)
        {
            return BoundaryValueRecipe::Rejected(format!(
                "plan-laid data `{name}` has {} members but its layout publishes {} offsets",
                runtime_fields.len(),
                geometry.offsets.len()
            ));
        }
        let mut fields = Vec::new();
        for member in typed.data_members(definition) {
            let Some(field) = member.as_field() else {
                let value = self.recipes.values.insert(BoundaryValueRecipe::Rejected(format!(
                    "case data `{name}` is not yet classifiable as a boundary value; pass it by reference"
                )));
                fields.push(BoundaryFieldRecipe {
                    value,
                    byte_offset: None,
                });
                break;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let field_ordinal = fields.len();
            let value = match geometry.and_then(|geometry| {
                geometry
                    .integer_fields
                    .iter()
                    .find(|integer| integer.field_index == field_ordinal)
            }) {
                Some(integer) => {
                    let bytes = integer.stored_width_bits / 8;
                    let recipe = if integer.stored_width_bits == 0
                        || integer.stored_width_bits % 8 != 0
                        || bytes > 8
                    {
                        BoundaryValueRecipe::Rejected(format!(
                            "plan-laid data `{name}` field `{}` has invalid stored-integer width {}",
                            field.name, integer.stored_width_bits
                        ))
                    } else {
                        BoundaryValueRecipe::Integer { byte_size: bytes }
                    };
                    self.recipes.values.insert(recipe)
                }
                None => self.value_type(field.type_reference),
            };
            fields.push(BoundaryFieldRecipe {
                value,
                byte_offset: geometry.map(|geometry| geometry.offsets[field_ordinal] as u64),
            });
        }
        BoundaryValueRecipe::Record {
            name: name.to_owned(),
            fields: self.recipes.fields.insert_many(fields),
            storage: geometry.map(|geometry| BoundaryRecordStorage {
                byte_size: geometry.size as u64,
                byte_alignment: geometry.align as u64,
            }),
        }
    }
}

/// Apply the already selected simultaneous boundary type frame. This preserves
/// the existing unresolved-parameter name correspondence until that source
/// compatibility path is retired; recipe consumers never see it.
pub fn substitute_boundary_type_reference(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    bindings: &[BoundaryTypeBinding],
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
