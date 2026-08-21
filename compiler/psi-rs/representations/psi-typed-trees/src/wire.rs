use crate::name::Identifier;
use crate::types::{DomainConstraint, TypeReferenceHandle};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

/// One wire field's DERIVED placement -- the plan the tagged codec walks
/// (mint arc rung 2a). The Rust-side mirror of the FieldPlan wire cases
/// (`Varint(tag)` / `LengthPrefixed(tag)`, programmable_layouts §3): a scalar
/// field encodes as tag varint + value varint; text/byte-slice/nested/repeated
/// fields encode as tag varint + length varint + payload. Placements are
/// stored SORTED BY TAG (the codec emits in field-number order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WirePlacement {
    Varint { tag: u64 },
    LengthPrefixed { tag: u64 },
}

impl Default for WirePlacement {
    /// The ZII zero placement (arena slots initialize to it): a varint at
    /// tag 0 -- meaningless until written, exactly like a zeroed offset.
    fn default() -> Self {
        Self::Varint { tag: 0 }
    }
}

impl WirePlacement {
    pub fn tag(self) -> u64 {
        match self {
            Self::Varint { tag } | Self::LengthPrefixed { tag } => tag,
        }
    }
}

/// A schema's derived wire plan: its placements as a span into the
/// `TypedTrees` placement arena (arena + span ownership, no nested vectors).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WireSchemaPlan {
    pub schema: SymbolHandle,
    pub placements: HandleSpan<WirePlacement>,
    /// Runtime resource obligations retained by the generated encoder. These
    /// are derived from carrier semantics, not authored placement policy.
    pub encode_obligations: HandleSpan<WireEncodeObligation>,
}

/// A dynamic obligation carried by a normalized wire encoder plan.
///
/// A borrowed scalar slice is encoded without allocation by walking the
/// runtime element count twice: once to measure the exact packed-varint body,
/// then once to emit it after proving the remaining output capacity covers
/// the length prefix and body. Retaining this row keeps the generated
/// requirement honest about dynamic length, work, and output space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WireEncodeObligation {
    pub field_number: u64,
    pub element: WireScalarEncoding,
    pub length: WireEncodeLengthObligation,
    pub work: WireEncodeWorkObligation,
    pub output_capacity: WireEncodeOutputCapacityObligation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WireEncodeLengthObligation {
    #[default]
    RuntimeElementCount,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WireEncodeWorkObligation {
    /// One exact-size pass and one emission pass over every live element.
    #[default]
    TwoPassesPerElement,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WireEncodeOutputCapacityObligation {
    /// Remaining output covers the exact packed-body byte count plus its
    /// canonical length varint. The field tag is emitted separately.
    #[default]
    ExactPackedPayload,
}

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
    pub number: u64,
    pub name: Identifier,
    pub relevance: psi_language_core::BindingRelevance,
    pub type_reference: TypeReferenceHandle,
}

impl Default for WireField {
    fn default() -> Self {
        Self {
            number: 0,
            name: Identifier::default(),
            relevance: Default::default(),
            type_reference: TypeReferenceHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireReserved {
    pub number: u64,
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
/// ordinary data selected by the boundary package).
pub const WIRE_DECODE_MACHINE_NAME: &str = "decode";

/// How one primitive scalar rides compact_binary v0 (wire stage 2a): the
/// runtime load width and whether the value zigzags before LEB128. The
/// vocabulary is shared by validation, instruction selection, and the
/// reference interpreter so all three agree byte-for-byte.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WireScalarEncoding {
    pub byte_size: usize,
    pub zigzag: bool,
}

/// Whether a declared type carries an inclusive integer range, looking
/// through reference and nested constraint shells.
pub fn type_reference_carries_range(
    program: &crate::TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
    if !handle.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(handle) {
        crate::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_carries_range(program, *referee)
        }
        crate::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint, crate::types::TypeConstraintNode::Range { .. })
                })
                || type_reference_carries_range(program, *base_type)
        }
        _ => false,
    }
}

/// Normalize a scalar type's declared representation invariants into one
/// inclusive interval. This includes authored integer range shells, `bool`'s
/// intrinsic `{0, 1}` representation, and the finite carrier bounds of
/// `i32`/`u32`.
/// Validation has already rejected non-constant or contradictory authored
/// ranges; `None` therefore means the scalar spans the full decoder value
/// width (`i64`/`u64`), never "skip an invariant we failed to understand".
/// Wire decoding and compact bit-layout validation deliberately share this
/// declaration-owned fact.
pub fn scalar_representation_range(
    program: &crate::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<psi_language_semantics::wire::WireScalarRange> {
    fn collect(
        program: &crate::TypedTrees,
        handle: TypeReferenceHandle,
        minimum: &mut i64,
        maximum: &mut i64,
        found: &mut bool,
    ) -> Option<()> {
        match program.type_reference_table.type_reference(handle) {
            crate::types::TypeReferenceNode::Reference { referee, .. } => {
                collect(program, *referee, minimum, maximum, found)
            }
            crate::types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let crate::types::TypeConstraintNode::Range {
                        minimum: lower,
                        maximum: upper,
                    } = constraint
                    {
                        *minimum = (*minimum)
                            .max(program.expression_table.constant_integer_value(*lower)?);
                        *maximum = (*maximum)
                            .min(program.expression_table.constant_integer_value(*upper)?);
                        *found = true;
                    }
                }
                collect(program, *base_type, minimum, maximum, found)
            }
            _ => Some(()),
        }
    }

    let primitive = program.primitive_type_reference(handle)?;
    if primitive == crate::types::PrimitiveType::Bool {
        return Some(psi_language_semantics::wire::WireScalarRange {
            minimum: 0,
            maximum: 1,
            signed: false,
        });
    }
    if !primitive.accepts_range_constraint() {
        return None;
    }
    let (mut minimum, mut maximum, mut found) = match primitive {
        crate::types::PrimitiveType::I32 => (i64::from(i32::MIN), i64::from(i32::MAX), true),
        crate::types::PrimitiveType::U32 => (0, i64::from(u32::MAX), true),
        _ => (i64::MIN, i64::MAX, false),
    };
    collect(program, handle, &mut minimum, &mut maximum, &mut found)?;
    (found && minimum <= maximum).then_some(psi_language_semantics::wire::WireScalarRange {
        minimum,
        maximum,
        signed: primitive.is_signed_integer(),
    })
}

/// Wire-facing name retained for the decode pipeline.
pub fn scalar_decode_range(
    program: &crate::TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<psi_language_semantics::wire::WireScalarRange> {
    scalar_representation_range(program, handle)
}

/// Resolve one named data field's declared type through reference/constraint
/// shells. Wire encode/decode consumers use the destination/source data
/// declaration rather than assuming the schema field's less-refined type.
pub fn data_field_type(
    program: &crate::TypedTrees,
    mut receiver: TypeReferenceHandle,
    field_name: &str,
) -> Option<TypeReferenceHandle> {
    loop {
        receiver = match program.type_reference_table.type_reference(receiver) {
            crate::types::TypeReferenceNode::Reference { referee, .. } => *referee,
            crate::types::TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            crate::types::TypeReferenceNode::Named { symbol, name } => {
                let data = program.data_definitions().iter().find(|data| {
                    (symbol.is_valid() && data.symbol == *symbol) || data.name == *name
                })?;
                return program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        crate::data::DataMember::Field(field)
                            if field.name.as_str() == field_name =>
                        {
                            Some(field.type_reference)
                        }
                        _ => None,
                    });
            }
            _ => return None,
        };
    }
}

/// Resolve a fixed array's declared element type through reference and
/// constraint shells. Decode uses the destination declaration's element
/// rather than the schema carrier so element-level facts cannot disappear.
pub fn fixed_array_element_type(
    program: &crate::TypedTrees,
    mut handle: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    loop {
        handle = match program.type_reference_table.type_reference(handle) {
            crate::types::TypeReferenceNode::Reference { referee, .. } => *referee,
            crate::types::TypeReferenceNode::Constrained { base_type, .. } => *base_type,
            crate::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                return Some(*element_type);
            }
            _ => return None,
        };
    }
}

/// Resolve the destination element type for a bounded repeated carrier.
pub fn repeated_element_type(
    program: &crate::TypedTrees,
    handle: TypeReferenceHandle,
    carrier: WireRepeatedCarrier,
) -> Option<TypeReferenceHandle> {
    match carrier {
        WireRepeatedCarrier::FixedArray => fixed_array_element_type(program, handle),
        WireRepeatedCarrier::FixedVec => {
            let mut handle = handle;
            let (symbol, name) = loop {
                match program.type_reference_table.type_reference(handle) {
                    crate::types::TypeReferenceNode::Reference { referee, .. } => {
                        handle = *referee;
                    }
                    crate::types::TypeReferenceNode::Constrained { base_type, .. } => {
                        handle = *base_type;
                    }
                    crate::types::TypeReferenceNode::Generic {
                        base_name,
                        arguments,
                        ..
                    } if base_name.as_str() == "FixedVec" => {
                        let [element, _] = program
                            .type_reference_table
                            .type_reference_handles(*arguments)
                        else {
                            return None;
                        };
                        return Some(*element);
                    }
                    crate::types::TypeReferenceNode::Named { symbol, name } => {
                        break (*symbol, name);
                    }
                    _ => return None,
                }
            };
            let data = program
                .data_definitions()
                .iter()
                .find(|data| (symbol.is_valid() && data.symbol == symbol) || data.name == *name)?;
            let items = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    crate::data::DataMember::Field(field) if field.name.as_str() == "items" => {
                        Some(field.type_reference)
                    }
                    _ => None,
                })?;
            fixed_array_element_type(program, items)
        }
    }
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
/// before (wire stage 2a), and borrowed byte/text fields ride as a LENGTH
/// varint (byte count) followed by raw bytes -- no NUL terminator or padding.
/// The vocabulary is shared by validation, instruction selection, and the
/// reference interpreter so all three agree byte-for-byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireFieldEncoding {
    Scalar(WireScalarEncoding),
    /// A borrowed byte/text field: the runtime value is a `{ptr, len}` slice
    /// descriptor, encoded as the len varint then len raw bytes read through
    /// ptr. Decode produces a checked zero-copy view into the input buffer.
    Text,
}

impl WireFieldEncoding {
    /// The primitive encode-side field set. Runtime-sized text is recognized
    /// from its borrowed byte-slice carrier before this primitive path.
    pub fn for_primitive(primitive: crate::types::PrimitiveType) -> Option<Self> {
        WireScalarEncoding::for_primitive(primitive).map(Self::Scalar)
    }
}

/// The most LEB128 bytes a text field's LENGTH varint can need: the length
/// half of a text descriptor is one 64-bit pointer wide, so ten groups.
pub const WIRE_TEXT_LENGTH_MAX_VARINT_LENGTH: usize = 10;

/// The bounded carrier semantics of a compact-binary repeated field.
///
/// A fixed array is always full: its extent is the encoded element count.
/// `FixedVec<T, N>` uses its own `length` member and inline `items` storage.
/// This deliberately replaces the former convention that gave every array a
/// synthetic `<field>_count` sibling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireRepeatedCarrier {
    FixedArray,
    FixedVec,
}

/// A bounded repeated field's normalized encoding: carrier semantics, scalar
/// element encoding, and static capacity. The capacity gives the generated
/// realization a finite worst-case work and byte budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireRepeatedEncoding {
    pub carrier: WireRepeatedCarrier,
    pub element: WireScalarEncoding,
    pub max_count: usize,
}

/// The unbounded borrowed scalar-slice encoding. Unlike bounded repeated
/// carriers, its live count comes from a fat descriptor and no static
/// `max_count` exists; the corresponding normalized-plan obligation carries
/// the dynamic work and capacity contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireBorrowedScalarSliceEncoding {
    pub element: WireScalarEncoding,
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
/// Returns `(schema_name, optional grammar argument)` when a legacy flattened
/// name spells an `OmegaLayout` instance. Current typed trees retain family
/// arguments separately; this parser remains for older in-memory producers.
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

/// Recognize the normalized carrier for an `OmegaLayout<Schema[, Grammar]>`
/// constraint. Domain arguments have their own typed handles now; accepting
/// only the old parser-flattened name would misclassify the layout family as a
/// user value domain after domain normalization.
pub fn is_layout_domain_constraint(domain: &DomainConstraint) -> bool {
    (domain.name.as_str() == "OmegaLayout" && matches!(domain.arguments.len(), 1 | 2))
        || is_layout_domain_name(domain.name.as_str())
}

/// Recover the authored schema and optional grammar labels from either the
/// normalized argument vector or a legacy flattened family name.
pub fn layout_domain_constraint_arguments(
    program: &crate::TypedTrees,
    domain: &DomainConstraint,
) -> Option<(String, Option<String>)> {
    if let Some((schema, grammar)) = layout_domain_arguments(domain.name.as_str()) {
        return Some((schema.to_owned(), grammar.map(str::to_owned)));
    }
    if domain.name.as_str() != "OmegaLayout" || !matches!(domain.arguments.len(), 1 | 2) {
        return None;
    }
    let schema = program.display_type_reference_with_constraints(domain.arguments[0]);
    let grammar = domain
        .arguments
        .get(1)
        .map(|argument| program.display_type_reference_with_constraints(*argument));
    Some((schema, grammar))
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
