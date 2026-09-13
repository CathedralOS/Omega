use super::*;

impl<'program> Evaluator<'program> {
    pub(super) fn assemble_scalar_byte_region(
        &self,
        cells: &[Cell],
        offset: usize,
        target: PrimitiveType,
    ) -> EvalResult<Value> {
        let size = target
            .scalar_byte_size()
            .ok_or_else(|| Halt::Trap("byte-region recast target is not scalar".to_owned()))?;
        let mut bits: u64 = 0;
        for byte_index in 0..size {
            let cell = cells.get(offset + byte_index).ok_or_else(|| {
                Halt::Trap(format!(
                    "interior recast reads byte {} past the region",
                    offset + byte_index
                ))
            })?;
            let byte = cell.borrow().as_int().unwrap_or(0) as u64 & 0xFF;
            bits |= byte << (8 * byte_index);
        }
        let assembled = match target {
            PrimitiveType::F32 => Value::Float(interpreter_f32_from_bits(bits as u32)),
            PrimitiveType::F64 => Value::Float(f64::from_bits(bits)),
            integer => Value::Int(wrap_to_width(bits as i64, integer)),
        };
        Ok(assembled)
    }

    pub(super) fn write_scalar_byte_region(
        &self,
        cells: &[Cell],
        offset: usize,
        target: PrimitiveType,
        value: Value,
    ) -> EvalResult<()> {
        let size = target
            .scalar_byte_size()
            .ok_or_else(|| Halt::Trap("byte-region recast target is not scalar".to_owned()))?;
        let bits = match target {
            PrimitiveType::F32 => interpreter_f32_to_bits(
                value
                    .as_float()
                    .ok_or_else(|| Halt::Trap("f32 recast write is not numeric".to_owned()))?,
            ) as u64,
            PrimitiveType::F64 => value
                .as_float()
                .ok_or_else(|| Halt::Trap("f64 recast write is not numeric".to_owned()))?
                .to_bits(),
            _ => value
                .as_int()
                .ok_or_else(|| Halt::Trap("integer recast write is not numeric".to_owned()))?
                as u64,
        };
        for byte_index in 0..size {
            let cell = cells.get(offset + byte_index).ok_or_else(|| {
                Halt::Trap(format!(
                    "mutable interior recast writes byte {} past the region",
                    offset + byte_index
                ))
            })?;
            *cell.borrow_mut() = Value::Int(((bits >> (8 * byte_index)) & 0xFF) as i64);
        }
        Ok(())
    }

    pub(super) fn write_stored_integer_byte_region(
        &self,
        cells: &[Cell],
        offset: usize,
        target_type: TypeReferenceHandle,
        stored_width_bits: u16,
        interpretation: layout_plans::IntegerInterpretation,
        value: Value,
    ) -> EvalResult<()> {
        if stored_width_bits == 0 || stored_width_bits > 64 || !stored_width_bits.is_multiple_of(8)
        {
            return trap("invalid stored-integer width reached mutable record view");
        }
        let primitive = self
            .program
            .primitive_type_reference(target_type)
            .ok_or_else(|| Halt::Trap("stored-integer projection is not scalar".to_owned()))?;
        let domain = self
            .program
            .arithmetic_domain_for_type_reference(target_type);
        let value = self.coerce_scalar_with(value, primitive, domain)?;
        let integer = value
            .as_int()
            .ok_or_else(|| Halt::Trap("stored-integer recast write is not integer".to_owned()))?;
        let bit_count = u32::from(stored_width_bits);
        let fits = match interpretation {
            layout_plans::IntegerInterpretation::Signed => {
                let magnitude = 1_i128 << (bit_count - 1);
                i128::from(integer) >= -magnitude && i128::from(integer) < magnitude
            }
            layout_plans::IntegerInterpretation::Unsigned => {
                i128::from(integer) >= 0 && i128::from(integer) < (1_i128 << bit_count)
            }
        };
        if !fits {
            return trap(format!(
                "stored-integer recast write value {integer} does not fit {stored_width_bits}-bit {} storage",
                match interpretation {
                    layout_plans::IntegerInterpretation::Signed => "signed",
                    layout_plans::IntegerInterpretation::Unsigned => "unsigned",
                }
            ));
        }
        let stored_byte_count = usize::from(stored_width_bits / 8);
        let bits = integer as u64;
        for byte_index in 0..stored_byte_count {
            let cell = cells.get(offset + byte_index).ok_or_else(|| {
                Halt::Trap(format!(
                    "mutable stored-integer recast writes byte {} past the region",
                    offset + byte_index
                ))
            })?;
            *cell.borrow_mut() = Value::Int(((bits >> (8 * byte_index)) & 0xFF) as i64);
        }
        Ok(())
    }

    /// Rung C2's record view: decode fixed records little-endian from the same
    /// geometry native lowering consumes. Ordinary records use natural
    /// packing; plan-laid records use their validated offsets. Named fields
    /// recurse, permitting a plain wrapper around a plan-laid foreign record.
    fn assemble_record_view_inner(
        &self,
        type_name: &str,
        cells: &[Cell],
        base_offset: usize,
        visiting: &mut HashSet<String>,
    ) -> EvalResult<Option<Value>> {
        if !visiting.insert(type_name.to_owned()) {
            return Ok(None);
        }
        let Some(data) = self.find_data_by_name(type_name) else {
            visiting.remove(type_name);
            return Ok(None);
        };
        let mut field_specs: Vec<(String, TypeReferenceHandle, usize, usize)> = Vec::new();
        let mut field_symbols = Vec::new();
        for member in self.program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                visiting.remove(type_name);
                return Ok(None);
            };
            if field.relevance.is_erased() {
                continue;
            }
            let Some((size, align)) = self.record_view_type_layout(field.type_reference, visiting)
            else {
                visiting.remove(type_name);
                return Ok(None);
            };
            field_specs.push((
                field.name.as_str().to_owned(),
                field.type_reference,
                size,
                align,
            ));
            field_symbols.push(field.symbol);
        }

        let plan = self
            .program
            .plan_laid_layouts
            .iter()
            .find(|plan| plan.data_symbol == data.symbol);
        let offsets = if let Some(plan) = plan {
            if plan.offsets.len() != field_specs.len() || plan.field_symbols != field_symbols {
                visiting.remove(type_name);
                return Ok(None);
            }
            plan.offsets.clone()
        } else {
            let mut offsets = Vec::with_capacity(field_specs.len());
            let mut offset = 0usize;
            for (_, _, size, align) in &field_specs {
                offset = offset.div_ceil(*align) * *align;
                offsets.push(offset);
                let Some(next) = offset.checked_add(*size) else {
                    visiting.remove(type_name);
                    return Ok(None);
                };
                offset = next;
            }
            offsets
        };

        let type_symbol = data.symbol;
        let mut field_values = std::collections::BTreeMap::new();
        for (field_index, ((name, type_reference, _, _), field_offset)) in
            field_specs.into_iter().zip(offsets).enumerate()
        {
            let value = if let Some(integer) = plan.and_then(|plan| {
                plan.integer_fields
                    .iter()
                    .find(|integer| integer.field_index == field_index)
            }) {
                let Some(primitive) = self.program.primitive_type_reference(type_reference) else {
                    visiting.remove(type_name);
                    return Ok(None);
                };
                self.assemble_stored_integer_byte_region(
                    cells,
                    base_offset + field_offset,
                    primitive,
                    integer.stored_width_bits,
                    integer.interpretation,
                )?
            } else if let Some(value) = self.assemble_record_view_type_inner(
                type_reference,
                cells,
                base_offset + field_offset,
                visiting,
                plan.and_then(|plan| {
                    plan.repeated_fields
                        .iter()
                        .find(|repeated| repeated.field_index == field_index)
                        .map(|repeated| repeated.element_stride)
                }),
            )? {
                value
            } else {
                visiting.remove(type_name);
                return Ok(None);
            };
            field_values.insert(name, self.allocate_cell(value)?);
        }
        visiting.remove(type_name);
        Ok(Some(Value::Struct {
            type_symbol,
            type_name: type_name.to_owned(),
            fields: field_values,
        }))
    }

    pub(super) fn assemble_record_view_type(
        &self,
        type_reference: TypeReferenceHandle,
        cells: &[Cell],
        base_offset: usize,
    ) -> EvalResult<Option<Value>> {
        self.assemble_record_view_type_inner(
            type_reference,
            cells,
            base_offset,
            &mut HashSet::new(),
            None,
        )
    }

    /// Decode one validated `IntegerAt` leaf from its exact physical width and
    /// extend it into the portable semantic carrier. The layout validator has
    /// already established a positive whole-byte width through 64 bits and a
    /// total decode range; the interpreter mirrors native projection here.
    pub(super) fn assemble_stored_integer_byte_region(
        &self,
        cells: &[Cell],
        offset: usize,
        target: PrimitiveType,
        stored_width_bits: u16,
        interpretation: layout_plans::IntegerInterpretation,
    ) -> EvalResult<Value> {
        if stored_width_bits == 0 || stored_width_bits > 64 || !stored_width_bits.is_multiple_of(8)
        {
            return trap("stored-integer record view has an invalid physical width");
        }
        let byte_count = usize::from(stored_width_bits / 8);
        let mut bits = 0u64;
        for byte_index in 0..byte_count {
            let cell = cells.get(offset + byte_index).ok_or_else(|| {
                Halt::Trap(format!(
                    "stored-integer record view reads byte {} past the region",
                    offset + byte_index
                ))
            })?;
            let byte = cell.borrow().as_int().unwrap_or(0) as u64 & 0xFF;
            bits |= byte << (8 * byte_index);
        }
        if matches!(interpretation, layout_plans::IntegerInterpretation::Signed)
            && stored_width_bits < 64
        {
            let sign_bit = 1u64 << (stored_width_bits - 1);
            if bits & sign_bit != 0 {
                bits |= !0u64 << stored_width_bits;
            }
        }
        Ok(Value::Int(wrap_to_width(bits as i64, target)))
    }

    fn assemble_record_view_type_inner(
        &self,
        type_reference: TypeReferenceHandle,
        cells: &[Cell],
        base_offset: usize,
        visiting: &mut HashSet<String>,
        outer_stride: Option<usize>,
    ) -> EvalResult<Option<Value>> {
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            return self
                .assemble_scalar_byte_region(cells, base_offset, primitive)
                .map(Some);
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.assemble_record_view_type_inner(
                *base_type,
                cells,
                base_offset,
                visiting,
                outer_stride,
            ),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let Some((semantic_stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return Ok(None);
                };
                let stride = outer_stride.unwrap_or(semantic_stride);
                let mut values = Vec::with_capacity(*length);
                for index in 0..*length {
                    let Some(offset) = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                    else {
                        return trap("record-view array offset overflow");
                    };
                    let Some(value) = self.assemble_record_view_type_inner(
                        *element_type,
                        cells,
                        offset,
                        visiting,
                        None,
                    )?
                    else {
                        return Ok(None);
                    };
                    values.push(self.allocate_cell(value)?);
                }
                Ok(Some(Value::Array(values)))
            }
            TypeReferenceNode::Slice { element_type } => {
                let Some((stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return Ok(None);
                };
                let remaining = cells.len().saturating_sub(base_offset);
                if stride == 0 || !remaining.is_multiple_of(stride) {
                    return trap("slice recast bytes do not tile the element layout");
                }
                let length = remaining / stride;
                let mut values = Vec::with_capacity(length);
                for index in 0..length {
                    let offset = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                        .ok_or_else(|| Halt::Trap("slice recast offset overflow".to_owned()))?;
                    let Some(value) = self.assemble_record_view_type_inner(
                        *element_type,
                        cells,
                        offset,
                        visiting,
                        None,
                    )?
                    else {
                        return Ok(None);
                    };
                    values.push(self.allocate_cell(value)?);
                }
                Ok(Some(Value::Array(values)))
            }
            TypeReferenceNode::Named { name, .. } => {
                self.assemble_record_view_inner(name.as_str(), cells, base_offset, visiting)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn record_view_type_layout(
        &self,
        type_reference: TypeReferenceHandle,
        visiting: &mut HashSet<String>,
    ) -> Option<(usize, usize)> {
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            let size = primitive.scalar_byte_size()?;
            return Some((size, size));
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.record_view_type_layout(*base_type, visiting),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let (element_size, element_align) =
                    self.record_view_type_layout(*element_type, visiting)?;
                Some((element_size.checked_mul(*length)?, element_align))
            }
            TypeReferenceNode::Named { name, .. } => {
                self.record_view_data_layout(name.as_str(), visiting)
            }
            _ => None,
        }
    }

    fn record_view_data_layout(
        &self,
        type_name: &str,
        visiting: &mut HashSet<String>,
    ) -> Option<(usize, usize)> {
        if !visiting.insert(type_name.to_owned()) {
            return None;
        }
        let data = self.find_data_by_name(type_name)?;
        let mut field_layouts = Vec::new();
        let mut field_symbols = Vec::new();
        for member in self.program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                visiting.remove(type_name);
                return None;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let Some(layout) = self.record_view_type_layout(field.type_reference, visiting) else {
                visiting.remove(type_name);
                return None;
            };
            field_layouts.push(layout);
            field_symbols.push(field.symbol);
        }
        let result = if let Some(plan) = self
            .program
            .plan_laid_layouts
            .iter()
            .find(|plan| plan.data_symbol == data.symbol)
        {
            (plan.offsets.len() == field_layouts.len() && plan.field_symbols == field_symbols)
                .then_some((plan.size, plan.align))
        } else {
            let mut offset = 0usize;
            let mut max_align = 1usize;
            for (size, align) in field_layouts {
                offset = offset.div_ceil(align) * align;
                offset = offset.checked_add(size)?;
                max_align = max_align.max(align);
            }
            Some((offset.div_ceil(max_align) * max_align, max_align))
        };
        visiting.remove(type_name);
        result
    }

    fn record_view_fields(
        &self,
        type_name: &str,
    ) -> Option<
        Vec<(
            String,
            TypeReferenceHandle,
            usize,
            Option<typed_trees::PlanLaidIntegerField>,
            Option<typed_trees::PlanLaidRepeatedField>,
        )>,
    > {
        let data = self.find_data_by_name(type_name)?;
        let mut fields = Vec::new();
        let mut layouts = Vec::new();
        let mut field_symbols = Vec::new();
        for member in self.program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let layout = self.record_view_type_layout(field.type_reference, &mut HashSet::new())?;
            fields.push((field.name.as_str().to_owned(), field.type_reference));
            layouts.push(layout);
            field_symbols.push(field.symbol);
        }
        let (offsets, integer_fields, repeated_fields) = if let Some(plan) = self
            .program
            .plan_laid_layouts
            .iter()
            .find(|plan| plan.data_symbol == data.symbol)
        {
            if plan.offsets.len() != fields.len() || plan.field_symbols != field_symbols {
                return None;
            }
            (
                plan.offsets.clone(),
                plan.integer_fields.clone(),
                plan.repeated_fields.clone(),
            )
        } else {
            let mut offsets = Vec::with_capacity(fields.len());
            let mut offset = 0usize;
            for (size, align) in layouts {
                offset = offset.div_ceil(align) * align;
                offsets.push(offset);
                offset = offset.checked_add(size)?;
            }
            (offsets, Vec::new(), Vec::new())
        };
        Some(
            fields
                .into_iter()
                .zip(offsets)
                .enumerate()
                .map(|(field_index, ((name, type_reference), offset))| {
                    let stored_integer = integer_fields
                        .iter()
                        .find(|integer| integer.field_index == field_index)
                        .copied();
                    let repeated_field = repeated_fields
                        .iter()
                        .find(|repeated| repeated.field_index == field_index)
                        .copied();
                    (name, type_reference, offset, stored_integer, repeated_field)
                })
                .collect(),
        )
    }

    fn record_view_projection(
        &mut self,
        type_name: &str,
        path: &[MutableRecordProjectionStep],
        base_offset: usize,
        region_len: usize,
        frame: &Frame,
    ) -> EvalResult<Option<MutableRecordProjection>> {
        let Some((MutableRecordProjectionStep::Field(field_name), rest)) = path.split_first()
        else {
            return Ok(None);
        };
        let (_, field_type, field_offset, stored_integer, repeated_field) = self
            .record_view_fields(type_name)
            .unwrap_or_default()
            .into_iter()
            .find(|(name, _, _, _, _)| name == field_name)
            .ok_or_else(|| {
                Halt::Trap(format!(
                    "record view `{type_name}` has no field `{field_name}`"
                ))
            })?;
        let offset = base_offset
            .checked_add(field_offset)
            .ok_or_else(|| Halt::Trap("record-view field offset overflow".to_owned()))?;
        if stored_integer.is_some() {
            return Ok(rest.is_empty().then_some(MutableRecordProjection {
                offset,
                type_reference: field_type,
                stored_integer,
            }));
        }
        self.record_view_type_projection(
            field_type,
            rest,
            offset,
            region_len,
            frame,
            repeated_field.map(|repeated| repeated.element_stride),
        )
    }

    pub(super) fn record_view_type_projection(
        &mut self,
        type_reference: TypeReferenceHandle,
        path: &[MutableRecordProjectionStep],
        base_offset: usize,
        region_len: usize,
        frame: &Frame,
        outer_stride: Option<usize>,
    ) -> EvalResult<Option<MutableRecordProjection>> {
        if path.is_empty() {
            return Ok(Some(MutableRecordProjection {
                offset: base_offset,
                type_reference,
                stored_integer: None,
            }));
        }
        let (step, rest) = path.split_first().expect("nonempty projection path");
        match step {
            MutableRecordProjectionStep::Field(_) => {
                let Some(nested_name) = self
                    .record_view_named_type(type_reference)
                    .map(str::to_owned)
                else {
                    return Ok(None);
                };
                self.record_view_projection(&nested_name, path, base_offset, region_len, frame)
            }
            MutableRecordProjectionStep::Index(index_handle) => {
                let node = self
                    .program
                    .type_reference_table
                    .type_reference(type_reference)
                    .clone();
                let (element_type, length) = match node {
                    TypeReferenceNode::Constrained { base_type, .. }
                    | TypeReferenceNode::Reference {
                        referee: base_type, ..
                    } => {
                        return self.record_view_type_projection(
                            base_type,
                            path,
                            base_offset,
                            region_len,
                            frame,
                            outer_stride,
                        );
                    }
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: FixedArrayLength::Literal(length),
                    } => (element_type, length),
                    TypeReferenceNode::Slice { element_type } => {
                        let Some((stride, _)) =
                            self.record_view_type_layout(element_type, &mut HashSet::new())
                        else {
                            return Ok(None);
                        };
                        if stride == 0 || base_offset > region_len {
                            return Ok(None);
                        }
                        (element_type, (region_len - base_offset) / stride)
                    }
                    _ => return Ok(None),
                };
                let index = self.eval_index(*index_handle, frame)?;
                if index >= length {
                    return trap(format!(
                        "record-view array index {index} out of bounds for length {length}"
                    ));
                }
                let Some((semantic_stride, _)) =
                    self.record_view_type_layout(element_type, &mut HashSet::new())
                else {
                    return Ok(None);
                };
                let stride = outer_stride.unwrap_or(semantic_stride);
                let offset = stride
                    .checked_mul(index)
                    .and_then(|delta| base_offset.checked_add(delta))
                    .ok_or_else(|| Halt::Trap("record-view array offset overflow".to_owned()))?;
                self.record_view_type_projection(
                    element_type,
                    rest,
                    offset,
                    region_len,
                    frame,
                    None,
                )
            }
        }
    }

    fn write_record_view(
        &self,
        type_name: &str,
        cells: &[Cell],
        base_offset: usize,
        value: Value,
    ) -> EvalResult<()> {
        let Value::Struct { fields, .. } = value else {
            return trap(format!(
                "mutable record recast `{type_name}` requires a record value"
            ));
        };
        let Some(field_specs) = self.record_view_fields(type_name) else {
            return trap(format!(
                "cannot lay out mutable record recast `{type_name}`"
            ));
        };
        for (name, type_reference, field_offset, stored_integer, repeated_field) in field_specs {
            let Some(field_cell) = fields.get(&name) else {
                return trap(format!(
                    "record value written through `{type_name}` has no field `{name}`"
                ));
            };
            let offset = base_offset
                .checked_add(field_offset)
                .ok_or_else(|| Halt::Trap("mutable record recast offset overflow".to_owned()))?;
            let field_value = field_cell.borrow().clone();
            if let Some(integer) = stored_integer {
                self.write_stored_integer_byte_region(
                    cells,
                    offset,
                    type_reference,
                    integer.stored_width_bits,
                    integer.interpretation,
                    field_value,
                )?;
            } else {
                self.write_record_view_type_with_outer_stride(
                    type_reference,
                    cells,
                    offset,
                    field_value,
                    repeated_field.map(|repeated| repeated.element_stride),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn declared_type_is_slice(&self, type_reference: TypeReferenceHandle) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Slice { .. } => true,
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.declared_type_is_slice(*base_type),
            _ => false,
        }
    }

    /// The interpreter stores typed records as semantic field cells rather
    /// than a contiguous byte allocation. A typed record recast nevertheless
    /// aliases the record's representation, so snapshot the established source
    /// through its own layout, operate on those bytes through the target
    /// layout, then decode writes back through the source layout. Validation
    /// has already proved identical geometry and leaf representation sets.
    pub(super) fn snapshot_typed_value_bytes(
        &self,
        source: &Cell,
        source_type: TypeReferenceHandle,
    ) -> EvalResult<Vec<Cell>> {
        let (size, _) = self
            .record_view_type_layout(source_type, &mut HashSet::new())
            .ok_or_else(|| {
                Halt::Trap(format!(
                    "cannot lay out typed mutable recast source `{}`",
                    self.program.display_type_reference(source_type)
                ))
            })?;
        let cells = (0..size)
            .map(|_| self.allocate_cell(Value::Int(0)))
            .collect::<EvalResult<Vec<_>>>()?;
        self.write_record_view_type(source_type, &cells, 0, source.borrow().clone())?;
        Ok(cells)
    }

    pub(super) fn commit_typed_value_bytes(
        &self,
        source: &Cell,
        source_type: TypeReferenceHandle,
        cells: &[Cell],
    ) -> EvalResult<()> {
        let value = self
            .assemble_record_view_type(source_type, cells, 0)?
            .ok_or_else(|| {
                Halt::Trap(format!(
                    "cannot restore typed mutable recast source `{}`",
                    self.program.display_type_reference(source_type)
                ))
            })?;
        *source.borrow_mut() = value;
        Ok(())
    }

    pub(super) fn write_record_view_type(
        &self,
        type_reference: TypeReferenceHandle,
        cells: &[Cell],
        base_offset: usize,
        value: Value,
    ) -> EvalResult<()> {
        self.write_record_view_type_with_outer_stride(
            type_reference,
            cells,
            base_offset,
            value,
            None,
        )
    }

    fn write_record_view_type_with_outer_stride(
        &self,
        type_reference: TypeReferenceHandle,
        cells: &[Cell],
        base_offset: usize,
        value: Value,
        outer_stride: Option<usize>,
    ) -> EvalResult<()> {
        if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
            let domain = self
                .program
                .arithmetic_domain_for_type_reference(type_reference);
            let value = self.coerce_scalar_with(value, primitive, domain)?;
            return self.write_scalar_byte_region(cells, base_offset, primitive, value);
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            TypeReferenceNode::Constrained { base_type, .. }
            | TypeReferenceNode::Reference {
                referee: base_type, ..
            } => self.write_record_view_type_with_outer_stride(
                *base_type,
                cells,
                base_offset,
                value,
                outer_stride,
            ),
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => {
                let Value::Array(values) = value else {
                    return trap("mutable record array projection requires an array value");
                };
                if values.len() != *length {
                    return trap(format!(
                        "mutable record array write has length {}, expected {length}",
                        values.len()
                    ));
                }
                let Some((semantic_stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return trap("cannot lay out mutable record array element");
                };
                let stride = outer_stride.unwrap_or(semantic_stride);
                for (index, value) in values.into_iter().enumerate() {
                    let offset = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                        .ok_or_else(|| {
                            Halt::Trap("mutable record array offset overflow".to_owned())
                        })?;
                    self.write_record_view_type(
                        *element_type,
                        cells,
                        offset,
                        value.borrow().clone(),
                    )?;
                }
                Ok(())
            }
            TypeReferenceNode::Slice { element_type } => {
                let Value::Array(values) = value else {
                    return trap("mutable slice recast requires an array value");
                };
                let Some((stride, _)) =
                    self.record_view_type_layout(*element_type, &mut HashSet::new())
                else {
                    return trap("cannot lay out mutable slice element");
                };
                let available = cells.len().saturating_sub(base_offset);
                if stride == 0
                    || !available.is_multiple_of(stride)
                    || values.len() != available / stride
                {
                    return trap("mutable slice recast write has the wrong length");
                }
                for (index, value) in values.into_iter().enumerate() {
                    let offset = stride
                        .checked_mul(index)
                        .and_then(|delta| base_offset.checked_add(delta))
                        .ok_or_else(|| Halt::Trap("mutable slice offset overflow".to_owned()))?;
                    self.write_record_view_type(
                        *element_type,
                        cells,
                        offset,
                        value.borrow().clone(),
                    )?;
                }
                Ok(())
            }
            TypeReferenceNode::Named { name, .. } => {
                self.write_record_view(name.as_str(), cells, base_offset, value)
            }
            _ => trap("mutable record projection is not a fixed-layout value"),
        }
    }

    fn record_view_named_type(&self, mut type_reference: TypeReferenceHandle) -> Option<&str> {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(type_reference)
            {
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::Reference {
                    referee: base_type, ..
                } => type_reference = *base_type,
                TypeReferenceNode::Named { name, .. } => return Some(name.as_str()),
                _ => return None,
            }
        }
    }

    pub(super) fn eval_recast(
        &self,
        value: Value,
        target: Option<PrimitiveType>,
    ) -> EvalResult<Value> {
        let Some(target) = target else {
            // Unreachable post-validation (targets are scalar primitives).
            return Ok(value);
        };
        // Look through a reference-valued source (a recast of a `&T`-typed
        // local re-views the pointee's bytes).
        let value = match value {
            Value::Ref(cell) => cell.borrow().clone(),
            other => other,
        };
        match target {
            PrimitiveType::F32 => {
                let bits = match value {
                    Value::Float(f) => interpreter_f32_to_bits(f),
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("recast to f32 of non-scalar".to_owned()))?
                        as u32,
                };
                Ok(Value::Float(interpreter_f32_from_bits(bits)))
            }
            PrimitiveType::F64 => {
                let bits = match value {
                    Value::Float(f) => f.to_bits(),
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("recast to f64 of non-scalar".to_owned()))?
                        as u64,
                };
                Ok(Value::Float(f64::from_bits(bits)))
            }
            integer => {
                let raw: i64 = match value {
                    // A float source's width equals the target's (validated),
                    // so 4-byte targets take the f32 bit pattern, 8-byte the
                    // f64's.
                    Value::Float(f) => match integer.scalar_byte_size() {
                        Some(4) => interpreter_f32_to_bits(f) as i64,
                        _ => f.to_bits() as i64,
                    },
                    other => other
                        .as_int()
                        .ok_or_else(|| Halt::Trap("recast to integer of non-scalar".to_owned()))?,
                };
                // Equal-width int<->int reinterpretation is exactly the
                // width-wrap (`u32` 0xFFFF_FFFF re-viewed as `i32` = -1).
                Ok(Value::Int(wrap_to_width(raw, integer)))
            }
        }
    }
}
