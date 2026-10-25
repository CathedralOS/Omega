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
        let prefix_length = path.len();
        crate::push_field_place_segments(program, path, field.symbol);
        collect_type_domain_rows(program, transfer, field.type_reference, path, visited, rows);
        path.truncate(prefix_length);
    }
}

/// All live declaration obligations below a readable value, including a
/// qualified scalar element. A collection element's own qualification and a
/// record element's fields use the same extent coordinates and invalidation.
/// This roster declares incoming obligations; it establishes no stored value.
pub fn declared_type_domain_rows(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Vec<FieldDomainRow> {
    let mut rows = Vec::new();
    collect_type_domain_rows(
        program,
        false,
        reference,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut rows,
    );
    rows
}

fn collect_type_domain_rows(
    program: &dyn crate::CheckingInput,
    transfer: bool,
    reference: TypeReferenceHandle,
    path: &mut Vec<PlaceSegment>,
    visited: &mut Vec<SymbolHandle>,
    rows: &mut Vec<FieldDomainRow>,
) {
    if readable_type_reference(program, reference).is_none() {
        return;
    }
    for (domain_symbol, semantic_domain) in super::domain_constraint_identities(program, reference)
    {
        rows.push(FieldDomainRow {
            path: path.clone(),
            domain_symbol,
            semantic_domain,
            transfer,
        });
    }
    if let Some(data) = readable_nominal_definition(program, reference)
        && !visited.contains(&data.symbol)
    {
        visited.push(data.symbol);
        collect_data_field_domain_rows(program, transfer, data, path, visited, rows);
        visited.pop();
    }
    // A slice has no enumerated extent: its whole-extent row is the incoming
    // obligation. A fixed array also seeds exact elements, so its rollup is
    // evidence only. Both preserve the element's normalized domain instance.
    if let Some(element) = readable_slice_element(program, reference) {
        path.push(WHOLE_ELEMENT_EXTENT);
        collect_type_domain_rows(program, transfer, element, path, visited, rows);
        path.pop();
    }
    if let Some((element, length)) = readable_fixed_array_elements(program, reference) {
        let mut element_rows = Vec::new();
        collect_type_domain_rows(
            program,
            transfer,
            element,
            &mut Vec::new(),
            visited,
            &mut element_rows,
        );
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

impl FieldDomainRow {
    /// Whether this declaration already names a selected subject's obligation.
    /// An all-element row applies to any element selection, but every field,
    /// case, and remaining segment must still match. This query neither proves
    /// index bounds nor observes a case tag; those remain access obligations.
    pub fn describes_projection(
        &self,
        path: &[PlaceSegment],
        is_point_index: impl Fn(typed_trees::expression::ExpressionHandle) -> bool,
    ) -> bool {
        self.path.len() == path.len()
            && self.path.iter().zip(path).all(|(declared, selected)| {
                declared == selected
                    || *declared == WHOLE_ELEMENT_EXTENT
                        && match selected {
                            PlaceSegment::FixedIndex { .. } => true,
                            // Canonical Index can retain an unresolved range;
                            // only a classified point selects one element.
                            PlaceSegment::Index { expression } => is_point_index(*expression),
                            _ => false,
                        }
            })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_element_projection_preserves_complete_path() {
        let field = PlaceSegment::Field {
            symbol: SymbolHandle::invalid(),
        };
        let case = PlaceSegment::Case {
            variant: SymbolHandle::invalid(),
        };
        let row = FieldDomainRow {
            path: vec![field, WHOLE_ELEMENT_EXTENT, case, field],
            domain_symbol: SymbolHandle::invalid(),
            semantic_domain: language_semantics::SemanticDomainId::NULL,
            transfer: true,
        };
        let runtime_index = PlaceSegment::Index {
            expression: typed_trees::expression::ExpressionHandle::invalid(),
        };
        let describes = |path: &[PlaceSegment]| row.describes_projection(path, |_| true);
        assert!(describes(&row.path));
        assert!(describes(&[field, runtime_index, case, field]));
        assert!(!row.describes_projection(&[field, runtime_index, case, field], |_| false));
        assert!(describes(&[
            field,
            PlaceSegment::FixedIndex { index: 1 },
            case,
            field
        ]));
        assert!(!describes(&[field, runtime_index, field, field]));
        assert!(!describes(&[field, runtime_index, case]));
        assert!(!describes(&[field, PlaceSegment::OpaqueRange, case, field]));
        assert!(!describes(&[
            field,
            PlaceSegment::FixedRange { start: 0, end: 2 },
            case,
            field
        ]));
        let exact = FieldDomainRow {
            path: vec![PlaceSegment::FixedIndex { index: 0 }],
            ..row
        };
        assert!(!exact.describes_projection(&[runtime_index], |_| true));
        assert!(!exact.describes_projection(&[PlaceSegment::FixedIndex { index: 1 }], |_| true));
    }
}
