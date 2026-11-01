//! Historical wire schema descriptions and their retained artifact metadata.

use crate::typed_trees::TypedTrees;
use crate::{types, wire};
use arena::HandleSpan;

impl TypedTrees {
    super::typed_tree_table! {
        wire_schemas => wire_schemas(wire::WireSchema) [push_wire_schema];
    }

    /// Record a schema's derived wire plan: placements land contiguously in
    /// the placement arena; the plan holds their span. `policy_verified` is
    /// true only when the authored grammar policy was evaluated for this
    /// schema and agreed with the derived placements.
    pub fn record_wire_schema_plan(
        &mut self,
        schema: symbols::SymbolHandle,
        placements: impl IntoIterator<Item = wire::WirePlacement>,
        encode_obligations: impl IntoIterator<Item = wire::WireEncodeObligation>,
        policy_verified: bool,
    ) {
        let span = self.wire_placements.insert_many(placements);
        let obligations = self.wire_encode_obligations.insert_many(encode_obligations);
        self.wire_schema_plans.push(wire::WireSchemaPlan {
            schema,
            placements: span,
            encode_obligations: obligations,
            policy_verified,
        });
    }

    pub fn append_wire_members(
        &mut self,
        members: Vec<wire::WireMember>,
    ) -> HandleSpan<wire::WireMember> {
        self.tables.wire_members.insert_many(members)
    }

    pub fn wire_members(&self, span: HandleSpan<wire::WireMember>) -> &[wire::WireMember] {
        self.tables.wire_members.span_or_empty(span)
    }

    /// The non-erased `Field` members of a wire `members` span, in order.
    pub fn relevant_wire_fields<'a>(
        &'a self,
        span: HandleSpan<wire::WireMember>,
    ) -> impl Iterator<Item = &'a wire::WireField> + 'a {
        self.wire_members(span).iter().filter_map(|member| {
            let wire::WireMember::Field(field) = member else {
                return None;
            };
            (!field.relevance.is_erased()).then_some(field)
        })
    }

    /// The era discriminator a schema's CURRENT body encodes (frozen decision
    /// 10): era 0 is the pre-versioning body, so a schema with no version
    /// blocks encodes era 0; declared version blocks snapshot earlier bodies
    /// in declaration order (the first block is era 0, the next era 1, ...),
    /// which leaves the current body at the era one past the newest block.
    pub fn wire_schema_current_era(&self, schema: &wire::WireSchema) -> u64 {
        self.wire_members(schema.members)
            .iter()
            .filter(|member| matches!(member, wire::WireMember::Version(_)))
            .count() as u64
    }

    /// The sibling wire schema a wire field's type references (a NESTED
    /// MESSAGE field, chapter 20), unwrapped through reference and constraint
    /// shells. `None` for primitives and ordinary program types.
    pub fn wire_field_nested_schema(&self, field: &wire::WireField) -> Option<&wire::WireSchema> {
        let name = self.named_type_reference_name(field.type_reference)?;
        self.wire_schemas()
            .iter()
            .find(|schema| schema.name.as_str() == name)
    }

    /// Whether `symbol` denotes the toolchain's `core::fixed_vec::FixedVec`,
    /// either the generic declaration or a concrete instance of it.
    ///
    /// Recognize the declaration, not a spelling: the same carrier may be
    /// written `FixedVec<T, N>`, reached through a qualified path, or resolved
    /// through a generated family head, and a reference's name then reads
    /// `omega::language::FixedVec<i32, 4>` while the instance's data is named
    /// `FixedVec<i32, 4>`. A concrete instance records the generic it came from
    /// in `generic_instance`; follow that to the declaration and require the
    /// toolchain's own `fixed_vec.omg`. An authored data type that merely
    /// shares the name is a different declaration and stays an ordinary
    /// nested message candidate.
    pub fn is_core_fixed_vec(&self, symbol: symbols::SymbolHandle) -> bool {
        if !symbol.is_valid() {
            return false;
        }
        let generic = self
            .data_definition_by_symbol(symbol)
            .and_then(|data| data.generic_instance)
            .and_then(
                |instance| match self.type_reference_table.type_reference(instance) {
                    types::TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
                    _ => None,
                },
            )
            .unwrap_or(symbol);
        let declaration = self.symbols.authored_origin(generic);
        declaration.is_valid()
            && self.symbols.name(declaration) == "FixedVec"
            && self
                .symbols
                .symbol_source_span(declaration)
                .and_then(|span| self.symbols.source_file(span))
                .is_some_and(|source| {
                    source.origin == source::SourceOrigin::Toolchain
                        && source.path.strip_prefix(&source.package_root).ok()
                            == Some(std::path::Path::new("fixed_vec.omg"))
                })
    }

    /// The `Named` name underneath reference and constraint shells, if the
    /// type reference bottoms out in one.
    fn named_type_reference_name(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Option<&str> {
        if !type_reference.is_valid() {
            return None;
        }
        match self.type_reference_table.type_reference(type_reference) {
            types::TypeReferenceNode::Reference { referee, .. } => {
                self.named_type_reference_name(*referee)
            }
            types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.named_type_reference_name(*base_type)
            }
            types::TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }

    /// A wire field's FIXED-ARRAY shape (`[element; max]`), unwrapped through
    /// reference and constraint shells: the element type reference and the
    /// literal maximum. `None` for non-array fields and for const-parameter
    /// lengths (a wire schema is a standalone contract -- its maximum must be
    /// a literal).
    pub fn wire_field_fixed_array(
        &self,
        field: &wire::WireField,
    ) -> Option<(types::TypeReferenceHandle, usize)> {
        let mut handle = field.type_reference;
        loop {
            if !handle.is_valid() {
                return None;
            }
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: types::FixedArrayLength::Literal(length),
                } => return Some((*element_type, *length)),
                _ => return None,
            }
        }
    }

    /// `true` when a wire field's type bottoms out in a SLICE (`[element]`)
    /// -- a repeated spelling with no declared maximum, which can never have
    /// a finite worst-case encoding and is rejected at the declaration.
    pub fn wire_field_is_unbounded_slice(&self, field: &wire::WireField) -> bool {
        let mut handle = field.type_reference;
        loop {
            if !handle.is_valid() {
                return false;
            }
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::Slice { .. } => return true,
                _ => return false,
            }
        }
    }

    /// A borrowed/unbounded slice's element type, looking through reference
    /// and constraint shells.
    pub fn wire_field_slice_element(
        &self,
        field: &wire::WireField,
    ) -> Option<types::TypeReferenceHandle> {
        let mut handle = field.type_reference;
        loop {
            if !handle.is_valid() {
                return None;
            }
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                _ => return None,
            }
        }
    }

    /// A general borrowed scalar slice supported by compact_binary's
    /// allocation-free encode path. `&[u8]` remains the distinct raw-byte
    /// path because `u8` is not in the packed scalar vocabulary.
    pub fn wire_field_borrowed_scalar_slice_encoding(
        &self,
        field: &wire::WireField,
    ) -> Option<wire::WireBorrowedScalarSliceEncoding> {
        let element = self.wire_field_slice_element(field)?;
        let element = self
            .primitive_type_reference(element)
            .and_then(wire::WireScalarEncoding::for_primitive)?;
        Some(wire::WireBorrowedScalarSliceEncoding { element })
    }

    /// A wire field's bounded repeated carrier. Fixed arrays carry exactly
    /// their static extent; `FixedVec<T, N>` carries a runtime length bounded
    /// by its inline array capacity.
    pub fn wire_field_repeated_carrier(
        &self,
        field: &wire::WireField,
    ) -> Option<(wire::WireRepeatedCarrier, types::TypeReferenceHandle, usize)> {
        if let Some((element, count)) = self.wire_field_fixed_array(field) {
            return Some((wire::WireRepeatedCarrier::FixedArray, element, count));
        }

        let mut handle = field.type_reference;
        loop {
            match self.type_reference_table.type_reference(handle) {
                types::TypeReferenceNode::Reference { referee, .. } => handle = *referee,
                types::TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                types::TypeReferenceNode::Generic {
                    base_symbol,
                    arguments,
                    ..
                } if self.is_core_fixed_vec(*base_symbol) => {
                    let arguments = self.type_reference_table.type_reference_handles(*arguments);
                    let [element, count] = arguments else {
                        return None;
                    };
                    let count = match self.type_reference_table.type_reference(*count) {
                        types::TypeReferenceNode::Named { name, .. } => {
                            name.as_str().parse::<usize>().ok()?
                        }
                        _ => return None,
                    };
                    return Some((wire::WireRepeatedCarrier::FixedVec, *element, count));
                }
                _ => break,
            }
        }

        // A specialized instance is a generated data definition whose
        // derivation leads back to the toolchain declaration.
        let types::TypeReferenceNode::Named { symbol, .. } =
            self.type_reference_table.type_reference(handle)
        else {
            return None;
        };
        if !self.is_core_fixed_vec(*symbol) {
            return None;
        }
        let data = self.data_definition_by_symbol(*symbol)?;
        let mut items = None;
        let mut has_u64_length = false;
        for member in self.data_members(data) {
            let crate::data::DataMember::Field(member) = member else {
                continue;
            };
            match member.name.as_str() {
                "items" => {
                    let mut synthetic = field.clone();
                    synthetic.type_reference = member.type_reference;
                    items = self.wire_field_fixed_array(&synthetic);
                }
                "length" => {
                    has_u64_length = matches!(
                        self.primitive_type_reference(member.type_reference),
                        Some(types::PrimitiveType::U64)
                    );
                }
                _ => {}
            }
        }
        let (element, count) = items?;
        has_u64_length.then_some((wire::WireRepeatedCarrier::FixedVec, element, count))
    }

    /// A wire field's bounded REPEATED encoding with a stage-2 scalar element
    /// (`i32`, `i64`, `u32`, `u64`, or `bool`).
    pub fn wire_field_repeated_encoding(
        &self,
        field: &wire::WireField,
    ) -> Option<wire::WireRepeatedEncoding> {
        let (carrier, element_type, max_count) = self.wire_field_repeated_carrier(field)?;
        let element = self
            .primitive_type_reference(element_type)
            .and_then(wire::WireScalarEncoding::for_primitive)?;
        Some(wire::WireRepeatedEncoding {
            carrier,
            element,
            max_count,
        })
    }

    /// The worst-case byte count of a schema's CURRENT-era body WITHOUT the
    /// era discriminator -- the sub-message framing a nested message field
    /// carries (chapter 20, decision 10: the era rides only the top-level
    /// envelope, never a nested struct). `Some` only when every current-era
    /// field is a plain stage 2 scalar: a String body is runtime-unbounded
    /// and a doubly-nested body is a deeper composition, so both make the
    /// caller reject with its own diagnostic. Erased fields retain schema
    /// identity but do not contribute a tag, value, or boundedness condition.
    pub fn wire_schema_scalar_body_worst_case(&self, schema: &wire::WireSchema) -> Option<usize> {
        let mut worst_case_bytes = 0usize;
        for member in self.wire_members(schema.members) {
            let wire::WireMember::Field(field) = member else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let scalar = self
                .primitive_type_reference(field.type_reference)
                .and_then(wire::WireScalarEncoding::for_primitive)?;
            worst_case_bytes +=
                wire::wire_varint_bytes(field.number).len() + scalar.max_varint_length();
        }
        Some(worst_case_bytes)
    }

    /// The worst-case byte budget a nested message field adds to its parent's
    /// encoding: the sub-message's scalar body worst case plus the LENGTH
    /// varint that prefixes it (the actual length never exceeds the worst
    /// case, so its varint never grows past the worst case's varint). The
    /// field's TAG varint is the caller's to add.
    pub fn wire_nested_field_worst_case(&self, child: &wire::WireSchema) -> Option<usize> {
        let body = self.wire_schema_scalar_body_worst_case(child)?;
        Some(wire::wire_varint_bytes(body as u64).len() + body)
    }
}
