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
/// wire stage 2a): `Schema::encode_wire(&value, &mut out, &mut written)`.
pub const WIRE_ENCODE_MACHINE_NAME: &str = "encode_wire";

/// The synthesized decoder entry point a wire schema exposes (chapter 20,
/// wire stage 2b):
/// `Schema::decode_wire(&mut value, &buffer, &mut read, &mut ok)`.
/// `read` receives the byte count consumed, `ok` the success flag; decoding
/// only accepts the schema's CURRENT era (historical eras await the stage 3
/// `Versioned<T>` container).
pub const WIRE_DECODE_MACHINE_NAME: &str = "decode_wire";

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
