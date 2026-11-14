/*
We define compile-time constant values (`ConstValue`), structural aggregate and
slice constants, and their canonical binary encoding (`encoding`).
*/

use numerics::literals::FloatFormat;

mod encoding {
    use super::{
        DecodedCanonicalConstValue, MAX_DECODE_DEPTH, MAX_DECODED_NODES, MAX_ENCODING_BYTES, framed,
    };

    pub(super) fn encode(value: &DecodedCanonicalConstValue) -> Option<String> {
        node(value, 0, &mut 0, false)
    }

    pub(super) fn encode_declaration(value: &DecodedCanonicalConstValue) -> Option<String> {
        node(value, 0, &mut 0, true)
    }

    fn node(
        value: &DecodedCanonicalConstValue,
        depth: usize,
        nodes: &mut usize,
        declaration: bool,
    ) -> Option<String> {
        if depth >= MAX_DECODE_DEPTH || *nodes >= MAX_DECODED_NODES {
            return None;
        }
        *nodes += 1;
        let encoding = match value {
            DecodedCanonicalConstValue::Integer { type_name, value } => {
                framed("integer", [type_name.as_str(), value.to_string().as_str()])
            }
            DecodedCanonicalConstValue::Boolean(value) => framed("boolean", [value.to_string()]),
            DecodedCanonicalConstValue::Float { format, bits } if declaration => match format {
                numerics::literals::FloatFormat::F32 if *bits <= u64::from(u32::MAX) => {
                    format!("float:f32:{bits:08x}")
                }
                numerics::literals::FloatFormat::F64 => format!("float:f64:{bits:016x}"),
                _ => return None,
            },
            DecodedCanonicalConstValue::Float { .. } => return None,
            DecodedCanonicalConstValue::Array { type_name, values } => {
                let mut pieces = vec![type_name.clone()];
                for value in values {
                    pieces.push(node(value, depth + 1, nodes, declaration)?);
                }
                framed("array", pieces)
            }
            DecodedCanonicalConstValue::Record { type_name, fields } => framed(
                "record",
                fields_encoding(vec![type_name.clone()], fields, depth, nodes, declaration)?,
            ),
            DecodedCanonicalConstValue::Variant {
                type_name,
                case_name,
                fields,
            } => framed(
                "variant",
                fields_encoding(
                    vec![type_name.clone(), case_name.clone()],
                    fields,
                    depth,
                    nodes,
                    declaration,
                )?,
            ),
        };
        (encoding.len() <= MAX_ENCODING_BYTES).then_some(encoding)
    }

    fn fields_encoding(
        mut pieces: Vec<String>,
        fields: &[(String, DecodedCanonicalConstValue)],
        depth: usize,
        nodes: &mut usize,
        declaration: bool,
    ) -> Option<Vec<String>> {
        for (name, value) in fields {
            pieces.push(name.clone());
            pieces.push(node(value, depth + 1, nodes, declaration)?);
        }
        Some(pieces)
    }
}

const PREFIX: &str = "#omega-const:";
const MAX_ENCODING_BYTES: usize = 64 * 1024;
const MAX_DECODED_NODES: usize = 4_096;
const MAX_DECODE_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedCanonicalConstValue {
    Integer {
        type_name: String,
        value: i128,
    },
    Boolean(bool),
    Float {
        format: FloatFormat,
        bits: u64,
    },
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

/// A closed scalar member of an enumerable value family. Consumers retain the
/// value; the canonical application key is an output, never a value encoding
/// for execution to parse back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FiniteScalarValue {
    Integer(i128),
    Boolean(bool),
}

impl FiniteScalarValue {
    pub fn from_decoded(value: &DecodedCanonicalConstValue) -> Option<Self> {
        match value {
            DecodedCanonicalConstValue::Integer { value, .. } => Some(Self::Integer(*value)),
            DecodedCanonicalConstValue::Boolean(value) => Some(Self::Boolean(*value)),
            _ => None,
        }
    }

    pub fn canonical_identity(self) -> String {
        match self {
            Self::Integer(value) => format!("named(integer-const({value}))"),
            Self::Boolean(value) => {
                canonical_application_identity(&CanonicalConstValue::boolean(value))
            }
        }
    }
}

fn canonical_application_identity(value: &CanonicalConstValue) -> String {
    fn escape(component: &str) -> String {
        let mut escaped = String::with_capacity(component.len());
        for character in component.chars() {
            if matches!(character, '\\' | '(' | ')' | ',') {
                escaped.push('\\');
            }
            escaped.push(character);
        }
        escaped
    }
    format!(
        "named(canonical-const(type({}),encoding({})))",
        escape(&value.type_name),
        escape(&value.encoding),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, compiler_storage::Record)]
pub struct CanonicalConstValue {
    pub type_name: String,
    pub encoding: String,
    pub display: String,
}

pub fn boolean_literal_spelling(name: &str) -> Option<bool> {
    match name {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, compiler_storage::Record)]
pub struct CanonicalConstIdentity {
    pub type_name: String,
    pub encoding: String,
}

impl CanonicalConstIdentity {
    pub fn integer(type_name: impl Into<String>, value: i128) -> Self {
        let type_name = type_name.into();
        let spelling = value.to_string();
        Self {
            encoding: framed("integer", [type_name.as_str(), spelling.as_str()]),
            type_name,
        }
    }

    pub fn from_declaration_value(value: &DecodedCanonicalConstValue) -> Option<Self> {
        let encoding = encoding::encode_declaration(value)?;
        let identity = Self {
            type_name: String::new(),
            encoding,
        };
        (identity.decode_declaration_encoding().as_ref() == Some(value)).then_some(identity)
    }

    pub fn decode_encoding(&self) -> Option<DecodedCanonicalConstValue> {
        decode_canonical_encoding(self.encoding.as_str(), false)
    }

    pub fn decode_declaration_encoding(&self) -> Option<DecodedCanonicalConstValue> {
        decode_canonical_encoding(self.encoding.as_str(), true)
    }
}

impl CanonicalConstValue {
    pub const MAXIMUM_DEPTH: usize = MAX_DECODE_DEPTH;
    pub const MAXIMUM_NODES: usize = MAX_DECODED_NODES;

    /// Preserves the established application key without a display-text
    /// round trip. Integer carriers are supplied by the receiving binder;
    /// other canonical atoms retain the existing typed encoding in the key.
    pub fn application_identity(&self) -> Option<String> {
        let decoded = self.decode_encoding()?;
        Some(FiniteScalarValue::from_decoded(&decoded).map_or_else(
            || canonical_application_identity(self),
            FiniteScalarValue::canonical_identity,
        ))
    }

    pub fn from_decoded(
        value: &DecodedCanonicalConstValue,
        display: impl Into<String>,
    ) -> Option<Self> {
        let type_name = match value {
            DecodedCanonicalConstValue::Integer { type_name, .. }
            | DecodedCanonicalConstValue::Array { type_name, .. }
            | DecodedCanonicalConstValue::Record { type_name, .. }
            | DecodedCanonicalConstValue::Variant { type_name, .. } => type_name.clone(),
            DecodedCanonicalConstValue::Boolean(_) => "bool".to_owned(),
            DecodedCanonicalConstValue::Float { .. } => return None,
        };
        let encoding = encoding::encode(value)?;
        let result = Self::new(type_name, encoding, display);
        (result.decode_encoding().as_ref() == Some(value)).then_some(result)
    }

    pub fn boolean(value: bool) -> Self {
        let spelling = value.to_string();
        Self::new("bool", framed("boolean", [spelling.as_str()]), spelling)
    }

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

    pub fn identity(&self) -> CanonicalConstIdentity {
        CanonicalConstIdentity {
            type_name: self.type_name.clone(),
            encoding: self.encoding.clone(),
        }
    }

    pub fn decode_encoding(&self) -> Option<DecodedCanonicalConstValue> {
        decode_canonical_encoding(self.encoding.as_str(), false)
    }
}

fn decode_canonical_encoding(
    encoding: &str,
    declaration: bool,
) -> Option<DecodedCanonicalConstValue> {
    if encoding.len() > MAX_ENCODING_BYTES {
        return None;
    }
    let mut decoded_nodes = 0;
    decode_node(encoding, 0, &mut decoded_nodes, declaration)
}

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

fn decode_node(
    encoding: &str,
    depth: usize,
    decoded_nodes: &mut usize,
    declaration: bool,
) -> Option<DecodedCanonicalConstValue> {
    if depth >= MAX_DECODE_DEPTH || *decoded_nodes >= MAX_DECODED_NODES {
        return None;
    }
    *decoded_nodes += 1;

    if let Some(rest) = encoding.strip_prefix("float:") {
        if !declaration {
            return None;
        }
        let (format, spelling) = rest.split_once(':')?;
        let bits = u64::from_str_radix(spelling, 16).ok()?;
        let format = match format {
            "f32"
                if bits <= u64::from(u32::MAX)
                    && spelling == format!("{bits:08x}")
                    && !f32::from_bits(bits as u32).is_nan() =>
            {
                FloatFormat::F32
            }
            "f64" if spelling == format!("{bits:016x}") && !f64::from_bits(bits).is_nan() => {
                FloatFormat::F64
            }
            _ => return None,
        };
        return Some(DecodedCanonicalConstValue::Float { format, bits });
    }

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
        return boolean_literal_spelling(spelling).map(DecodedCanonicalConstValue::Boolean);
    }

    if let Some(mut rest) = encoding.strip_prefix("array") {
        let type_name = take_canonical_piece(&mut rest)?;
        if type_name.is_empty() {
            return None;
        }
        let mut values = Vec::new();
        while !rest.is_empty() {
            let child = take_canonical_piece(&mut rest)?;
            let value = decode_node(child, depth + 1, decoded_nodes, declaration)?;
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
        let fields = decode_fields(&mut rest, depth, decoded_nodes, declaration)?;
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
        let fields = decode_fields(&mut rest, depth, decoded_nodes, declaration)?;
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
    declaration: bool,
) -> Option<Vec<(String, DecodedCanonicalConstValue)>> {
    let mut fields = Vec::new();
    while !rest.is_empty() {
        let field_name = take_canonical_piece(rest)?;
        if field_name.is_empty() || fields.iter().any(|(existing, _)| existing == field_name) {
            return None;
        }
        let child = take_canonical_piece(rest)?;
        let value = decode_node(child, depth + 1, decoded_nodes, declaration)?;
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
        CanonicalConstIdentity, CanonicalConstValue, DecodedCanonicalConstValue, MAX_DECODE_DEPTH,
        MAX_DECODED_NODES, MAX_ENCODING_BYTES, boolean_literal_spelling, framed,
    };

    fn decode(encoding: impl Into<String>) -> Option<DecodedCanonicalConstValue> {
        CanonicalConstValue::new("ignored outer carrier", encoding, "ignored display")
            .decode_encoding()
    }

    #[test]
    fn finite_scalar_application_keys_preserve_values_not_display_text() {
        use super::FiniteScalarValue;
        for value in [i128::MIN, -42, 0, 16, i128::MAX] {
            let constant = CanonicalConstValue::new(
                "i128",
                framed("integer", ["i128", &value.to_string()]),
                "not an integer spelling",
            );
            let expected = format!("named(integer-const({value}))");
            assert_eq!(constant.application_identity(), Some(expected.clone()));
            assert_eq!(
                FiniteScalarValue::Integer(value).canonical_identity(),
                expected
            );
        }
        assert_eq!(
            FiniteScalarValue::Boolean(false).canonical_identity(),
            "named(canonical-const(type(bool),encoding(boolean5:false)))"
        );
        assert_eq!(
            FiniteScalarValue::Boolean(true).canonical_identity(),
            "named(canonical-const(type(bool),encoding(boolean4:true)))"
        );
        assert!(
            CanonicalConstValue::new("u32", "broken", "16")
                .application_identity()
                .is_none()
        );
    }

    #[test]
    fn structural_writer_preserves_existing_canonical_encoding() {
        let integer = framed("integer", ["u64", "7"]);
        let record = framed("record", ["Configuration", "minimum", integer.as_str()]);
        let value = CanonicalConstValue::new("Configuration", record.clone(), "source")
            .decode_encoding()
            .expect("existing record encoding");
        let written = CanonicalConstValue::from_decoded(&value, "different diagnostic display")
            .expect("bounded structural writer");
        assert_eq!(written.encoding.as_str(), record);
        assert_eq!(written.decode_encoding(), Some(value));
    }

    #[test]
    fn canonical_const_atom_round_trips_delimiters_and_unicode() {
        let value =
            CanonicalConstValue::new("pkg::Unit", "record(2:a=1:b)", "Unit { symbol: \"μ:m\" }");
        assert_eq!(CanonicalConstValue::from_atom(&value.atom()), Some(value));
    }

    #[test]
    fn boolean_literal_spellings_round_trip_through_the_canonical_atom() {
        for value in [true, false] {
            let spelling = value.to_string();
            assert_eq!(boolean_literal_spelling(&spelling), Some(value));
            let atom = CanonicalConstValue::boolean(value);
            assert_eq!(atom.display, spelling);
            assert_eq!(
                atom.decode_encoding(),
                Some(DecodedCanonicalConstValue::Boolean(value))
            );
        }
    }

    #[test]
    fn semantic_identity_omits_display_and_integer_builder_is_canonical() {
        let first = CanonicalConstValue::new("u64", framed("integer", ["u64", "4"]), "2 + 2");
        let second = CanonicalConstValue::new("u64", framed("integer", ["u64", "4"]), "4");
        assert_eq!(first.identity(), second.identity());
        assert_eq!(first.identity(), CanonicalConstIdentity::integer("u64", 4));
        assert_eq!(
            first.identity().decode_encoding(),
            Some(DecodedCanonicalConstValue::Integer {
                type_name: "u64".to_owned(),
                value: 4,
            })
        );
    }

    #[test]
    fn canonical_inner_scalars_decode() {
        let canonical_true = CanonicalConstValue::boolean(true);
        assert_eq!(
            CanonicalConstValue::from_atom(&canonical_true.atom()),
            Some(canonical_true.clone())
        );
        assert_eq!(
            canonical_true.decode_encoding(),
            Some(DecodedCanonicalConstValue::Boolean(true))
        );
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
            "integer3:i641:1junk".to_string(),
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
    fn declaration_floats_do_not_become_structural_indices() {
        for (scalar, format, bits) in [
            (
                "float:f32:80000000",
                numerics::literals::FloatFormat::F32,
                0x80000000,
            ),
            (
                "float:f64:8000000000000000",
                numerics::literals::FloatFormat::F64,
                0x8000000000000000,
            ),
        ] {
            let record = framed("record", ["Cell", "value", scalar]);
            let array = framed("array", ["[Cell; 1]", record.as_str()]);
            for encoding in [scalar.to_owned(), record, array] {
                let value = CanonicalConstValue::new("encoded claim", &encoding, "display");
                assert!(value.decode_encoding().is_none());
                assert!(value.identity().decode_encoding().is_none());
                let forged = CanonicalConstValue::from_atom(&value.atom()).unwrap();
                assert!(forged.decode_encoding().is_none());
                assert!(value.identity().decode_declaration_encoding().is_some());
            }
            assert_eq!(
                CanonicalConstValue::new("", scalar, "")
                    .identity()
                    .decode_declaration_encoding(),
                Some(DecodedCanonicalConstValue::Float { format, bits }),
            );
        }
        for encoding in [
            "float:f32:7fc00000",
            "float:f64:7ff8000000000000",
            "float:f32:0",
            "float:f32:000000000",
            "float:f32:0000000A",
            "float:f32:100000000",
            "float:f64:0000000000000000x",
            "float:f16:0000",
        ] {
            let nested = framed("array", ["[f32; 1]", encoding]);
            assert!(
                CanonicalConstValue::new("", nested, "")
                    .identity()
                    .decode_declaration_encoding()
                    .is_none(),
                "{encoding}"
            );
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
