//! Wire schema plans, members and the encode and decode call schemas.

use crate::typed_trees::TypedTrees;
use crate::{types, wire};
use arena::HandleSpan;

impl TypedTrees {
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

    /// The derived wire plan for a schema, when one was computed -- the
    /// placements in tag order. `None` for schemas the plan pass skipped.
    pub fn wire_schema_plan(
        &self,
        schema: symbols::SymbolHandle,
    ) -> Option<&[wire::WirePlacement]> {
        self.wire_schema_plans
            .iter()
            .find(|plan| plan.schema == schema)
            .and_then(|plan| self.wire_placements.span(plan.placements))
    }

    /// Whether the schema's recorded plan was checked against the authored
    /// grammar policy. `false` when no plan was recorded (the pass skipped
    /// the schema) or no `CompactBinary::plan` policy was defined.
    pub fn wire_schema_plan_policy_verified(&self, schema: symbols::SymbolHandle) -> bool {
        self.wire_schema_plans
            .iter()
            .find(|plan| plan.schema == schema)
            .is_some_and(|plan| plan.policy_verified)
    }

    /// Dynamic encode obligations retained beside one schema's placements.
    pub fn wire_schema_encode_obligations(
        &self,
        schema: symbols::SymbolHandle,
    ) -> Option<&[wire::WireEncodeObligation]> {
        self.wire_schema_plans
            .iter()
            .find(|plan| plan.schema == schema)
            .and_then(|plan| self.wire_encode_obligations.span(plan.encode_obligations))
    }

    pub fn push_wire_schema(&mut self, wire_schema: wire::WireSchema) {
        self.tables
            .wire_schemas
            .append_to_span(&mut self.roots.wire_schemas, wire_schema);
    }

    pub fn wire_schemas(&self) -> &[wire::WireSchema] {
        self.tables
            .wire_schemas
            .span_or_empty(self.roots.wire_schemas)
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

    /// Recognize the compiler-synthesized wire encoder call shape
    /// `Schema::encode(&value, &mut out, &mut written)` (chapter 20,
    /// wire stage 2a): a statement call whose receiver names a wire schema
    /// and whose target is `encode`.
    pub fn wire_encode_call_schema(
        &self,
        call: &crate::statement::TableCall,
    ) -> Option<&wire::WireSchema> {
        if call.target.as_str() != wire::WIRE_ENCODE_MACHINE_NAME {
            return None;
        }
        self.wire_call_receiver_schema(call)
    }

    /// Recognize the compiler-synthesized wire decoder call shape
    /// `Schema::decode(&mut value, &buffer, &mut read, &mut ok)`
    /// (chapter 20, wire stage 2b): a statement call whose receiver names a
    /// wire schema and whose target is `decode`.
    pub fn wire_decode_call_schema(
        &self,
        call: &crate::statement::TableCall,
    ) -> Option<&wire::WireSchema> {
        if call.target.as_str() != wire::WIRE_DECODE_MACHINE_NAME {
            return None;
        }
        self.wire_call_receiver_schema(call)
    }

    /// Locate the wire schema a codec call's receiver names. A codec
    /// receiver is a compile-time declaration, never current-state storage,
    /// so it binds by declaration path rather than leaf spelling: a resolved
    /// `receiver_symbol` already carries the authored path's source-scoped
    /// precedence (imports, local module, shadowing storage), and a
    /// qualified `module::Schema` receiver that storage-path resolution
    /// cannot follow still selects its schema through the authored path
    /// itself. Same-named schemas in sibling modules stay distinct, and a
    /// leaf that resolves ambiguously or to unrelated storage cannot claim
    /// the call.
    pub fn wire_call_receiver_schema(
        &self,
        call: &crate::statement::TableCall,
    ) -> Option<&wire::WireSchema> {
        if call.receiver_symbol.is_valid() {
            let declaration_path = self.symbols.display_path(call.receiver_symbol, "::");
            return self
                .wire_schemas()
                .iter()
                .find(|schema| self.symbols.display_path(schema.symbol, "::") == declaration_path);
        }
        let members = self.statement_table.name_path_members(call.receiver);
        let authored_path = members
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        if authored_path.is_empty() {
            return None;
        }
        self.wire_schemas()
            .iter()
            .find(|schema| self.symbols.display_path(schema.symbol, "::") == authored_path)
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

    /// The era discriminator a declared version block's payloads carry: its
    /// zero-based position in the declaration-ordered version chain.
    pub fn wire_schema_version_era(
        &self,
        schema: &wire::WireSchema,
        version_name: &str,
    ) -> Option<u64> {
        self.wire_members(schema.members)
            .iter()
            .filter_map(|member| match member {
                wire::WireMember::Version(version) => Some(version),
                _ => None,
            })
            .position(|version| version.name.as_str() == version_name)
            .map(|position| position as u64)
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
                    base_name,
                    arguments,
                    ..
                } if base_name.as_str() == "FixedVec" => {
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

        let name = self.named_type_reference_name(handle)?;
        if !name.starts_with("FixedVec<") {
            return None;
        }
        let data = self
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == name)?;
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
