/*
The tag tables in `wiki/spec/terminal-psi/encoding.md` are closed: a tag the
table omits must reject, and a tag it lists must decode. These tests read each
marked table straight from the page and feed every `u8` to the codec's decoder
for that space, so a codec tag the page does not list, or a page row the codec
no longer accepts, fails here instead of surviving as silent drift.

Each probe is the lone tag byte. A listed tag then either decodes (a form with
no fields) or runs out of input while reading its fields; only an omitted tag
returns `InvalidTag` for this space. Decoders that read past the tag before
matching it cannot be probed this way and are not listed.
*/

use crate::CodecError;
use crate::wire_fields::wire::Reader;
use crate::wire_fields::Field;

const ENCODING_PAGE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../wiki/spec/terminal-psi/encoding.md"
));

type Probe = fn(&mut Reader<'_>) -> Result<(), CodecError>;

/// Returns the live tags in the table under `<!-- marker -->`. A row named
/// `retired` records a tag that must reject, so it is not returned.
fn listed_tags(marker: &str) -> Vec<u8> {
    let anchor = format!("<!-- {marker} -->");
    let mut lines = ENCODING_PAGE
        .lines()
        .skip_while(|line| line.trim() != anchor)
        .skip(1)
        .skip_while(|line| line.trim().is_empty());
    let header = lines.next().unwrap_or_default();
    assert!(header.starts_with('|'), "`{anchor}` is not followed by a table");
    lines
        .skip(1)
        .take_while(|line| line.starts_with('|'))
        .filter(|row| row.split('|').nth(2).is_none_or(|name| name.trim() != "retired"))
        .map(|row| {
            let cell = row.split('|').nth(1).unwrap_or_default().trim();
            cell.parse()
                .unwrap_or_else(|_| panic!("`{marker}` row has a non-`u8` tag: {row}"))
        })
        .collect()
}

fn assert_table_matches_decoder(marker: &str, space: &str, probe: Probe) {
    let listed = listed_tags(marker);
    assert!(!listed.is_empty(), "`{marker}` lists no tags");
    let mut drift = Vec::new();
    for tag in 0..=u8::MAX {
        let rejected = matches!(
            probe(&mut Reader::new(&[tag])),
            Err(CodecError::InvalidTag(name, rejected)) if name == space && rejected == tag
        );
        match (listed.contains(&tag), rejected) {
            (true, true) => drift.push(format!("lists {tag}, which the decoder rejects")),
            (false, false) => drift.push(format!("omits {tag}, which the decoder accepts")),
            _ => {}
        }
    }
    assert!(drift.is_empty(), "`{marker}` disagrees with the {space} decoder: {drift:?}");
}

#[test]
fn domain_establishment_routes_match_the_page() {
    assert_table_matches_decoder(
        "domain-establishment-route-tags",
        "StructuralEstablishmentRoute",
        |reader| crate::module_wire::declaration_wire::Route::read(reader).map(drop),
    );
}

#[test]
fn ieee_float_comparison_kinds_match_the_page() {
    assert_table_matches_decoder(
        "ieee-comparison-kind-tags",
        "IeeeFloatComparisonKind",
        |reader| {
            crate::structural_wire::structural_field_wire::decode_ieee_float_comparison_kind(
                reader,
            )
            .map(drop)
        },
    );
}

#[test]
fn operations_match_the_page() {
    assert_table_matches_decoder("operation-tags", "OperationKind", |reader| {
        crate::block_wire::decode_operation_kind(reader).map(drop)
    });
}

#[test]
fn terminators_match_the_page() {
    assert_table_matches_decoder("terminator-tags", "Terminator", |reader| {
        crate::block_wire::terminator_wire::TerminatorRow::read(reader).map(drop)
    });
}

#[test]
fn scalar_types_match_the_page() {
    assert_table_matches_decoder("scalar-type-tags", "ScalarType", |reader| {
        crate::term_wire::scalar_wire::decode_scalar_type(reader).map(drop)
    });
}

#[test]
fn ieee_float_values_match_the_page() {
    assert_table_matches_decoder("ieee-float-value-tags", "IeeeFloatValue", |reader| {
        crate::term_wire::scalar_wire::decode_ieee_float_value(reader).map(drop)
    });
}
