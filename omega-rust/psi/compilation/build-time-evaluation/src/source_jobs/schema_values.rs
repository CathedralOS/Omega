//! Ordinary Schema construction and returned-layout projection. The actual
//! invocation receipt retains the whole value; this projection grants no access
//! permission and never substitutes for geometry or access-plan validation.

use crate::{BuildTimeValue, PureTerminalEvaluationOutcome, PureTerminalEvaluationReceipt};
use language_core::core_vocabulary::CoreSource;
use terminal_psi::{TargetLayoutSchemaArgument, TargetLayoutSchemaField};
use typed_trees::{data::DataMember, types::TypeReferenceHandle};

const CAPACITY: usize = 32;
const ABI_VERSION: u16 = 4;

pub(super) fn schema_value(
    geometry_receipt: &PureTerminalEvaluationReceipt,
    policy_source: &typed_trees::TypedTrees,
    schema_type: TypeReferenceHandle,
    retained: &TargetLayoutSchemaArgument,
) -> Result<BuildTimeValue, String> {
    let schema = policy_source
        .closed_data_definition(schema_type)
        .ok_or("policy Schema formal has no exact closed declaration")?;
    if policy_source.symbols.core_source(schema.symbol) != Some(CoreSource::Layout)
        || policy_source.symbols.display_path(schema.symbol, "::") != "Schema"
    {
        return Err("policy Schema formal is not the unchanged bundled declaration".into());
    }
    if retained.abi_version != ABI_VERSION {
        return Err("policy Schema input has an unsupported retained ABI revision".into());
    }
    let source = geometry_receipt.source();
    let semantic_type = super::schema_geometry::query_reference(geometry_receipt)?;
    let row = super::schema_geometry::source_row(geometry_receipt)?;
    let semantic = source
        .closed_data_definition(semantic_type)
        .ok_or("policy semantic type has no exact closed application")?;
    let declared = source
        .data_members(semantic)
        .iter()
        .filter_map(DataMember::as_field)
        .filter(|field| !field.relevance.is_erased())
        .collect::<Vec<_>>();
    if source
        .data_members(semantic)
        .iter()
        .any(|member| matches!(member, DataMember::Variant(_)))
    {
        return Err("policy schema requires its complete sum/case geometry producer".into());
    }
    let fields = &retained.fields;
    let retired = &retained.retired_field_identities;
    if fields.len() != declared.len() || fields.len() > CAPACITY || retired.len() > CAPACITY {
        return Err(
            "policy Schema input differs from its complete source field roster or capacity".into(),
        );
    }
    for (position, field) in fields.iter().enumerate() {
        let mut matching = row.fields.iter().filter(|source| {
            source.field == field.record_field && source.field == field.schema_field
        });
        let actual = matching
            .next()
            .ok_or("policy Schema field lost its actual catalogue coordinate")?;
        if matching.next().is_some() {
            return Err("policy Schema field has ambiguous source correspondence".into());
        }
        let mut members = declared
            .iter()
            .filter(|member| member.symbol == actual.declaration);
        let member = members
            .next()
            .ok_or("policy Schema field belongs to another source declaration")?;
        if members.next().is_some()
            || actual.schema != member.symbol
            || member.identity != field.identity
            || field.key == 0
            || !(1..=16).contains(&field.byte_alignment)
            || fields[..position]
                .iter()
                .any(|prior| prior.record_field == field.record_field || prior.key == field.key)
        {
            return Err(
                "policy Schema field lost its unique source/geometry correspondence".into(),
            );
        }
    }
    let mut declared_retired = semantic.retired_identities.clone();
    declared_retired.sort_unstable();
    if *retired != declared_retired
        || retired.windows(2).any(|pair| pair[0] == pair[1])
        || fields
            .iter()
            .filter_map(|field| field.identity)
            .any(|identity| retired.binary_search(&identity).is_ok())
    {
        return Err("policy Schema input changed its source retired identities".into());
    }
    Ok(record(
        "Schema",
        vec![
            (
                "fields",
                BuildTimeValue::Array(
                    (0..CAPACITY)
                        .map(|index| field_value(fields.get(index)))
                        .collect(),
                ),
            ),
            ("field_count", integer(fields.len() as u64)),
            ("retired_field_identities", identities(retired)),
            (
                "retired_field_identity_count",
                integer(retired.len() as u64),
            ),
            (
                "cases",
                BuildTimeValue::Array((0..CAPACITY).map(|_| empty_case()).collect()),
            ),
            ("case_count", integer(0)),
            ("retired_case_identities", identities(&[])),
            ("retired_case_identity_count", integer(0)),
        ],
    ))
}

fn integer(value: u64) -> BuildTimeValue {
    // BuildTimeValue::Int retains the complete u64 bit pattern; the actual
    // formal determines its signedness in the shared invocation constructor.
    BuildTimeValue::Int(value as i64)
}
fn record(name: &str, fields: Vec<(&str, BuildTimeValue)>) -> BuildTimeValue {
    BuildTimeValue::Struct {
        type_name: name.into(),
        fields: fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    }
}
fn case(name: &str) -> BuildTimeValue {
    BuildTimeValue::Case {
        variant: name.into(),
        payload: Vec::new(),
    }
}
fn optional_identity(identity: Option<u64>) -> BuildTimeValue {
    match identity {
        Some(identity) => BuildTimeValue::Case {
            variant: "Some".into(),
            payload: vec![("value".into(), integer(identity))],
        },
        None => case("None"),
    }
}
fn identities(values: &[u64]) -> BuildTimeValue {
    BuildTimeValue::Array(
        (0..CAPACITY)
            .map(|index| integer(values.get(index).copied().unwrap_or(0)))
            .collect(),
    )
}
fn field_value(field: Option<&TargetLayoutSchemaField>) -> BuildTimeValue {
    use terminal_psi::{
        TargetLayoutFieldLayoutSource as Layout, TargetLayoutSchemaFieldKind as Kind,
    };
    let (key, size, alignment, identity, kind, layout) =
        field.map_or((0, 0, 1, None, "Scalar", "Default"), |field| {
            (
                field.key,
                field.byte_size,
                field.byte_alignment,
                field.identity,
                match field.kind {
                    Kind::Scalar => "Scalar",
                    Kind::Nested => "Nested",
                    Kind::Repeated => "Repeated",
                },
                match field.layout_source {
                    Layout::Default => "Default",
                    Layout::Scalar => "Scalar",
                    Layout::Explicit => "Explicit",
                },
            )
        });
    record(
        "SchemaField",
        vec![
            ("key", integer(key)),
            ("size", integer(size)),
            ("align", integer(alignment)),
            ("identity", optional_identity(identity)),
            (
                "number",
                BuildTimeValue::Int(identity.map_or(-1, |identity| identity as i64)),
            ),
            ("kind", case(kind)),
            ("layout_source", case(layout)),
        ],
    )
}
fn empty_case() -> BuildTimeValue {
    record(
        "SchemaCase",
        vec![
            ("key", integer(0)),
            ("identity", optional_identity(None)),
            (
                "payload_fields",
                BuildTimeValue::Array((0..CAPACITY).map(|_| field_value(None)).collect()),
            ),
            ("payload_field_count", integer(0)),
            ("retired_payload_identities", identities(&[])),
            ("retired_payload_identity_count", integer(0)),
        ],
    )
}

/// Decode ordinary returned data. This does not validate geometry or mint an
/// installation: the shared layout validator must consume the report together
/// with this exact source/geometry roster before any projection uses it.
pub(super) fn layout_value(
    geometry_receipt: &PureTerminalEvaluationReceipt,
    policy_receipt: &PureTerminalEvaluationReceipt,
    retained: &TargetLayoutSchemaArgument,
) -> Result<
    (
        layout_plans::LayoutPlanReport,
        Vec<terminal_psi::TargetLayoutFieldStorage>,
    ),
    String,
> {
    use layout_plans::{IntegerInterpretation, LayoutFieldEntryReport, LayoutPlacementReport};
    if geometry_receipt.admission().target() != policy_receipt.admission().target() {
        return Err("policy result belongs to another geometry target".into());
    }
    let source = geometry_receipt.source();
    let semantic_type = super::schema_geometry::query_reference(geometry_receipt)?;
    let row = super::schema_geometry::source_row(geometry_receipt)?;
    let semantic = source
        .closed_data_definition(semantic_type)
        .ok_or("policy result lost its exact semantic application")?;
    let (schema_position, schema_type) = schema_parameter(policy_receipt)?;
    let expected = schema_value(
        geometry_receipt,
        policy_receipt.source(),
        schema_type,
        retained,
    )?;
    if policy_receipt.arguments().get(schema_position) != Some(&expected) {
        return Err("policy result is not bound to this exact Schema input".into());
    }
    let value = returned_layout(policy_receipt)?;
    let supplied = record_fields(
        value,
        &[
            "entries",
            "entry_count",
            "size_fixed",
            "size_is_dynamic",
            "align",
        ],
    )?;
    let BuildTimeValue::Array(entries) = field(supplied, "entries")? else {
        return Err("policy LayoutPlan entries is not its actual returned array".into());
    };
    if entries.len() != 64 {
        return Err("policy LayoutPlan entries changed its declared 64-element extent".into());
    }
    let count = unsigned(field(supplied, "entry_count")?)?;
    if count > entries.len() as u64 {
        return Err("policy LayoutPlan entry_count exceeds its actual array".into());
    }
    let fields = &retained.fields;
    let mut decoded = Vec::with_capacity(count as usize);
    for entry in entries.iter().take(count as usize) {
        let entry = record_fields(entry, &["key", "representation"])?;
        let key = unsigned(field(entry, "key")?)?;
        let mut matching = fields.iter().filter(|field| field.key == key);
        let selected = matching
            .next()
            .ok_or("policy result names a key outside its actual Schema input")?;
        if key == 0 || matching.next().is_some() {
            return Err("policy result key does not select one exact Schema field".into());
        }
        let mut correspondences = row
            .fields
            .iter()
            .filter(|field| field.field == selected.record_field);
        let actual = correspondences
            .next()
            .ok_or("policy result lost its exact structural field coordinate")?;
        if correspondences.next().is_some() || selected.schema_field != actual.field {
            return Err("policy result has ambiguous source/schema field correspondence".into());
        }
        let mut members = source
            .data_members(semantic)
            .iter()
            .filter_map(DataMember::as_field)
            .filter(|member| member.symbol == actual.declaration);
        let member = members
            .next()
            .ok_or("policy result key lost its exact authored member")?;
        if members.next().is_some()
            || member.identity != selected.identity
            || actual.schema != member.symbol
        {
            return Err("policy result key has ambiguous source correspondence".into());
        }
        let representation = field(entry, "representation")?;
        let BuildTimeValue::Case { variant, payload } = representation else {
            return Err("policy representation is not its actual returned alternative".into());
        };
        let placement = match variant.as_str() {
            "At" => {
                require_fields(payload, &["offset"])?;
                LayoutPlacementReport::At {
                    offset: unsigned(field(payload, "offset")?)?,
                }
            }
            "IntegerAt" => {
                require_fields(payload, &["offset", "stored_width", "interpretation"])?;
                let BuildTimeValue::Case {
                    variant,
                    payload: interpretation_payload,
                } = field(payload, "interpretation")?
                else {
                    return Err(
                        "policy integer interpretation is not a returned alternative".into(),
                    );
                };
                require_fields(interpretation_payload, &[])?;
                let interpretation = match variant.as_str() {
                    "Signed" => IntegerInterpretation::Signed,
                    "Unsigned" => IntegerInterpretation::Unsigned,
                    _ => return Err("policy integer interpretation is not served".into()),
                };
                LayoutPlacementReport::IntegerAt {
                    offset: unsigned(field(payload, "offset")?)?,
                    stored_width: unsigned(field(payload, "stored_width")?)?,
                    interpretation,
                }
            }
            "Bits" => {
                require_fields(
                    payload,
                    &[
                        "container",
                        "container_width",
                        "destination_lsb",
                        "source_lsb",
                        "width",
                    ],
                )?;
                LayoutPlacementReport::Bits {
                    container: unsigned(field(payload, "container")?)?,
                    container_width: unsigned(field(payload, "container_width")?)?,
                    destination_lsb: unsigned(field(payload, "destination_lsb")?)?,
                    source_lsb: unsigned(field(payload, "source_lsb")?)?,
                    width: unsigned(field(payload, "width")?)?,
                }
            }
            _ => return Err("policy result is not a fixed-memory field representation".into()),
        };
        decoded.push(LayoutFieldEntryReport {
            field: member.name.as_str().into(),
            member_identity: member.identity,
            placement,
        });
    }
    let BuildTimeValue::Bool(dynamic) = field(supplied, "size_is_dynamic")? else {
        return Err("policy LayoutPlan dynamic-size flag is not a Boolean".into());
    };
    let report = layout_plans::LayoutPlanReport {
        schema_report_fingerprint:
            typed_trees::names::identity::normalized_schema_report_fingerprint(source, semantic),
        entries: decoded,
        offsets: None,
        size: if *dynamic {
            None
        } else {
            Some(unsigned(field(supplied, "size_fixed")?)?)
        },
        align: unsigned(field(supplied, "align")?)?,
    };
    let adopted = adopt_layout(geometry_receipt, retained, &report)?;
    Ok((report, adopted))
}

fn unsigned(value: &BuildTimeValue) -> Result<u64, String> {
    match value {
        // The exact core formal is u64, not an untyped signed integer.
        BuildTimeValue::Int(value) => Ok(*value as u64),
        _ => Err("policy output lost its declared u64 carrier".into()),
    }
}
fn record_fields<'value>(
    value: &'value BuildTimeValue,
    names: &[&str],
) -> Result<&'value [(String, BuildTimeValue)], String> {
    let BuildTimeValue::Struct { fields, .. } = value else {
        return Err("policy value is not the actual returned record".into());
    };
    require_fields(fields, names)?;
    Ok(fields)
}
fn require_fields(fields: &[(String, BuildTimeValue)], names: &[&str]) -> Result<(), String> {
    if fields.len() != names.len()
        || names
            .iter()
            .any(|name| fields.iter().filter(|(actual, _)| actual == name).count() != 1)
    {
        return Err("policy value differs from its complete declared field roster".into());
    }
    Ok(())
}
fn field<'value>(
    fields: &'value [(String, BuildTimeValue)],
    name: &str,
) -> Result<&'value BuildTimeValue, String> {
    let mut matching = fields.iter().filter(|(actual, _)| actual == name);
    let (_, value) = matching
        .next()
        .ok_or_else(|| format!("policy value is missing `{name}`"))?;
    if matching.next().is_some() {
        return Err(format!("policy value repeats `{name}`"));
    }
    Ok(value)
}

fn schema_parameter(
    receipt: &PureTerminalEvaluationReceipt,
) -> Result<(usize, TypeReferenceHandle), String> {
    schema_parameter_in_source(receipt.source(), receipt.machine())
}

pub(super) fn schema_parameter_in_source(
    source: &typed_trees::TypedTrees,
    selected: symbols::SymbolHandle,
) -> Result<(usize, TypeReferenceHandle), String> {
    let machine = source
        .machine_by_symbol(selected)
        .ok_or("policy invocation lost its actual selected machine")?;
    let [entry] = source.machine_states(machine) else {
        return Err("policy invocation has no unique entry signature".into());
    };
    let mut candidates = source
        .state_parameters(entry)
        .iter()
        .enumerate()
        .filter_map(|(position, parameter)| {
            let data = source.closed_data_definition(parameter.type_reference)?;
            (source.symbols.core_source(data.symbol) == Some(CoreSource::Layout)
                && source.symbols.display_path(data.symbol, "::") == "Schema")
                .then_some((position, parameter.type_reference))
        });
    let selected = candidates
        .next()
        .ok_or("policy invocation has no actual core Schema parameter")?;
    if candidates.next().is_some() {
        return Err("policy invocation has ambiguous Schema input positions".into());
    }
    Ok(selected)
}

fn returned_layout(receipt: &PureTerminalEvaluationReceipt) -> Result<&BuildTimeValue, String> {
    let source = receipt.source();
    let machine = source
        .machine_by_symbol(receipt.machine())
        .ok_or("policy result lost its selected machine")?;
    let [entry] = source.machine_states(machine) else {
        return Err("policy result has no unique return signature".into());
    };
    let data = source
        .closed_data_definition(entry.return_type)
        .ok_or("policy result has no closed returned declaration")?;
    if source.symbols.core_source(data.symbol) != Some(CoreSource::Layout) {
        return Err("policy returned another declaration, not a core plan".into());
    }
    let PureTerminalEvaluationOutcome::Materialized { value, .. } = receipt.outcome() else {
        return Err("policy did not complete with an actual materialized result".into());
    };
    match source.symbols.display_path(data.symbol, "::").as_str() {
        "LayoutPlan" => Ok(value),
        "AccessPlan" => {
            // Project geometry only. The whole ordinary AccessPlan remains in
            // the actual receipt; this grants none of its access decisions.
            let returned = record_fields(value, &["layout", "fields", "field_count", "reach"])?;
            field(returned, "layout")
        }
        _ => Err("policy did not return an actual core LayoutPlan or AccessPlan".into()),
    }
}

// Adopt an ordinary result into the existing field-storage recipes. Exact
// input coverage belongs here; extents, fragments and destination disjointness
// use the same geometry operations as the foreign artifact boundary.
fn adopt_layout(
    receipt: &PureTerminalEvaluationReceipt,
    schema: &TargetLayoutSchemaArgument,
    report: &layout_plans::LayoutPlanReport,
) -> Result<Vec<terminal_psi::TargetLayoutFieldStorage>, String> {
    use layout_plans::LayoutPlacementReport as Placement;
    use semantic_vocabulary::{AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage};
    let size = report
        .size
        .ok_or("policy returned a dynamic plan for fixed storage")?;
    if !report.align.is_power_of_two() || !size.is_multiple_of(report.align) {
        return Err("policy fixed storage has invalid size or alignment".into());
    }
    let row = super::schema_geometry::source_row(receipt)?;
    let module = receipt.program().view().module();
    let declaration = module
        .structural_type(row.record)
        .ok_or("policy placement lost its actual record declaration")?;
    let terminal_psi::StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err("policy fixed placement requires its actual record catalogue".into());
    };
    let source = receipt.source();
    let semantic_type = super::schema_geometry::query_reference(receipt)?;
    let semantic = source
        .closed_data_definition(semantic_type)
        .ok_or("policy placement lost its closed source declaration")?;
    let mut destinations = Vec::with_capacity(report.entries.len());
    let mut adopted = Vec::with_capacity(schema.fields.len());
    for input in &schema.fields {
        let field = fields
            .iter()
            .find(|field| field.id == input.record_field)
            .ok_or("policy placement lost its exact field type")?;
        let correspondence = row
            .fields
            .iter()
            .find(|field| field.field == input.record_field)
            .ok_or("policy placement lost its exact source field")?;
        let member = source
            .data_members(semantic)
            .iter()
            .filter_map(DataMember::as_field)
            .find(|field| field.symbol == correspondence.declaration)
            .ok_or("policy placement lost its authored field")?;
        let entries = report
            .entries
            .iter()
            .filter(|entry| {
                entry.field == member.name.as_str() && entry.member_identity == member.identity
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(format!("policy plan omits field `{}`", member.name));
        }
        let storage = if let [entry] = entries.as_slice()
            && let Placement::At { offset } = entry.placement
        {
            if !offset.is_multiple_of(input.byte_alignment)
                || !report.align.is_multiple_of(input.byte_alignment)
            {
                return Err(format!(
                    "policy whole-field placement misaligns `{}`",
                    member.name
                ));
            }
            AuthoredFieldStorage::Direct {
                byte_offset: offset,
            }
        } else if let [entry] = entries.as_slice()
            && let Placement::IntegerAt {
                offset,
                stored_width,
                interpretation,
            } = entry.placement
        {
            let stored_scalar =
                stored_integer(module, &field.field_type, stored_width, interpretation)?;
            AuthoredFieldStorage::Scalar {
                byte_offset: offset,
                stored_scalar: Some(stored_scalar),
            }
        } else {
            let fragments = entries
                .iter()
                .map(|entry| {
                    let Placement::Bits {
                        container,
                        container_width,
                        destination_lsb,
                        source_lsb,
                        width,
                    } = entry.placement
                    else {
                        return Err(format!(
                            "policy field `{}` mixes or repeats whole/integer placements",
                            member.name
                        ));
                    };
                    Ok(AuthoredBitFragment {
                        container_byte_offset: container,
                        container_width_bits: container_width,
                        destination_lsb,
                        source_lsb,
                        width,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            AuthoredFieldStorage::Fragmented(AuthoredBitFragments::new(fragments))
        };
        let extent = terminal_semantics::authored_field_storage_extent(
            module.structural_types.iter(),
            field,
            &storage,
            size,
            |child| {
                (field.field_type.as_structural() == Some(child))
                    .then_some((input.byte_size, input.byte_alignment))
            },
        )
        .map_err(|reason| format!("policy field `{}`: {reason}", member.name))?;
        if let AuthoredFieldStorage::Fragmented(recipe) = &storage {
            for fragment in recipe.fragments() {
                let start = u128::from(fragment.container_byte_offset) * 8
                    + u128::from(fragment.destination_lsb);
                destinations.push((start, start + u128::from(fragment.width)));
            }
        } else {
            let (offset, extent) = extent.ok_or("policy field has no actual resolved extent")?;
            if extent != 0 {
                let start = u128::from(offset) * 8;
                destinations.push((start, start + u128::from(extent) * 8));
            }
        }
        adopted.push(terminal_psi::TargetLayoutFieldStorage {
            field: input.record_field,
            storage,
        });
    }
    terminal_semantics::validate_storage_destination_intervals(destinations)
        .map_err(|reason| format!("policy placement: {reason}"))?;
    // Schema order follows stable numbers; canonical storage follows the
    // actual record roster. Both orders retain the same exact field IDs.
    let mut ordered = Vec::with_capacity(adopted.len());
    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let position = adopted
            .iter()
            .position(|row| row.field == field.id)
            .ok_or("adopted policy storage omitted an actual record field")?;
        ordered.push(adopted.swap_remove(position));
    }
    if !adopted.is_empty() {
        return Err("adopted policy storage names a foreign field".into());
    }
    Ok(ordered)
}

fn stored_integer(
    module: &terminal_psi::TerminalModule,
    field: &terminal_psi::StructuralFieldType,
    stored_width: u64,
    interpretation: layout_plans::IntegerInterpretation,
) -> Result<semantic_vocabulary::ScalarType, String> {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
    use terminal_psi::{StructuralFieldType as Field, StructuralTypeShape as Shape};
    if stored_width == 0 || stored_width > 64 || !stored_width.is_multiple_of(8) {
        return Err("policy integer storage width is not a served complete byte carrier".into());
    }
    let (integer, bounds) = match field {
        Field::Scalar(ScalarType::Integer(integer)) => (*integer, None),
        Field::BoundedInteger(bounds) => (bounds.integer_type(), Some(*bounds)),
        Field::Structural(identity) => {
            let data = module
                .structural_type(*identity)
                .ok_or("policy integer lost its exact carrier")?;
            let Shape::PrimitiveScalar(ScalarType::Integer(integer)) = data.shape else {
                return Err("policy integer placement requires an integer semantic carrier".into());
            };
            if !data.requires.is_empty()
                || data.quotient_carrier.is_some()
                || data.scalar_restriction.is_some()
            {
                return Err("policy integer decode has unserved semantic carrier premises".into());
            }
            (integer, None)
        }
        _ => return Err("policy integer decode requires its checked integer range".into()),
    };
    let sign = match interpretation {
        layout_plans::IntegerInterpretation::Signed => IntegerSign::Signed,
        layout_plans::IntegerInterpretation::Unsigned => IntegerSign::Unsigned,
    };
    let stored = ScalarType::Integer(
        IntegerType::new(
            sign,
            u16::try_from(stored_width)
                .map_err(|_| "policy stored width exceeds its scalar carrier")?,
        )
        .map_err(|reason| format!("policy stored scalar: {reason}"))?,
    );
    if !terminal_semantics::reference_storage_view_is_compatible(
        stored,
        ScalarType::Integer(integer),
    ) {
        return Err("policy integer decode has incompatible stored and logical carriers".into());
    }
    let (minimum, maximum) = interpretation.stored_bounds(stored_width);
    let convert = |value| match integer.sign() {
        IntegerSign::Signed => Ok(IntegerValue::Signed(value)),
        IntegerSign::Unsigned => u128::try_from(value)
            .map(IntegerValue::Unsigned)
            .map_err(|_| "signed policy storage cannot decode into unsigned semantics"),
    };
    for value in [convert(minimum)?, convert(maximum)?] {
        if !integer.admits(value) || bounds.is_some_and(|bounds| !bounds.contains(value)) {
            return Err("policy stored integer range exceeds its checked semantic range".into());
        }
    }
    Ok(stored)
}
