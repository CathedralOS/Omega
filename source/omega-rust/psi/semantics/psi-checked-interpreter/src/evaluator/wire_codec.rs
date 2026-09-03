use super::*;

impl<'program> Evaluator<'program> {
    /// `Schema::encode(&value, &mut out, &mut written)` -- the
    /// compact_binary v0 encoder the compiler synthesizes for a wire schema
    /// (chapter 20, wire stage 2a). The interpreter implements the identical
    /// framing the native backends emit: the current era discriminator varint,
    /// then fields in field-number order. Nested messages omit their own era
    /// discriminator, and bounded output drops bytes beyond the destination
    /// capacity exactly like the native encoder.
    pub(super) fn try_wire_encode_call(
        &mut self,
        call: &TableCall,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        use psi_typed_trees::wire::{WireFieldEncoding, WireMember, wire_varint_bytes};

        let Some(schema) = self.program.wire_encode_call_schema(call) else {
            return Ok(None);
        };
        let schema_name = schema.name.as_str().to_owned();
        let era = self.program.wire_schema_current_era(schema);

        // (field name, number, content) of the CURRENT era, in field-number
        // order -- validation has already enforced the stage 2 field set
        // (scalars, at most one trailing String, scalar-only nested
        // messages).
        let mut fields = Vec::new();
        for member in self.program.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            if let Some(repeated) = self.program.wire_field_repeated_encoding(field) {
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::Repeated(repeated),
                ));
                continue;
            }
            if let Some(child) = self.program.wire_field_nested_schema(field) {
                let children = wire_nested_scalar_fields(self.program, child)?;
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::Nested(children),
                ));
                continue;
            }
            // A borrowed `&[u8]` field encodes as RAW bytes (length + the bytes),
            // read from the field's element array.
            if self.program.is_borrowed_byte_slice(field.type_reference) {
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::ByteSlice,
                ));
                continue;
            }
            if let Some(slice) = self
                .program
                .wire_field_borrowed_scalar_slice_encoding(field)
            {
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpField::ScalarSlice(slice),
                ));
                continue;
            }
            let encoding = self
                .program
                .primitive_type_reference(field.type_reference)
                .and_then(WireFieldEncoding::for_primitive)
                .ok_or_else(|| {
                    Halt::Unsupported(format!(
                        "data `{schema_name}` field `{}` is not a stage 2a scalar or String",
                        field.name
                    ))
                })?;
            fields.push((
                field.name.as_str().to_owned(),
                field.number,
                WireInterpField::Direct(encoding),
            ));
        }
        fields.sort_by_key(|(_, number, _)| *number);
        let has_text_field = fields.iter().any(|(_, _, content)| {
            matches!(
                content,
                WireInterpField::Direct(WireFieldEncoding::Text)
                    | WireInterpField::ByteSlice
                    | WireInterpField::ScalarSlice(_)
            )
        });

        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        let [value_argument, out_argument, written_argument] = arguments else {
            return Err(Halt::Trap(format!(
                "`{schema_name}::encode` expects 3 arguments, got {}",
                arguments.len()
            )));
        };
        let (value_argument, out_argument, written_argument) =
            (*value_argument, *out_argument, *written_argument);

        let value_cell = self.eval_argument(value_argument, frame)?;
        let value_cell = self.deref_cell(value_cell);
        let out_cell = self.eval_argument(out_argument, frame)?;
        let out_cell = self.deref_cell(out_cell);
        let written_cell = self.eval_argument(written_argument, frame)?;
        let written_cell = self.deref_cell(written_cell);

        let mut bytes = wire_varint_bytes(era);
        for (field_name, number, content) in &fields {
            bytes.extend(wire_varint_bytes(*number));

            let raw = match &*value_cell.borrow() {
                Value::Struct { fields, .. } => fields
                    .get(field_name)
                    .map(|cell| self.deref_cell(cell.clone()))
                    .ok_or_else(|| {
                        Halt::Trap(format!(
                            "`{schema_name}::encode` value has no field `{field_name}`"
                        ))
                    })?,
                _ => {
                    return Err(Halt::Trap(format!(
                        "`{schema_name}::encode` value argument is not a data value"
                    )));
                }
            };
            match content {
                WireInterpField::Direct(WireFieldEncoding::Scalar(scalar)) => {
                    let raw = raw.borrow().as_int().ok_or_else(|| {
                        Halt::Trap(format!(
                            "`{schema_name}::encode` field `{field_name}` is not a scalar value"
                        ))
                    })?;
                    bytes.extend(wire_varint_bytes(wire_scalar_varint_value(raw, *scalar)?));
                }
                WireInterpField::Nested(children) => {
                    // The sub-message's fields into a staging body first --
                    // mirroring the native scratch staging -- then the LENGTH
                    // varint and the body. NO era discriminator: the era
                    // rides only the top-level envelope (decision 10).
                    let mut body = Vec::new();
                    for (child_name, child_number, scalar) in children {
                        body.extend(wire_varint_bytes(*child_number));
                        let child_raw = match &*raw.borrow() {
                            Value::Struct { fields, .. } => fields
                                .get(child_name)
                                .map(|cell| self.deref_cell(cell.clone()))
                                .ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::encode` nested field `{field_name}` has no member `{child_name}`"
                                    ))
                                })?,
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::encode` nested field `{field_name}` is not a data value"
                                )));
                            }
                        };
                        let child_raw = child_raw.borrow().as_int().ok_or_else(|| {
                            Halt::Trap(format!(
                                "`{schema_name}::encode` nested field `{field_name}.{child_name}` is not a scalar value"
                            ))
                        })?;
                        body.extend(wire_varint_bytes(wire_scalar_varint_value(
                            child_raw, *scalar,
                        )?));
                    }
                    bytes.extend(wire_varint_bytes(body.len() as u64));
                    bytes.extend(body);
                }
                WireInterpField::Repeated(repeated) => {
                    // Fixed arrays are exactly full. FixedVec carries its
                    // runtime length beside the inline items array.
                    let (items, live) = match repeated.carrier {
                        psi_typed_trees::wire::WireRepeatedCarrier::FixedArray => {
                            (raw.clone(), repeated.max_count)
                        }
                        psi_typed_trees::wire::WireRepeatedCarrier::FixedVec => {
                            let (items, length) = match &*raw.borrow() {
                                Value::Struct { fields, .. } => {
                                    let items = fields.get("items").cloned().ok_or_else(|| {
                                        Halt::Trap(format!(
                                            "`{schema_name}::encode` FixedVec field `{field_name}` has no `items`"
                                        ))
                                    })?;
                                    let length =
                                        fields.get("length").cloned().ok_or_else(|| {
                                            Halt::Trap(format!(
                                                "`{schema_name}::encode` FixedVec field `{field_name}` has no `length`"
                                            ))
                                        })?;
                                    (self.deref_cell(items), self.deref_cell(length))
                                }
                                _ => {
                                    return Err(Halt::Trap(format!(
                                        "`{schema_name}::encode` repeated field `{field_name}` is not a FixedVec value"
                                    )));
                                }
                            };
                            let length = length.borrow().as_int().ok_or_else(|| {
                                Halt::Trap(format!(
                                    "`{schema_name}::encode` FixedVec field `{field_name}.length` is not a scalar value"
                                ))
                            })? as u64;
                            (items, length.min(repeated.max_count as u64) as usize)
                        }
                    };
                    let mut body = Vec::new();
                    match &*items.borrow() {
                        Value::Array(elements) => {
                            for element in elements.iter().take(live) {
                                let element_raw =
                                    self.deref_cell(element.clone()).borrow().as_int().ok_or_else(
                                        || {
                                            Halt::Trap(format!(
                                                "`{schema_name}::encode` repeated field `{field_name}` element is not a scalar value"
                                            ))
                                        },
                                    )?;
                                body.extend(wire_varint_bytes(wire_scalar_varint_value(
                                    element_raw,
                                    repeated.element,
                                )?));
                            }
                        }
                        _ => {
                            return Err(Halt::Trap(format!(
                                "`{schema_name}::encode` repeated field `{field_name}` has no inline array storage"
                            )));
                        }
                    }
                    bytes.extend(wire_varint_bytes(body.len() as u64));
                    bytes.extend(body);
                }
                WireInterpField::ScalarSlice(slice) => {
                    let elements = match &*raw.borrow() {
                        Value::Array(elements) => elements.clone(),
                        _ => {
                            return Err(Halt::Trap(format!(
                                "`{schema_name}::encode` field `{field_name}` is not a borrowed scalar-slice value"
                            )));
                        }
                    };
                    let mut body = Vec::new();
                    for element in &elements {
                        let element_raw =
                            self.deref_cell(element.clone()).borrow().as_int().ok_or_else(
                                || {
                                    Halt::Trap(format!(
                                        "`{schema_name}::encode` borrowed scalar-slice field `{field_name}` element is not a scalar value"
                                    ))
                                },
                            )?;
                        body.extend(wire_varint_bytes(wire_scalar_varint_value(
                            element_raw,
                            slice.element,
                        )?));
                    }
                    bytes.extend(wire_varint_bytes(body.len() as u64));
                    bytes.extend(body);
                }
                WireInterpField::Direct(WireFieldEncoding::Text) => {
                    // Length varint (byte count) then the raw UTF-8 bytes --
                    // the same framing the native text-bytes append emits.
                    let text = match &*raw.borrow() {
                        Value::Str(text) => text.borrow().clone(),
                        _ => {
                            return Err(Halt::Trap(format!(
                                "`{schema_name}::encode` field `{field_name}` is not a String value"
                            )));
                        }
                    };
                    bytes.extend(wire_varint_bytes(text.len() as u64));
                    bytes.extend_from_slice(&text);
                }
                WireInterpField::ByteSlice => {
                    // Length varint (byte count) then the raw bytes, framed like Text. A `&[u8]`
                    // field is text BYTES (`Value::Str`, after the text=bytes model) OR a fixed
                    // array of byte cells; both yield the raw content.
                    let str_bytes = if let Value::Str(text) = &*raw.borrow() {
                        Some(text.borrow().clone())
                    } else {
                        None
                    };
                    let content: Vec<u8> = if let Some(content) = str_bytes {
                        content
                    } else {
                        let elements = match &*raw.borrow() {
                            Value::Array(elements) => elements.clone(),
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::encode` field `{field_name}` is not a byte-slice value"
                                )));
                            }
                        };
                        let mut content = Vec::with_capacity(elements.len());
                        for element in &elements {
                            let byte = self
                                .deref_cell(element.clone())
                                .borrow()
                                .as_int()
                                .ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::encode` byte-slice field `{field_name}` element is not a byte"
                                    ))
                                })?;
                            content.push(byte as u8);
                        }
                        content
                    };
                    bytes.extend(wire_varint_bytes(content.len() as u64));
                    bytes.extend(content);
                }
            }
        }

        match &*out_cell.borrow() {
            Value::Array(elements) => {
                if bytes.len() > elements.len() && !has_text_field {
                    // Without a runtime-sized text field validation's worst-case budget
                    // covers every byte, so an overflow here is a compiler
                    // bug, not a program state -- trap loudly.
                    return Err(Halt::Trap(format!(
                        "`{schema_name}::encode` produced {} bytes into a {}-byte buffer",
                        bytes.len(),
                        elements.len()
                    )));
                }
                // With a runtime-sized text field the native byte-copy bounds every store
                // against the buffer's capacity and DROPS overflowing content
                // (the text field encodes last); `zip` clamps identically.
                for (element, byte) in elements.iter().zip(&bytes) {
                    *element.borrow_mut() = Value::Int(i64::from(*byte));
                }
            }
            _ => {
                return Err(Halt::Trap(format!(
                    "`{schema_name}::encode` out argument is not a fixed byte array"
                )));
            }
        }
        let buffer_capacity = match &*out_cell.borrow() {
            Value::Array(elements) => elements.len(),
            _ => unreachable!("out argument validated as an array above"),
        };
        *written_cell.borrow_mut() = Value::Int(bytes.len().min(buffer_capacity) as i64);

        Ok(Some(Value::Unit))
    }

    /// `Schema::decode(&mut value, &buffer, &mut read, &mut verdict)` -- the
    /// compact_binary v0 decoder the compiler synthesizes for a wire schema
    /// (chapter 20, wire stage 2b). The interpreter simulates the IDENTICAL
    /// operation sequence the native backends emit -- expected framing bytes
    /// for the CURRENT era discriminator and each field-number tag, then a
    /// bounds-checked LEB128 value read per field -- including the sticky
    /// failure semantics: the first violation (wrong era, unexpected tag,
    /// truncated input, overlong varint) clears `ok`, but the remaining
    /// operations still run so cursor and field side effects match the native
    /// sequences byte for byte even on the failure path.
    pub(super) fn try_wire_decode_call(
        &mut self,
        call: &TableCall,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        use psi_typed_trees::wire::{WireMember, WireScalarEncoding, wire_varint_bytes};

        let Some(schema) = self.program.wire_decode_call_schema(call) else {
            return Ok(None);
        };
        let schema_name = schema.name.as_str().to_owned();
        let era = self.program.wire_schema_current_era(schema);
        let arguments = self
            .program
            .statement_table
            .expression_handles(call.arguments);
        let [value_argument, buffer_argument, read_argument, ok_argument] = arguments else {
            return Err(Halt::Trap(format!(
                "`{schema_name}::decode` expects 4 arguments, got {}",
                arguments.len()
            )));
        };
        let (value_argument, buffer_argument, read_argument, ok_argument) = (
            *value_argument,
            *buffer_argument,
            *read_argument,
            *ok_argument,
        );
        let value_type = wire_argument_declared_type(self.program, frame, value_argument)
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "`{schema_name}::decode` cannot resolve the declared value type"
                ))
            })?;

        // (field name, number, content) of the CURRENT era, in field-number
        // order -- validation has already enforced the stage 2 field set
        // (scalars plus scalar-only nested messages).
        let mut fields = Vec::new();
        for member in self.program.wire_members(schema.members) {
            let WireMember::Field(field) = member else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let target_type = psi_typed_trees::wire::data_field_type(
                self.program,
                value_type,
                field.name.as_str(),
            )
            .ok_or_else(|| {
                Halt::Unsupported(format!(
                    "`{schema_name}::decode` cannot resolve destination field `{}`",
                    field.name
                ))
            })?;
            if let Some(repeated) = self.program.wire_field_repeated_encoding(field) {
                let range = psi_typed_trees::wire::repeated_element_type(
                    self.program,
                    target_type,
                    repeated.carrier,
                )
                .and_then(|element| {
                    psi_typed_trees::wire::scalar_decode_range(self.program, element)
                });
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpScalarField::Repeated {
                        encoding: repeated,
                        range,
                    },
                ));
                continue;
            }
            if let Some(child) = self.program.wire_field_nested_schema(field) {
                let children = wire_nested_decode_scalar_fields(self.program, child, target_type)?;
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpScalarField::Nested(children),
                ));
                continue;
            }
            // A borrowed `&[u8]` field decodes zero-copy: length-prefixed bytes
            // viewed in the buffer (validation requires the value field `&[u8]`).
            // A DOMAIN on the slice (`&[u8] in Utf8`) is a decode-boundary
            // obligation: the wire carries UNTRUSTED bytes no compile-time
            // proof covers, so the decoder evaluates the domain's recognized
            // byte predicate and fails the verdict when it does not hold. A
            // declared domain not reducible to one recognized byte-predicate
            // fact refuses LOUDLY -- silently skipping validation would
            // deliver a domain-tagged slice with unchecked bytes (the pinned
            // utf8_decode_accepts_invalid_bytes soundness hole).
            if self.program.is_borrowed_byte_slice(field.type_reference) {
                let mut predicates = Vec::new();
                for (domain_name, predicate) in
                    psi_typed_trees::byte_predicates::type_reference_domain_predicates(
                        self.program,
                        field.type_reference,
                    )
                {
                    let Some(predicate) = predicate else {
                        return Err(Halt::Unsupported(format!(
                            "`{schema_name}::decode` field `{}` carries domain `{domain_name}`, which is not exactly one recognized byte-predicate fact -- the decode boundary cannot validate it yet",
                            field.name
                        )));
                    };
                    predicates.push(predicate);
                }
                fields.push((
                    field.name.as_str().to_owned(),
                    field.number,
                    WireInterpScalarField::ByteSlice { predicates },
                ));
                continue;
            }
            let encoding = self
                .program
                .primitive_type_reference(field.type_reference)
                .and_then(WireScalarEncoding::for_primitive)
                .ok_or_else(|| {
                    Halt::Unsupported(format!(
                        "data `{schema_name}` field `{}` is not a stage 2 scalar",
                        field.name
                    ))
                })?;
            fields.push((
                field.name.as_str().to_owned(),
                field.number,
                WireInterpScalarField::Scalar {
                    encoding,
                    range: psi_typed_trees::wire::scalar_decode_range(self.program, target_type),
                },
            ));
        }
        fields.sort_by_key(|(_, number, _)| *number);

        let value_cell = self.eval_argument(value_argument, frame)?;
        let value_cell = self.deref_cell(value_cell);
        let buffer_cell = self.eval_argument(buffer_argument, frame)?;
        let buffer_cell = self.deref_cell(buffer_cell);
        let read_cell = self.eval_argument(read_argument, frame)?;
        let read_cell = self.deref_cell(read_cell);
        let ok_cell = self.eval_argument(ok_argument, frame)?;
        let ok_cell = self.deref_cell(ok_cell);

        // The decode buffer's bytes and compile-time length.
        let buffer: Vec<u8> = match &*buffer_cell.borrow() {
            Value::Array(elements) => elements
                .iter()
                .map(|element| {
                    element
                        .borrow()
                        .as_int()
                        .map(|byte| byte as u8)
                        .ok_or_else(|| {
                            Halt::Trap(format!(
                                "`{schema_name}::decode` buffer element is not a byte"
                            ))
                        })
                })
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(Halt::Trap(format!(
                    "`{schema_name}::decode` buffer argument is not a fixed byte array"
                )));
            }
        };

        // read = 0, ok = true -- then the sticky flag only ever clears.
        let mut cursor = 0usize;
        let mut ok = true;

        // One expected framing byte: out of bounds clears ok without
        // consuming; a mismatch consumes the byte and clears ok.
        let expect_byte = |cursor: &mut usize, ok: &mut bool, expected: u8| {
            let Some(byte) = buffer.get(*cursor).copied() else {
                *ok = false;
                return;
            };
            *cursor += 1;
            if byte != expected {
                *ok = false;
            }
        };

        // One canonical LEB128 value read, mirroring the native loop exactly:
        // truncation, more than ten groups, a zero terminal payload after the
        // first group, or a tenth payload above one clear ok. The accumulated
        // value is returned regardless (failure permits partial output).
        let read_varint = |cursor: &mut usize, ok: &mut bool| -> u64 {
            let mut value = 0u64;
            let mut shift = 0u32;
            loop {
                if shift > 63 {
                    *ok = false;
                    return value;
                }
                let Some(byte) = buffer.get(*cursor).copied() else {
                    *ok = false;
                    return value;
                };
                *cursor += 1;
                let payload = u64::from(byte & 0x7f);
                if shift == 63 && payload > 1 {
                    *ok = false;
                }
                value |= payload << shift;
                let terminal = byte & 0x80 == 0;
                if terminal && shift > 0 && payload == 0 {
                    *ok = false;
                }
                shift += 7;
                if terminal {
                    return value;
                }
            }
        };

        for byte in wire_varint_bytes(era) {
            expect_byte(&mut cursor, &mut ok, byte);
        }

        for (field_name, number, content) in &fields {
            for byte in wire_varint_bytes(*number) {
                expect_byte(&mut cursor, &mut ok, byte);
            }

            let field_cell = match &*value_cell.borrow() {
                Value::Struct { fields, .. } => {
                    fields.get(field_name).cloned().ok_or_else(|| {
                        Halt::Trap(format!(
                            "`{schema_name}::decode` value has no field `{field_name}`"
                        ))
                    })?
                }
                _ => {
                    return Err(Halt::Trap(format!(
                        "`{schema_name}::decode` value argument is not a data value"
                    )));
                }
            };
            let field_cell = self.deref_cell(field_cell);

            match content {
                WireInterpScalarField::Scalar { encoding, range } => {
                    let raw = read_varint(&mut cursor, &mut ok);
                    let decoded = wire_decoded_scalar_value(raw, *encoding)?;
                    if range
                        .is_none_or(|range| wire_scalar_in_range(raw, *encoding, &decoded, range))
                    {
                        *field_cell.borrow_mut() = decoded;
                    } else {
                        ok = false;
                    }
                }
                WireInterpScalarField::ByteSlice { predicates } => {
                    // A borrowed `&[u8]`: a byte-LENGTH varint then that many
                    // bytes, stored as an owned Array of byte values
                    // (observationally identical to a buffer view for any read).
                    // A length past the buffer clears ok and the cursor stops at
                    // the buffer end -- the native byte-copy bounds-checks the
                    // same way.
                    let length = read_varint(&mut cursor, &mut ok) as usize;
                    let available = buffer.len().saturating_sub(cursor);
                    if length > available {
                        ok = false;
                    }
                    let take = length.min(available);
                    let bytes = &buffer[cursor..cursor + take];
                    // Decode-boundary domain validation: untrusted wire bytes
                    // must satisfy the slice's declared byte predicates or the
                    // verdict is Invalid (a truncated read is already !ok
                    // above; validating the truncated view is harmless).
                    for predicate in predicates {
                        if !predicate.holds_for(bytes) {
                            ok = false;
                        }
                    }
                    let elements: Vec<Cell> = bytes
                        .iter()
                        .map(|byte| self.allocate_cell(Value::Int(i64::from(*byte))))
                        .collect::<EvalResult<_>>()?;
                    cursor += take;
                    *field_cell.borrow_mut() = Value::Array(elements);
                }
                WireInterpScalarField::Nested(children) => {
                    // LENGTH varint, then the absolute end bound -- the same
                    // two checks the native nested OPEN applies: the raw
                    // length must fit the buffer (so the 64-bit sum cannot
                    // wrap back inside it) and so must the bound. The child's
                    // fields decode WITHOUT an era discriminator, and the
                    // CLOSE check fails ok unless the cursor landed exactly
                    // on the bound.
                    let length = read_varint(&mut cursor, &mut ok);
                    if length > buffer.len() as u64 {
                        ok = false;
                    }
                    let end = cursor.wrapping_add(length as usize);
                    if end > buffer.len() {
                        ok = false;
                    }
                    for (child_name, child_number, encoding, range) in children {
                        for byte in wire_varint_bytes(*child_number) {
                            expect_byte(&mut cursor, &mut ok, byte);
                        }
                        let raw = read_varint(&mut cursor, &mut ok);
                        let decoded = wire_decoded_scalar_value(raw, *encoding)?;
                        let child_cell = match &*field_cell.borrow() {
                            Value::Struct { fields, .. } => {
                                fields.get(child_name).cloned().ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::decode` nested field `{field_name}` has no member `{child_name}`"
                                    ))
                                })?
                            }
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::decode` nested field `{field_name}` is not a data value"
                                )));
                            }
                        };
                        let child_cell = self.deref_cell(child_cell);
                        if range.is_none_or(|range| {
                            wire_scalar_in_range(raw, *encoding, &decoded, range)
                        }) {
                            *child_cell.borrow_mut() = decoded;
                        } else {
                            ok = false;
                        }
                    }
                    if cursor != end {
                        ok = false;
                    }
                }
                WireInterpScalarField::Repeated { encoding, range } => {
                    // Byte-LENGTH varint, the same OPEN bound checks as a
                    // nested message, then the carrier-specific bounded
                    // reads. Fixed arrays require exactly N elements;
                    // FixedVec guards on the region end and updates its own
                    // length. The CLOSE check rejects framing that disagrees
                    // with either carrier.
                    let length = read_varint(&mut cursor, &mut ok);
                    if length > buffer.len() as u64 {
                        ok = false;
                    }
                    let end = cursor.wrapping_add(length as usize);
                    if end > buffer.len() {
                        ok = false;
                    }
                    let (items_cell, count_cell) = match encoding.carrier {
                        psi_typed_trees::wire::WireRepeatedCarrier::FixedArray => {
                            (field_cell.clone(), None)
                        }
                        psi_typed_trees::wire::WireRepeatedCarrier::FixedVec => {
                            match &*field_cell.borrow() {
                                Value::Struct { fields, .. } => {
                                    let items =
                                        fields.get("items").cloned().ok_or_else(|| {
                                            Halt::Trap(format!(
                                                "`{schema_name}::decode` FixedVec field `{field_name}` has no `items`"
                                            ))
                                        })?;
                                    let length =
                                        fields.get("length").cloned().ok_or_else(|| {
                                            Halt::Trap(format!(
                                                "`{schema_name}::decode` FixedVec field `{field_name}` has no `length`"
                                            ))
                                        })?;
                                    (self.deref_cell(items), Some(self.deref_cell(length)))
                                }
                                _ => {
                                    return Err(Halt::Trap(format!(
                                        "`{schema_name}::decode` repeated field `{field_name}` is not a FixedVec value"
                                    )));
                                }
                            }
                        }
                    };
                    let mut decoded = 0i64;
                    if let Some(count_cell) = &count_cell {
                        *count_cell.borrow_mut() = Value::Int(0);
                    }
                    for index in 0..encoding.max_count {
                        if matches!(
                            encoding.carrier,
                            psi_typed_trees::wire::WireRepeatedCarrier::FixedVec
                        ) && cursor >= end
                        {
                            continue;
                        }
                        let raw_value = read_varint(&mut cursor, &mut ok);
                        let decoded_value = wire_decoded_scalar_value(raw_value, encoding.element)?;
                        let element_cell = match &*items_cell.borrow() {
                            Value::Array(elements) => {
                                elements.get(index).cloned().ok_or_else(|| {
                                    Halt::Trap(format!(
                                        "`{schema_name}::decode` repeated field `{field_name}` has no element {index}"
                                    ))
                                })?
                            }
                            _ => {
                                return Err(Halt::Trap(format!(
                                    "`{schema_name}::decode` repeated field `{field_name}` is not a fixed array value"
                                )));
                            }
                        };
                        let element_cell = self.deref_cell(element_cell);
                        if range.is_none_or(|range| {
                            wire_scalar_in_range(raw_value, encoding.element, &decoded_value, range)
                        }) {
                            *element_cell.borrow_mut() = decoded_value;
                        } else {
                            ok = false;
                        }
                        decoded += 1;
                        if let Some(count_cell) = &count_cell {
                            *count_cell.borrow_mut() = Value::Int(decoded);
                        }
                    }
                    if cursor != end {
                        ok = false;
                    }
                }
            }
        }

        *read_cell.borrow_mut() = Value::Int(cursor as i64);
        // The verdict enum (`WireVerdict`): Sound on a clean decode, Invalid
        // on the first violation -- mirrors the native tag write (Invalid = 0
        // = the ZII zero case, Sound = 1). The declaring type resolves by
        // name (invalid when the program declares no WireVerdict, and the
        // name-global fallback covers it).
        *ok_cell.borrow_mut() = Value::Enum {
            type_symbol: self
                .find_data_by_name("WireVerdict")
                .map(|data| data.symbol)
                .unwrap_or_else(SymbolHandle::invalid),
            variant_name: if ok { "Sound" } else { "Invalid" }.to_owned(),
            payload: Vec::new(),
        };

        Ok(Some(Value::Unit))
    }
}
