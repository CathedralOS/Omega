#![forbid(unsafe_code)]
#![allow(
    clippy::obfuscated_if_else,
    reason = "valid-handle copy expressions use one uniform then/fallback idiom"
)]

/*
We represent typed Omega programs (`TypedTrees`, `FrozenTypedTrees`,
`TypedTreeRoots`, and `TypedTreeTables`) after symbol resolution and type
inference (`Psi 03`) and through typed elaboration (`Psi 04`). Every expression,
declaration, callable application, and type identity is resolved into durable
tables before proof and borrow checking (`Psi 05`).
*/

/*
We separate typed semantic structure and canonical type identity from flow-proven
facts and target layout. `TypedTrees` owns the complete closed conformance rows,
applied domain theories, operator applications, and binder-aware `TypeIdentity`
interners so later checking and realization stages read exact typed commitments
without re-running type inference or inspecting surface syntax.
*/

/*
- Produced by: `Psi 03` (`03_symbol-resolved-trees-to-typed-trees`) and frozen by `Psi 04` (`04_typed-trees-to-typed-trees`).
- Consumed by: later Psi stages, `proof`, and Omega realization crates.
- Flat module layout:
  - `lib.rs`: `TypedTrees`, `FrozenTypedTrees`, `TypedTreeRoots`, `TypedTreeTables`, `boundary_values`, `direct_calls`, and `source_review`.
  - `references.rs`: typed declaration and expression reference occurrences (`references`).
  - `declaration_projection.rs`: narrowing a program to selected declarations and their closure (`declaration_projection`).
  - `service_declarations.rs`: boundary service and provider declaration projections (`service_declarations`, `provider_declarations`).
  - `access_views_and_commitments.rs`: placed/plan-laid access views, conformance commitments, constant recipes, and wire schema queries.
  - `callable_applications.rs`: closed machine, trait, requirement, and domain callable applications.
  - `callable_signature_shapes.rs`: source calling-shape correspondence before body or proof elaboration.
  - `calls_and_control_flow.rs`: typed call signatures, dynamic/static dispatch families, machines, states, statements, and names.
  - `declaration_tables_and_index.rs`: declaration tables, relationship indexes, type parameters, and type-reference queries.
  - `declared_bindings.rs`: the declared type of the parameter or local a name denotes inside one state (`declared_binding_type`).
  - `deferred_refusals.rs`: requests no whole-program pass served, recorded on the declarations that hold them (`DeferredRefusal`).
  - `declaration_reflection.rs`: semantic schema graphs, selected applications, nominal edges, and deterministic query images.
  - `declarations.rs`, `operator_declarations.rs`, `product_and_trait_declarations.rs`: typed domain, data, wire, visibility, operator, product-operand, and trait declarations.
  - `evidence_and_inspection.rs`: typed propositions, mathematical definitions, ranking functions, and `TypedTreesSnapshot`.
  - `type_system.rs`, `type_identity.rs`: typed `TypeReference`, closed numerics, type predicates, and canonical `TypeIdentity` construction.
  - `values.rs`: typed expressions (`Expression`), constant integers, and table copy routines.
*/

pub mod boundary_values {
    use crate::TypedTrees;
    use crate::type_identity::TypeIdentityId;
    use crate::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
    use arena::Handle;
    use language_semantics::boundary_values::{
        BoundaryFieldRecipe, BoundaryTargetRecipe, BoundaryValueRecipe, BoundaryValueRecipes,
    };

    #[derive(Clone, Default)]
    pub struct BoundaryTypeBinding {
        pub parameter_symbol: symbols::SymbolHandle,
        pub parameter_name: String,
        pub actual: TypeReferenceHandle,
    }

    pub struct BoundaryRepresentationType {
        pub opaque: symbols::SymbolHandle,
        pub carrier: TypeReferenceHandle,
    }

    pub struct BoundaryValuePreparation<'input> {
        typed: &'input TypedTrees,
        bindings: &'input [BoundaryTypeBinding],
        representations: &'input [BoundaryRepresentationType],
        visiting: Vec<DataApplicationKey>,
        closed_values: Vec<(TypeIdentityId, Handle<BoundaryValueRecipe>)>,
        recipes: BoundaryValueRecipes,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum DataApplicationKey {
        Nominal(symbols::SymbolHandle),
        Applied(TypeIdentityId),
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
            if let Some(primitive) = typed.primitive_type_reference(type_reference) {
                return match primitive
                    .scalar_byte_size()
                    .and_then(|size| u16::try_from(size).ok())
                {
                    Some(byte_size)
                        if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) =>
                    {
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
                TypeReferenceNode::Constrained { base_type, .. } => self.type_recipe(
                    substitute_boundary_type_reference(typed, *base_type, self.bindings),
                ),
                TypeReferenceNode::Reference { referee, .. } => {
                    let mut referee =
                        substitute_boundary_type_reference(typed, *referee, self.bindings);
                    while let TypeReferenceNode::Constrained { base_type, .. } =
                        typed.type_reference_table.type_reference(referee)
                    {
                        referee =
                            substitute_boundary_type_reference(typed, *base_type, self.bindings);
                    }
                    let descriptor = foreign_reference_uses_descriptor(typed, referee);
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
                TypeReferenceNode::Named { symbol, name } => {
                    self.data_recipe(*symbol, name.as_str())
                }
                TypeReferenceNode::Generic { .. } => match typed
                    .closed_data_definition(type_reference)
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

        fn data_recipe(
            &mut self,
            symbol: symbols::SymbolHandle,
            name: &str,
        ) -> BoundaryValueRecipe {
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
                        let recipe = self.record_geometry(definition);
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
                _ => self.record_geometry(definition),
            };
            self.visiting.pop();
            recipe
        }

        fn record_geometry(
            &mut self,
            definition: &'input crate::data::DataDefinition,
        ) -> BoundaryValueRecipe {
            let typed = self.typed;
            let name = definition.name.as_str();
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
                let value = self.value_type(field.type_reference);
                fields.push(BoundaryFieldRecipe {
                    value,
                    byte_offset: None,
                });
            }
            BoundaryValueRecipe::Record {
                name: name.to_owned(),
                fields: self.recipes.fields.insert_many(fields),
                storage: None,
            }
        }
    }

    pub fn foreign_reference_uses_descriptor(
        typed: &TypedTrees,
        mut referent: TypeReferenceHandle,
    ) -> bool {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            typed.type_reference_table.type_reference(referent)
        {
            referent = *base_type;
        }
        matches!(
            typed.type_reference_table.type_reference(referent),
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::DynamicTrait { .. }
        )
    }

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
}
pub(crate) mod source_review {
    use crate::{PlanLaidTargetLayout, types::TypeReferenceHandle};
    use arena::Handle;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SourceTypeApplication {
        pub reference: TypeReferenceHandle,
        pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SourceRecordCorrespondence {
        pub record: StructuralTypeId,
        pub declaration: SymbolHandle,
        /// The type occurrence the record was read from. `None` when the
        /// program never writes the type: a declaration with no type
        /// parameter, read as a declaration of the program, is the whole
        /// identity of its record. A generic record always has one.
        pub application: Option<SourceTypeApplication>,
        pub storage_identity: String,
        pub schema: SymbolHandle,
        pub policy: SymbolHandle,
        pub layout: Option<SourceLayoutPlanCoordinate>,
        pub fields: Vec<SourceFieldCorrespondence>,
        pub realizations: Vec<SourceLayoutRealizationCoordinate>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SourceFieldCorrespondence {
        pub field: StructuralFieldId,
        pub declaration: SymbolHandle,
        pub schema: SymbolHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SourceLayoutPlanCoordinate {
        pub ordinal: u32,
        pub application: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SourceLayoutRealizationCoordinate {
        pub target: String,
        pub member: Handle<PlanLaidTargetLayout>,
    }
}
pub use source_review::{
    SourceFieldCorrespondence, SourceLayoutPlanCoordinate, SourceLayoutRealizationCoordinate,
    SourceRecordCorrespondence, SourceTypeApplication,
};
pub mod direct_calls {
    use crate::TypedTrees;
    use crate::inspection::callees::ExplicitStateCallees;
    use language_semantics::direct_calls::{
        DirectCall, DirectCallDeclarations, DirectCallFamily, DirectCallFamilyMember,
        DirectCallMachine, DirectCallState,
    };
    use symbols::SymbolHandle;

    pub fn collect(
        typed: &TypedTrees,
        scope: source::DependencyScope,
    ) -> DirectCallDeclarations<SymbolHandle> {
        let mut machine_positions = Vec::new();
        for (position, machine) in typed.machines().iter().enumerate() {
            if machine.symbol.is_valid() {
                machine_positions.push((machine.symbol, position));
            }
        }
        for machine in typed.machines() {
            for state in typed.machine_states(machine) {
                if let Some(owner) = called_machine_position(typed, state.symbol) {
                    machine_positions.push((state.symbol, owner));
                }
            }
        }
        let mut callees = ExplicitStateCallees::default();
        let machines = typed
            .machines()
            .iter()
            .map(|machine| {
                let states = typed
                    .machine_states(machine)
                    .iter()
                    .map(|state| {
                        let calls = callees
                            .collect(typed, state)
                            .iter()
                            .map(|called| DirectCall {
                                declaration: *called,
                                machine: called_machine_position(typed, *called),
                            })
                            .collect();
                        DirectCallState {
                            declaration: state.symbol,
                            name: state.name.as_str().to_owned(),
                            calls,
                        }
                    })
                    .collect();
                DirectCallMachine {
                    declaration: machine.symbol,
                    name: machine.name.as_str().to_owned(),
                    in_scope: typed
                        .symbols
                        .symbol_is_in_dependency_scope(machine.symbol, scope),
                    family_interface: machine.target_family_interface,
                    authored_target: typed.machine_authored_target(machine).map(str::to_owned),
                    source_target_context: typed.source_target_context(machine).map(str::to_owned),
                    target: machine
                        .target
                        .as_ref()
                        .map(|target| target.as_str().to_owned()),
                    states,
                }
            })
            .collect();
        let families = typed
            .target_machine_families
            .iter()
            .map(|family| DirectCallFamily {
                canonical: typed.machine_position_by_symbol(family.canonical),
                members: family
                    .members
                    .iter()
                    .map(|member| DirectCallFamilyMember {
                        target: member.target.as_str().to_owned(),
                        machine: typed.machine_position_by_symbol(member.machine),
                    })
                    .collect(),
            })
            .collect();
        DirectCallDeclarations::new(scope, machines, families, machine_positions)
    }

    fn called_machine_position(typed: &TypedTrees, called: SymbolHandle) -> Option<usize> {
        typed
            .machine_by_symbol(called)
            .or_else(|| typed.machine_holding_state(called))
            .and_then(|machine| typed.machine_position_by_symbol(machine.symbol))
    }
}

macro_rules! typed_tree_table {
    ($( $field:ident => $get:ident ($ty:ty) $( [ $push:ident ] )? $(, $inline:ident)? ;)*) => {
        $(
            $( pub fn $push(&mut self, value: $ty) {
                self.tables
                    .$field
                    .append_to_span(&mut self.roots.$field, value);
            } )?

            $( #[ $inline ] )?
            pub fn $get(&self) -> &[$ty] {
                self.tables
                    .$field
                    .span_or_empty(self.roots.$field)
            }
        )*
    };
}
use crate::product_operand::ProductOperandApplication;
pub(crate) use typed_tree_table;

macro_rules! declaration_columns {
    ($( $(#[$attr:meta])* $name:ident($node:ident: $node_ty:ty) -> &[$ty:ty]
        = $column:ident.$span:ident $( [ $push:ident ] )? ; )*) => {
        $(
            $(#[$attr])*
            pub fn $name(&self, $node: &$node_ty) -> &[$ty] {
                self.$column.span_or_empty($node.$span)
            }

            $(
                pub fn $push(&mut self, $node: &mut $node_ty, value: $ty) {
                    self.$column.append_to_span(&mut $node.$span, value);
                }
            )?
        )*
    };

    ($( append $push:ident($node:ident: $node_ty:ty, $ty:ty)
        = $column:ident.$span:ident ; )*) => {
        $(
            pub fn $push(&mut self, $node: &mut $node_ty, value: $ty) {
                self.$column.append_to_span(&mut $node.$span, value);
            }
        )*
    };
}
pub(crate) use declaration_columns;

macro_rules! span_columns {
    ($( $arena:ident($ty:ty)
        reserve $reserve:ident($message:literal)
        set $set:ident
        $( push $push:ident )? ; )*) => {
        $(
            pub fn $reserve(&mut self, count: u32) -> HandleSpan<$ty> {
                self.$arena.insert_many(
                    std::iter::repeat_with(<$ty as Default>::default)
                        .take(usize::try_from(count).expect($message)),
                )
            }

            pub fn $set(&mut self, span: HandleSpan<$ty>, offset: u32, value: $ty) {
                *self.$arena.get_mut(span.handle_at(offset)) = value;
            }

            $(
                pub fn $push(&mut self, span: &mut HandleSpan<$ty>, value: $ty) {
                    self.$arena.append_to_span(span, value);
                }
            )?
        )*
    };
}
pub(crate) use span_columns;

pub(crate) mod authored_targets {
    use super::TypedTrees;
    use symbols::SymbolHandle;

    impl TypedTrees {
        pub fn authored_machine_target(
            &self,
            mut symbol: SymbolHandle,
        ) -> Result<Option<&str>, &'static str> {
            for _ in 0..=self.machine_specializations.len() + self.machines().len() {
                let mut machines = self
                    .machines()
                    .iter()
                    .filter(|machine| machine.symbol == symbol);
                let machine = machines
                    .next()
                    .ok_or("authored target has no exact machine")?;
                if machines.next().is_some() {
                    return Err("authored target has repeated machine coordinates");
                }
                let mut members = self
                    .target_machine_families
                    .iter()
                    .flat_map(|family| &family.members)
                    .filter(|member| member.machine == symbol);
                if let Some(member) = members.next() {
                    if members.next().is_some() || member.target.as_str().is_empty() {
                        return Err("machine has no unique authored target member");
                    }
                    return Ok(Some(member.target.as_str()));
                }
                let mut origins = self
                    .machine_specializations
                    .iter()
                    .filter(|specialization| specialization.instance == symbol);
                if let Some(origin) = origins.next() {
                    if origins.next().is_some() {
                        return Err("machine has repeated specialization origins");
                    }
                    if origin.template != symbol {
                        symbol = origin.template;
                        continue;
                    }
                }
                if let Some(target) = machine.target.as_ref() {
                    if self.declared_outside_product_scope(machine.symbol) {
                        return Ok(Some(target.as_str()));
                    }
                    return Err("target-tagged machine has no authored family member");
                }
                return Ok(None);
            }
            Err("authored target machine origins contain a cycle")
        }

        fn declared_outside_product_scope(&self, symbol: SymbolHandle) -> bool {
            self.symbols
                .symbol_provenance_source_span(symbol)
                .and_then(|span| self.symbols.source_file(span))
                .is_some_and(|file| file.dependency_scope != source::DependencyScope::Product)
        }
    }
}
pub(crate) mod build_selections {
    use crate::TypedTrees;
    use crate::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
    use crate::types::{TypeReferenceHandle, TypeReferenceNode};
    use symbols::SymbolHandle;

    impl TypedTrees {
        pub fn build_provider_requirement_scope(
            &self,
            reference: TypeReferenceHandle,
            lexical_owner: SymbolHandle,
        ) -> Result<AppliedTraitScope<'_>, &'static str> {
            if !reference.is_valid() {
                return Err("provider readiness lost its requirement application");
            }
            let (symbol, arguments, lifetimes) =
                match self.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::Named { symbol, .. } => (*symbol, &[][..], &[][..]),
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        lifetime_arguments,
                        ..
                    } => (
                        *base_symbol,
                        self.type_reference_table.type_reference_handles(*arguments),
                        lifetime_arguments.as_slice(),
                    ),
                    _ => return Err("provider readiness requires a boundary trait application"),
                };
            let declaration = self
                .trait_definition_by_symbol(symbol)
                .filter(|declaration| declaration.is_boundary)
                .ok_or("provider readiness requires a known boundary trait")?;
            let scope = AppliedTraitScope::new(
                self,
                declaration,
                TraitApplicationArguments {
                    lexical_owner,
                    enclosing_data_applications: &[],
                    arguments,
                    lifetimes,
                },
            )
            .ok_or("provider readiness requires every trait argument and lifetime")?;
            self.require_receiving_trait_operands(&scope)?;
            Ok(scope)
        }
    }
}
pub mod callable_signature_shapes;
pub mod declaration_projection;
mod declared_bindings;
pub mod deferred_refusals;
pub mod references;
pub mod service_declarations;
pub use service_declarations::provider_declarations;
mod access_views_and_commitments;
pub use access_views_and_commitments::constant_recipes;
pub(crate) use access_views_and_commitments::{access_views, commitments};
pub(crate) mod callable_applications;
mod calls_and_control_flow;
pub use calls_and_control_flow::{calls, control_flow, names};
mod data_applications;
mod declaration_tables_and_index;
pub use declaration_tables_and_index::declaration_index;
pub(crate) use declaration_tables_and_index::{
    data_definitions, declaration_tables, machine_declarations, machine_parameter_signatures,
    spelled_declarations, type_parameters, type_reference_queries,
};
pub mod declaration_authority;
mod declaration_reflection;
pub mod declarations;
pub use declaration_reflection::reflection;

mod evidence_and_inspection;
mod operator_declarations;
mod product_and_trait_declarations;
pub use evidence_and_inspection::{evidence, inspection};
pub mod opaque_representations;
pub mod type_identity;
pub mod type_system;
pub mod values;
pub(crate) mod canonical_values {
    use crate::TypedTrees;
    use crate::expression::{ExpressionNode, TableStructLiteral, TableStructLiteralField};
    use crate::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};

    pub(crate) fn materialize(
        program: &mut TypedTrees,
        declared_type: TypeReferenceHandle,
        value: &DecodedCanonicalConstValue,
    ) -> Option<ExpressionNode> {
        match value {
            DecodedCanonicalConstValue::Float { .. } => None,
            DecodedCanonicalConstValue::Integer { type_name, value } => {
                let primitive = program.type_reference_table.primitive_type(declared_type)?;
                if *type_name != primitive.name() {
                    return None;
                }
                let literal = IntegerLiteral::from_parts(
                    *value < 0,
                    IntegerRadix::Decimal,
                    &value.unsigned_abs().to_string(),
                )
                .ok()?;
                integer_value(program, declared_type, literal)
            }
            DecodedCanonicalConstValue::Boolean(value) => {
                (program.type_reference_table.primitive_type(declared_type)
                    == Some(PrimitiveType::Bool))
                .then_some(ExpressionNode::Boolean(*value))
            }
            DecodedCanonicalConstValue::Array { type_name, values } => {
                if *type_name != program.display_type_reference(declared_type) {
                    return None;
                }
                let declared_type = program.unwrapped_type_reference(declared_type)?;
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } = program.type_reference_node(declared_type).clone()
                else {
                    return None;
                };
                if length != values.len() {
                    return None;
                }
                let mut elements = Vec::with_capacity(length);
                for value in values {
                    let element = materialize(program, element_type, value)?;
                    elements.push(program.expression_table.insert(element));
                }
                Some(ExpressionNode::ArrayLiteral(
                    program.expression_table.insert_expression_handles(elements),
                ))
            }
            DecodedCanonicalConstValue::Record { type_name, fields } => {
                record(program, declared_type, type_name, None, fields)
            }
            DecodedCanonicalConstValue::Variant {
                type_name,
                case_name,
                fields,
            } => record(program, declared_type, type_name, Some(case_name), fields),
        }
    }

    fn record(
        program: &mut TypedTrees,
        declared_type: TypeReferenceHandle,
        type_name: &str,
        case_name: Option<&str>,
        fields: &[(String, DecodedCanonicalConstValue)],
    ) -> Option<ExpressionNode> {
        let declared_type = program.unwrapped_type_reference(declared_type)?;
        let symbol = &program
            .type_reference_node(declared_type)
            .as_named_symbol()?;
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol.is_valid() && definition.symbol == *symbol)?;
        if definition.name.as_str() != type_name {
            return None;
        }
        let mut literal = TableStructLiteral {
            type_name: definition.name.clone(),
            type_symbol: definition.symbol,
            ..TableStructLiteral::default()
        };
        let declared_fields = if let Some(case_name) = case_name {
            let variant = program.data_members(definition).iter().find_map(|member| {
                let variant = member.as_variant()?;
                (variant.name.as_str() == case_name).then_some(variant)
            })?;
            literal.case_name = Some(variant.name.clone());
            literal.case_symbol = Some(variant.symbol);
            program.data_payload_fields(variant).to_vec()
        } else {
            program
                .data_members(definition)
                .iter()
                .map(|member| {
                    let field = member.as_field()?;
                    Some(field.clone())
                })
                .collect::<Option<Vec<_>>>()?
        };
        if declared_fields.len() != fields.len() {
            return None;
        }
        let mut materialized = Vec::with_capacity(fields.len());
        for (declared, (name, value)) in declared_fields.iter().zip(fields) {
            if declared.name.as_str() != name || !declared.symbol.is_valid() {
                return None;
            }
            let value = materialize(program, declared.type_reference, value)?;
            materialized.push(TableStructLiteralField {
                name: declared.name.clone(),
                field_symbol: declared.symbol,
                value: program.expression_table.insert(value),
            });
        }
        literal.fields = program.expression_table.insert_struct_fields(materialized);
        Some(ExpressionNode::StructLiteral(literal))
    }

    fn integer_value(
        program: &TypedTrees,
        declared_type: TypeReferenceHandle,
        literal: IntegerLiteral,
    ) -> Option<ExpressionNode> {
        let primitive = program.type_reference_table.primitive_type(declared_type)?;
        let landed_type = primitive.landed_landing()?;
        Some(ExpressionNode::Integer(
            literal.with_landing(IntegerLanding {
                landed_type,
                domain: program
                    .type_reference_table
                    .arithmetic_domain(declared_type),
            }),
        ))
    }

    impl TypedTrees {
        pub fn materialize_canonical_const_value(
            &mut self,
            carrier: TypeReferenceHandle,
            value: &DecodedCanonicalConstValue,
        ) -> Option<ExpressionNode> {
            materialize(self, carrier, value)
        }
    }
}
pub use callable_applications::{
    AppliedMachineCallable, AppliedMachineCallableId, CallableContractScope, CallableSubstitution,
    ClosedRequirementApplication, RequirementLocalArgument,
};
pub(crate) mod frozen {
    use super::TypedTrees;
    use crate::service_declarations::signatures::PreparedBoundarySignatures;
    use std::sync::{Arc, OnceLock};

    /// Exact invocation-local custody of one immutable typed source owner.
    /// This is an association, never a checking verdict or a content identity.
    #[derive(Clone, Debug)]
    pub struct ProductSourceIdentity(std::sync::Weak<FrozenTypedTrees>);

    impl ProductSourceIdentity {
        pub fn new(program: &Arc<FrozenTypedTrees>) -> Self {
            Self(Arc::downgrade(program))
        }

        pub fn same_source(&self, other: &Self) -> bool {
            std::sync::Weak::ptr_eq(&self.0, &other.0)
        }

        pub fn is_for(&self, program: &Arc<FrozenTypedTrees>) -> bool {
            self.same_source(&Self::new(program))
        }
    }

    impl PartialEq for ProductSourceIdentity {
        fn eq(&self, other: &Self) -> bool {
            self.same_source(other)
        }
    }

    impl Eq for ProductSourceIdentity {}

    #[derive(Clone, Default)]
    pub struct FrozenTypedTrees(TypedTrees, OnceLock<Arc<PreparedBoundarySignatures>>);

    impl FrozenTypedTrees {
        pub fn boundary_signatures(&self) -> &Arc<PreparedBoundarySignatures> {
            self.1
                .get_or_init(|| Arc::new(PreparedBoundarySignatures::prepare(&self.0)))
        }

        pub fn into_inner(mut self) -> TypedTrees {
            self.0.unseal_declaration_indexes();
            self.0
        }
    }

    impl From<TypedTrees> for FrozenTypedTrees {
        fn from(mut program: TypedTrees) -> Self {
            program.seal_declaration_indexes();
            Self(program, OnceLock::new())
        }
    }

    impl std::fmt::Debug for FrozenTypedTrees {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_tuple("FrozenTypedTrees")
                .field(&self.0)
                .finish()
        }
    }

    impl PartialEq for FrozenTypedTrees {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl Eq for FrozenTypedTrees {}

    impl std::ops::Deref for FrozenTypedTrees {
        type Target = TypedTrees;

        fn deref(&self) -> &TypedTrees {
            &self.0
        }
    }

    impl AsRef<TypedTrees> for FrozenTypedTrees {
        fn as_ref(&self) -> &TypedTrees {
            &self.0
        }
    }

    impl compiler_storage::Record for FrozenTypedTrees {
        fn write(
            &self,
            output: &mut compiler_storage::RecordWriter,
        ) -> Result<(), compiler_storage::RecordError> {
            compiler_storage::Record::write(&self.0, output)
        }
        fn read<Context>(
            input: &mut compiler_storage::RecordReader<'_, Context>,
        ) -> Result<Self, compiler_storage::RecordError> {
            <TypedTrees as compiler_storage::Record>::read(input).map(Self::from)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn source_identity_rejects_equal_content_in_another_allocation() {
            let owner = Arc::new(FrozenTypedTrees::from(TypedTrees::default()));
            let other = Arc::new(owner.as_ref().clone());
            let identity = ProductSourceIdentity::new(&owner);
            assert!(identity.is_for(&owner));
            assert!(!identity.is_for(&other));
            assert_ne!(identity, ProductSourceIdentity::new(&other));
        }

        #[test]
        fn source_identity_does_not_keep_the_source_alive() {
            let owner = Arc::new(FrozenTypedTrees::from(TypedTrees::default()));
            let weak = Arc::downgrade(&owner);
            let identity = ProductSourceIdentity::new(&owner);
            let retained = identity.clone();
            drop(owner);
            assert!(weak.upgrade().is_none());
            assert_eq!(identity, retained);
        }

        #[test]
        fn immutable_clones_share_census_but_reopened_source_does_not() {
            let frozen = FrozenTypedTrees::from(TypedTrees::default());
            let census = Arc::clone(frozen.boundary_signatures());
            let cloned = frozen.clone();
            assert!(Arc::ptr_eq(&census, cloned.boundary_signatures()));
            let reopened = FrozenTypedTrees::from(frozen.into_inner());
            assert!(!Arc::ptr_eq(&census, reopened.boundary_signatures()));
        }

        #[test]
        fn derived_census_does_not_change_program_equality() {
            let frozen = FrozenTypedTrees::from(TypedTrees::default());
            let unprepared = frozen.clone();
            frozen.boundary_signatures();
            assert_eq!(frozen, unprepared);
        }
    }
}
pub use data_definitions::DataDefinitionTable;
pub use frozen::{FrozenTypedTrees, ProductSourceIdentity};
pub use machine_declarations::MachineDeclarationTable;
pub use machine_parameter_signatures::MachineParameterSignatureQueries;
pub use spelled_declarations::SpelledDeclarationTable;
pub use type_parameters::TypeParameterTable;
pub(crate) mod run_custody {
    use crate::TypedTrees;
    use diagnostics::Diagnostic;
    use language_semantics::MachineSupplyMode;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EmptyRunReleaseDeclaration {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub template: SymbolHandle,
    }

    impl EmptyRunReleaseDeclaration {
        pub fn authenticate(
            program: &TypedTrees,
            symbol: SymbolHandle,
        ) -> Result<Option<Self>, Diagnostic> {
            let Some(machine) = program
                .machine(symbol)
                .or_else(|| program.machine_holding_state(symbol))
            else {
                return Ok(None);
            };
            let template = program
                .machine_specialization(machine.symbol)
                .map_or(machine.symbol, |application| application.template);
            if program.symbols.core_source(template)
                != Some(language_core::core_vocabulary::CoreSource::Run)
                || program
                    .normalized_hermetic_symbol_identity(template)
                    .ok()
                    .as_deref()
                    != Some("toolchain::Run::retire_empty")
            {
                return Ok(None);
            }
            let malformed = |reason: &str| Diagnostic::error(format!("empty-run release {reason}"));
            if machine.body_is_present
                || machine.supply_mode != MachineSupplyMode::AdmissionClaim
                || !machine.lifetime_parameters.is_empty()
                || machine.spelling.is_some()
                || machine.suspends
                || machine.blocks
                || machine.target.is_some()
                || machine.authored_target.is_some()
                || machine.target_family_interface
                || !program.machine_trait_conformances(machine).is_empty()
                || !machine.invokes.is_empty()
                || machine.service_reach_is_installation_bound
            {
                return Err(malformed(&format!(
                    "changed its core envelope (body={}, supply={:?}, lifetimes={}, spelling={}, suspends={}, blocks={}, target={}, authored_target={}, family={}, conformances={}, invokes={}, installed_reach={})",
                    machine.body_is_present,
                    machine.supply_mode,
                    machine.lifetime_parameters.len(),
                    machine.spelling.is_some(),
                    machine.suspends,
                    machine.blocks,
                    machine.target.is_some(),
                    machine.authored_target.is_some(),
                    machine.target_family_interface,
                    program.machine_trait_conformances(machine).len(),
                    machine.invokes.len(),
                    machine.service_reach_is_installation_bound,
                )));
            }
            let [state] = program.machine_states(machine) else {
                return Err(malformed("must retain exactly one entry state"));
            };
            let [parameter] = program.state_parameters(state) else {
                return Err(malformed("must retain exactly one run parameter"));
            };
            if (symbol != machine.symbol && symbol != state.symbol)
                || parameter.is_const
                || parameter.is_mutable
                || parameter.relevance.is_erased()
            {
                return Err(malformed(&format!(
                    "changed its entry parameter (entry={}, const={}, mutable={}, erased={})",
                    symbol == machine.symbol || symbol == state.symbol,
                    parameter.is_const,
                    parameter.is_mutable,
                    parameter.relevance.is_erased(),
                )));
            }
            Ok(Some(Self {
                machine: machine.symbol,
                state: state.symbol,
                template,
            }))
        }
    }
}
pub use run_custody::EmptyRunReleaseDeclaration;
pub(crate) mod selection_exclusions {
    use crate::AuthoredDeclarationSelectionOccurrenceId;

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct AuthoredSelectionExclusions {
        occurrences: Box<[AuthoredDeclarationSelectionOccurrenceId]>,
        fingerprint: u64,
    }

    impl From<Vec<AuthoredDeclarationSelectionOccurrenceId>> for AuthoredSelectionExclusions {
        fn from(mut occurrences: Vec<AuthoredDeclarationSelectionOccurrenceId>) -> Self {
            occurrences.sort_unstable();
            occurrences.dedup();
            let fingerprint = occurrences
                .iter()
                .fold(occurrences.len() as u64, |fingerprint, occurrence| {
                    fingerprint.rotate_left(7) ^ occurrence.ordinal()
                });
            Self {
                occurrences: occurrences.into_boxed_slice(),
                fingerprint,
            }
        }
    }

    impl AuthoredSelectionExclusions {
        pub fn contains(&self, occurrence: AuthoredDeclarationSelectionOccurrenceId) -> bool {
            self.occurrences.binary_search(&occurrence).is_ok()
        }

        pub fn into_occurrences(self) -> Vec<AuthoredDeclarationSelectionOccurrenceId> {
            self.occurrences.into_vec()
        }

        pub(crate) fn fingerprint(&self) -> u64 {
            self.fingerprint
        }
    }

    #[cfg(test)]
    mod tests {
        use super::AuthoredSelectionExclusions;
        use crate::{
            AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
            AuthoredDeclarationSelections,
        };
        use source::SourceSpan;
        use symbols::SymbolHandle;

        #[test]
        fn exclusion_membership_and_fingerprint_are_published_together() {
            let mut ledger = AuthoredDeclarationSelections::default();
            let occurrences = (0..4)
                .map(|_| {
                    ledger
                        .record_resolved(
                            SourceSpan::default(),
                            AuthoredDeclarationSelectionExposure::PrivateImplementation,
                            AuthoredDeclarationSelectionKind::MemberAccess,
                            SymbolHandle::from_arena_index(1),
                        )
                        .expect("valid symbol and available ledger capacity")
                })
                .collect::<Vec<_>>();
            let mut exclusions = AuthoredSelectionExclusions::from(vec![
                occurrences[2],
                occurrences[0],
                occurrences[2],
            ]);
            let expected = [occurrences[0], occurrences[2]];
            let expected_fingerprint = expected
                .iter()
                .fold(expected.len() as u64, |fingerprint, occurrence| {
                    fingerprint.rotate_left(7) ^ occurrence.ordinal()
                });
            assert_eq!(exclusions.occurrences.as_ref(), expected.as_slice());
            assert_eq!(exclusions.fingerprint(), expected_fingerprint);
            assert!(exclusions.contains(occurrences[0]));
            assert!(!exclusions.contains(occurrences[1]));
            assert_eq!(exclusions.clone(), exclusions);

            let mut replacement = std::mem::take(&mut exclusions).into_occurrences();
            assert_eq!(exclusions.fingerprint(), 0);
            assert!(!exclusions.contains(occurrences[0]));
            replacement[1] = occurrences[3];
            exclusions = replacement.into();
            assert!(exclusions.contains(occurrences[3]));
            assert!(!exclusions.contains(occurrences[2]));
            assert_ne!(exclusions.fingerprint(), expected_fingerprint);
            assert_eq!(
                AuthoredSelectionExclusions::from(Vec::new()),
                AuthoredSelectionExclusions::default(),
            );
        }
    }
}
pub(crate) mod symbol_rebinding {
    use symbols::SymbolHandle;

    type SymbolPairs = [(SymbolHandle, SymbolHandle)];

    #[derive(Clone, Copy)]
    pub struct SymbolRebinding<'scope> {
        execution: &'scope SymbolPairs,
        authored: &'scope SymbolPairs,
    }

    impl<'scope> SymbolRebinding<'scope> {
        pub fn new(execution: &'scope SymbolPairs, authored: &'scope SymbolPairs) -> Self {
            Self {
                execution,
                authored,
            }
        }

        pub(crate) fn execution(self) -> &'scope SymbolPairs {
            self.execution
        }

        pub fn authored(self) -> &'scope SymbolPairs {
            self.authored
        }
    }

    impl<'scope> From<&'scope SymbolPairs> for SymbolRebinding<'scope> {
        fn from(symbols: &'scope SymbolPairs) -> Self {
            Self::new(symbols, symbols)
        }
    }

    impl<'scope> From<&'scope Vec<(SymbolHandle, SymbolHandle)>> for SymbolRebinding<'scope> {
        fn from(symbols: &'scope Vec<(SymbolHandle, SymbolHandle)>) -> Self {
            Self::from(symbols.as_slice())
        }
    }

    impl<'scope, const N: usize> From<&'scope [(SymbolHandle, SymbolHandle); N]>
        for SymbolRebinding<'scope>
    {
        fn from(symbols: &'scope [(SymbolHandle, SymbolHandle); N]) -> Self {
            Self::from(symbols.as_slice())
        }
    }

    impl std::ops::Deref for SymbolRebinding<'_> {
        type Target = SymbolPairs;

        fn deref(&self) -> &Self::Target {
            self.execution
        }
    }
}
pub use selection_exclusions::AuthoredSelectionExclusions;
pub use symbol_rebinding::SymbolRebinding;
pub use type_reference_queries::ApplicationValueIndices;

pub use access_views::{
    AccessAtomicObservingResultContract, AccessAtomicResidentContract, AccessBackingOutcome,
    AccessBackingTransfer, AccessContentShape, AccessExecutionDeclaration, AccessFieldPlan,
    AccessLifecycleOperation, AccessLifecycleOwner, AccessLifecyclePlan, AccessLifecycleStorage,
    AccessViewPlan, AccessorPrimitive, AccessorTarget, PlanLaidFieldLayoutSource, PlanLaidGeometry,
    PlanLaidLayout, PlanLaidRealizations, PlanLaidRetiredFieldIdentity, PlanLaidSchemaArgument,
    PlanLaidSchemaField, PlanLaidSchemaFieldKind, PlanLaidTargetLayout,
    PrivateCallbackSlotApplication, ScalarAccessorDeclaration, ScalarAccessorOperation,
};
pub use commitments::{
    CapturedMachineApplication, CapturedMachineState, ClosedConformanceApplication,
    ClosedConformanceApplicationCommitment, ClosedConformanceApplicationSite,
    ClosedConformanceConstArgument, ClosedConformanceRowIdentity, DataImplementationApplication,
    MachineSpecialization, MachineSpecializationCommitment, MachineTemplateCommitment,
    StaticRequirementDispatch, TraitImplementationApplication,
    machine_specialization_matches_template_identity,
};

use arena::{Arena, Handle, HandleSpan, OwnedIndex, OwnedInterner};
use diagnostics::PhaseSnapshot;
use std::ops::{Deref, DerefMut};
use symbols::{SymbolHandle, SymbolTable};

#[derive(Debug, Clone, Copy, Eq)]
pub struct ProgramIdentity(u64);

impl ProgramIdentity {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl Default for ProgramIdentity {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl PartialEq for ProgramIdentity {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
#[record(write_scope = crate::type_identity::write_under_own_numbering)]
pub struct TypedTrees {
    pub identity: ProgramIdentity,
    pub product_declaration_frontier: Option<std::sync::Arc<TypedTrees>>,
    pub excluded_authored_selection_occurrences: AuthoredSelectionExclusions,
    pub roots: TypedTreeRoots,
    pub tables: TypedTreeTables,
    pub symbols: SymbolTable,
    pub service_reaches: language_semantics::ServiceReachTable,
    pub service_reach_rows: language_semantics::ServiceReachRowTable,
    pub authored_service_reach_rows: Vec<signature::AuthoredServiceReachRow>,
    pub semantic_domains: language_semantics::SemanticDomainTable,
    pub applied_domain_applications: Vec<domain::AppliedDomainApplication>,
    pub applied_data_cleanup_selections: Vec<domain::AppliedDataCleanupSelection>,
    pub external_bindings: language_semantics::ExternalBindingTable,
    pub plan_laid_layouts: Vec<PlanLaidLayout>,
    pub plan_laid_target_layouts: Arena<PlanLaidTargetLayout>,
    pub plan_laid_schema_fields: Arena<PlanLaidSchemaField>,
    pub plan_laid_retired_field_identities: Arena<PlanLaidRetiredFieldIdentity>,
    pub access_view_plans: Vec<AccessViewPlan>,
    pub access_lifecycle_plans: Arena<AccessLifecyclePlan>,
    pub access_policy_invocations: Arena<source::SourceSpan>,
    pub access_borrow_factories: Vec<expression::AccessBorrowOperationRequest>,
    pub wire_placements: Arena<wire::WirePlacement>,
    pub wire_encode_obligations: Arena<wire::WireEncodeObligation>,
    pub wire_schema_plans: Vec<wire::WireSchemaPlan>,
    pub machine_specializations: Vec<MachineSpecialization>,
    pub captured_machine_applications: Arena<CapturedMachineApplication>,
    pub captured_machine_states: Arena<CapturedMachineState>,
    pub trait_implementation_applications: Arena<TraitImplementationApplication>,
    pub data_implementation_applications: Arena<DataImplementationApplication>,
    pub dynamic_slots: Vec<crate::dynamic_traits::DynamicSlotSignature>,
    pub target_machine_families: Vec<crate::machine::TargetMachineFamily>,
    /// Requests no pass served, on the declarations that hold them, ordered
    /// by declaration (`deferred_refusals.rs`).
    pub deferred_refusals: Vec<deferred_refusals::DeferredRefusal>,
    pub boundary_completion_declarations: Arena<BoundaryCompletionDeclaration>,
    pub evidence_forwardings: Vec<EvidenceForwarding>,
    pub proof_output_calls: Vec<ProofOutputCall>,
    pub declaration_indexes: declaration_index::DeclarationIndexes,
    pub ranking_expression_custody: Vec<crate::ranking::RankingExpressionCustody>,
    pub pending_const_range_endpoints:
        std::collections::HashSet<crate::expression::ExpressionHandle>,
    pub constant_recipes: Arena<constant_recipes::RetainedConstantRecipe>,
    pub constant_expression_contexts: Arena<constant_recipes::ConstantExpressionContext>,
    pub constant_recipe_closure: constant_recipes::ConstantRecipeClosure,
    pub callable_identities: OwnedInterner<crate::type_identity::CallableIdentityTable>,
    pub callable_identity_memo: OwnedIndex<crate::type_identity::CallableIdentityMemo>,
    pub type_identities: crate::type_identity::TypeIdentityTable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct BoundaryCompletionDeclaration {
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub owner: symbols::SymbolHandle,
    pub owner_path: String,
    pub requirement: symbols::SymbolHandle,
    pub requirement_path: String,
    pub completion: semantic_vocabulary::BoundaryCompletion,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct EvidenceForwarding {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub source_statement_index: usize,
    pub target: crate::name::Identifier,
    pub source: crate::name::Identifier,
    pub source_conformance: Option<symbols::SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofOutputCall {
    pub machine_symbol: symbols::SymbolHandle,
    pub state_symbol: symbols::SymbolHandle,
    pub statement_index: usize,
    pub source_statement_index: usize,
    pub runtime_call_statement_index: Option<usize>,
    pub bindings: Box<[ProofOutputSelector]>,
    pub call: crate::expression::ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ProofOutputSelector {
    pub output_field: crate::name::Identifier,
    pub binding: crate::name::Identifier,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DataApplicationView {
    definition: Handle<data::DataDefinition>,
    is_closed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct DesignatedStaticTypeArgument {
    pub owner: symbols::SymbolHandle,
    pub reference: types::TypeReferenceHandle,
    pub source_span: source::SourceSpan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TypedTreeTables {
    pub designated_static_type_arguments: Arena<DesignatedStaticTypeArgument>,
    pub retained_product_application_handles: Arena<Handle<ProductOperandApplication>>,
    pub product_input_selections: Arena<crate::product_operand::ProductInputSelection>,
    pub product_input_argument_steps: Arena<crate::product_operand::ProductArgumentStep>,
    pub product_operand_applications: Arena<ProductOperandApplication>,
    authored_declaration_selections:
        language_semantics::declaration_selection::AuthoredDeclarationSelections,
    pub const_declarations: Arena<crate::constant::ConstDeclaration>,
    pub data_definitions: DataDefinitionTable,
    pub type_aliases: Arena<crate::type_aliases::TypeAliasDefinition>,
    pub data_application_definitions: Arena<DataApplicationView>,
    pub data_type_parameters: TypeParameterTable,
    pub declared_type_equations: Arena<machine::DeclaredTypeEquation>,
    pub static_type_contexts: crate::type_system::type_predicates::StaticTypeContexts,
    pub data_members: Arena<data::DataMember>,
    pub data_payload_fields: Arena<data::DataField>,
    pub domain_definitions: SpelledDeclarationTable<domain::DomainDefinition>,
    pub proof_facts: Arena<domain::ProofFact>,
    proof_fact_source_spans: Vec<Option<source::SourceSpan>>,
    pub propositions: Arena<proposition::PropositionDefinition>,
    pub proposition_binders: Arena<proposition::PropositionBinder>,
    pub mathematical_definitions: Arena<mathematical::MathematicalDefinition>,
    pub mathematical_parameters: Arena<mathematical::MathematicalParameter>,
    pub mathematical_types: Arena<mathematical::MathematicalType>,
    pub domain_path_members: Arena<crate::name::Identifier>,
    pub operator_path_members: Arena<crate::name::Identifier>,
    pub machines: MachineDeclarationTable,
    pub measures: Arena<measure::MeasureDefinition>,
    pub measure_path_members: Arena<crate::name::Identifier>,
    pub operators: SpelledDeclarationTable<crate::operator::OperatorDefinition>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_trait_conformances: Arena<machine::TraitConformance>,
    pub machine_states: Arena<crate::state::State>,
    pub state_parameters: Arena<signature::StateParameter>,
    pub traits: Arena<trait_definition::TraitDefinition>,
    pub conformances: Arena<trait_definition::Conformance>,
    pub trait_requirements: Arena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: Arena<signature::StateSignature>,
    pub(crate) applied_callable_signatures: callable_applications::AppliedCallableTable,
    pub(crate) callable_substitutions: Arena<callable_applications::CallableSubstitution>,
    pub(crate) trait_application_frames: Arena<trait_definition::RetainedTraitApplicationFrame>,
    pub(crate) callable_argument_identities: Arena<crate::type_identity::NormalizedTypeIdentity>,
    pub(crate) callable_arguments: Arena<callable_applications::RequirementLocalArgument>,
    pub(crate) callable_lifetime_substitutions:
        Arena<crate::static_applications::StaticMachineLifetimeBinding>,
    pub signature_invokes: Arena<signature::AuthoredInvocation>,
    pub signature_contracts: Arena<signature::SignatureContract>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub wire_schemas: Arena<wire::WireSchema>,
    pub wire_members: Arena<wire::WireMember>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct TypedTreeRoots {
    pub retained_product_applications: HandleSpan<Handle<ProductOperandApplication>>,
    pub const_declarations: HandleSpan<crate::constant::ConstDeclaration>,
    pub data_definitions: HandleSpan<data::DataDefinition>,
    pub type_aliases: HandleSpan<crate::type_aliases::TypeAliasDefinition>,
    pub domain_definitions: HandleSpan<domain::DomainDefinition>,
    pub machines: HandleSpan<machine::Machine>,
    pub measures: HandleSpan<measure::MeasureDefinition>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub machine_token_bindings: HandleSpan<crate::operator::OperatorDefinition>,
    pub propositions: HandleSpan<proposition::PropositionDefinition>,
    pub mathematical_definitions: HandleSpan<mathematical::MathematicalDefinition>,
    pub traits: HandleSpan<trait_definition::TraitDefinition>,
    pub conformances: HandleSpan<trait_definition::Conformance>,
    pub wire_schemas: HandleSpan<wire::WireSchema>,
}

impl TypedTreeRoots {
    pub fn with_roots(
        data_definitions: HandleSpan<data::DataDefinition>,
        domain_definitions: HandleSpan<domain::DomainDefinition>,
        machines: HandleSpan<machine::Machine>,
        operators: HandleSpan<crate::operator::OperatorDefinition>,
        traits: HandleSpan<trait_definition::TraitDefinition>,
    ) -> Self {
        Self {
            retained_product_applications: HandleSpan::default(),
            const_declarations: HandleSpan::default(),
            data_definitions,
            type_aliases: HandleSpan::empty(),
            domain_definitions,
            machines,
            measures: HandleSpan::default(),
            operators,
            machine_token_bindings: HandleSpan::default(),
            propositions: HandleSpan::default(),
            mathematical_definitions: HandleSpan::default(),
            traits,
            conformances: HandleSpan::default(),
            wire_schemas: HandleSpan::default(),
        }
    }
}

impl TypedTrees {
    pub fn product_declarations(&self) -> &TypedTrees {
        self.product_declaration_frontier.as_deref().unwrap_or(self)
    }

    // Build checking may retain the Product snapshot separately from its own
    // execution graph. Prepare designated Product operands in that same owner.
    pub fn product_declarations_mut(&mut self) -> &mut TypedTrees {
        match self {
            Self {
                product_declaration_frontier: Some(frontier),
                ..
            } => std::sync::Arc::make_mut(frontier),
            original => original,
        }
    }

    pub fn machine_specialization(
        &self,
        instance: symbols::SymbolHandle,
    ) -> Option<&MachineSpecialization> {
        self.machine_specializations
            .iter()
            .find(|specialization| specialization.instance == instance)
    }

    /*
        The core vocabulary row of `symbol`'s authored declaration. A machine
        specialization keeps its template's source span but lives under a
        generated namespace, so it is resolved to the template first; the
        catalog then judges the authored declaration's file, path and bytes.
    */
    pub fn core_vocabulary(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Option<language_core::core_vocabulary::CoreVocabulary> {
        let original = self
            .machine_specialization(symbol)
            .map_or(symbol, |specialization| specialization.template);
        self.symbols.core_vocabulary(original)
    }

    pub fn checks_authored_selection(
        &self,
        selection: &language_semantics::declaration_selection::AuthoredDeclarationSelection,
    ) -> bool {
        !self
            .excluded_authored_selection_occurrences
            .contains(selection.occurrence_id())
            && (self.product_declaration_frontier.is_none()
                || self
                    .symbols
                    .source_file(selection.source_span())
                    .is_none_or(|file| {
                        file.origin.is_toolchain()
                            || file.dependency_scope == source::DependencyScope::Build
                    }))
    }

    pub fn authored_service_reach_rows_for(
        &self,
        owner: symbols::SymbolHandle,
    ) -> impl Iterator<Item = &signature::AuthoredServiceReachRow> {
        self.authored_service_reach_rows
            .iter()
            .filter(move |row| row.owner == owner)
    }

    pub fn with_roots(
        roots: TypedTreeRoots,
        tables: TypedTreeTables,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            identity: ProgramIdentity::default(),
            product_declaration_frontier: None,
            excluded_authored_selection_occurrences: AuthoredSelectionExclusions::default(),
            roots,
            tables,
            symbols,
            service_reaches: language_semantics::ServiceReachTable::default(),
            service_reach_rows: language_semantics::ServiceReachRowTable::default(),
            authored_service_reach_rows: Vec::new(),
            semantic_domains: language_semantics::SemanticDomainTable::default(),
            applied_domain_applications: Vec::new(),
            applied_data_cleanup_selections: Vec::new(),
            external_bindings: language_semantics::ExternalBindingTable::default(),
            plan_laid_layouts: Vec::new(),
            plan_laid_target_layouts: Arena::new(),
            plan_laid_schema_fields: Arena::new(),
            plan_laid_retired_field_identities: Arena::new(),
            access_view_plans: Vec::new(),
            access_lifecycle_plans: Arena::new(),
            access_policy_invocations: Arena::new(),
            access_borrow_factories: Vec::new(),
            wire_placements: Arena::new(),
            wire_encode_obligations: Arena::new(),
            wire_schema_plans: Vec::new(),
            machine_specializations: Vec::new(),
            captured_machine_applications: Arena::new(),
            captured_machine_states: Arena::new(),
            trait_implementation_applications: Arena::new(),
            data_implementation_applications: Arena::new(),
            dynamic_slots: Vec::new(),
            target_machine_families: Vec::new(),
            deferred_refusals: Vec::new(),
            boundary_completion_declarations: Arena::default(),
            evidence_forwardings: Vec::new(),
            proof_output_calls: Vec::new(),
            declaration_indexes: declaration_index::DeclarationIndexes::default(),
            ranking_expression_custody: Vec::new(),
            pending_const_range_endpoints: std::collections::HashSet::new(),
            constant_recipes: Arena::new(),
            constant_expression_contexts: Arena::new(),
            constant_recipe_closure: constant_recipes::ConstantRecipeClosure::default(),
            callable_identities: OwnedInterner::default(),
            callable_identity_memo: OwnedIndex::default(),
            type_identities: Default::default(),
        }
    }

    pub fn ranking_expression_custody_for(
        &self,
        machine: symbols::SymbolHandle,
    ) -> Option<&crate::ranking::RankingExpressionCustody> {
        self.ranking_expression_custody
            .iter()
            .find(|custody| custody.machine == machine)
    }

    pub fn snapshot(&self) -> snapshot::TypedTreesSnapshot {
        snapshot::TypedTreesSnapshot::from_typed_trees(self)
    }

    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json()
    }

    pub fn snapshot_json_pretty(&self) -> Result<String, serde_json::Error> {
        self.snapshot().to_json_pretty()
    }
}

fn proof_fact_source_span_index(handle: Handle<domain::ProofFact>) -> usize {
    usize::try_from(handle.arena_index())
        .expect("proof fact source-span index exceeds usize")
        .checked_sub(1)
        .expect("proof fact source-span handle must be valid")
}

pub fn canonical_const_declaration_value(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<language_semantics::const_value::CanonicalConstValue> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    let declaration = program.const_declaration_by_symbol(symbol)?;
    let encoding = declaration.evaluated_value.as_ref()?.encoding()?;
    let decoded = CanonicalConstValue::new("", encoding, "").decode_encoding()?;
    let carrier = match decoded {
        DecodedCanonicalConstValue::Integer { type_name, .. } => type_name,
        DecodedCanonicalConstValue::Boolean(_) => "bool".to_owned(),
        DecodedCanonicalConstValue::Array { .. }
        | DecodedCanonicalConstValue::Record { .. }
        | DecodedCanonicalConstValue::Variant { .. }
        | DecodedCanonicalConstValue::Float { .. } => {
            program.normalized_type_identity_text(declaration.declared_type)
        }
    };
    Some(CanonicalConstValue::new(
        carrier,
        encoding,
        program.symbols.name(symbol),
    ))
}

pub fn integer_const_declaration_extent(
    program: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<usize> {
    let language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. } =
        canonical_const_declaration_value(program, symbol)?.decode_encoding()?
    else {
        return None;
    };
    usize::try_from(value).ok()
}

pub fn static_const_argument_spelling(
    program: &TypedTrees,
    argument: &expression::StaticMachineArgument,
) -> Option<String> {
    match static_const_argument_value(program, argument)? {
        StaticConstArgumentValue::Integer(value) => Some(value.to_string()),
        StaticConstArgumentValue::Literal(literal) => Some(literal.to_string()),
        StaticConstArgumentValue::Canonical(value) => canonical_static_const_spelling(value),
    }
}

enum StaticConstArgumentValue<'argument> {
    Integer(i128),
    Literal(&'argument numerics::literals::IntegerLiteral),
    Canonical(language_semantics::const_value::CanonicalConstValue),
}

fn static_const_argument_value<'argument>(
    program: &TypedTrees,
    argument: &'argument expression::StaticMachineArgument,
) -> Option<StaticConstArgumentValue<'argument>> {
    use language_semantics::const_value::{CanonicalConstValue, boolean_literal_spelling};

    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
    {
        return None;
    }
    if let Some(literal) = &argument.const_literal {
        return Some(
            literal
                .value_i64()
                .map(i128::from)
                .or_else(|| literal.value_u64().map(i128::from))
                .or_else(|| literal.decimal_value_i128())
                .map_or(
                    StaticConstArgumentValue::Literal(literal),
                    StaticConstArgumentValue::Integer,
                ),
        );
    }
    if argument.symbol.is_valid() {
        let value = canonical_const_declaration_value(program, argument.symbol)?;
        return Some(StaticConstArgumentValue::Canonical(value));
    }
    let [name] = argument.path.as_ref() else {
        return None;
    };
    if let Some(value) = boolean_literal_spelling(name.as_str()) {
        return Some(StaticConstArgumentValue::Canonical(
            CanonicalConstValue::boolean(value),
        ));
    }
    CanonicalConstValue::from_atom(name.as_str()).map(StaticConstArgumentValue::Canonical)
}

pub fn static_const_value_identity(
    identity: &language_semantics::const_value::CanonicalConstIdentity,
) -> Option<String> {
    language_semantics::const_value::CanonicalConstValue::new(
        identity.type_name.clone(),
        identity.encoding.clone(),
        "",
    )
    .application_identity()
}

fn canonical_static_const_spelling(
    value: language_semantics::const_value::CanonicalConstValue,
) -> Option<String> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    Some(match value.decode_encoding()? {
        DecodedCanonicalConstValue::Integer { value, .. } => value.to_string(),
        DecodedCanonicalConstValue::Boolean(value) => CanonicalConstValue::boolean(value).atom(),
        _ => value.atom(),
    })
}

impl PhaseSnapshot for TypedTrees {
    type Snapshot = snapshot::TypedTreesSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        TypedTrees::snapshot(self)
    }
}

impl Deref for TypedTrees {
    type Target = TypedTreeTables;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tables
    }
}

impl DerefMut for TypedTrees {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.invalidate_state_declaration_index();
        &mut self.tables
    }
}

pub(crate) fn remapped<'scope>(
    symbol: SymbolHandle,
    symbols: impl Into<SymbolRebinding<'scope>>,
) -> SymbolHandle {
    symbols
        .into()
        .iter()
        .find_map(|(source, target)| (*source == symbol).then_some(*target))
        .unwrap_or(symbol)
}

pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            out.push('\\');
        }
        out.push(character);
    }
    out
}

impl compiler_storage::Record for ProgramIdentity {
    fn write(
        &self,
        _: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        Ok(())
    }
    fn read<Context>(
        _: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        Ok(Self::default())
    }
}

pub use calls::{boundary, dynamic_traits, finite_family, signature, static_applications};
pub use control_flow::{machine, state, statement};
pub use declarations::{
    boundary_completions, calling_vocabulary, constant, data, domain, float_providers, invocations,
    layout_storage, measure, operator, product_operand, trait_definition, type_aliases, visibility,
    wire,
};
pub use evidence::{byte_predicates, mathematical, proof_only, proposition, ranking};
pub use inspection::snapshot;
pub use language_semantics::declaration_selection::{
    AuthoredDeclarationSelection, AuthoredDeclarationSelectionExposure,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
    AuthoredDeclarationSelectionTarget, AuthoredDeclarationSelections,
    ResolvedAuthoredDeclarationSelection,
};
pub use names::{identity, name};
pub use snapshot::TypedTreesSnapshot;
pub use type_system::type_predicates::ProvenTypeContext;
pub use type_system::{CaseMembershipTypeFacts, closed_numeric, types};
pub use values::const_integer;
pub use values::expression;
pub use values::subtrees::{SubtreeEdges, SubtreeVisit};

pub mod source_subjects;

pub mod typed_trees {
    pub use crate::*;
}
