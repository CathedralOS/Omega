use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

/// A `wire data` protocol schema carried through the typed stage: stable field
/// numbers, reserved (retired) numbers, and historical version eras. Wire
/// schemas are external-representation contracts, kept separate from runtime
/// `data` definitions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireSchema {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub encoding: Option<Identifier>,
    pub members: HandleSpan<WireMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireMember {
    Field(WireField),
    Reserved(WireReserved),
    Version(WireVersion),
}

impl Default for WireMember {
    fn default() -> Self {
        Self::Reserved(WireReserved::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireField {
    pub number: i64,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
}

impl Default for WireField {
    fn default() -> Self {
        Self {
            number: 0,
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireReserved {
    pub number: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireVersion {
    pub name: Identifier,
    pub members: HandleSpan<WireMember>,
}

/// The synthesized encoder entry point a wire schema exposes (chapter 20,
/// wire stage 2a): `Schema::encode(&value, &mut out, &mut written)`.
pub const WIRE_ENCODE_MACHINE_NAME: &str = "encode";

/// The synthesized decoder entry point a wire schema exposes (chapter 20,
/// wire stage 2b):
/// `Schema::decode(&mut value, &buffer, &mut read, &mut ok)`.
/// `read` receives the byte count consumed, `ok` the success flag; decoding
/// only accepts the schema's CURRENT era (historical eras await the stage 3
/// `Versioned<T>` container).
pub const WIRE_DECODE_MACHINE_NAME: &str = "decode";

/// How one primitive scalar rides compact_binary v0 (wire stage 2a): the
/// runtime load width and whether the value zigzags before LEB128. The
/// vocabulary is shared by validation, instruction selection, and the
/// reference interpreter so all three agree byte-for-byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireScalarEncoding {
    pub byte_size: usize,
    pub zigzag: bool,
}

impl WireScalarEncoding {
    /// The stage 2a scalar set: i32/i64/u32/u64/bool. Everything else is
    /// rejected by validation with a clear diagnostic.
    pub fn for_primitive(primitive: crate::types::PrimitiveType) -> Option<Self> {
        use crate::types::PrimitiveType;
        match primitive {
            PrimitiveType::Bool => Some(Self {
                byte_size: 1,
                zigzag: false,
            }),
            PrimitiveType::U32 => Some(Self {
                byte_size: 4,
                zigzag: false,
            }),
            PrimitiveType::U64 => Some(Self {
                byte_size: 8,
                zigzag: false,
            }),
            PrimitiveType::I32 => Some(Self {
                byte_size: 4,
                zigzag: true,
            }),
            PrimitiveType::I64 => Some(Self {
                byte_size: 8,
                zigzag: true,
            }),
            _ => None,
        }
    }

    /// The most LEB128 bytes a value of this scalar can need (zigzag widens
    /// a 32-bit signed source to 33 significant bits, still five groups).
    pub fn max_varint_length(self) -> usize {
        match (self.byte_size, self.zigzag) {
            (1, _) => 1,
            (4, _) => 5,
            (8, _) => 10,
            _ => 10,
        }
    }
}

/// How one schema field rides compact_binary v0: integer scalars LEB128 as
/// before (wire stage 2a), and `String` fields ride as a LENGTH varint (byte
/// count) followed by the raw UTF-8 bytes -- no NUL terminator, no padding.
/// The vocabulary is shared by validation, instruction selection, and the
/// reference interpreter so all three agree byte-for-byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFieldEncoding {
    Scalar(WireScalarEncoding),
    /// A `String` field: the runtime value is a `{ptr, len}` text descriptor,
    /// encoded as the len varint then len raw bytes read through ptr.
    /// ENCODE-only today -- decoding would need a storage decision for the
    /// produced descriptor (see chapter 20).
    Text,
}

impl WireFieldEncoding {
    /// The encode-side field set: the stage 2a scalars plus `String`.
    pub fn for_primitive(primitive: crate::types::PrimitiveType) -> Option<Self> {
        if primitive == crate::types::PrimitiveType::String {
            return Some(Self::Text);
        }
        WireScalarEncoding::for_primitive(primitive).map(Self::Scalar)
    }
}

/// The most LEB128 bytes a text field's LENGTH varint can need: the length
/// half of a text descriptor is one 64-bit pointer wide, so ten groups.
pub const WIRE_TEXT_LENGTH_MAX_VARINT_LENGTH: usize = 10;

/// The suffix of the COUNT COMPANION field a repeated wire field requires on
/// the runtime value type (chapter 20, repeated fields): a schema field
/// `1: samples: [i32; 4];` encodes/decodes through a value type declaring
/// BOTH `samples: [i32; 4]` and `samples_count: usize`. The wire never
/// carries the count -- compact_binary v0 packs repeated scalars
/// LENGTH-delimited (tag + byte-length varint + back-to-back element
/// varints, protobuf's packed encoding) -- so the companion is the runtime
/// element count: the encoder emits the first `min(count, max)` elements and
/// the decoder writes back how many elements it read.
pub const WIRE_REPEATED_COUNT_SUFFIX: &str = "_count";

/// The name of the count companion field a repeated wire field `name` reads
/// (encode) and writes (decode) on the runtime value type.
pub fn wire_repeated_count_field_name(field_name: &str) -> String {
    format!("{field_name}{WIRE_REPEATED_COUNT_SUFFIX}")
}

/// A REPEATED wire field's shape (chapter 20): a fixed maximum element count
/// declared in the schema spelling `[element; max]` plus the element's scalar
/// encoding. The declared maximum is part of the wire contract -- it is what
/// gives the packed payload a finite worst case, so the compile-time
/// out-buffer capacity guarantee survives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireRepeatedEncoding {
    pub element: WireScalarEncoding,
    pub max_count: usize,
}

impl WireRepeatedEncoding {
    /// The worst-case byte count of the PACKED payload (byte-length varint +
    /// max_count elements at their widest). The field's TAG varint is the
    /// caller's to add. The actual byte length never exceeds the worst-case
    /// body, so its varint never grows past the worst-case body's varint.
    pub fn worst_case_payload_bytes(self) -> usize {
        let body = self.max_count * self.element.max_varint_length();
        wire_varint_bytes(body as u64).len() + body
    }

    /// The worst-case byte count of the packed elements alone (the staging
    /// buffer the encoder needs while it two-passes the byte length).
    pub fn worst_case_body_bytes(self) -> usize {
        self.max_count * self.element.max_varint_length()
    }
}

/// The Omega-native LAYOUT-POLICY domain family on byte carriers
/// (`[u8; N] in OmegaLayout<Save>`; ch20 "grammars are layout policies").
/// Returns `(schema_name, optional grammar argument)` when `name` spells an
/// `OmegaLayout` instance. The flat instance name is produced by the parser's
/// parameterized-domain flattening (monomorphization-by-instantiation -- the
/// name IS the identity, no unification).
pub fn layout_domain_arguments(name: &str) -> Option<(&str, Option<&str>)> {
    let rest = name.strip_prefix("OmegaLayout<")?;
    let rest = rest.strip_suffix('>')?;
    Some(match rest.split_once(',') {
        Some((schema, grammar)) => (schema.trim(), Some(grammar.trim())),
        None => (rest.trim(), None),
    })
}

/// True when a domain-constraint name belongs to the `OmegaLayout` family.
/// Consumers that reclassify `[u8; N] in <named-domain>` as the owned bounded
/// TEXT carrier (`{len, bytes}` -- the layout builder, descriptor layouts, the
/// interpreter's carrier detection) must EXCLUDE this family: an OmegaLayout
/// refinement records what the bytes hold, it never changes what they are --
/// the carrier stays a plain byte array the wire codec addresses directly.
pub fn is_layout_domain_name(name: &str) -> bool {
    layout_domain_arguments(name).is_some()
}

/// The unsigned LEB128 byte sequence for a compile-time value (era
/// discriminators and field-number tags are known at compile time).
pub fn wire_varint_bytes(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2);
    loop {
        let low = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            bytes.push(low);
            return bytes;
        }
        bytes.push(low | 0x80);
    }
}
