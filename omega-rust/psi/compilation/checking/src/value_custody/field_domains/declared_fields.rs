//! Source declaration rows for entry assumptions and restored field contracts.
//!
//! A `Case` segment makes the remaining field contract conditional on that
//! exact active alternative. Whole-value transport retains the implication;
//! it supplies no tag evidence and cannot authorize payload extraction.
//! Readable-reference traversal belongs to this roster. Owned-result custody
//! keeps its separate no-reference-crossing traversal.

use super::{readable_fixed_array_elements, readable_nominal_definition, readable_type_reference};
use facts::PlaceSegment;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::types::TypeReferenceHandle;

/// All declared field rows beneath an established readable data value. Exact
/// domain identities and evidence-only extent rows survive until each consumer
/// selects the obligations it owns.
pub fn declared_data_field_domain_rows(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    transfer: bool,
    data: &typed_trees::data::DataDefinition,
    path: &mut Vec<PlaceSegment>,
    visited: &mut Vec<SymbolHandle>,
    rows: &mut Vec<FieldDomainRow>,
) {
    for field in program.data_member_fields(program.data_members(data)) {
        if readable_type_reference(program, field.type_reference).is_none() {
            continue;
        }
        let prefix_length = path.len();
        crate::push_field_place_segments(program, path, field.symbol);
        for (domain_symbol, semantic_domain) in
            super::domain_constraint_identities(program, field.type_reference)
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
        //
        // Every element declares the same rows, so the element is walked once
        // and its rows are placed under each index. An element that declares
        // none adds no row at any index, however long the array.
        if let Some((element_type, length)) =
            readable_fixed_array_elements(program, field.type_reference)
            && let Some(nested) = readable_nominal_definition(program, element_type)
            && !visited.contains(&nested.symbol)
        {
            visited.push(nested.symbol);
            let mut element_rows = Vec::new();
            collect_data_field_domain_rows(
                program,
                transfer,
                nested,
                &mut Vec::new(),
                visited,
                &mut element_rows,
            );
            visited.pop();
            if !element_rows.is_empty() {
                for index in 0..length {
                    place_element_rows(
                        path,
                        PlaceSegment::FixedIndex { index },
                        &element_rows,
                        None,
                        rows,
                    );
                }
                place_element_rows(path, WHOLE_ELEMENT_EXTENT, &element_rows, Some(true), rows);
            }
        }
        path.truncate(prefix_length);
    }
}

/// Place one element's rows below `prefix` at `segment`, exactly as walking
/// the element there would produce them; `transfer` overrides each row's flag
/// as the evidence-only whole-element extent does.
fn place_element_rows(
    prefix: &[PlaceSegment],
    segment: PlaceSegment,
    element_rows: &[FieldDomainRow],
    transfer: Option<bool>,
    rows: &mut Vec<FieldDomainRow>,
) {
    for row in element_rows {
        let mut path = Vec::with_capacity(prefix.len() + 1 + row.path.len());
        path.extend_from_slice(prefix);
        path.push(segment);
        path.extend_from_slice(&row.path);
        rows.push(FieldDomainRow {
            path,
            domain_symbol: row.domain_symbol,
            semantic_domain: row.semantic_domain,
            transfer: transfer.unwrap_or(row.transfer),
        });
    }
}

/// One declared field domain beneath a parameter's nominal type: the field
/// path from the value, its domain, and whether a whole-element extent on the
/// way makes the row evidence (`StatementTransfer`) rather than an obligation.
/// The rows depend only on the data definition, so every state parameter of
/// that type replays one walk.
pub struct FieldDomainRow {
    pub path: Vec<PlaceSegment>,
    pub domain_symbol: SymbolHandle,
    pub semantic_domain: language_semantics::SemanticDomainId,
    pub transfer: bool,
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
pub const WHOLE_ELEMENT_EXTENT: PlaceSegment = PlaceSegment::FixedRange {
    start: 0,
    end: usize::MAX,
};

/// The declared field carrying `symbol`, or `None` when `symbol` does not
/// name a field. A projection symbol produced by exact member resolution is
/// itself the provenance this lookup reads.
pub fn exact_declared_field<'program>(
    program: &'program dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&'program typed_trees::data::DataField> {
    if !symbol.is_valid() || program.symbols.get(symbol).kind != SymbolKind::Field {
        return None;
    }
    program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_member_fields(program.data_members(data)))
        .find(|field| field.symbol == symbol)
}

/// The element type of a slice (`[T]`, `&[T]`, `&mut [T]`), looking through a
/// leading reference exactly as `readable_fixed_array_elements` does for fixed
/// arrays. `None` for non-slice or write-only types.
pub fn readable_slice_element(
    program: &dyn crate::CheckingInput,
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
