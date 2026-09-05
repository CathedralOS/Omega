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
//! pre-resolution wrapper also makes that exact expansion callable as
//! `Type::equals`; direct calls to the compiler-owned surface expand in the
//! caller's storage scope, and the wrapper is excluded from the hand-written-
//! override lookup so its own `self == other` body cannot recursively target
//! itself. A hand-written `Type::equals` still wins.
//!
//! The conformance PREREQUISITES are validated up front (before machine
//! lowering) so violations error at the conformance item, not at some later
//! `==` site: every field must be a scalar primitive, a `String` (text
//! content equality -- length AND bytes -- through the backend's dedicated
//! value-position text-equals operand), a payload-less sum, or itself an
//! Equatable-conforming data type; recursive types are rejected (inline
//! expansion would not terminate).

use diagnostics::Diagnostic;
use resolved::SymbolResolvedTrees;
use resolved::data::{DataDefinition, DataField, DataMember, DataShapeKind};
use symbol_resolved_trees as resolved;
use typed_trees::types::PrimitiveType;

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
            && conformance
                .carrier_name()
                .is_some_and(|carrier| carrier.as_str() == type_name)
    })
}

/// A hand-written `machine Type::equals(...)` wins over synthesis (check-
/// then-synthesize, chapter 13): `==` on a conforming type with a written
/// member lowers to a call targeting its entry state.
pub(crate) fn written_equals_state_symbol(
    program: &SymbolResolvedTrees,
    type_name: &str,
) -> Option<symbols::SymbolHandle> {
    program
        .machines
        .iter()
        .filter(|machine| {
            !machine
                .name
                .as_str()
                .starts_with("__omega_synthesized_equatable::")
                && machine
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

/// The one compiler-generated callable face for structural equality.
///
/// Authored `value.equals(...)` syntax selects this declaration even though
/// typed lowering expands its body inline. Returning only an exact singleton
/// keeps declaration custody fail-closed if synthesis ever duplicates or
/// omits the wrapper.
pub(crate) fn synthesized_equals_state_symbol(
    program: &SymbolResolvedTrees,
    type_name: &str,
) -> Option<symbols::SymbolHandle> {
    let matches = program
        .machines
        .iter()
        .filter(|machine| {
            machine
                .name
                .as_str()
                .starts_with("__omega_synthesized_equatable::")
                && machine
                    .attached_data
                    .as_ref()
                    .is_some_and(|attached| attached.as_str() == type_name)
        })
        .flat_map(|machine| {
            program
                .machine_state_handles(machine.states)
                .iter()
                .map(|handle| program.machine_state(*handle))
        })
        .filter(|state| state.name.as_str() == "equals")
        .map(|state| state.symbol)
        .collect::<Vec<_>>();
    let [symbol] = matches.as_slice() else {
        return None;
    };
    Some(*symbol)
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
    // A `&[u8]` byte-slice text VIEW (`&[u8] in Utf8`, bare `&[u8]`) shares the
    // identical 16-byte `{ptr, len}` descriptor with `String` and is likewise
    // CONTENT-comparable (length AND single-byte loop) -- the value-position
    // `==` over such a fat-slice place already lowers to the dedicated
    // TextEquals leaf. Classify it as a text field BEFORE the base-name
    // collapse (which would discard the slice shape and reject it). Scoped to a
    // BYTE slice on purpose: TextEquals compares `len` bytes, so a wider-element
    // slice would compare too few bytes.
    if byte_sequence_text_carrier(program, &field.type_reference) {
        return Ok(FieldEquality::Text);
    }

    let Some(base_name) = value_type_base_name(program, &field.type_reference) else {
        return Err(Diagnostic::error(format!(
            "conformance `{conforming_type} satisfies Equatable`: field `{}` of `{owner}` is not Equatable: only scalar primitives, payload-less sums, and Equatable-conforming data types participate in synthesized structural equality",
            field.name
        )));
    };

    if PrimitiveType::from_name(&base_name).is_some() {
        return Ok(FieldEquality::Direct);
    }

    let Some(field_data) = data_definition_by_name(program, &base_name) else {
        return Err(Diagnostic::error(format!(
            "conformance `{conforming_type} satisfies Equatable`: field `{}` of `{owner}` is not Equatable: `{base_name}` is not a comparable data type",
            field.name
        )));
    };

    if field_data.quotient.is_some() {
        return Err(Diagnostic::error(format!(
            "conformance `{conforming_type} satisfies Equatable`: field `{}` of `{owner}` has quotient type `{base_name}`; quotient representatives have no synthesized structural equality, so use a named lifted equality operation instead",
            field.name
        )));
    }

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
/// `==` site. Unknown data/trait names are left to `validation`.
pub(crate) fn validate_equatable_conformances(
    program: &SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    for conformance in &program.conformances {
        if conformance.trait_name.as_str() != EQUATABLE_TRAIT {
            continue;
        }
        let Some(type_name) = conformance.carrier_name().map(|name| name.as_str()) else {
            continue;
        };
        let Some(data) = data_definition_by_name(program, type_name) else {
            continue;
        };
        if data.quotient.is_some() {
            return Err(Diagnostic::error(format!(
                "conformance `{type_name} satisfies Equatable` cannot synthesize equality for a quotient type; quotient representatives are opaque, so equality requires a named lifted operation"
            )));
        }

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

/// Whether a declaration-storage type reference is a `&[u8]` byte-slice text
/// VIEW -- a reference whose referee (after unwrapping any `Constrained`
/// domain, e.g. `in Utf8`) is a `Slice` of `u8`. Such a field is the same
/// 16-byte `{ptr, len}` fat descriptor as `String` and is content-comparable
/// through the same value-position TextEquals path, so synthesized structural
/// equality treats it as a text field. The domain NAME is irrelevant (any
/// declared domain over `[u8]` qualifies); only the byte-slice shape matters.
pub(crate) fn byte_slice_text_view(
    program: &SymbolResolvedTrees,
    type_reference: &resolved::types::TypeReference,
) -> bool {
    let resolved::types::TypeReference::Reference(reference) = type_reference else {
        return false;
    };
    let mut referee = program.child_type_reference(reference.storage.referee);
    while let resolved::types::TypeReference::Constrained(constrained) = referee {
        referee = program.child_type_reference(constrained.storage.base_type);
    }
    let resolved::types::TypeReference::Slice(slice) = referee else {
        return false;
    };
    matches!(
        program.child_type_reference(slice.storage.element_type),
        resolved::types::TypeReference::Named { name, .. }
            if PrimitiveType::from_name(name.as_str()) == Some(PrimitiveType::U8)
    )
}

/// Whether a declaration-storage type is one of the byte-sequence carriers
/// whose equality is live-length plus live-byte-prefix equality. In addition
/// to borrowed byte slices, this admits a named-domain-qualified fixed byte
/// array (`[u8; N] in Domain`), whose runtime storage is the bounded
/// `{len, bytes}` carrier rather than an always-full array.
pub(crate) fn byte_sequence_text_carrier(
    program: &SymbolResolvedTrees,
    type_reference: &resolved::types::TypeReference,
) -> bool {
    if byte_slice_text_view(program, type_reference) {
        return true;
    }
    let resolved::types::TypeReference::Constrained(constrained) = type_reference else {
        return false;
    };
    if !program
        .tables
        .types
        .constraints
        .span_or_empty(constrained.storage.constraints)
        .iter()
        .any(|constraint| matches!(constraint, resolved::types::TypeConstraint::Domain(_)))
    {
        return false;
    }
    let resolved::types::TypeReference::FixedArray(array) =
        program.child_type_reference(constrained.storage.base_type)
    else {
        return false;
    };
    matches!(
        array.storage.length,
        resolved::types::FixedArrayLength::Literal(_)
    ) && matches!(
        program.child_type_reference(array.storage.element_type),
        resolved::types::TypeReference::Named { name, .. }
            if PrimitiveType::from_name(name.as_str()) == Some(PrimitiveType::U8)
    )
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
