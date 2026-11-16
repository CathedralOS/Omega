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
        pub actual: TypeReferenceHandle,
    }
}
pub mod callable_lifetimes;
pub mod source_names;
pub(crate) mod source_review {
    use crate::{PlanLaidTargetLayout, types::TypeReferenceHandle};
    use arena::Handle;
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use symbols::SymbolHandle;

    /// Typed occurrence identity belongs to the frozen source kept beside a
    /// publication. Spans are diagnostic positions, never this association.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SourceNode {
        Expression(crate::expression::ExpressionHandle),
        Statement(crate::statement::StatementHandle),
    }

    /// One actual emitted invocation and the typed occurrence that produced it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SourceCallCorrespondence {
        /// The lowered machine that executes this invocation.
        pub machine: semantic_vocabulary::MachineId,
        /// The original member before call-component folding.
        pub source_member: semantic_vocabulary::MachineId,
        pub site: semantic_vocabulary::InvocationSite,
        /// Combining a call cycle into one loop does not change this original
        /// source owner or its static argument environment.
        pub owner: SymbolHandle,
        pub source: SourceNode,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
    pub struct SourceTypeApplication {
        pub reference: TypeReferenceHandle,
        pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    }

    /// The canonical service coordinate paired with its source declaration.
    /// The lowering owner prepares this association; consumers retain it
    /// beside the checked module instead of comparing display identities.
    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct SourceServiceCorrespondence {
        pub service: semantic_vocabulary::ServiceId,
        pub source: language_semantics::ServiceReachId,
        pub declaration: SymbolHandle,
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
    SourceCallCorrespondence, SourceFieldCorrespondence, SourceLayoutPlanCoordinate,
    SourceLayoutRealizationCoordinate, SourceNode, SourceRecordCorrespondence,
    SourceServiceCorrespondence, SourceTypeApplication,
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
            pub fn $get(&self) -> arena::RootRows<'_, $ty> {
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
    () => {};
    (@column $(#[$attr:meta])* $name:ident($node:ident: $node_ty:ty)
        -> $rows:ty, $ty:ty = $column:ident.$span:ident $( [ $push:ident ] )? ) => {
        $(#[$attr])*
        pub fn $name(&self, $node: &$node_ty) -> $rows {
            self.$column.span_or_empty($node.$span)
        }
        $(
            pub fn $push(&mut self, $node: &mut $node_ty, value: $ty) {
                self.$column.append_to_span(&mut $node.$span, value);
            }
        )?
    };

    ($(#[$attr:meta])* $name:ident($node:ident: $node_ty:ty) -> rows[$ty:ty]
        = $column:ident.$span:ident $( [ $push:ident ] )? ; $($rest:tt)*) => {
            $crate::declaration_columns!(@column $(#[$attr])* $name($node: $node_ty)
                -> arena::RootRows<'_, $ty>, $ty = $column.$span $( [ $push ] )?);
            $crate::declaration_columns!($($rest)*);
    };

    ($(#[$attr:meta])* $name:ident($node:ident: $node_ty:ty) -> &[$ty:ty]
        = $column:ident.$span:ident $( [ $push:ident ] )? ; $($rest:tt)*) => {
            $crate::declaration_columns!(@column $(#[$attr])* $name($node: $node_ty)
                -> &[$ty], $ty = $column.$span $( [ $push ] )?);
            $crate::declaration_columns!($($rest)*);
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
mod callable_copies;
pub mod callable_signature_shapes;
pub mod callable_values;
pub mod compiler_intrinsics;
pub mod contract_structure;
pub mod declaration_projection;
mod declared_bindings;
pub mod deferred_refusals;
mod machine_specializations;
pub mod native_locators;
pub use machine_specializations::MachineSpecializations;
pub mod references;
pub mod runtime_callable_identity;
pub mod service_declarations;
pub mod source_call_occurrences;
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
    use crate::data::{DataDefinition, DataField, DataVariant};
    use crate::expression::{
        ExpressionNode, TableCastExpression, TableStructLiteral, TableStructLiteralField,
    };
    use crate::types::{
        DomainConstraint, DomainConstraintSubject, FixedArrayLength, PrimitiveType,
        TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };
    use language_core::cast_form::{CastEntry, CastEntryKind, CastForm};
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use numerics::literals::{IntegerLanding, IntegerLiteral, IntegerRadix};

    /// The declared domains a carrier qualifies its value with, innermost
    /// first, and the carrier beneath them.
    ///
    /// An evaluated snapshot holds a value's bits and declared carrier, not
    /// the evidence its invocation checked for each domain on it. The
    /// materializer re-establishes each domain with an `as +D` entry, so the
    /// reader of the materialized value owes that domain's ordinary issuer
    /// and predicate checks (domains.md, "Domain-only casts"); the
    /// evaluator refuses a domain whose issuance is restricted before this
    /// point, since a constant grants no authority (constants.md). An
    /// arithmetic policy stays on the carrier, where integer landing reads
    /// it; any other constraint, alone or beside a declared domain, has no
    /// materialization and yields `None`.
    pub(crate) fn declared_domain_qualifications(
        program: &TypedTrees,
        carrier: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Vec<&DomainConstraint>)> {
        let mut base = carrier;
        let mut outermost_first = Vec::new();
        while base.is_valid() {
            let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_node(base)
            else {
                break;
            };
            let constraints = program.type_reference_table.constraints(*constraints);
            if constraints
                .iter()
                .all(|constraint| matches!(constraint, TypeConstraintNode::ArithmeticDomain(_)))
            {
                break;
            }
            for constraint in constraints.iter().rev() {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    return None;
                };
                if domain.subject != DomainConstraintSubject::Declared || !domain.symbol.is_valid()
                {
                    return None;
                }
                outermost_first.push(domain);
            }
            base = *base_type;
        }
        outermost_first.reverse();
        Some((base, outermost_first))
    }

    pub(crate) fn materialize(
        program: &mut TypedTrees,
        declared_type: TypeReferenceHandle,
        value: &DecodedCanonicalConstValue,
    ) -> Option<ExpressionNode> {
        let (base, domains) = declared_domain_qualifications(program, declared_type)?;
        if domains.is_empty() {
            return materialize_carrier(program, declared_type, value);
        }
        let domains = domains.into_iter().cloned().collect::<Vec<_>>();
        let mut node = materialize_carrier(program, base, value)?;
        for (position, domain) in domains.iter().enumerate() {
            // Each entry adds one domain; the type after it carries that
            // domain and every one added beneath it.
            let result_type = if position + 1 == domains.len() {
                declared_type
            } else {
                let constraints = program.type_reference_table.insert_constraints(
                    domains[..=position]
                        .iter()
                        .cloned()
                        .map(TypeConstraintNode::Domain),
                );
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Constrained {
                        base_type: base,
                        constraints,
                    })
            };
            node = ExpressionNode::Cast(TableCastExpression {
                value: program.expression_table.insert(node),
                target_type: result_type,
                result_type,
                target_label: arena::HandleSpan::empty(),
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain: arena::HandleSpan::empty(),
                semantic_domain_arguments: arena::HandleSpan::empty(),
                semantic_domain_symbol: domain.symbol,
                semantic_domain_id: domain.semantic_id,
                form: added(position == 0),
            });
        }
        Some(node)
    }

    fn added(first: bool) -> CastForm {
        CastForm::Entry(CastEntry {
            kind: CastEntryKind::Add,
            first,
            after_carrier: false,
        })
    }

    fn materialize_carrier(
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

    fn aggregate_declaration<'a>(
        program: &'a TypedTrees,
        carrier: TypeReferenceHandle,
        type_name: &str,
    ) -> Option<&'a DataDefinition> {
        let carrier = program.unwrapped_type_reference(carrier)?;
        // A layout storage application holds its semantic record's fields;
        // the literal names that record and is applied at the storage type.
        if let Some(storage) = program.layout_storage_arguments(carrier) {
            let declaration = program.closed_data_definition(storage.semantic_type)?;
            return (declaration.symbol.is_valid() && declaration.name.as_str() == type_name)
                .then_some(declaration);
        }
        let symbol = program.type_reference_node(carrier).as_named_symbol()?;
        let declaration = program.data_definition_by_symbol(symbol)?;
        (symbol.is_valid() && declaration.name.as_str() == type_name).then_some(declaration)
    }

    /// The type a constant's record literal is applied at: the storage
    /// application when the carrier is one, so the literal constructs that
    /// representation rather than the bare semantic record.
    fn literal_application(
        program: &TypedTrees,
        carrier: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        let underlying = program.unwrapped_type_reference(carrier)?;
        program
            .layout_storage_arguments(underlying)
            .map(|_| underlying)
    }

    fn named_case<'a>(
        program: &'a TypedTrees,
        declaration: &'a DataDefinition,
        name: &str,
    ) -> Option<&'a DataVariant> {
        let mut matching = program
            .data_members(declaration)
            .iter()
            .filter_map(|member| member.as_variant())
            .filter(|case| case.name.as_str() == name);
        let (Some(case), None) = (matching.next(), matching.next()) else {
            return None;
        };
        case.symbol.is_valid().then_some(case)
    }

    fn record(
        program: &mut TypedTrees,
        declared_type: TypeReferenceHandle,
        type_name: &str,
        case_name: Option<&str>,
        fields: &[(String, DecodedCanonicalConstValue)],
    ) -> Option<ExpressionNode> {
        let definition = aggregate_declaration(program, declared_type, type_name)?;
        let mut literal = TableStructLiteral {
            type_name: definition.name.clone(),
            type_symbol: definition.symbol,
            ..TableStructLiteral::default()
        };
        if let Some(application) = literal_application(program, declared_type) {
            literal.application_type = application;
        }
        let declared_fields = if let Some(case_name) = case_name {
            let variant = named_case(program, definition, case_name)?;
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
        let Some((base, domains)) = declared_domain_qualifications(program, carrier) else {
            return false;
        };
        if !domains.is_empty() {
            let mut expression = expression;
            for (position, domain) in domains.iter().enumerate().rev() {
                if !program.expression_table.expression_is_valid(expression) {
                    return false;
                }
                let ExpressionNode::Cast(cast) = program.expression_table.expression(expression)
                else {
                    return false;
                };
                if cast.form != added(position == 0)
                    || cast.semantic_domain_symbol != domain.symbol
                    || cast.semantic_domain_id != domain.semantic_id
                    || (position + 1 == domains.len() && cast.result_type != carrier)
                {
                    return false;
                }
                expression = cast.value;
            }
            return matches_materialized(program, base, expression, value);
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
            (ExpressionNode::StructLiteral(actual), value) => {
                let (type_name, case_name, fields) = match value {
                    DecodedCanonicalConstValue::Record { type_name, fields } => {
                        (type_name, None, fields)
                    }
                    DecodedCanonicalConstValue::Variant {
                        type_name,
                        case_name,
                        fields,
                    } => (type_name, Some(case_name.as_str()), fields),
                    _ => return false,
                };
                let Some(declaration) = aggregate_declaration(program, carrier, type_name) else {
                    return false;
                };
                if actual.type_symbol != declaration.symbol
                    || literal_application(program, carrier)
                        .is_some_and(|application| actual.application_type != application)
                {
                    return false;
                }
                let expressions = program.expression_table.struct_fields(actual.fields);
                if let Some(case_name) = case_name {
                    let Some(case) = named_case(program, declaration, case_name) else {
                        return false;
                    };
                    actual.case_symbol == Some(case.symbol)
                        && actual.case_name.as_ref() == Some(&case.name)
                        && matches_fields(
                            program,
                            program.data_payload_fields(case).iter().map(Some),
                            expressions,
                            fields,
                        )
                } else {
                    actual.case_symbol.is_none()
                        && actual.case_name.is_none()
                        && matches_fields(
                            program,
                            program
                                .data_members(declaration)
                                .iter()
                                .map(|member| member.as_field()),
                            expressions,
                            fields,
                        )
                }
            }
            _ => false,
        }
    }

    fn matches_fields<'a>(
        program: &TypedTrees,
        declared: impl ExactSizeIterator<Item = Option<&'a DataField>>,
        expressions: &[TableStructLiteralField],
        fields: &[(String, DecodedCanonicalConstValue)],
    ) -> bool {
        declared.len() == fields.len()
            && expressions.len() == fields.len()
            && declared
                .zip(expressions)
                .zip(fields)
                .all(|((field, expression), (name, value))| {
                    field.is_some_and(|field| {
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
                })
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

        /// The declared domains on a constant carrier, innermost first
        /// (`declared_domain_qualifications`).
        pub fn constant_domain_qualifications(
            &self,
            carrier: TypeReferenceHandle,
        ) -> Option<Vec<&DomainConstraint>> {
            declared_domain_qualifications(self, carrier).map(|(_, domains)| domains)
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
    ClosedRequirementApplication, MachineInstantiation, NamedConformanceCaptureIndex,
    RequirementLocalArgument,
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
    AccessLifecycleOperation, AccessLifecycleOwner, AccessLifecyclePlacement, AccessLifecyclePlan,
    AccessLifecycleStorage,
    AccessViewPlan, AccessorPrimitive, AccessorTarget, PlanLaidFieldLayoutSource, PlanLaidGeometry,
    PlanLaidLayout, PlanLaidRealizations, PlanLaidRetiredFieldIdentity, PlanLaidSchemaArgument,
    PlanLaidSchemaField, PlanLaidSchemaFieldKind, PlanLaidTargetLayout,
    PrivateCallbackSlotApplication, ScalarAccessorDeclaration, ScalarAccessorOperation,
    access_atomic_observing_results,
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

#[derive(Debug, Default, PartialEq, Eq, compiler_storage::Record)]
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

impl Clone for TypedTrees {
    fn clone(&self) -> Self {
        self.clone_with_symbols(self.symbols.clone())
    }
}

impl TypedTrees {
    /// Install the continuation's symbols without copying the old symbol table
    /// that they replace. Other storage follows its owner's clone contract.
    /// The caller must establish the replacement's source authority first.
    pub fn with_replaced_symbols(
        mut self: std::sync::Arc<Self>,
        symbols: SymbolTable,
    ) -> std::sync::Arc<Self> {
        if let Some(program) = std::sync::Arc::get_mut(&mut self) {
            program.symbols = symbols;
            return self;
        }
        let program = match std::sync::Arc::try_unwrap(self) {
            Ok(mut program) => {
                program.symbols = symbols;
                program
            }
            Err(program) => program.clone_with_symbols(symbols),
        };
        std::sync::Arc::new(program)
    }

    fn clone_with_symbols(&self, symbols: SymbolTable) -> Self {
        macro_rules! clone_storage {
            ($($field:ident),+ $(,)?) => {{
                let Self { identity, symbols: _, $($field,)+ } = self;
                Self { identity: *identity, symbols, $($field: $field.clone(),)+ }
            }};
        }
        clone_storage!(
            product_declaration_frontier,
            excluded_authored_selection_occurrences,
            roots,
            tables,
            service_reaches,
            service_reach_rows,
            authored_service_reach_rows,
            semantic_domains,
            applied_domain_applications,
            applied_data_cleanup_selections,
            external_bindings,
            plan_laid_layouts,
            plan_laid_target_layouts,
            plan_laid_schema_fields,
            plan_laid_retired_field_identities,
            access_view_plans,
            access_lifecycle_plans,
            access_policy_invocations,
            access_borrow_factories,
            wire_placements,
            wire_encode_obligations,
            wire_schema_plans,
            machine_specializations,
            captured_machine_applications,
            captured_machine_states,
            trait_implementation_applications,
            data_implementation_applications,
            target_machine_families,
            deferred_refusals,
            evidence_forwardings,
            proof_output_calls,
            declaration_indexes,
            ranking_expression_custody,
            pending_const_range_endpoints,
            constant_recipes,
            constant_expression_contexts,
            constant_recipe_closure,
            callable_identities,
            type_identities,
        )
    }
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
    pub runtime_callable_contracts: Arena<crate::callable_values::RuntimeCallableContract>,
    pub runtime_callable_lowering: crate::callable_values::RuntimeCallableLowering,
    pub designated_static_type_arguments: Arena<DesignatedStaticTypeArgument>,
    pub retained_product_application_handles: Arena<Handle<ProductOperandApplication>>,
    pub product_input_selections: Arena<crate::product_operand::ProductInputSelection>,
    pub product_input_argument_steps: Arena<crate::product_operand::ProductArgumentStep>,
    pub product_operand_applications: Arena<ProductOperandApplication>,
    authored_declaration_selections:
        language_semantics::declaration_selection::AuthoredDeclarationSelections,
    pub const_declarations: arena::RootedArena<crate::constant::ConstDeclaration>,
    pub data_definitions: DataDefinitionTable,
    pub type_aliases: arena::RootedArena<crate::type_aliases::TypeAliasDefinition>,
    pub data_application_definitions: arena::RootedArena<DataApplicationView>,
    pub data_type_parameters: TypeParameterTable,
    pub declared_type_equations: Arena<machine::DeclaredTypeEquation>,
    pub static_type_contexts: crate::type_system::type_predicates::StaticTypeContexts,
    pub data_members: arena::RootedArena<data::DataMember>,
    pub data_payload_fields: arena::RootedArena<data::DataField>,
    pub domain_definitions: SpelledDeclarationTable<domain::DomainDefinition>,
    pub proof_facts: arena::RootedArena<domain::ProofFact>,
    proof_fact_source_spans: Vec<Option<source::SourceSpan>>,
    pub propositions: arena::RootedArena<proposition::PropositionDefinition>,
    pub proposition_binders: Arena<proposition::PropositionBinder>,
    pub mathematical_definitions: arena::RootedArena<mathematical::MathematicalDefinition>,
    pub mathematical_parameters: Arena<mathematical::MathematicalParameter>,
    pub mathematical_types: Arena<mathematical::MathematicalType>,
    pub domain_path_members: Arena<crate::name::Identifier>,
    pub operator_path_members: Arena<crate::name::Identifier>,
    pub machines: MachineDeclarationTable,
    pub measures: arena::RootedArena<measure::MeasureDefinition>,
    pub measure_path_members: Arena<crate::name::Identifier>,
    pub operators: SpelledDeclarationTable<crate::operator::OperatorDefinition>,
    pub machine_owned_data: Arena<machine::OwnedData>,
    pub machine_trait_conformances: Arena<machine::TraitConformance>,
    pub machine_states: arena::RootedArena<crate::state::State>,
    pub state_parameters: arena::RootedArena<signature::StateParameter>,
    pub traits: arena::RootedArena<trait_definition::TraitDefinition>,
    pub conformances: arena::RootedArena<trait_definition::Conformance>,
    pub trait_requirements: Arena<trait_definition::TraitRequirement>,
    pub trait_machine_signatures: arena::RootedArena<signature::StateSignature>,
    pub(crate) applied_callable_signatures: callable_applications::AppliedCallableTable,
    pub(crate) callable_substitutions: Arena<callable_applications::CallableSubstitution>,
    pub(crate) trait_application_frames: Arena<trait_definition::RetainedTraitApplicationFrame>,
    pub(crate) callable_argument_identities: Arena<crate::type_identity::NormalizedTypeIdentity>,
    pub(crate) callable_arguments: Arena<callable_applications::RequirementLocalArgument>,
    pub(crate) callable_lifetime_substitutions:
        Arena<crate::static_applications::StaticMachineLifetimeBinding>,
    pub signature_invokes: Arena<signature::AuthoredInvocation>,
    pub signature_contracts: arena::RootedArena<signature::SignatureContract>,
    pub expression_table: expression::ExpressionTable,
    pub statement_table: crate::statement::StatementTable,
    pub type_reference_table: types::TypeReferenceTable,
    pub wire_schemas: arena::RootedArena<wire::WireSchema>,
    pub wire_members: Arena<wire::WireMember>,
}

impl TypedTreeTables {
    /// Keep one declaration or completed body's expressions and statements
    /// together. A later source-application update may rewrite that owner,
    /// but must not detach unrelated bodies from the earlier completion batch.
    pub fn begin_declaration_output(&mut self) {
        self.expression_table.finish_output_page();
        self.statement_table.finish_output_page();
    }
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

/// Read a represented static value without evaluating source or proving its
/// carrier obligations. Anonymous integer landing remains the caller's work.
pub fn canonical_const_argument_value(
    program: &TypedTrees,
    reference: types::TypeReferenceHandle,
) -> Option<language_semantics::const_value::CanonicalConstValue> {
    use language_semantics::const_value::{CanonicalConstValue, boolean_literal_spelling};
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return None;
    }
    let value = match program.type_reference_table.type_reference(reference) {
        types::TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
            if !program.symbols.symbols().nodes().is_valid(*symbol) {
                return None;
            }
            canonical_const_declaration_value(program, *symbol)?
        }
        types::TypeReferenceNode::Named { name, .. } => {
            CanonicalConstValue::from_atom(name.as_str()).or_else(|| {
                boolean_literal_spelling(name.as_str()).map(CanonicalConstValue::boolean)
            })?
        }
        types::TypeReferenceNode::ConstExpression(expression) => {
            let expression::ExpressionNode::Boolean(value) =
                program.expression_table.expression(*expression)
            else {
                return None;
            };
            CanonicalConstValue::boolean(*value)
        }
        _ => return None,
    };
    value.decode_encoding()?;
    Some(value)
}

/// Whether this closed carrier has the copyable, address-free structural
/// shape represented by canonical index values. This proves no value predicate.
pub fn canonical_const_carrier_is_supported(
    program: &TypedTrees,
    reference: types::TypeReferenceHandle,
) -> bool {
    use data::DataMember;
    use types::{FixedArrayLength, TypeReferenceNode};
    let mut pending = vec![(reference, false)];
    let mut active = Vec::new();
    while let Some((reference, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            continue;
        }
        if !program
            .type_reference_table
            .contains_type_reference(reference)
            || active.contains(&reference)
        {
            return false;
        }
        let node = program.type_reference_table.type_reference(reference);
        if let TypeReferenceNode::Named { symbol, .. } = node
            && program.symbols.symbols().nodes().is_valid(*symbol)
            && matches!(
                program.symbols.builtin_type_atom(*symbol),
                Some(symbols::BuiltinTypeAtom::Integer(_) | symbols::BuiltinTypeAtom::Bool)
            )
        {
            continue;
        }
        active.push(reference);
        pending.push((reference, true));
        if let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } = node
        {
            pending.push((*element_type, false));
            continue;
        }
        if !matches!(
            node,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
        ) {
            return false;
        }
        let Some(data) = program.closed_data_definition(reference) else {
            return false;
        };
        if !data.lifetime_parameters.is_empty()
            || data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || data.properties.carry.is_some()
            || data.cleanup_selection.is_some()
            || data.quotient.is_some()
            || program.stored_value_multiplicity(reference)
                != language_semantics::Multiplicity::Unrestricted
        {
            return false;
        }
        for member in program.data_members(data) {
            match member {
                DataMember::Field(field) => pending.push((field.type_reference, false)),
                DataMember::Variant(variant) => pending.extend(
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .map(|field| (field.type_reference, false)),
                ),
            }
        }
    }
    true
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

pub use calls::{signature, static_applications};
pub use control_flow::{machine, state, statement};
pub use declarations::{
    boundary_completions, calling_vocabulary, constant, data, domain, invocations, layout_storage,
    measure, operator, product_operand, trait_definition, type_aliases, visibility, wire,
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

#[cfg(test)]
mod continuation_storage_tests {
    use super::TypedTrees;
    use crate::expression::ExpressionNode;
    use std::sync::Arc;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};

    fn symbols(name: &str) -> SymbolTable {
        let mut builder = SymbolTableBuilder::new();
        builder.insert_root(SymbolKind::Machine, SymbolNameRef::Borrowed(name));
        builder.finish()
    }

    #[test]
    fn selected_continuation_shares_callable_children_and_isolates_writes() {
        use crate::name::Identifier;
        use crate::{callable_applications, domain, signature, state};

        let mut original = TypedTrees::default();
        let parameters = original
            .state_parameters
            .insert_many((0..260).map(|_| signature::StateParameter::default()));
        let facts = original
            .proof_facts
            .insert_many([domain::ProofFact::default(), domain::ProofFact::default()]);
        let contracts = original.signature_contracts.insert_many([
            signature::SignatureContract {
                facts,
                ..Default::default()
            },
            signature::SignatureContract::default(),
        ]);
        let states = original.machine_states.insert_many([
            state::State {
                parameters,
                contracts,
                ..Default::default()
            },
            state::State::default(),
        ]);
        let signatures = original.trait_machine_signatures.insert_many([
            signature::StateSignature {
                parameters,
                contracts,
                ..Default::default()
            },
            signature::StateSignature::default(),
        ]);
        let applied = original
            .applied_callable_signatures
            .insert(callable_applications::AppliedCallableSignature::default());
        let other_applied = original
            .applied_callable_signatures
            .insert(callable_applications::AppliedCallableSignature::default());
        let original = Arc::new(original);
        let predecessor = Arc::clone(&original);
        let mut successor = original.with_replaced_symbols(symbols("successor"));
        let fork = Arc::get_mut(&mut successor).unwrap();

        macro_rules! shares {
            ($table:ident, $handle:expr) => {
                assert!(std::ptr::eq(
                    predecessor.$table.get($handle),
                    fork.$table.get($handle)
                ));
            };
        }
        shares!(state_parameters, parameters.start());
        shares!(proof_facts, facts.start());
        shares!(signature_contracts, contracts.start());
        shares!(machine_states, states.start());
        shares!(trait_machine_signatures, signatures.start());
        shares!(applied_callable_signatures, applied);

        fork.state_parameters.get_mut(parameters.start()).name = Identifier::generated("changed");
        fork.proof_facts.set(
            facts.start(),
            domain::ProofFact::Expression(arena::Handle::from_parts(1, 1)),
        );
        fork.signature_contracts
            .get_mut(contracts.start())
            .token_count = 7;
        fork.machine_states.get_mut(states.start()).name = Identifier::generated("changed");
        fork.trait_machine_signatures
            .get_mut(signatures.start())
            .name = Identifier::generated("changed");
        fork.applied_callable_signatures
            .replace(
                applied,
                callable_applications::AppliedCallableSignature::default(),
            )
            .unwrap();
        assert!(!std::ptr::eq(
            predecessor.applied_callable_signatures.get(applied),
            fork.applied_callable_signatures.get(applied)
        ));

        assert_eq!(
            predecessor.state_parameters.get(parameters.start()).name,
            Identifier::default()
        );
        assert_eq!(
            predecessor.proof_facts.get(facts.start()),
            &domain::ProofFact::default()
        );
        assert_eq!(
            predecessor
                .signature_contracts
                .get(contracts.start())
                .token_count,
            0
        );
        assert_eq!(
            predecessor.machine_states.get(states.start()).name,
            Identifier::default()
        );
        assert_eq!(
            predecessor
                .trait_machine_signatures
                .get(signatures.start())
                .name,
            Identifier::default()
        );
        assert_eq!(
            predecessor.applied_callable_signatures.get(applied),
            &callable_applications::AppliedCallableSignature::default()
        );
        shares!(state_parameters, parameters.handle_at(259));
        shares!(proof_facts, facts.handle_at(1));
        shares!(signature_contracts, contracts.handle_at(1));
        shares!(machine_states, states.handle_at(1));
        shares!(trait_machine_signatures, signatures.handle_at(1));
        shares!(applied_callable_signatures, other_applied);
        assert!(fork.applied_callable_signatures.free(applied));
        let recycled = fork
            .applied_callable_signatures
            .insert(callable_applications::AppliedCallableSignature::default());
        assert_eq!(recycled.arena_index(), applied.arena_index());
        assert_ne!(recycled.generation(), applied.generation());
        assert!(!fork.applied_callable_signatures.is_valid(applied));
        assert!(predecessor.applied_callable_signatures.is_valid(applied));
        assert!(!predecessor.applied_callable_signatures.is_valid(recycled));
    }

    #[test]
    fn selected_continuation_shares_data_children_and_replaces_only_its_application() {
        use crate::data::{DataDefinition, DataField, DataMember, DataVariant, TypeParameter};
        use crate::name::Identifier;
        use crate::types::TypeReferenceNode;

        let mut original = TypedTrees::default();
        let parameters = original.data_type_parameters.insert_many([
            TypeParameter {
                name: Identifier::generated("first"),
                ..Default::default()
            },
            TypeParameter {
                name: Identifier::generated("other"),
                ..Default::default()
            },
        ]);
        let payload = original.data_payload_fields.insert_many([DataField {
            name: Identifier::generated("payload"),
            ..Default::default()
        }]);
        let members = original.data_members.insert_many([
            DataMember::Field(DataField {
                name: Identifier::generated("field"),
                ..Default::default()
            }),
            DataMember::Variant(DataVariant {
                payload,
                ..Default::default()
            }),
        ]);
        let mut applications = Vec::new();
        for name in ["first", "other"] {
            let reference = original
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: Default::default(),
                    name: Identifier::generated(name),
                });
            original.retain_data_application_definition(
                reference,
                DataDefinition {
                    name: Identifier::generated(name),
                    type_parameters: parameters,
                    members,
                    ..Default::default()
                },
                true,
            );
            applications.push(reference);
        }
        let original = Arc::new(original);
        let predecessor = Arc::clone(&original);
        let mut successor = original.with_replaced_symbols(symbols("successor"));
        let fork = Arc::get_mut(&mut successor).unwrap();
        assert!(std::ptr::eq(
            predecessor.data_type_parameters.get(parameters.start()),
            fork.data_type_parameters.get(parameters.start())
        ));
        assert!(std::ptr::eq(
            predecessor.data_members.get(members.start()),
            fork.data_members.get(members.start())
        ));
        assert!(std::ptr::eq(
            predecessor.data_payload_fields.get(payload.start()),
            fork.data_payload_fields.get(payload.start())
        ));
        let row =
            arena::Handle::from_parts(applications[0].arena_index(), applications[0].generation());
        assert!(std::ptr::eq(
            predecessor.data_application_definitions.get(row),
            fork.data_application_definitions.get(row)
        ));

        fork.data_type_parameters.get_mut(parameters.start()).name =
            Identifier::generated("changed");
        fork.data_payload_fields.get_mut(payload.start()).name =
            Identifier::generated("changed payload");
        let DataMember::Field(field) = fork.data_members.get_mut(members.start()) else {
            panic!("authored field")
        };
        field.name = Identifier::generated("changed field");
        fork.forget_data_application_definition(applications[0]);
        assert!(fork.data_application_definition(applications[0]).is_none());
        assert!(
            predecessor
                .data_application_definition(applications[0])
                .is_some()
        );
        assert!(std::ptr::eq(
            predecessor
                .data_application_definition(applications[1])
                .unwrap(),
            fork.data_application_definition(applications[1]).unwrap()
        ));
        assert!(std::ptr::eq(
            predecessor
                .data_type_parameters
                .get(parameters.handle_at(1)),
            fork.data_type_parameters.get(parameters.handle_at(1))
        ));
        assert_eq!(
            predecessor
                .data_type_parameters
                .get(parameters.start())
                .name
                .as_str(),
            "first"
        );
        assert_eq!(
            predecessor
                .data_payload_fields
                .get(payload.start())
                .name
                .as_str(),
            "payload"
        );
        let DataMember::Field(field) = predecessor.data_members.get(members.start()) else {
            panic!("authored field")
        };
        assert_eq!(field.name.as_str(), "field");
        let old = predecessor
            .data_application_definition(applications[1])
            .unwrap();
        assert_eq!(fork.data_members(old).len(), 2);
        assert!(matches!(fork.data_members(old)[1], DataMember::Variant(_)));
    }

    #[test]
    fn shared_continuation_moves_replacement_symbols_and_preserves_the_predecessor() {
        let mut original = TypedTrees {
            symbols: symbols("old"),
            ..Default::default()
        };
        let expression = original
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let original = Arc::new(original);
        let predecessor = Arc::clone(&original);
        let replacement = symbols("new");
        let replacement_rows = std::ptr::from_ref(replacement.get(replacement.root()));
        let successor = original.with_replaced_symbols(replacement);
        assert_eq!(
            std::ptr::from_ref(successor.symbols.get(successor.symbols.root())),
            replacement_rows
        );
        assert_eq!(predecessor.symbols.name(predecessor.symbols.root()), "old");
        assert_eq!(successor.symbols.name(successor.symbols.root()), "new");
        assert_eq!(
            successor.expression_table.expression(expression),
            &ExpressionNode::Boolean(true)
        );
        assert_eq!(
            predecessor.expression_table.expression(expression),
            &ExpressionNode::Boolean(true)
        );
        assert!(!Arc::ptr_eq(&predecessor, &successor));
    }

    #[test]
    fn owned_continuation_keeps_its_allocation_and_disassociates_weak_predecessors() {
        let original = Arc::new(TypedTrees::default());
        let storage = Arc::as_ptr(&original);
        let successor = original.with_replaced_symbols(symbols("owned"));
        assert_eq!(Arc::as_ptr(&successor), storage);
        let weak = Arc::downgrade(&successor);
        let successor = successor.with_replaced_symbols(symbols("weak"));
        assert!(weak.upgrade().is_none());
        assert_eq!(successor.symbols.name(successor.symbols.root()), "weak");
        let ordinary_clone = successor.as_ref().clone();
        assert_eq!(ordinary_clone.symbols, successor.symbols);
        assert_eq!(ordinary_clone.tables, successor.tables);
    }
}
