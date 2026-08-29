//! Opt-in value-sensitive custody for exact fixed materialization bytes.

use psi_language_semantics::{DataSupplyMode, Multiplicity};
use psi_layout_plans::{
    ByteOrder, LayoutPlacementReport, LayoutPlanReport, MaterializationDiagnostic,
    layout_plan_reports_match_for_replay, normalized_layout_plan_fingerprint,
};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataMember, DataShapeKind};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};

use super::{
    BuildTimeValue, materialize_typed_owned_layout_into, normalized_schema_identity, schema_fields,
};

/// Validated evidence that one closed typed value and fixed layout determine
/// every byte of one materialization.
///
/// This carrier deliberately does not implement `Clone`: callers either retain
/// this exact validation result or independently reconstruct it. It is not a
/// target capsule, source admission, quotient canonicalization, or proof of a
/// complete evaluator-origin chain.
#[derive(Debug)]
pub struct ValidatedConstMaterialization {
    schema_name: String,
    schema_identity: u64,
    value: BuildTimeValue,
    layout: LayoutPlanReport,
    non_authoritative_layout_report_fingerprint: u64,
    byte_order: ByteOrder,
    bytes: Vec<u8>,
    non_authoritative_materialization_report_fingerprint: u64,
}

impl ValidatedConstMaterialization {
    pub fn schema_name(&self) -> &str {
        &self.schema_name
    }

    pub fn schema_identity(&self) -> u64 {
        self.schema_identity
    }

    pub fn value(&self) -> &BuildTimeValue {
        &self.value
    }

    pub fn layout(&self) -> &LayoutPlanReport {
        &self.layout
    }

    /// Compact report coordinate retained for compatibility and diagnostics.
    /// Replay authority comes from the exact retained layout, value, and bytes.
    pub fn layout_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }

    /// Explicitly named accessor for the non-authoritative layout report
    /// coordinate.
    pub fn non_authoritative_layout_report_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_report_fingerprint
    }

    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Compact report coordinate retained for compatibility and diagnostics.
    /// It is not materialization or replay authority.
    pub fn identity(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Explicitly named accessor for the non-authoritative materialization
    /// report coordinate.
    pub fn non_authoritative_materialization_report_fingerprint(&self) -> u64 {
        self.non_authoritative_materialization_report_fingerprint
    }

    /// Independently replay this evidence against caller-supplied semantic and
    /// layout inputs. Compact fingerprints are checked but never substitute for
    /// exact value, layout, or byte equality.
    pub fn replay_against(
        &self,
        typed: &TypedTrees,
        schema_name: &str,
        layout: &LayoutPlanReport,
        value: &BuildTimeValue,
        byte_order: ByteOrder,
    ) -> Result<(), MaterializationDiagnostic> {
        if schema_name != self.schema_name {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable schema `{schema_name}` does not match retained schema `{}`",
                self.schema_name
            )));
        }
        if value != &self.value {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable typed owned value drifted from retained custody".into(),
            ));
        }
        if byte_order != self.byte_order {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable target byte order drifted from retained custody".into(),
            ));
        }
        let layout_report_fingerprint = normalized_layout_plan_fingerprint(layout);
        if layout_report_fingerprint != self.non_authoritative_layout_report_fingerprint
            || !layout_plan_reports_match_for_replay(layout, &self.layout)
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable layout drifted from retained custody".into(),
            ));
        }

        let replayed = derive_bytes(typed, schema_name, layout, value, byte_order)?;
        if replayed.schema_identity != self.schema_identity {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable typed schema identity drifted from retained custody".into(),
            ));
        }
        if replayed.bytes != self.bytes {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable staged bytes drifted from exact zero-initialized replay"
                    .into(),
            ));
        }
        let materialization_report_fingerprint =
            non_authoritative_materialization_report_fingerprint(
                schema_name,
                replayed.schema_identity,
                layout_report_fingerprint,
                byte_order,
                value,
                &replayed.bytes,
            );
        if materialization_report_fingerprint
            != self.non_authoritative_materialization_report_fingerprint
        {
            return Err(MaterializationDiagnostic(
                "ConstMaterializable deterministic report fingerprint drifted from exact replay"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Replay the retained evidence and atomically copy its exact bytes. Any
    /// rejection leaves `destination` unchanged.
    pub fn apply(
        &self,
        typed: &TypedTrees,
        destination: &mut [u8],
    ) -> Result<(), MaterializationDiagnostic> {
        self.replay_against(
            typed,
            &self.schema_name,
            &self.layout,
            &self.value,
            self.byte_order,
        )?;
        if destination.len() < self.bytes.len() {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable copy needs {} bytes, destination has {}",
                self.bytes.len(),
                destination.len()
            )));
        }
        destination[..self.bytes.len()].copy_from_slice(&self.bytes);
        Ok(())
    }
}

/// Validate the first closed `ConstMaterializable(value, layout)` subset.
///
/// The admitted subset is intentionally smaller than legacy typed-owned
/// materialization: closed non-generic unrestricted records containing only
/// integer/Boolean values, non-NaN binary32/binary64 values, literal fixed
/// arrays, and records of those shapes.
pub fn validate_const_materializable_typed_owned_layout(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<ValidatedConstMaterialization, MaterializationDiagnostic> {
    let derived = derive_bytes(typed, schema_name, layout, value, byte_order)?;
    let layout_report_fingerprint = normalized_layout_plan_fingerprint(layout);
    let materialization_report_fingerprint = non_authoritative_materialization_report_fingerprint(
        schema_name,
        derived.schema_identity,
        layout_report_fingerprint,
        byte_order,
        value,
        &derived.bytes,
    );
    Ok(ValidatedConstMaterialization {
        schema_name: schema_name.to_owned(),
        schema_identity: derived.schema_identity,
        value: value.clone(),
        layout: layout.clone(),
        non_authoritative_layout_report_fingerprint: layout_report_fingerprint,
        byte_order,
        bytes: derived.bytes,
        non_authoritative_materialization_report_fingerprint: materialization_report_fingerprint,
    })
}

struct DerivedMaterialization {
    schema_identity: u64,
    bytes: Vec<u8>,
}

fn derive_bytes(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    value: &BuildTimeValue,
    byte_order: ByteOrder,
) -> Result<DerivedMaterialization, MaterializationDiagnostic> {
    let data = unique_data_by_name(typed, schema_name)?;
    validate_record_value(typed, data, value, "value", &mut Vec::new())?;
    let schema_identity = normalized_schema_identity(typed, data);
    if layout.schema_identity != schema_identity {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable layout schema identity does not match `{schema_name}`"
        )));
    }
    validate_fixed_layout(typed, schema_name, layout, schema_identity)?;
    let byte_len = layout
        .size
        .ok_or_else(|| {
            MaterializationDiagnostic(
                "ConstMaterializable requires one exact fixed layout extent".into(),
            )
        })
        .and_then(|size| {
            usize::try_from(size).map_err(|_| {
                MaterializationDiagnostic(
                    "ConstMaterializable fixed extent exceeds this compiler host".into(),
                )
            })
        })?;
    let mut bytes = vec![0; byte_len];
    materialize_typed_owned_layout_into(typed, schema_name, layout, value, byte_order, &mut bytes)?;
    Ok(DerivedMaterialization {
        schema_identity,
        bytes,
    })
}

fn validate_fixed_layout(
    typed: &TypedTrees,
    schema_name: &str,
    layout: &LayoutPlanReport,
    schema_identity: u64,
) -> Result<(), MaterializationDiagnostic> {
    let (fields, reflected_identity) =
        schema_fields(typed, schema_name).map_err(MaterializationDiagnostic)?;
    if reflected_identity != schema_identity {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable schema reflection identity drifted during construction".into(),
        ));
    }
    let size = layout.size.ok_or_else(|| {
        MaterializationDiagnostic("ConstMaterializable layout is not fixed-size".into())
    })?;
    if layout.align == 0 || !layout.align.is_power_of_two() || !size.is_multiple_of(layout.align) {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable fixed extent {size} is inconsistent with alignment {}",
            layout.align
        )));
    }
    let required_align = fields.iter().map(|field| field.align).max().unwrap_or(1);
    if layout.align < required_align {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable layout alignment {} is below schema alignment {required_align}",
            layout.align
        )));
    }

    let mut expected_offsets = Vec::with_capacity(fields.len());
    for field in &fields {
        let entries = layout
            .entries
            .iter()
            .filter(|entry| match field.identity {
                Some(identity) => entry.member_identity == Some(identity),
                None => entry.member_identity.is_none() && entry.field == field.name,
            })
            .collect::<Vec<_>>();
        let [entry] = entries.as_slice() else {
            if layout.offsets.is_some() {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable derived offsets claim one placement per field, but the exact layout does not"
                        .into(),
                ));
            }
            return Ok(());
        };
        let LayoutPlacementReport::At { offset } = entry.placement else {
            if layout.offsets.is_some() {
                return Err(MaterializationDiagnostic(
                    "ConstMaterializable derived offsets are present for a non-At field placement"
                        .into(),
                ));
            }
            return Ok(());
        };
        if !offset.is_multiple_of(field.align) {
            return Err(MaterializationDiagnostic(format!(
                "ConstMaterializable field `{}` offset {offset} violates alignment {}",
                field.name, field.align
            )));
        }
        expected_offsets.push(offset);
    }
    if layout.offsets.as_deref() != Some(expected_offsets.as_slice()) {
        return Err(MaterializationDiagnostic(
            "ConstMaterializable derived offsets do not replay the exact field placements".into(),
        ));
    }
    Ok(())
}

pub(super) fn unique_data_by_name<'a>(
    typed: &'a TypedTrees,
    name: &str,
) -> Result<&'a DataDefinition, MaterializationDiagnostic> {
    let mut definitions = typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.name.as_str() == name);
    let definition = definitions.next().ok_or_else(|| {
        MaterializationDiagnostic(format!("ConstMaterializable names unknown schema `{name}`"))
    })?;
    if definitions.next().is_some() {
        return Err(MaterializationDiagnostic(format!(
            "ConstMaterializable schema name `{name}` is ambiguous"
        )));
    }
    Ok(definition)
}

fn validate_record_value(
    typed: &TypedTrees,
    data: &DataDefinition,
    value: &BuildTimeValue,
    path: &str,
    active: &mut Vec<psi_symbols::SymbolHandle>,
) -> Result<(), MaterializationDiagnostic> {
    if !data.symbol.is_valid()
        || data.supply_mode != DataSupplyMode::CheckedShape
        || !data.type_parameters.is_empty()
        || !data.lifetime_parameters.is_empty()
        || data.generic_instance.is_some()
        || data.quotient.is_some()
    {
        return Err(MaterializationDiagnostic(format!(
            "{path} type `{}` is generic, opaque, quotient, or lacks one exact closed checked-shape identity",
            data.name
        )));
    }
    if data.properties.multiplicity != Multiplicity::Unrestricted {
        return Err(MaterializationDiagnostic(format!(
            "{path} type `{}` is not a `[copy]` record",
            data.name
        )));
    }
    if active.contains(&data.symbol) {
        return Err(MaterializationDiagnostic(format!(
            "{path} recursively reaches `{}`",
            data.name
        )));
    }
    let members = typed.data_members(data);
    if DataDefinition::shape_kind_from_members(members) != DataShapeKind::Record {
        return Err(MaterializationDiagnostic(format!(
            "{path} type `{}` is a sum, empty, or mixed data shape",
            data.name
        )));
    }
    let BuildTimeValue::Struct { type_name, fields } = value else {
        return Err(MaterializationDiagnostic(format!(
            "{path} expected record `{}`, found {}",
            data.name,
            value_kind(value)
        )));
    };
    if type_name != data.name.as_str() {
        return Err(MaterializationDiagnostic(format!(
            "{path} record `{type_name}` does not match `{}`",
            data.name
        )));
    }
    let mut supplied = std::collections::BTreeMap::new();
    for (name, value) in fields {
        if supplied.insert(name.as_str(), value).is_some() {
            return Err(MaterializationDiagnostic(format!(
                "{path} supplies field `{name}` more than once"
            )));
        }
    }
    if supplied.len() != members.len() {
        return Err(MaterializationDiagnostic(format!(
            "{path} supplies {} field(s), expected {} for `{}`",
            supplied.len(),
            members.len(),
            data.name
        )));
    }

    active.push(data.symbol);
    let result = (|| {
        for member in members {
            let DataMember::Field(field) = member else {
                return Err(MaterializationDiagnostic(format!(
                    "{path} type `{}` contains a sum case",
                    data.name
                )));
            };
            let field_value = supplied.get(field.name.as_str()).ok_or_else(|| {
                MaterializationDiagnostic(format!("{path} has no declared field `{}`", field.name))
            })?;
            validate_value(
                typed,
                field.type_reference,
                field_value,
                &format!("{path}.{}", field.name),
                active,
            )?;
        }
        Ok(())
    })();
    active.pop();
    result
}

pub(super) fn validate_value(
    typed: &TypedTrees,
    type_reference: TypeReferenceHandle,
    value: &BuildTimeValue,
    path: &str,
    active: &mut Vec<psi_symbols::SymbolHandle>,
) -> Result<(), MaterializationDiagnostic> {
    if !type_reference.is_valid() {
        return Err(MaterializationDiagnostic(format!(
            "{path} has an invalid declared type"
        )));
    }
    if matches!(value, BuildTimeValue::Text(_)) {
        return Err(MaterializationDiagnostic(format!(
            "{path} contains Text, which is not ConstMaterializable"
        )));
    }
    if let TypeReferenceNode::Named { name, .. } =
        typed.type_reference_table.type_reference(type_reference)
    {
        if name.as_str().starts_with("Atomic") {
            return Err(MaterializationDiagnostic(format!(
                "{path} has atomic type `{name}`, which is not ConstMaterializable"
            )));
        }
    }
    if let Some(primitive) = typed.primitive_type_reference(type_reference) {
        return match (primitive, value) {
            (PrimitiveType::Bool, BuildTimeValue::Bool(_)) => Ok(()),
            (
                PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64,
                BuildTimeValue::Int(_),
            ) => Ok(()),
            (PrimitiveType::F32, BuildTimeValue::Float(value)) => {
                if value.is_nan() {
                    return Err(MaterializationDiagnostic(format!(
                        "{path} is NaN without an exact raw-NaN realization"
                    )));
                }
                let round_trip = f64::from(*value as f32);
                if round_trip.to_bits() != value.to_bits() {
                    return Err(MaterializationDiagnostic(format!(
                        "{path} does not retain one exact binary32 value"
                    )));
                }
                Ok(())
            }
            (PrimitiveType::F64, BuildTimeValue::Float(value)) => {
                if value.is_nan() {
                    return Err(MaterializationDiagnostic(format!(
                        "{path} is NaN without an exact raw-NaN realization"
                    )));
                }
                Ok(())
            }
            (PrimitiveType::Addr, _) => Err(MaterializationDiagnostic(format!(
                "{path} is an address and is not ConstMaterializable"
            ))),
            _ => Err(MaterializationDiagnostic(format!(
                "{path} value {} does not match primitive `{}`",
                value_kind(value),
                primitive.name()
            ))),
        };
    }

    match typed.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let BuildTimeValue::Array(elements) = value else {
                return Err(MaterializationDiagnostic(format!(
                    "{path} expected a fixed array, found {}",
                    value_kind(value)
                )));
            };
            if elements.len() != *length {
                return Err(MaterializationDiagnostic(format!(
                    "{path} has {} element(s), expected {length}",
                    elements.len()
                )));
            }
            for (index, element) in elements.iter().enumerate() {
                validate_value(
                    typed,
                    *element_type,
                    element,
                    &format!("{path}[{index}]"),
                    active,
                )?;
            }
            Ok(())
        }
        TypeReferenceNode::Named { symbol, name } => {
            if !symbol.is_valid() {
                return Err(MaterializationDiagnostic(format!(
                    "{path} names `{name}` without an exact type identity"
                )));
            }
            let mut matches = typed
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == *symbol);
            let data = matches.next().ok_or_else(|| {
                MaterializationDiagnostic(format!("{path} names unknown record `{name}`"))
            })?;
            if matches.next().is_some() || data.name.as_str() != name.as_str() {
                return Err(MaterializationDiagnostic(format!(
                    "{path} has ambiguous or mismatched nominal identity for `{name}`"
                )));
            }
            validate_record_value(typed, data, value, path, active)
        }
        TypeReferenceNode::FixedArray { .. } => Err(MaterializationDiagnostic(format!(
            "{path} has a non-literal fixed-array length"
        ))),
        TypeReferenceNode::Reference { .. } => Err(MaterializationDiagnostic(format!(
            "{path} is a reference and is not ConstMaterializable"
        ))),
        TypeReferenceNode::Slice { .. } => Err(MaterializationDiagnostic(format!(
            "{path} is a slice and is not ConstMaterializable"
        ))),
        TypeReferenceNode::Generic { .. } => Err(MaterializationDiagnostic(format!(
            "{path} is generic and is not ConstMaterializable"
        ))),
        TypeReferenceNode::DynamicTrait { .. } => Err(MaterializationDiagnostic(format!(
            "{path} is dynamic and is not ConstMaterializable"
        ))),
        TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => Err(MaterializationDiagnostic(format!(
            "{path} is outside the bounded ConstMaterializable value subset"
        ))),
    }
}

pub(super) fn value_kind(value: &BuildTimeValue) -> &'static str {
    match value {
        BuildTimeValue::Unit => "Unit",
        BuildTimeValue::Int(_) => "integer",
        BuildTimeValue::Bool(_) => "Boolean",
        BuildTimeValue::Float(_) => "float",
        BuildTimeValue::Text(_) => "Text",
        BuildTimeValue::Struct { .. } => "record",
        BuildTimeValue::Case { .. } => "sum case",
        BuildTimeValue::Array(_) => "array",
    }
}

fn non_authoritative_materialization_report_fingerprint(
    schema_name: &str,
    schema_identity: u64,
    layout_report_fingerprint: u64,
    byte_order: ByteOrder,
    value: &BuildTimeValue,
    bytes: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_bytes(&mut hash, b"omega.const-materializable.v1");
    hash_text(&mut hash, schema_name);
    hash_u64(&mut hash, schema_identity);
    hash_u64(&mut hash, layout_report_fingerprint);
    hash_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_value(&mut hash, value);
    hash_u64(&mut hash, bytes.len() as u64);
    hash_bytes(&mut hash, bytes);
    if hash == 0 { 1 } else { hash }
}

pub(super) fn hash_value(hash: &mut u64, value: &BuildTimeValue) {
    match value {
        BuildTimeValue::Unit => hash_byte(hash, 0),
        BuildTimeValue::Int(value) => {
            hash_byte(hash, 1);
            hash_bytes(hash, &value.to_le_bytes());
        }
        BuildTimeValue::Bool(value) => {
            hash_byte(hash, 2);
            hash_byte(hash, u8::from(*value));
        }
        BuildTimeValue::Float(value) => {
            hash_byte(hash, 3);
            hash_u64(hash, value.to_bits());
        }
        BuildTimeValue::Text(bytes) => {
            hash_byte(hash, 4);
            hash_u64(hash, bytes.len() as u64);
            hash_bytes(hash, bytes);
        }
        BuildTimeValue::Struct { type_name, fields } => {
            hash_byte(hash, 5);
            hash_text(hash, type_name);
            hash_u64(hash, fields.len() as u64);
            for (name, value) in fields {
                hash_text(hash, name);
                hash_value(hash, value);
            }
        }
        BuildTimeValue::Case { variant, payload } => {
            hash_byte(hash, 6);
            hash_text(hash, variant);
            hash_u64(hash, payload.len() as u64);
            for (name, value) in payload {
                hash_text(hash, name);
                hash_value(hash, value);
            }
        }
        BuildTimeValue::Array(elements) => {
            hash_byte(hash, 7);
            hash_u64(hash, elements.len() as u64);
            for element in elements {
                hash_value(hash, element);
            }
        }
    }
}

pub(super) fn hash_text(hash: &mut u64, value: &str) {
    hash_u64(hash, value.len() as u64);
    hash_bytes(hash, value.as_bytes());
}

pub(super) fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

pub(super) fn hash_bytes(hash: &mut u64, values: &[u8]) {
    for value in values {
        hash_byte(hash, *value);
    }
}

pub(super) fn hash_byte(hash: &mut u64, value: u8) {
    *hash ^= u64::from(value);
    *hash = hash.wrapping_mul(0x100000001b3);
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport};
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;

    use crate::layout_plans::schema_fields;

    const SOURCE: &str = r#"
        data Inner [copy] { enabled: bool; code: u32; }
        data Sample [copy] { tag: u8; inner: Inner; words: [u16; 2]; }
        data Floating [copy] { value: f64; }
        data Floating32 [copy] { value: f32; }
        data FloatSample [copy] {
            narrow: f32;
            wide: f64;
            signed_zero: f64;
            infinite: f32;
        }
        data Choice [copy] { case Number(value: u8); case Empty; }
        data Borrowed [copy] { value: &u8; }
    "#;

    const UNSUPPORTED_SOURCE: &str = r#"
        boundary data Opaque;
        data Generic<T [copy]> [copy] { value: T; }
        data Sliced [copy] { values: [u8]; }
        trait Shape { machine code(&self) -> u8; }
        data Dynamic [copy] { value: dyn Shape; }
        data Carrier [copy] { case Unit; }
        proposition same(left: Carrier, right: Carrier) = left == right;
        data Quotient = Carrier % same;
    "#;

    #[test]
    fn nested_records_and_arrays_replay_exact_bytes_and_zero_padding() {
        let typed = typed(SOURCE);
        let layout = sample_layout(&typed);
        let value = sample_value();

        let little = validate_const_materializable_typed_owned_layout(
            &typed,
            "Sample",
            &layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("closed copy record should be ConstMaterializable");
        assert_eq!(
            little.bytes(),
            &[7, 0, 0, 0, 1, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 2, 1, 4, 3]
        );
        assert_ne!(little.identity(), 0);
        little
            .replay_against(&typed, "Sample", &layout, &value, ByteOrder::LittleEndian)
            .expect("exact inputs replay");

        let big = validate_const_materializable_typed_owned_layout(
            &typed,
            "Sample",
            &layout,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("same value should bind target byte order");
        assert_eq!(
            big.bytes(),
            &[7, 0, 0, 0, 1, 0, 0, 0, 0x11, 0x22, 0x33, 0x44, 1, 2, 3, 4]
        );
        assert_ne!(little.identity(), big.identity());

        let mut destination = [0xa5; 20];
        little
            .apply(&typed, &mut destination)
            .expect("validated evidence copies atomically");
        assert_eq!(&destination[..16], little.bytes());
        assert_eq!(&destination[16..], &[0xa5; 4]);
    }

    #[test]
    fn replay_rejects_every_retained_input_axis_and_preserves_destination() {
        let typed = typed(SOURCE);
        let layout = sample_layout(&typed);
        let value = sample_value();
        let carrier = validate_const_materializable_typed_owned_layout(
            &typed,
            "Sample",
            &layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("fixture should validate");

        let mut wrong_schema = layout.clone();
        wrong_schema.schema_identity ^= 1;
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Sample",
                    &wrong_schema,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        let mut wrong_member = layout.clone();
        wrong_member.entries[0].member_identity = Some(99);
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Sample",
                    &wrong_member,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        let mut wrong_extent = layout.clone();
        wrong_extent.size = Some(17);
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Sample",
                    &wrong_extent,
                    &value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );

        let mut wrong_offsets = layout.clone();
        wrong_offsets.offsets = Some(vec![0, 5, 12]);
        assert!(
            validate_const_materializable_typed_owned_layout(
                &typed,
                "Sample",
                &wrong_offsets,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
        );

        let mut wrong_alignment = layout.clone();
        wrong_alignment.align = 3;
        assert!(
            validate_const_materializable_typed_owned_layout(
                &typed,
                "Sample",
                &wrong_alignment,
                &value,
                ByteOrder::LittleEndian,
            )
            .is_err()
        );

        let mut wrong_value = value.clone();
        let BuildTimeValue::Struct { fields, .. } = &mut wrong_value else {
            unreachable!("fixture is a record")
        };
        fields[0].1 = BuildTimeValue::Int(8);
        assert!(
            carrier
                .replay_against(
                    &typed,
                    "Sample",
                    &layout,
                    &wrong_value,
                    ByteOrder::LittleEndian,
                )
                .is_err()
        );
        assert!(
            carrier
                .replay_against(&typed, "Sample", &layout, &value, ByteOrder::BigEndian,)
                .is_err()
        );

        let mut destination = [0x5a; 15];
        carrier
            .apply(&typed, &mut destination)
            .expect_err("short destination rejects");
        assert_eq!(destination, [0x5a; 15]);

        let mut corrupted = carrier;
        corrupted.bytes[1] = 9;
        let mut destination = [0x6b; 16];
        corrupted
            .apply(&typed, &mut destination)
            .expect_err("stored-byte drift rejects before copying");
        assert_eq!(destination, [0x6b; 16]);
    }

    #[test]
    fn replay_rejects_layout_substitution_when_compact_report_fingerprint_is_forced_equal() {
        let typed = typed(SOURCE);
        let layout = sample_layout(&typed);
        let value = sample_value();
        let mut carrier = validate_const_materializable_typed_owned_layout(
            &typed,
            "Sample",
            &layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("fixture should validate");

        let mut substituted_layout = layout.clone();
        substituted_layout.size = Some(17);
        carrier.non_authoritative_layout_report_fingerprint =
            normalized_layout_plan_fingerprint(&substituted_layout);

        let error = carrier
            .replay_against(
                &typed,
                "Sample",
                &substituted_layout,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect_err("exact retained layout rejects a compact-equal substitute");
        assert!(error.0.contains("layout drifted"));
    }

    #[test]
    fn non_nan_float_leaves_retain_exact_format_bits_and_byte_order() {
        let typed = typed(SOURCE);
        let layout = layout(&typed, "FloatSample", &[0, 8, 16, 24], 32, 8);
        let value = BuildTimeValue::Struct {
            type_name: "FloatSample".into(),
            fields: vec![
                ("narrow".into(), BuildTimeValue::Float(1.5)),
                ("wide".into(), BuildTimeValue::Float(-3.25)),
                ("signed_zero".into(), BuildTimeValue::Float(-0.0)),
                ("infinite".into(), BuildTimeValue::Float(f64::INFINITY)),
            ],
        };

        let little = validate_const_materializable_typed_owned_layout(
            &typed,
            "FloatSample",
            &layout,
            &value,
            ByteOrder::LittleEndian,
        )
        .expect("non-NaN binary32/binary64 leaves should materialize");
        assert_eq!(
            little.bytes(),
            &[
                0x00, 0x00, 0xc0, 0x3f, 0, 0, 0, 0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0xc0,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x80, 0x7f, 0, 0, 0, 0,
            ]
        );

        let big = validate_const_materializable_typed_owned_layout(
            &typed,
            "FloatSample",
            &layout,
            &value,
            ByteOrder::BigEndian,
        )
        .expect("target byte order should remain explicit for float leaves");
        assert_eq!(
            big.bytes(),
            &[
                0x3f, 0xc0, 0x00, 0x00, 0, 0, 0, 0, 0xc0, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x7f, 0x80, 0x00, 0x00, 0, 0, 0, 0,
            ]
        );
        assert_ne!(little.identity(), big.identity());
        little
            .replay_against(
                &typed,
                "FloatSample",
                &layout,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect("exact float custody should replay");
    }

    #[test]
    fn unsupported_or_malformed_value_shapes_fail_closed() {
        let typed = typed(SOURCE);

        let float_layout = one_field_layout(&typed, "Floating", 8, 8);
        let float = BuildTimeValue::Struct {
            type_name: "Floating".into(),
            fields: vec![("value".into(), BuildTimeValue::Float(f64::NAN))],
        };
        let error = validate_const_materializable_typed_owned_layout(
            &typed,
            "Floating",
            &float_layout,
            &float,
            ByteOrder::LittleEndian,
        )
        .expect_err("NaN without exact raw representation evidence remains fenced");
        assert!(error.0.contains("exact raw-NaN realization"), "{error:?}");

        let narrow_layout = one_field_layout(&typed, "Floating32", 4, 4);
        let rounded = BuildTimeValue::Struct {
            type_name: "Floating32".into(),
            fields: vec![("value".into(), BuildTimeValue::Float(0.1))],
        };
        let error = validate_const_materializable_typed_owned_layout(
            &typed,
            "Floating32",
            &narrow_layout,
            &rounded,
            ByteOrder::LittleEndian,
        )
        .expect_err("a forged f64 value cannot acquire binary32 custody by rounding");
        assert!(error.0.contains("exact binary32 value"), "{error:?}");

        let choice = BuildTimeValue::Case {
            variant: "Empty".into(),
            payload: Vec::new(),
        };
        let choice_data = unique_data_by_name(&typed, "Choice").expect("choice");
        let choice_layout = LayoutPlanReport {
            schema_identity: normalized_schema_identity(&typed, choice_data),
            entries: Vec::new(),
            offsets: Some(Vec::new()),
            size: Some(0),
            align: 1,
        };
        assert!(
            validate_const_materializable_typed_owned_layout(
                &typed,
                "Choice",
                &choice_layout,
                &choice,
                ByteOrder::LittleEndian,
            )
            .is_err()
        );

        let borrowed = BuildTimeValue::Struct {
            type_name: "Borrowed".into(),
            fields: vec![("value".into(), BuildTimeValue::Int(1))],
        };
        let borrowed_data = unique_data_by_name(&typed, "Borrowed").expect("borrowed");
        let borrowed_layout = LayoutPlanReport {
            schema_identity: normalized_schema_identity(&typed, borrowed_data),
            entries: Vec::new(),
            offsets: Some(Vec::new()),
            size: Some(0),
            align: 1,
        };
        let error = validate_const_materializable_typed_owned_layout(
            &typed,
            "Borrowed",
            &borrowed_layout,
            &borrowed,
            ByteOrder::LittleEndian,
        )
        .expect_err("references remain fenced");
        assert!(error.0.contains("reference"), "{error:?}");

        let mut malformed = sample_value();
        let BuildTimeValue::Struct { fields, .. } = &mut malformed else {
            unreachable!("fixture is a record")
        };
        fields[2].1 = BuildTimeValue::Text(vec![1, 2, 3, 4]);
        let error = validate_const_materializable_typed_owned_layout(
            &typed,
            "Sample",
            &sample_layout(&typed),
            &malformed,
            ByteOrder::LittleEndian,
        )
        .expect_err("Text cannot substitute for an array");
        assert!(error.0.contains("contains Text"), "{error:?}");
    }

    #[test]
    fn generic_opaque_slice_dynamic_and_quotient_shapes_fail_closed() {
        let typed = typed(UNSUPPORTED_SOURCE);

        let cases = [
            ("Opaque", BuildTimeValue::Unit, "generic, opaque, quotient"),
            (
                "Generic",
                BuildTimeValue::Struct {
                    type_name: "Generic".into(),
                    fields: vec![("value".into(), BuildTimeValue::Int(1))],
                },
                "generic, opaque, quotient",
            ),
            (
                "Sliced",
                BuildTimeValue::Struct {
                    type_name: "Sliced".into(),
                    fields: vec![("values".into(), BuildTimeValue::Array(Vec::new()))],
                },
                "slice",
            ),
            (
                "Dynamic",
                BuildTimeValue::Struct {
                    type_name: "Dynamic".into(),
                    fields: vec![("value".into(), BuildTimeValue::Unit)],
                },
                "dynamic",
            ),
            (
                "Quotient",
                BuildTimeValue::Case {
                    variant: "Unit".into(),
                    payload: Vec::new(),
                },
                "generic, opaque, quotient",
            ),
        ];

        for (schema, value, expected) in cases {
            let data = unique_data_by_name(&typed, schema).expect("fixture data");
            let layout = LayoutPlanReport {
                schema_identity: normalized_schema_identity(&typed, data),
                entries: Vec::new(),
                offsets: Some(Vec::new()),
                size: Some(0),
                align: 1,
            };
            let error = validate_const_materializable_typed_owned_layout(
                &typed,
                schema,
                &layout,
                &value,
                ByteOrder::LittleEndian,
            )
            .expect_err("unsupported shape must reject before byte materialization");
            assert!(error.0.contains(expected), "{schema}: {error:?}");
        }
    }

    fn typed(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn sample_layout(typed: &TypedTrees) -> LayoutPlanReport {
        layout(typed, "Sample", &[0, 4, 12], 16, 4)
    }

    fn one_field_layout(
        typed: &TypedTrees,
        schema: &str,
        size: u64,
        align: u64,
    ) -> LayoutPlanReport {
        layout(typed, schema, &[0], size, align)
    }

    fn layout(
        typed: &TypedTrees,
        schema: &str,
        offsets: &[u64],
        size: u64,
        align: u64,
    ) -> LayoutPlanReport {
        let (fields, schema_identity) = schema_fields(typed, schema).expect("reflect schema");
        assert_eq!(fields.len(), offsets.len());
        LayoutPlanReport {
            schema_identity,
            entries: fields
                .iter()
                .zip(offsets)
                .map(|(field, offset)| LayoutFieldEntryReport {
                    field: field.name.clone(),
                    member_identity: field.identity,
                    placement: LayoutPlacementReport::At { offset: *offset },
                })
                .collect(),
            offsets: Some(offsets.to_vec()),
            size: Some(size),
            align,
        }
    }

    fn sample_value() -> BuildTimeValue {
        BuildTimeValue::Struct {
            type_name: "Sample".into(),
            fields: vec![
                ("tag".into(), BuildTimeValue::Int(7)),
                (
                    "inner".into(),
                    BuildTimeValue::Struct {
                        type_name: "Inner".into(),
                        fields: vec![
                            ("enabled".into(), BuildTimeValue::Bool(true)),
                            ("code".into(), BuildTimeValue::Int(0x1122_3344)),
                        ],
                    },
                ),
                (
                    "words".into(),
                    BuildTimeValue::Array(vec![
                        BuildTimeValue::Int(0x0102),
                        BuildTimeValue::Int(0x0304),
                    ]),
                ),
            ],
        }
    }
}
