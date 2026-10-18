//! Publish a checked value's declared IEEE field facts into ordinary flow.
//!
//! Entry arguments and completed whole-value stores establish the data domain.
//! Its field bounds then share the normal write/call invalidation and arrival
//! joins. Reading a declaration at each use instead would resurrect a predicate
//! while mutation has opened an invariant window. Write-only references grant
//! no readable value, and recursive storage does not justify infinite projection.

use super::float_arithmetic::float_bound_from;
use super::value_environment::ValueEnvironment;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// A declaration-backed bound, with exact common-field identities and IEEE format.
/// This is a temporary projection roster, not an established fact about storage.
pub struct DeclaredFloatFieldBound {
    pub path: Vec<SymbolHandle>,
    pub primitive: PrimitiveType,
    pub operator: typed_trees::expression::BinaryOperator,
    pub value: f64,
}

impl DeclaredFloatFieldBound {
    pub fn predicate(&self) -> Option<facts::FloatPredicate> {
        let format = match self.primitive {
            PrimitiveType::F32 => numerics::literals::FloatFormat::F32,
            PrimitiveType::F64 => numerics::literals::FloatFormat::F64,
            _ => return None,
        };
        facts::FloatPredicate::comparison(
            format,
            self.operator,
            &numerics::bignum::ExactFloat::from_f64(self.value),
        )
    }
}

/// Project only selected IEEE comparisons. Consumers choose the establishment
/// point and their own ordinary invalidation-aware representation.
pub fn declared_float_field_bounds(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Vec<DeclaredFloatFieldBound> {
    let mut rows = Vec::new();
    collect(
        program,
        reference,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut rows,
    );
    rows
}

pub(crate) fn record_declared_float_fields(
    program: &dyn crate::CheckingInput,
    environment: &mut ValueEnvironment,
    path: &str,
    reference: TypeReferenceHandle,
) {
    for row in declared_float_field_bounds(program, reference) {
        if let Some(interval) = float_bound_from(row.operator, row.value, true, row.primitive) {
            let mut field_path = path.to_owned();
            for symbol in row.path {
                field_path.push('.');
                field_path.push_str(program.symbols.name(symbol));
            }
            environment.narrow_float(field_path.clone(), interval);
            environment.mark_non_nan(field_path);
        }
    }
}

fn collect(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    path: &mut Vec<SymbolHandle>,
    active: &mut Vec<SymbolHandle>,
    rows: &mut Vec<DeclaredFloatFieldBound>,
) {
    for _ in 0..program.type_reference_table.type_reference_count() {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::WriteOnly,
                ..
            } => return,
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            _ => break,
        }
    }
    let Some(definition) =
        crate::value_custody::places::data_definition_for_type(program, reference)
    else {
        return;
    };
    if active.contains(&definition.symbol) {
        return;
    }
    active.push(definition.symbol);
    if let Some(bounds) = crate::proof_contracts::default_domains::data_where_field_literal_bounds(
        program, definition,
    ) {
        for bound in bounds {
            let Some(field) = program.data_field(definition, bound.field) else {
                continue;
            };
            let Some(primitive @ (PrimitiveType::F32 | PrimitiveType::F64)) =
                program.primitive_type_reference(field.type_reference)
            else {
                continue;
            };
            let Some(meaning) =
                crate::proof_contracts::default_domains::float_field_literal_bound_meaning(
                    program,
                    definition,
                    bound.conjunct,
                )
            else {
                continue;
            };
            let value = meaning.to_f64();
            if !value.is_finite() {
                continue;
            }
            let mut field_path = path.clone();
            field_path.push(field.symbol);
            rows.push(DeclaredFloatFieldBound {
                path: field_path,
                primitive,
                operator: bound.operator,
                value,
            });
        }
    }
    for member in program.data_members(definition) {
        if let DataMember::Field(field) = member {
            path.push(field.symbol);
            collect(program, field.type_reference, path, active, rows);
            path.pop();
        }
    }
    active.pop();
}
