//! One declaration roster for entry assumptions and restored field contracts.
//!
//! A `Case` segment makes the remaining field contract conditional on that
//! exact active alternative. Whole-value transport retains the implication;
//! it supplies no tag evidence and cannot authorize payload extraction.
//! Readable-reference traversal belongs to this roster. Owned-result custody
//! keeps its separate no-reference-crossing traversal.

use super::{readable_fixed_array_elements, readable_nominal_definition, readable_type_reference};
use facts::PlaceSegment;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, data::DataMember, types::TypeReferenceHandle};

/// All declared field rows beneath an established readable data value. Exact
/// domain identities and evidence-only extent rows survive until each consumer
/// selects the obligations it owns.
pub(crate) fn declared_data_field_domain_rows(
    program: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
) -> Vec<FieldDomainRow> {
    let mut rows = Vec::new();
    collect_data_field_domain_rows(
        program,
        false,
        data,
        &mut Vec::new(),
        &mut vec![data.symbol],
        &mut rows,
    );
    rows
}

/// Seed the declared field domains of `data` below `path`. The recursion
/// shares one path buffer and one visited stack, restoring both before it
/// returns, so a parameter whose type nests many fields does not copy its
/// prefix for every field.
fn collect_data_field_domain_rows(
    program: &TypedTrees,
    transfer: bool,
    data: &typed_trees::data::DataDefinition,
    path: &mut Vec<PlaceSegment>,
    visited: &mut Vec<SymbolHandle>,
    rows: &mut Vec<FieldDomainRow>,
) {
    for field in program
        .data_members(data)
        .iter()
        .flat_map(|member| match member {
            DataMember::Field(field) => std::slice::from_ref(field),
            DataMember::Variant(variant) => program.data_payload_fields(variant),
        })
    {
        if readable_type_reference(program, field.type_reference).is_none() {
            continue;
        }
        let prefix_length = path.len();
        crate::flow::push_field_place_segments(program, path, field.symbol);
        for (domain_symbol, semantic_domain) in
            crate::facts::field_domain::domain_constraint_identities(program, field.type_reference)
        {
            rows.push(FieldDomainRow {
                path: path.clone(),
                domain_symbol,
                semantic_domain,
                transfer,
            });
        }
        if let Some(nested) = readable_nominal_definition(program, field.type_reference)
            && !visited.contains(&nested.symbol)
        {
            visited.push(nested.symbol);
            collect_data_field_domain_rows(program, transfer, nested, path, visited, rows);
            visited.pop();
        }
        // A slice-typed field's elements carry their declared fields through
        // the whole-extent row alone: no static extent exists to enumerate, so
        // it stays under `origin` -- for a slice the rollup IS the obligation.
        if let Some(element_type) = readable_slice_element(program, field.type_reference)
            && let Some(nested) = readable_nominal_definition(program, element_type)
            && !visited.contains(&nested.symbol)
        {
            visited.push(nested.symbol);
            path.push(WHOLE_ELEMENT_EXTENT);
            collect_data_field_domain_rows(program, transfer, nested, path, visited, rows);
            path.pop();
            visited.pop();
        }
        // A fixed array field enumerates each nominal element's declared fields
        // at `field[i]` -- element coverage rides the same entry facts and the
        // same call-boundary obligations as one-level fields. The whole-extent
        // `field[0..usize::MAX]` row is seeded alongside so a runtime selector
        // or a slice view can narrow coverage from it; it is EVIDENCE ONLY
        // (`StatementTransfer`) because the per-element rows already spell the
        // complete obligation.
        if let Some((element_type, length)) =
            readable_fixed_array_elements(program, field.type_reference)
            && let Some(nested) = readable_nominal_definition(program, element_type)
            && !visited.contains(&nested.symbol)
        {
            visited.push(nested.symbol);
            for index in 0..length {
                path.push(PlaceSegment::FixedIndex { index });
                collect_data_field_domain_rows(program, transfer, nested, path, visited, rows);
                path.pop();
            }
            path.push(WHOLE_ELEMENT_EXTENT);
            collect_data_field_domain_rows(program, true, nested, path, visited, rows);
            path.pop();
            visited.pop();
        }
        path.truncate(prefix_length);
    }
}

/// One declared field domain beneath a parameter's nominal type: the field
/// path from the value, its domain, and whether a whole-element extent on the
/// way makes the row evidence (`StatementTransfer`) rather than an obligation.
/// The rows depend only on the data definition, so every state parameter of
/// that type replays one walk.
pub(crate) struct FieldDomainRow {
    pub(crate) path: Vec<PlaceSegment>,
    pub(crate) domain_symbol: SymbolHandle,
    pub(crate) semantic_domain: language_semantics::SemanticDomainId,
    pub(crate) transfer: bool,
}

/// The `FixedRange` spelling of "every element of this collection": the
/// half-open extent `0..usize::MAX` covers any element index a place can
/// select -- a literal `x[i]` because `i < usize::MAX` always, and a runtime
/// `x[index]` because whichever element the index names sits inside the
/// extent. An elementwise fact at this segment is one row of evidence for
/// every element; a narrower or unresolved index never mints one, and a write
/// to any element retires it through the usual overlap machinery
/// (`flow/place/comparison.rs` already overlaps `Index`/`FixedIndex` with a
/// containing `FixedRange`). Entry seeding and declaration restoration share
/// this coordinate; projection and proof consumers recognize the same extent.
pub(crate) const WHOLE_ELEMENT_EXTENT: PlaceSegment = PlaceSegment::FixedRange {
    start: 0,
    end: usize::MAX,
};

/// The element type of a slice (`[T]`, `&[T]`, `&mut [T]`), looking through a
/// leading reference exactly as `readable_fixed_array_elements` does for fixed
/// arrays. `None` for non-slice or write-only types.
pub(crate) fn readable_slice_element(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program
        .type_reference_table
        .type_reference(readable_type_reference(program, type_reference)?)
    {
        typed_trees::types::TypeReferenceNode::Slice { element_type } => Some(*element_type),
        _ => None,
    }
}
