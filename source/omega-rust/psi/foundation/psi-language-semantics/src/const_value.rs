//! Canonical, compiler-generated const-generic value atoms.
//!
//! Generic arguments historically carry integer values as unnameable decimal
//! `Named` leaves. Structured const values use the same erased carrier, but a
//! reserved length-delimited atom keeps them disjoint from source-spellable
//! type names. The canonical encoding, not the source expression or evaluation
//! trace, is semantic identity. `display` is canonical diagnostic text and is
//! included in the atom only so every downstream tree can render the value
//! without retaining the pre-resolution expression arena.

const PREFIX: &str = "#omega-const:";
const MAX_ENCODING_BYTES: usize = 64 * 1024;
const MAX_DECODED_NODES: usize = 4_096;
const MAX_DECODE_DEPTH: usize = 64;

/// A completely decoded canonical const-value encoding.
///
/// This is an immutable syntax-free view for consumers that must replay the
/// evaluated value against an independently known type. In particular, the
/// names here are encoded claims rather than resolved type authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedCanonicalConstValue {
    Integer {
        type_name: String,
        value: i128,
    },
    Boolean(bool),
    Array {
        type_name: String,
        values: Vec<Self>,
    },
    Record {
        type_name: String,
        fields: Vec<(String, Self)>,
    },
    Variant {
        type_name: String,
        case_name: String,
        fields: Vec<(String, Self)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalConstValue {
    pub type_name: String,
    pub encoding: String,
    pub display: String,
}

impl CanonicalConstValue {
    pub fn new(
        type_name: impl Into<String>,
        encoding: impl Into<String>,
        display: impl Into<String>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            encoding: encoding.into(),
            display: display.into(),
        }
    }

    /// The reserved leaf stored in generic argument position. Source
    /// identifiers cannot contain `#`, so no authored type or parameter can
    /// collide with this namespace.
    pub fn atom(&self) -> String {
        format!(
            "{PREFIX}{}:{}{}:{}{}:{}",
            self.type_name.len(),
            self.type_name,
            self.encoding.len(),
            self.encoding,
            self.display.len(),
            self.display,
        )
    }

    pub fn from_atom(atom: &str) -> Option<Self> {
        let mut rest = atom.strip_prefix(PREFIX)?;
        let type_name = take_length_delimited(&mut rest)?;
        let encoding = take_length_delimited(&mut rest)?;
        let display = take_length_delimited(&mut rest)?;
        if !rest.is_empty() {
            return None;
        }
        Some(Self::new(type_name, encoding, display))
    }

    /// Decode the canonical inner encoding, failing closed on malformed or
    /// non-canonical framing and on values outside the fixed resource bounds.
    pub fn decode_encoding(&self) -> Option<DecodedCanonicalConstValue> {
        if self.encoding.len() > MAX_ENCODING_BYTES {
            return None;
        }
        let mut decoded_nodes = 0;
        decode_node(self.encoding.as_str(), 0, &mut decoded_nodes)
    }
}

fn decode_node(
    encoding: &str,
    depth: usize,
    decoded_nodes: &mut usize,
) -> Option<DecodedCanonicalConstValue> {
    if depth >= MAX_DECODE_DEPTH || *decoded_nodes >= MAX_DECODED_NODES {
        return None;
    }
    *decoded_nodes += 1;

    if let Some(mut rest) = encoding.strip_prefix("integer") {
        let type_name = take_canonical_piece(&mut rest)?;
        let spelling = take_canonical_piece(&mut rest)?;
        if type_name.is_empty() || !rest.is_empty() {
            return None;
        }
        let value = spelling.parse::<i128>().ok()?;
        if value.to_string() != spelling {
            return None;
        }
        return Some(DecodedCanonicalConstValue::Integer {
            type_name: type_name.to_owned(),
            value,
        });
    }

    if let Some(mut rest) = encoding.strip_prefix("boolean") {
        let spelling = take_canonical_piece(&mut rest)?;
        if !rest.is_empty() {
            return None;
        }
        return match spelling {
            "true" => Some(DecodedCanonicalConstValue::Boolean(true)),
            "false" => Some(DecodedCanonicalConstValue::Boolean(false)),
            _ => None,
        };
    }

    if let Some(mut rest) = encoding.strip_prefix("array") {
        let type_name = take_canonical_piece(&mut rest)?;
        if type_name.is_empty() {
            return None;
        }
        let mut values = Vec::new();
        while !rest.is_empty() {
            let child = take_canonical_piece(&mut rest)?;
            let value = decode_node(child, depth + 1, decoded_nodes)?;
            values.try_reserve_exact(1).ok()?;
            values.push(value);
        }
        return Some(DecodedCanonicalConstValue::Array {
            type_name: type_name.to_owned(),
            values,
        });
    }

    if let Some(mut rest) = encoding.strip_prefix("record") {
        let type_name = take_canonical_piece(&mut rest)?;
        if type_name.is_empty() {
            return None;
        }
        let fields = decode_fields(&mut rest, depth, decoded_nodes)?;
        return Some(DecodedCanonicalConstValue::Record {
            type_name: type_name.to_owned(),
            fields,
        });
    }

    if let Some(mut rest) = encoding.strip_prefix("variant") {
        let type_name = take_canonical_piece(&mut rest)?;
        let case_name = take_canonical_piece(&mut rest)?;
        if type_name.is_empty() || case_name.is_empty() {
            return None;
        }
        let fields = decode_fields(&mut rest, depth, decoded_nodes)?;
        return Some(DecodedCanonicalConstValue::Variant {
            type_name: type_name.to_owned(),
            case_name: case_name.to_owned(),
            fields,
        });
    }

    None
}

fn decode_fields(
    rest: &mut &str,
    depth: usize,
    decoded_nodes: &mut usize,
) -> Option<Vec<(String, DecodedCanonicalConstValue)>> {
    let mut fields = Vec::new();
    while !rest.is_empty() {
        let field_name = take_canonical_piece(rest)?;
        if field_name.is_empty() || fields.iter().any(|(existing, _)| existing == field_name) {
            return None;
        }
        let child = take_canonical_piece(rest)?;
        let value = decode_node(child, depth + 1, decoded_nodes)?;
        fields.try_reserve_exact(1).ok()?;
        fields.push((field_name.to_owned(), value));
    }
    Some(fields)
}

fn take_canonical_piece<'a>(rest: &mut &'a str) -> Option<&'a str> {
    let separator = rest.find(':')?;
    let length_spelling = &rest[..separator];
    if length_spelling.is_empty()
        || !length_spelling.bytes().all(|byte| byte.is_ascii_digit())
        || (length_spelling.len() > 1 && length_spelling.starts_with('0'))
    {
        return None;
    }
    let length = length_spelling.parse::<usize>().ok()?;
    let payload = &rest[separator + 1..];
    if payload.len() < length || !payload.is_char_boundary(length) {
        return None;
    }
    let (value, tail) = payload.split_at(length);
    *rest = tail;
    Some(value)
}

fn take_length_delimited(rest: &mut &str) -> Option<String> {
    let separator = rest.find(':')?;
    let length = rest[..separator].parse::<usize>().ok()?;
    let payload = &rest[separator + 1..];
    if !payload.is_char_boundary(length) || payload.len() < length {
        return None;
    }
    let (value, tail) = payload.split_at(length);
    *rest = tail;
    Some(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        CanonicalConstValue, DecodedCanonicalConstValue, MAX_DECODE_DEPTH, MAX_DECODED_NODES,
        MAX_ENCODING_BYTES,
    };

    fn framed(tag: &str, pieces: impl IntoIterator<Item = impl AsRef<str>>) -> String {
        let mut encoding = tag.to_owned();
        for piece in pieces {
            let piece = piece.as_ref();
            encoding.push_str(piece.len().to_string().as_str());
            encoding.push(':');
            encoding.push_str(piece);
        }
        encoding
    }

    fn decode(encoding: impl Into<String>) -> Option<DecodedCanonicalConstValue> {
        CanonicalConstValue::new("ignored outer carrier", encoding, "ignored display")
            .decode_encoding()
    }

    #[test]
    fn canonical_const_atom_round_trips_delimiters_and_unicode() {
        let value =
            CanonicalConstValue::new("pkg::Unit", "record(2:a=1:b)", "Unit { symbol: \"μ:m\" }");
        assert_eq!(CanonicalConstValue::from_atom(&value.atom()), Some(value));
    }

    #[test]
    fn ordinary_names_are_not_const_atoms() {
        assert!(CanonicalConstValue::from_atom("Unit").is_none());
        assert!(CanonicalConstValue::from_atom("42").is_none());
    }

    #[test]
    fn canonical_inner_scalars_decode() {
        assert_eq!(
            decode(framed("integer", ["i64", "-42"])),
            Some(DecodedCanonicalConstValue::Integer {
                type_name: "i64".to_owned(),
                value: -42,
            })
        );
        assert_eq!(
            decode(framed("boolean", ["true"])),
            Some(DecodedCanonicalConstValue::Boolean(true))
        );
    }

    #[test]
    fn canonical_inner_compounds_preserve_order_and_decode_variants() {
        let one = framed("integer", ["u8", "1"]);
        let yes = framed("boolean", ["false"]);
        let record = framed(
            "record",
            ["pkg::Pair", "left", one.as_str(), "right", yes.as_str()],
        );
        let array = framed("array", ["[pkg::Pair; 1]", record.as_str()]);
        let variant = framed("variant", ["pkg::Maybe", "Some", "items", array.as_str()]);

        assert_eq!(
            decode(variant),
            Some(DecodedCanonicalConstValue::Variant {
                type_name: "pkg::Maybe".to_owned(),
                case_name: "Some".to_owned(),
                fields: vec![(
                    "items".to_owned(),
                    DecodedCanonicalConstValue::Array {
                        type_name: "[pkg::Pair; 1]".to_owned(),
                        values: vec![DecodedCanonicalConstValue::Record {
                            type_name: "pkg::Pair".to_owned(),
                            fields: vec![
                                (
                                    "left".to_owned(),
                                    DecodedCanonicalConstValue::Integer {
                                        type_name: "u8".to_owned(),
                                        value: 1,
                                    },
                                ),
                                (
                                    "right".to_owned(),
                                    DecodedCanonicalConstValue::Boolean(false),
                                ),
                            ],
                        }],
                    },
                )],
            })
        );
    }

    #[test]
    fn malformed_tags_arities_and_scalar_spellings_are_rejected() {
        for encoding in [
            framed("mystery", ["true"]),
            framed("integer", ["i64"]),
            framed("integer", ["i64", "1", "extra"]),
            framed("integer", ["i64", "+1"]),
            framed("integer", ["i64", "01"]),
            framed("integer", ["i64", "-0"]),
            framed(
                "integer",
                ["i64", "170141183460469231731687303715884105728"],
            ),
            framed("boolean", [] as [&str; 0]),
            framed("boolean", ["TRUE"]),
            framed("boolean", ["false", "extra"]),
            framed("array", [] as [&str; 0]),
            framed("record", ["pkg::R", "field"]),
            framed("variant", ["pkg::V"]),
            framed("variant", ["pkg::V", "Case", "field"]),
        ] {
            assert!(decode(encoding).is_none());
        }
    }

    #[test]
    fn malformed_framing_and_nested_trailing_bytes_are_rejected() {
        let malformed_child = format!("{}junk", framed("boolean", ["true"]));
        for encoding in [
            "integer03:i641:1".to_owned(),
            "integerx:i641:1".to_owned(),
            "integer3:i641".to_owned(),
            "integer1:\u{00b5}1:1".to_owned(),
            format!("integer3:i641:1junk"),
            framed("array", ["[bool; 1]", malformed_child.as_str()]),
        ] {
            assert!(decode(encoding).is_none());
        }
    }

    #[test]
    fn empty_names_and_duplicate_fields_are_rejected() {
        let value = framed("boolean", ["true"]);
        for encoding in [
            framed("integer", ["", "1"]),
            framed("array", [""]),
            framed("record", [""]),
            framed("record", ["R", "", value.as_str()]),
            framed("record", ["R", "x", value.as_str(), "x", value.as_str()]),
            framed("variant", ["", "Case"]),
            framed("variant", ["V", ""]),
        ] {
            assert!(decode(encoding).is_none());
        }
    }

    #[test]
    fn decoder_enforces_depth_node_and_byte_bounds() {
        let mut too_deep = framed("boolean", ["true"]);
        for _ in 0..MAX_DECODE_DEPTH {
            too_deep = framed("array", ["Nested", too_deep.as_str()]);
        }
        assert!(decode(too_deep).is_none());

        let child = framed("array", ["A"]);
        let too_many_nodes = framed(
            "array",
            std::iter::once("Many").chain(std::iter::repeat_n(child.as_str(), MAX_DECODED_NODES)),
        );
        assert!(too_many_nodes.len() <= MAX_ENCODING_BYTES);
        assert!(decode(too_many_nodes).is_none());

        assert!(decode("x".repeat(MAX_ENCODING_BYTES + 1)).is_none());
    }
}
