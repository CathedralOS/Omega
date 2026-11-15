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
  - `lib.rs`: `TypedTrees`, `FrozenTypedTrees`, `TypedTreeRoots`, `TypedTreeTables`, `boundary_values`, and `source_review`.
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
    use crate::types::TypeReferenceHandle;

    #[derive(Clone, Default)]
    pub struct BoundaryTypeBinding {
        pub parameter_symbol: symbols::SymbolHandle,
        pub parameter_name: String,
        pub actual: TypeReferenceHandle,
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
                let mut origins = self.machine_specializations.for_instance(symbol);
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
pub mod compiler_intrinsics;
pub mod declaration_projection;
mod declared_bindings;
pub mod deferred_refusals;
mod machine_specializations;
pub use machine_specializations::MachineSpecializations;
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
            DecodedCanonicalConstValue::Integer {
                integer_type,
                value,
            } => {
                let primitive = program.primitive_type_reference(declared_type)?;
                if Some(*integer_type) != primitive.landed_landing() {
                    return None;
                }
                let literal = IntegerLiteral::from_parts(
                    value.is_negative(),
                    IntegerRadix::Decimal,
                    &value.abs().to_string(),
                )
                .ok()?;
                integer_value(program, declared_type, literal)
            }
            DecodedCanonicalConstValue::Boolean(value) => {
                (program.primitive_type_reference(declared_type) == Some(PrimitiveType::Bool))
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
        let primitive = program.primitive_type_reference(declared_type)?;
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

    fn matches_materialized(
        program: &TypedTrees,
        carrier: TypeReferenceHandle,
        expression: crate::expression::ExpressionHandle,
        value: &DecodedCanonicalConstValue,
    ) -> bool {
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        match (program.expression_table.expression(expression), value) {
            (ExpressionNode::Boolean(actual), DecodedCanonicalConstValue::Boolean(expected)) => {
                actual == expected
            }
            (
                ExpressionNode::Integer(actual),
                DecodedCanonicalConstValue::Integer {
                    integer_type,
                    value,
                },
            ) => {
                actual.landing()
                    == Some(IntegerLanding {
                        landed_type: *integer_type,
                        domain: program.type_reference_table.arithmetic_domain(carrier),
                    })
                    && actual.value_bignum().as_ref() == Some(value)
            }
            (
                ExpressionNode::ArrayLiteral(actual),
                DecodedCanonicalConstValue::Array { type_name, values },
            ) => {
                let Some(underlying) = program.unwrapped_type_reference(carrier) else {
                    return false;
                };
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } = program.type_reference_node(underlying)
                else {
                    return false;
                };
                let expressions = program.expression_table.expression_handles(*actual);
                *type_name == program.display_type_reference(carrier)
                    && *length == values.len()
                    && expressions.len() == values.len()
                    && expressions.iter().zip(values).all(|(expression, value)| {
                        matches_materialized(program, *element_type, *expression, value)
                    })
            }
            (
                ExpressionNode::StructLiteral(actual),
                DecodedCanonicalConstValue::Record { type_name, fields },
            ) => {
                let Some(underlying) = program.unwrapped_type_reference(carrier) else {
                    return false;
                };
                let Some(symbol) = program.type_reference_node(underlying).as_named_symbol() else {
                    return false;
                };
                let Some(record) = program.data_definition_by_symbol(symbol) else {
                    return false;
                };
                let members = program.data_members(record);
                let expressions = program.expression_table.struct_fields(actual.fields);
                actual.type_symbol == symbol
                    && actual.case_symbol.is_none()
                    && actual.case_name.is_none()
                    && type_name == record.name.as_str()
                    && members.len() == fields.len()
                    && expressions.len() == fields.len()
                    && members.iter().zip(expressions).zip(fields).all(
                        |((member, expression), (name, value))| {
                            member.as_field().is_some_and(|field| {
                                field.symbol == expression.field_symbol
                                    && field.name == expression.name
                                    && field.name.as_str() == name
                                    && matches_materialized(
                                        program,
                                        field.type_reference,
                                        expression.value,
                                        value,
                                    )
                            })
                        },
                    )
            }
            _ => false,
        }
    }

    impl TypedTrees {
        pub fn canonical_const_materialization_matches(
            &self,
            carrier: TypeReferenceHandle,
            expression: crate::expression::ExpressionHandle,
            value: &DecodedCanonicalConstValue,
        ) -> bool {
            matches_materialized(self, carrier, expression, value)
        }

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
    ClosedRequirementApplication, NamedConformanceCaptureIndex, RequirementLocalArgument,
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
    }

    impl From<Vec<AuthoredDeclarationSelectionOccurrenceId>> for AuthoredSelectionExclusions {
        fn from(mut occurrences: Vec<AuthoredDeclarationSelectionOccurrenceId>) -> Self {
            occurrences.sort_unstable();
            occurrences.dedup();
            Self {
                occurrences: occurrences.into_boxed_slice(),
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
        fn exclusion_membership_follows_normalized_occurrences() {
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
            assert_eq!(exclusions.occurrences.as_ref(), expected.as_slice());
            assert!(exclusions.contains(occurrences[0]));
            assert!(!exclusions.contains(occurrences[1]));
            assert_eq!(exclusions.clone(), exclusions);

            let mut replacement = std::mem::take(&mut exclusions).into_occurrences();
            assert!(exclusions.occurrences.is_empty());
            assert!(!exclusions.contains(occurrences[0]));
            replacement[1] = occurrences[3];
            exclusions = replacement.into();
            assert!(exclusions.contains(occurrences[3]));
            assert!(!exclusions.contains(occurrences[2]));
            assert_eq!(
                exclusions.occurrences.as_ref(),
                &[occurrences[0], occurrences[3]]
            );
            assert_eq!(
                AuthoredSelectionExclusions::from(Vec::new()),
                AuthoredSelectionExclusions::default(),
            );
        }
    }
}
pub(crate) mod symbol_rebinding {
    use symbols::{SymbolHandle, SymbolMap};

    type SymbolPairs = [(SymbolHandle, SymbolHandle)];

    #[derive(Debug, Default)]
    struct IndexedPairs {
        pairs: Vec<(SymbolHandle, SymbolHandle)>,
        index: SymbolMap<SymbolHandle>,
    }

    impl IndexedPairs {
        fn insert(&mut self, source: SymbolHandle, target: SymbolHandle) -> Option<SymbolHandle> {
            if let Some(previous) = self.index.get(&source) {
                return Some(*previous);
            }
            self.index.insert(source, target);
            self.pairs.push((source, target));
            None
        }
    }

    /// Prepared lookup for the same structural execution/authored association.
    /// Copied symbols are a sparse subset of the retained program: index only
    /// those pairs, without allocating from its full symbol-number range.
    /// Pair order remains explicit; hashing never chooses declaration order.
    #[derive(Debug, Default)]
    pub struct PreparedSymbolRebinding {
        execution: IndexedPairs,
        authored: IndexedPairs,
    }

    impl PreparedSymbolRebinding {
        pub fn insert_execution(
            &mut self,
            source: SymbolHandle,
            target: SymbolHandle,
        ) -> Option<SymbolHandle> {
            self.execution.insert(source, target)
        }

        pub fn insert_authored(
            &mut self,
            source: SymbolHandle,
            target: SymbolHandle,
        ) -> Option<SymbolHandle> {
            self.authored.insert(source, target)
        }

        pub fn execution_symbol(&self, source: SymbolHandle) -> Option<SymbolHandle> {
            self.execution.index.get(&source).copied()
        }

        pub fn authored_symbol(&self, source: SymbolHandle) -> Option<SymbolHandle> {
            self.authored.index.get(&source).copied()
        }

        pub fn rebinding(&self) -> SymbolRebinding<'_> {
            SymbolRebinding {
                execution: Pairs::Indexed(&self.execution),
                authored: Pairs::Indexed(&self.authored),
            }
        }
    }

    #[derive(Clone, Copy)]
    enum Pairs<'scope> {
        Borrowed(&'scope SymbolPairs),
        Indexed(&'scope IndexedPairs),
    }

    impl<'scope> Pairs<'scope> {
        fn lookup(self, source: SymbolHandle) -> Option<SymbolHandle> {
            match self {
                Self::Borrowed(pairs) => pairs
                    .iter()
                    .find_map(|(old, new)| (*old == source).then_some(*new)),
                Self::Indexed(pairs) => pairs.index.get(&source).copied(),
            }
        }

        fn ordered(self) -> &'scope SymbolPairs {
            match self {
                Self::Borrowed(pairs) => pairs,
                Self::Indexed(pairs) => &pairs.pairs,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct SymbolRebinding<'scope> {
        execution: Pairs<'scope>,
        authored: Pairs<'scope>,
    }

    impl<'scope> SymbolRebinding<'scope> {
        pub fn new(execution: &'scope SymbolPairs, authored: &'scope SymbolPairs) -> Self {
            Self {
                execution: Pairs::Borrowed(execution),
                authored: Pairs::Borrowed(authored),
            }
        }

        pub fn lookup(self, source: SymbolHandle) -> Option<SymbolHandle> {
            self.execution.lookup(source)
        }

        pub(crate) fn execution(self) -> Self {
            Self {
                execution: self.execution,
                authored: self.execution,
            }
        }

        pub fn authored(self) -> Self {
            Self {
                execution: self.authored,
                authored: self.authored,
            }
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
            self.execution.ordered()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn prepared_rebinding_preserves_scope_order_and_first_match() {
            let declaration = SymbolHandle::from_arena_index(7);
            let formal = SymbolHandle::from_arena_index(9);
            let copied_declaration = SymbolHandle::from_arena_index(17);
            let copied_formal = SymbolHandle::from_arena_index(19);
            let mut prepared = PreparedSymbolRebinding::default();
            assert!(
                prepared
                    .insert_execution(declaration, copied_declaration)
                    .is_none()
            );
            assert!(prepared.insert_execution(formal, copied_formal).is_none());
            assert!(prepared.insert_authored(formal, copied_formal).is_none());
            assert_eq!(
                prepared.insert_execution(declaration, copied_formal),
                Some(copied_declaration)
            );
            let view = prepared.rebinding();
            assert_eq!(crate::remapped(declaration, view), copied_declaration);
            assert_eq!(crate::remapped(declaration, view.authored()), declaration);
            assert_eq!(crate::remapped(formal, view.authored()), copied_formal);
            let external = SymbolHandle::from_arena_index(1_000_000);
            assert_eq!(crate::remapped(external, view), external);
            assert_eq!(
                view.iter().copied().collect::<Vec<_>>(),
                [(declaration, copied_declaration), (formal, copied_formal)]
            );
        }
    }
}

pub use selection_exclusions::AuthoredSelectionExclusions;
pub use symbol_rebinding::{PreparedSymbolRebinding, SymbolRebinding};
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
    MachineSpecialization, StaticRequirementDispatch, TraitImplementationApplication,
};

use arena::{Arena, Handle, HandleSpan, OwnedInterner};
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
    pub applied_domain_applications: domain::AppliedDomainApplications,
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
    pub machine_specializations: MachineSpecializations,
    pub captured_machine_applications: Arena<CapturedMachineApplication>,
    pub captured_machine_states: Arena<CapturedMachineState>,
    pub trait_implementation_applications: Arena<TraitImplementationApplication>,
    pub data_implementation_applications: Arena<DataImplementationApplication>,
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
        match self.machine_instantiation(instance).ok()?? {
            callable_applications::MachineInstantiation::Specialization(application) => {
                Some(application)
            }
            callable_applications::MachineInstantiation::Capture(_) => None,
        }
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
            applied_domain_applications: Default::default(),
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
            machine_specializations: MachineSpecializations::default(),
            captured_machine_applications: Arena::new(),
            captured_machine_states: Arena::new(),
            trait_implementation_applications: Arena::new(),
            data_implementation_applications: Arena::new(),
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
        DecodedCanonicalConstValue::Integer { integer_type, .. } => integer_type.name(),
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
        &mut self.tables
    }
}

pub(crate) fn remapped<'scope>(
    symbol: SymbolHandle,
    symbols: impl Into<SymbolRebinding<'scope>>,
) -> SymbolHandle {
    symbols.into().lookup(symbol).unwrap_or(symbol)
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

pub use calls::{boundary, finite_family, signature, static_applications};
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
