//! Equatable synthesis (frozen decisions 8 + 11): `Type satisfies Equatable;`
//! on a record or payload-bearing sum makes `==`/`!=` legal on that type by
//! synthesizing STRUCTURAL equality. This module owns the shared vocabulary:
//! which types need a declared conformance, whether a conformance is declared,
//! whether every member can participate in synthesized equality, and the
//! typing scope the expression lowerer uses to find an operand's data type.
//!
//! Mechanism choice: equality is expanded INLINE at the same resolved->typed
//! lowering point where case membership already synthesizes tag compares (see
//! `expression::table::structural_equality`). The expansion is an AND/OR tree
//! of field compares and tag-guarded payload compares, so it rides every
//! existing backend and interpreter path with zero new runtime surface. A
//! CALLABLE synthesized `Type::equals` machine is deliberately NOT emitted --
//! that is the comptime/trait-generator arc (a hand-written `Type::equals`
//! still wins: `==` lowers to a call to it).
//!
//! The conformance PREREQUISITES are validated up front (before machine
//! lowering) so violations error at the conformance item, not at some later
//! `==` site: every field must be a scalar primitive, a `String` (text
//! content equality -- length AND bytes -- through the backend's dedicated
//! value-position text-equals operand), a payload-less sum, or itself an
//! Equatable-conforming data type; recursive types are rejected (inline
//! expansion would not terminate).

use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees::types::PrimitiveType;
use resolved::SymbolResolvedTrees;
use resolved::data::{DataDefinition, DataField, DataMember, DataShapeKind};

pub(crate) const EQUATABLE_TRAIT: &str = "Equatable";

/// How `==` treats a data definition (decision 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataEqualityShape {
    /// Empty records and payload-less sums: tag/value identity IS total
    /// equality, so `==` stays the existing direct compare.
    Implicit,
    /// Records, payload-bearing sums, and mixed shapes: `==` requires a
    /// declared `Type satisfies Equatable;` conformance and expands
    /// structurally (mixed = common fields AND tag AND matching payload).
    Structural,
}

pub(crate) fn data_definition_by_name<'program>(
    program: &'program SymbolResolvedTrees,
    type_name: &str,
) -> Option<&'program DataDefinition> {
    program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == type_name)
}

pub(crate) fn data_equality_shape(
    program: &SymbolResolvedTrees,
    data: &DataDefinition,
) -> DataEqualityShape {
    let members = program.data_members(data.members);
    match DataDefinition::shape_kind_from_members(members) {
        DataShapeKind::Empty => DataEqualityShape::Implicit,
        DataShapeKind::Record | DataShapeKind::Mixed => DataEqualityShape::Structural,
        DataShapeKind::Enum => {
            let payload_bearing = members.iter().any(|member| {
                matches!(member, DataMember::Variant(variant) if variant.payload.count() > 0)
            });
            if payload_bearing {
                DataEqualityShape::Structural
            } else {
                DataEqualityShape::Implicit
            }
        }
    }
}

pub(crate) fn equatable_conformance_declared(
    program: &SymbolResolvedTrees,
    type_name: &str,
) -> bool {
    program.conformances.iter().any(|conformance| {
        conformance.trait_name.as_str() == EQUATABLE_TRAIT
            && conformance.type_name.as_str() == type_name
    })
}

/// A hand-written `machine Type::equals(...)` wins over synthesis (check-
/// then-synthesize, chapter 13): `==` on a conforming type with a written
/// member lowers to a call targeting its entry state.
pub(crate) fn written_equals_state_symbol(
    program: &SymbolResolvedTrees,
    type_name: &str,
) -> Option<omega_core::symbols::SymbolHandle> {
    program
        .machines
        .iter()
        .filter(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|attached| attached.as_str() == type_name)
        })
        .find_map(|machine| {
            program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| program.machine_state(*handle))
                .find(|state| state.name.as_str() == "equals")
                .map(|state| state.symbol)
        })
}

/// How one field participates in synthesized structural equality.
pub(crate) enum FieldEquality<'program> {
    /// Scalar primitives and payload-less sums compare directly with `==`.
    Direct,
    /// A `String` field: the same `==` member-compare shape, but the operands
    /// must be stored PLACES -- the backend lowers it to the value-position
    /// text content compare (length AND bytes), not a scalar word compare.
    Text,
    /// A nested Equatable-conforming record / payload-bearing sum: recurse.
    Structural(&'program DataDefinition),
}

pub(crate) fn field_equality<'program>(
    program: &'program SymbolResolvedTrees,
    conforming_type: &str,
    owner: &str,
    field: &DataField,
) -> Result<FieldEquality<'program>, Diagnostic> {
    let Some(base_name) = value_type_base_name(program, &field.type_reference) else {
        return Err(Diagnostic::error(format!(
            "conformance `{conforming_type} satisfies Equatable`: field `{}` of `{owner}` is not Equatable: only scalar primitives, payload-less sums, and Equatable-conforming data types participate in synthesized structural equality",
            field.name
        )));
    };

    if let Some(primitive) = PrimitiveType::from_name(&base_name) {
        if primitive == PrimitiveType::String {
            return Ok(FieldEquality::Text);
        }
        return Ok(FieldEquality::Direct);
    }

    let Some(field_data) = data_definition_by_name(program, &base_name) else {
        return Err(Diagnostic::error(format!(
            "conformance `{conforming_type} satisfies Equatable`: field `{}` of `{owner}` is not Equatable: `{base_name}` is not a comparable data type",
            field.name
        )));
    };

    match data_equality_shape(program, field_data) {
        DataEqualityShape::Implicit => Ok(FieldEquality::Direct),
        DataEqualityShape::Structural => {
            if equatable_conformance_declared(program, &base_name) {
                Ok(FieldEquality::Structural(field_data))
            } else {
                Err(Diagnostic::error(format!(
                    "conformance `{conforming_type} satisfies Equatable`: field `{}` of `{owner}` is not Equatable: add `{base_name} satisfies Equatable;`",
                    field.name
                )))
            }
        }
    }
}

/// Validate every declared Equatable conformance BEFORE machine lowering so
/// prerequisite violations error at the conformance item, not at a later
/// `==` site. Unknown data/trait names are left to `omega-validation`.
pub(crate) fn validate_equatable_conformances(
    program: &SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    for conformance in &program.conformances {
        if conformance.trait_name.as_str() != EQUATABLE_TRAIT {
            continue;
        }
        let type_name = conformance.type_name.as_str();
        let Some(data) = data_definition_by_name(program, type_name) else {
            continue;
        };

        let mut visiting = vec![type_name.to_owned()];
        validate_equatable_data(program, type_name, data, &mut visiting)?;
    }
    Ok(())
}

fn validate_equatable_data(
    program: &SymbolResolvedTrees,
    conforming_type: &str,
    data: &DataDefinition,
    visiting: &mut Vec<String>,
) -> Result<(), Diagnostic> {
    let owner = data.name.as_str();
    for member in program.data_members(data.members) {
        let fields: &[DataField] = match member {
            DataMember::Field(field) => std::slice::from_ref(field),
            DataMember::Variant(variant) => program.data_payload_fields(variant.payload),
        };
        for field in fields {
            let FieldEquality::Structural(nested) =
                field_equality(program, conforming_type, owner, field)?
            else {
                continue;
            };
            let nested_name = nested.name.as_str();
            if visiting.iter().any(|name| name == nested_name) {
                return Err(Diagnostic::error(format!(
                    "conformance `{conforming_type} satisfies Equatable`: type `{nested_name}` is recursive through field `{}` of `{owner}`: synthesized structural equality would not terminate, so recursive Equatable conformance is not supported yet",
                    field.name
                )));
            }
            visiting.push(nested_name.to_owned());
            validate_equatable_data(program, conforming_type, nested, visiting)?;
            visiting.pop();
        }
    }
    Ok(())
}

/// The base NAMED type of a declaration-storage type reference (`&T` and
/// constrained forms unwrap to `T`); `None` for shapes that never carry
/// synthesized equality (slices, arrays, generics, dyn traits, unit).
pub(crate) fn value_type_base_name(
    program: &SymbolResolvedTrees,
    type_reference: &resolved::types::TypeReference,
) -> Option<String> {
    match type_reference {
        resolved::types::TypeReference::Reference(reference) => value_type_base_name(
            program,
            program.child_type_reference(reference.storage.referee),
        ),
        resolved::types::TypeReference::Constrained(constrained) => value_type_base_name(
            program,
            program.child_type_reference(constrained.storage.base_type),
        ),
        resolved::types::TypeReference::Named { name, .. } => Some(name.as_str().to_owned()),
        _ => None,
    }
}

/// The base NAMED type of a table-stored type reference (local declarations).
pub(crate) fn table_type_base_name(
    program: &SymbolResolvedTrees,
    type_reference: resolved::types::TypeReferenceHandle,
) -> Option<String> {
    if !type_reference.is_valid() {
        return None;
    }
    match program
        .tables
        .types
        .references
        .type_reference(type_reference)
    {
        resolved::types::TypeReferenceNode::Reference { referee, .. } => {
            table_type_base_name(program, *referee)
        }
        resolved::types::TypeReferenceNode::Constrained { base_type, .. } => {
            table_type_base_name(program, *base_type)
        }
        resolved::types::TypeReferenceNode::Named { name, .. } => Some(name.as_str().to_owned()),
        _ => None,
    }
}

/// The value names in scope while lowering one state's body, each with the
/// base name of its declared type. Used by the expression lowerer to type
/// `==` operands; equality on places it cannot type lowers unchanged.
#[derive(Debug, Clone)]
pub(crate) struct EqualityScope {
    /// The enclosing machine's attached data type (`self`'s type).
    pub(crate) attached_data: Option<String>,
    /// Parameter, owned-data, and local names mapped to base type names.
    pub(crate) value_types: Vec<(String, String)>,
}

impl EqualityScope {
    pub(crate) fn for_state(
        program: &SymbolResolvedTrees,
        machine: &resolved::machine::Machine,
        state: &resolved::state::State,
    ) -> Self {
        let attached_data = machine
            .attached_data
            .as_ref()
            .map(|name| name.as_str().to_owned());

        let mut value_types = Vec::new();
        for owned in program.machine_owned_data(machine.owned_data) {
            if let Some(base) = value_type_base_name(program, &owned.type_reference) {
                value_types.push((owned.name.as_str().to_owned(), base));
            }
        }
        for parameter in program.state_parameters(state.parameters) {
            if let Some(base) = value_type_base_name(program, &parameter.type_reference) {
                value_types.push((parameter.name.as_str().to_owned(), base));
            }
        }
        for statement in program
            .tables
            .bodies
            .statements
            .statements(state.statement_nodes)
        {
            if let resolved::statement::StatementNode::LocalData(local) = statement
                && let Some(base) = table_type_base_name(program, local.type_reference)
            {
                value_types.push((local.name.as_str().to_owned(), base));
            }
        }

        Self {
            attached_data,
            value_types,
        }
    }

    pub(crate) fn value_type(&self, name: &str) -> Option<&str> {
        self.value_types
            .iter()
            .rev()
            .find(|(value_name, _)| value_name == name)
            .map(|(_, type_name)| type_name.as_str())
    }
}
