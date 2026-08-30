use super::*;

impl<'program> Evaluator<'program> {
    pub(super) fn field_cell(&self, container: &Cell, field: &str) -> EvalResult<Cell> {
        let container = self.deref_cell(container.clone());
        let borrowed = container.borrow();
        match &*borrowed {
            Value::Struct {
                fields, type_name, ..
            } => fields
                .get(field)
                .cloned()
                .ok_or_else(|| Halt::Trap(format!("no field `{field}` on `{type_name}`"))),
            // A case value's payload field (`subject.text` after a case-pattern
            // binding rewrote `text`). The cell is shared, preserving aliasing.
            Value::Enum {
                variant_name,
                payload,
                ..
            } => payload
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, cell)| cell.clone())
                .ok_or_else(|| {
                    Halt::Trap(format!(
                        "case `{variant_name}` carries no payload field `{field}`"
                    ))
                }),
            // `slice.len` / `array.len` in member form produces a fresh length.
            Value::Array(elements) if field == "len" => {
                self.allocate_cell(Value::Int(elements.len() as i64))
            }
            // A text literal flowing into a `&[u8] in Utf8` parameter observes
            // its UTF-8 byte count, matching the native `<literal>.len` fold.
            Value::Str(text) if field == "len" => {
                self.allocate_cell(Value::Int(text.borrow().len() as i64))
            }
            other => trap(format!("cannot read field `{field}` of {other:?}")),
        }
    }

    pub(super) fn eval_name(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Value> {
        // The boolean keywords `true`/`false` can arrive as single-member name paths in
        // value/transition position (the parser does not always fold them to a literal).
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        if members.len() == 1 {
            match members[0].as_str() {
                "true" => return Ok(Value::Bool(true)),
                "false" => return Ok(Value::Bool(false)),
                _ => {}
            }
        }
        // An enum value reference (`CellId::R02` / `Command::Look`) resolves to an Enum.
        if let Some(enum_value) = self.enum_value_from_path(members)? {
            return Ok(enum_value);
        }
        let cell = self.resolve_name_place(path, frame)?;
        let value = cell.borrow().clone();
        // A `Ref` read through a name dereferences transparently (param of `&mut T`).
        if let Value::Ref(inner) = value {
            if members.len() == 1
                && let Some(recast) = frame
                    .mutable_scalar_recasts
                    .borrow()
                    .get(members[0].as_str())
                    .cloned()
            {
                return match recast {
                    MutableScalarRecast::Direct { target, .. } => {
                        self.eval_recast(inner.borrow().clone(), Some(target))
                    }
                    MutableScalarRecast::ByteRegion {
                        cells,
                        offset,
                        target,
                    } => self.assemble_scalar_byte_region(&cells, offset, target),
                    MutableScalarRecast::AggregateByteRegion {
                        cells,
                        offset,
                        target_type,
                    } => self
                        .assemble_record_view_type(target_type, &cells, offset)?
                        .ok_or_else(|| {
                            Halt::Trap(format!(
                                "cannot assemble mutable aggregate recast `{}`",
                                self.program.display_type_reference(target_type)
                            ))
                        }),
                    MutableScalarRecast::AggregateTyped {
                        source,
                        source_type,
                        target_type,
                    } => {
                        let cells = self.snapshot_typed_value_bytes(&source, source_type)?;
                        self.assemble_record_view_type(target_type, &cells, 0)?
                            .ok_or_else(|| {
                                Halt::Trap(format!(
                                    "cannot assemble typed mutable aggregate recast `{}`",
                                    self.program.display_type_reference(target_type)
                                ))
                            })
                    }
                };
            }
            return Ok(inner.borrow().clone());
        }
        Ok(value)
    }

    /// Recognize the parser's `Mutable(Cast(RecastMutable))` initializer and
    /// retain either one equal-width scalar cell or the complete indexed byte
    /// region behind the stated scalar view.
    pub(super) fn mutable_scalar_recast_initializer(
        &mut self,
        initializer: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Option<(Cell, MutableScalarRecast)>> {
        let cast_handle = match self.program.expression_table.expression(initializer) {
            ExpressionNode::Borrow(inner) => inner.target,
            ExpressionNode::Cast(cast)
                if cast.form == psi_language_core::CastForm::RecastMutable =>
            {
                initializer
            }
            _ => return Ok(None),
        };
        let ExpressionNode::Cast(cast) = self.program.expression_table.expression(cast_handle)
        else {
            return Ok(None);
        };
        if cast.form != psi_language_core::CastForm::RecastMutable {
            return Ok(None);
        }
        let target = self.cast_target_primitive(cast.target_type);
        // A forwarded mutable view can acquire another same-target recast at
        // the parameter boundary. Semantically that is still one view over the
        // original storage, so peel the recast-only chain before resolving the
        // source place. Treating the inner Cast as a place is unsupported and,
        // more importantly, would lose the original alias identity.
        let mut source_handle = cast.value;
        loop {
            match self.program.expression_table.expression(source_handle) {
                ExpressionNode::Cast(inner) if inner.form.is_recast() => {
                    source_handle = inner.value;
                }
                ExpressionNode::Borrow(inner)
                    if matches!(
                        self.program.expression_table.expression(inner.target),
                        ExpressionNode::Cast(cast) if cast.form.is_recast()
                    ) =>
                {
                    source_handle = inner.target;
                }
                _ => break,
            }
        }
        if let ExpressionNode::Indexed(indexed) = self
            .program
            .expression_table
            .expression(source_handle)
            .clone()
        {
            let offset = self.eval_index(indexed.index, frame)?;
            if let Value::Array(cells) = self.eval_expression(indexed.collection, frame)? {
                let source_cell = cells.get(offset).cloned().ok_or_else(|| {
                    Halt::Trap(format!(
                        "mutable interior recast starts at byte {offset} past the region"
                    ))
                })?;
                let recast = match target {
                    Some(target) => MutableScalarRecast::ByteRegion {
                        cells,
                        offset,
                        target,
                    },
                    None => MutableScalarRecast::AggregateByteRegion {
                        cells,
                        offset,
                        target_type: cast.target_type,
                    },
                };
                return Ok(Some((source_cell, recast)));
            }
        }
        if target.is_none() {
            let source = self.resolve_place(source_handle, frame)?;
            let source = self.deref_cell(source);
            let source_type = self.expression_type_reference(source_handle, frame);
            let source_layout = source_type.and_then(|source_type| {
                self.record_view_type_layout(source_type, &mut HashSet::new())
            });
            if let Some(source_type) = source_type
                && source_layout.is_some()
            {
                return Ok(Some((
                    source.clone(),
                    MutableScalarRecast::AggregateTyped {
                        source,
                        source_type,
                        target_type: cast.target_type,
                    },
                )));
            }
        }
        let Some(target) = target else {
            return Ok(None);
        };
        let Some((source, _)) = self.expression_scalar_type(source_handle, frame) else {
            return Ok(None);
        };
        let source_cell = self.resolve_place(source_handle, frame)?;
        Ok(Some((
            self.deref_cell(source_cell),
            MutableScalarRecast::Direct { source, target },
        )))
    }

    pub(super) fn mutable_scalar_recast_target(
        &self,
        target: ExpressionHandle,
        frame: &Frame,
    ) -> Option<MutableScalarRecast> {
        let target = match self.program.expression_table.expression(target) {
            ExpressionNode::Borrow(inner) => inner.target,
            _ => target,
        };
        let ExpressionNode::Name(path) = self.program.expression_table.expression(target) else {
            return None;
        };
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        let [name] = members else {
            return None;
        };
        frame
            .mutable_scalar_recasts
            .borrow()
            .get(name.as_str())
            .cloned()
    }

    /// Recover a mutable recast local and any record-field path projected from
    /// it. Typed expressions use both `Name([view, field])` and nested Member
    /// nodes, so normalize both spellings here.
    pub(super) fn mutable_recast_path(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Option<(MutableScalarRecast, Vec<MutableRecordProjectionStep>)>> {
        match self.program.expression_table.expression(handle).clone() {
            ExpressionNode::Borrow(inner) => {
                if let Some((_, recast)) = self.mutable_scalar_recast_initializer(handle, frame)? {
                    return Ok(Some((recast, Vec::new())));
                }
                self.mutable_recast_path(inner.target, frame)
            }
            ExpressionNode::Cast(cast) if cast.form.is_recast() => Ok(self
                .mutable_scalar_recast_initializer(handle, frame)?
                .map(|(_, recast)| (recast, Vec::new()))),
            ExpressionNode::Name(path) => {
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
                let Some(root) = members.first() else {
                    return Ok(None);
                };
                let recast = frame
                    .mutable_scalar_recasts
                    .borrow()
                    .get(root.as_str())
                    .cloned();
                Ok(recast.map(|recast| {
                    (
                        recast,
                        members[1..]
                            .iter()
                            .map(|member| {
                                MutableRecordProjectionStep::Field(member.as_str().to_owned())
                            })
                            .collect(),
                    )
                }))
            }
            ExpressionNode::Member(member) => {
                let Some((recast, mut path)) = self.mutable_recast_path(member.receiver, frame)?
                else {
                    return Ok(None);
                };
                path.push(MutableRecordProjectionStep::Field(
                    member.member.as_str().to_owned(),
                ));
                Ok(Some((recast, path)))
            }
            ExpressionNode::Indexed(indexed) => {
                let Some((recast, mut path)) =
                    self.mutable_recast_path(indexed.collection, frame)?
                else {
                    return Ok(None);
                };
                path.push(MutableRecordProjectionStep::Index(indexed.index));
                Ok(Some((recast, path)))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn read_mutable_record_recast_target(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Option<Value>> {
        let Some((recast, path)) = self.mutable_recast_path(handle, frame)? else {
            return Ok(None);
        };
        let (cells, offset, target_type) = match recast {
            MutableScalarRecast::AggregateByteRegion {
                cells,
                offset,
                target_type,
            } => (cells, offset, target_type),
            MutableScalarRecast::AggregateTyped {
                source,
                source_type,
                target_type,
            } => (
                self.snapshot_typed_value_bytes(&source, source_type)?,
                0,
                target_type,
            ),
            _ => return Ok(None),
        };
        if path.is_empty() {
            return self.assemble_record_view_type(target_type, &cells, offset);
        }
        if matches!(
            path.as_slice(),
            [MutableRecordProjectionStep::Field(field)] if field == "len"
        ) && self.declared_type_is_slice(target_type)
            && let Some(element_type) = self.collection_element_type(target_type)
            && let Some((stride, _)) =
                self.record_view_type_layout(element_type, &mut HashSet::new())
        {
            let remaining = cells.len().saturating_sub(offset);
            if stride == 0 || remaining % stride != 0 {
                return trap("slice recast bytes do not tile the element layout");
            }
            return Ok(Some(Value::Int((remaining / stride) as i64)));
        }
        let Some(projection) =
            self.record_view_type_projection(target_type, &path, offset, cells.len(), frame, None)?
        else {
            return trap(format!(
                "cannot project `{}` through mutable aggregate recast `{}`",
                Self::mutable_record_projection_display(&path),
                self.program.display_type_reference(target_type),
            ));
        };
        if let Some(integer) = projection.stored_integer {
            let primitive = self
                .program
                .primitive_type_reference(projection.type_reference)
                .ok_or_else(|| Halt::Trap("stored-integer projection is not scalar".to_owned()))?;
            return self
                .assemble_stored_integer_byte_region(
                    &cells,
                    projection.offset,
                    primitive,
                    integer.stored_width_bits,
                    integer.interpretation,
                )
                .map(Some);
        }
        self.assemble_record_view_type(projection.type_reference, &cells, projection.offset)
    }

    pub(super) fn write_mutable_record_recast_target(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
        value: Value,
    ) -> EvalResult<bool> {
        let Some((recast, path)) = self.mutable_recast_path(handle, frame)? else {
            return Ok(false);
        };
        let (cells, offset, target_type, typed_source) = match recast {
            MutableScalarRecast::AggregateByteRegion {
                cells,
                offset,
                target_type,
            } => (cells, offset, target_type, None),
            MutableScalarRecast::AggregateTyped {
                source,
                source_type,
                target_type,
            } => (
                self.snapshot_typed_value_bytes(&source, source_type)?,
                0,
                target_type,
                Some((source, source_type)),
            ),
            _ => return Ok(false),
        };
        if path.is_empty() {
            self.write_record_view_type(target_type, &cells, offset, value)?;
            if let Some((source, source_type)) = typed_source {
                self.commit_typed_value_bytes(&source, source_type, &cells)?;
            }
            return Ok(true);
        }
        let Some(projection) =
            self.record_view_type_projection(target_type, &path, offset, cells.len(), frame, None)?
        else {
            return trap(format!(
                "cannot project `{}` through mutable aggregate recast `{}`",
                Self::mutable_record_projection_display(&path),
                self.program.display_type_reference(target_type),
            ));
        };
        if let Some(integer) = projection.stored_integer {
            self.write_stored_integer_byte_region(
                &cells,
                projection.offset,
                projection.type_reference,
                integer.stored_width_bits,
                integer.interpretation,
                value,
            )?;
        } else {
            self.write_record_view_type(
                projection.type_reference,
                &cells,
                projection.offset,
                value,
            )?;
        }
        if let Some((source, source_type)) = typed_source {
            self.commit_typed_value_bytes(&source, source_type, &cells)?;
        }
        Ok(true)
    }

    fn mutable_record_projection_display(path: &[MutableRecordProjectionStep]) -> String {
        let mut rendered = String::new();
        for step in path {
            match step {
                MutableRecordProjectionStep::Field(field) => {
                    if !rendered.is_empty() {
                        rendered.push('.');
                    }
                    rendered.push_str(field);
                }
                MutableRecordProjectionStep::Index(_) => rendered.push_str("[..]"),
            }
        }
        rendered
    }

    /// `Type::Variant` paths whose head is an enum/data symbol with a matching variant.
    fn enum_value_from_path(
        &self,
        members: &[psi_typed_trees::name::Identifier],
    ) -> EvalResult<Option<Value>> {
        if members.len() != 2 {
            return Ok(None);
        }
        let type_name = members[0].as_str();
        let variant_name = members[1].as_str();
        let Some(data) = self.find_data_by_name(type_name) else {
            return Ok(None);
        };
        let is_variant = self.program.data_members(data).iter().any(|member| {
            matches!(member, DataMember::Variant(variant) if variant.name.as_str() == variant_name)
        });
        if !is_variant {
            return Ok(None);
        }
        let common: Vec<(String, Cell)> = self
            .program
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                _ => None,
            })
            .map(|field| {
                Ok((
                    field.name.as_str().to_owned(),
                    self.allocate_cell(self.default_for_type(field.type_reference)?)?,
                ))
            })
            .collect::<EvalResult<_>>()?;
        Ok(Some(
            // MIXED shapes: a bare payload-less case still carries the COMMON
            // fields, zero-initialized (scalar-only by validation, so the
            // primitive default is the zero value). Pure sums add nothing.
            Value::Enum {
                type_symbol: data.symbol,
                variant_name: variant_name.to_owned(),
                payload: common,
            },
        ))
    }

    // ---- place resolution ---------------------------------------------------

    /// Resolve an lvalue expression to its storage cell (for assignment / `&mut`).
    pub(super) fn resolve_place(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<Cell> {
        match self.program.expression_table.expression(handle).clone() {
            ExpressionNode::Name(path) => self.resolve_name_place(&path, frame),
            ExpressionNode::Member(member) => {
                let receiver = self.resolve_place(member.receiver, frame)?;
                self.field_cell(&receiver, member.member.as_str())
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = self.resolve_place(indexed.collection, frame)?;
                let index = self.eval_index(indexed.index, frame)?;
                self.element_cell(&collection, index)
            }
            ExpressionNode::Borrow(inner) => self.resolve_place(inner.target, frame),
            other => unsupported(format!("place expression not supported: {other:?}")),
        }
    }

    fn resolve_name_place(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Cell> {
        let members = self
            .program
            .expression_table
            .name_path_members(path.members)
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect::<Vec<_>>();
        if members.is_empty() {
            return trap("empty name path");
        }

        // Head: `self`, a local, or a self-field (implicit self).
        let head = members[0].as_str();
        let mut cell = if head == "self" {
            frame.self_cell.clone()
        } else if let Some(local) = frame.get(head) {
            local
        } else {
            // Implicit self-field: `n` means `self.n`.
            self.field_cell(&frame.self_cell, head)?
        };

        // Walk the remaining members as field accesses, dereferencing refs along the way.
        for member in &members[1..] {
            cell = self.deref_cell(cell);
            cell = self.field_cell(&cell, member)?;
        }
        Ok(cell)
    }

    /// True when a declared type is an owned fixed array `[T; N]` -- seeing THROUGH a domain
    /// `Constrained` wrapper (`[i32; N] in Wrapping`) -- as opposed to a slice `&[T]`. Drives
    /// the value-copy gate: a whole-array assignment/`let` into a FixedArray place is a deep
    /// copy, while a slice is a shared view that must NOT be deep-cloned.
    pub(super) fn declared_type_is_fixed_array(
        &self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            psi_typed_trees::types::TypeReferenceNode::FixedArray { .. } => true,
            psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.declared_type_is_fixed_array(*base_type)
            }
            _ => false,
        }
    }

    /// True for the owned variable-fill text carrier `[u8; N] in <domain>`.
    /// This mirrors `omega-layout`'s `BoundedByteBuffer` classification rather
    /// than treating the carrier as an always-full `[u8; N]` array.
    pub(super) fn declared_type_is_bounded_byte_buffer(
        &self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        let psi_typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = self
            .program
            .type_reference_table
            .type_reference(type_reference)
        else {
            return false;
        };
        let has_value_domain = self
            .program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| match constraint {
                psi_typed_trees::types::TypeConstraintNode::Domain(name) => {
                    !psi_typed_trees::wire::is_layout_domain_constraint(name)
                        && psi_language_semantics::CarryPermission::from_name(name.as_str())
                            .is_none()
                }
                _ => false,
            });
        if !has_value_domain {
            return false;
        }
        let psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
            self.program.type_reference_table.type_reference(*base_type)
        else {
            return false;
        };
        self.program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    }

    /// The ELEMENT type of an owned fixed array `[T; N]` -- seeing THROUGH a
    /// domain `Constrained` wrapper (`[u8; N] in Wrapping`). `None` for a slice,
    /// scalar, or invalid reference. Used to wrap an array-element store to the
    /// element's width/domain (the field-store truncation, for `arr[i] = v`).
    fn fixed_array_element_type(
        &self,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
        if !type_reference.is_valid() {
            return None;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            psi_typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                Some(*element_type)
            }
            psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.fixed_array_element_type(*base_type)
            }
            _ => None,
        }
    }

    /// The (primitive, arithmetic-domain) an assignment target coerces its stored
    /// SCALAR to -- the decision-17 truncation/clamp/trap the interpreter applies
    /// on a write to match the native store. For a FIELD/local place it is the
    /// declared type's own primitive + domain. For an ARRAY ELEMENT `arr[i]` it
    /// is the element's PRIMITIVE with the ARRAY's DOMAIN (`[u8;N] in Saturating`
    /// clamps its elements). `None` for a non-scalar / unresolved target, which
    /// is then left un-coerced.
    pub(super) fn assignment_target_coercion(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> Option<(psi_typed_trees::types::PrimitiveType, ArithmeticDomain)> {
        if let ExpressionNode::Indexed(indexed) =
            self.program.expression_table.expression(handle).clone()
        {
            let array_type = self.assignment_target_type_reference(indexed.collection, frame)?;
            let element_type = self.fixed_array_element_type(array_type)?;
            let primitive = self.program.primitive_type_reference(element_type)?;
            // The arithmetic domain lives on the ARRAY (`[T;N] in D`), not the
            // bare element type, so read it from the array reference.
            let domain = self
                .program
                .arithmetic_domain_for_type_reference(array_type);
            return Some((primitive, domain));
        }
        let type_reference = self.assignment_target_type_reference(handle, frame)?;
        let primitive = self.program.primitive_type_reference(type_reference)?;
        let domain = self
            .program
            .arithmetic_domain_for_type_reference(type_reference);
        Some((primitive, domain))
    }

    /// The CORE value-landing coercion: coerce a stored SCALAR to an already-
    /// resolved (primitive, arithmetic-domain), matching the native store into
    /// that typed slot -- the decision-17 truncate/clamp/trap for an integer, f32
    /// rounding for a float. A non-scalar value (Struct, Array, Ref, ...) passes
    /// through unchanged. Every interpreter value-landing seam funnels here.
    pub(super) fn coerce_scalar_with(
        &self,
        value: Value,
        primitive: psi_typed_trees::types::PrimitiveType,
        domain: ArithmeticDomain,
    ) -> EvalResult<Value> {
        match &value {
            Value::Int(raw) => Ok(Value::Int(apply_arithmetic_domain(
                *raw, primitive, domain,
            )?)),
            Value::Float(f) if primitive == PrimitiveType::F32 => {
                if f.is_nan() {
                    return Ok(Value::Float(interpreter_f32_from_bits(
                        interpreter_f32_to_bits(*f),
                    )));
                }
                let meaning = FloatMeaning::from_f64(*f);
                Ok(Value::Float(
                    FloatSemantics::convert(SemanticFloatFormat::BINARY32, &meaning)
                        .to_interpreter_value(SemanticFloatFormat::BINARY32),
                ))
            }
            _ => Ok(value),
        }
    }

    /// Coerce a stored SCALAR to a declared TYPE reference (resolves its primitive
    /// + domain, then [`coerce_scalar_with`]). A non-primitive type passes through.
    /// Used where a value lands in a typed slot with the type in hand: struct/case
    /// literal FIELD init + the LocalData store (the type carries its own domain).
    pub(super) fn coerce_scalar_value(
        &self,
        value: Value,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) -> EvalResult<Value> {
        match self.program.primitive_type_reference(type_reference) {
            Some(primitive) => {
                let domain = self
                    .program
                    .arithmetic_domain_for_type_reference(type_reference);
                self.coerce_scalar_with(value, primitive, domain)
            }
            None => Ok(value),
        }
    }

    /// Declared integer primitive of an assignment target, when it is a FIELD whose
    /// receiver resolves to a typed struct (`self.c`, `obj.field`, or the equivalent
    /// name path). Used to wrap an assigned integer to the field's declared width,
    /// matching the native backend's truncating store. Returns `None` for bare locals
    /// (whose cells carry no declared type) and non-field places.
    pub(super) fn assignment_target_type_reference(
        &mut self,
        handle: ExpressionHandle,
        frame: &Frame,
    ) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
        let (receiver, field_name) = match self.program.expression_table.expression(handle).clone()
        {
            ExpressionNode::Member(member) => {
                let receiver = self.resolve_place(member.receiver, frame).ok()?;
                (receiver, member.member.as_str().to_owned())
            }
            ExpressionNode::Name(path) => {
                let names = self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect::<Vec<_>>();
                match names.as_slice() {
                    [] => return None,
                    [single] => {
                        // A single name is either a local (no declared-type record
                        // here) or an implicit self-field.
                        if single == "self" || frame.get(single).is_some() {
                            return None;
                        }
                        (frame.self_cell.clone(), single.clone())
                    }
                    [head, middle @ .., last] => {
                        let mut cell = if head == "self" {
                            frame.self_cell.clone()
                        } else if let Some(local) = frame.get(head) {
                            local
                        } else {
                            self.field_cell(&frame.self_cell, head).ok()?
                        };
                        for member in middle {
                            cell = self.deref_cell(cell);
                            cell = self.field_cell(&cell, member).ok()?;
                        }
                        (cell, last.clone())
                    }
                }
            }
            _ => return None,
        };
        let receiver = self.deref_cell(receiver);
        let type_symbol = match &*receiver.borrow() {
            Value::Struct { type_symbol, .. } => *type_symbol,
            _ => return None,
        };
        self.field_type_reference(type_symbol, &field_name)
    }

    /// Declared type reference of `field_name` on the data record or machine
    /// identified by `type_symbol`. A machine instance's struct carries the
    /// MACHINE's symbol while its fields come from the attached data (plus the
    /// machine-owned cells), so both field sources are searched. The caller
    /// derives the primitive type and arithmetic domain from the reference.
    pub(super) fn field_type_reference(
        &self,
        type_symbol: SymbolHandle,
        field_name: &str,
    ) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
        if let Some(data) = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == type_symbol)
        {
            return self.data_field_type_reference(data, field_name);
        }
        if let Some(machine) = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == type_symbol)
        {
            if let Some(data) = machine
                .attached_data
                .as_ref()
                .and_then(|name| self.find_data_by_name(name.as_str()))
                && let Some(type_reference) = self.data_field_type_reference(data, field_name)
            {
                return Some(type_reference);
            }
            for owned in self.program.machine_owned_data(machine) {
                if owned.name.as_str() == field_name {
                    return Some(owned.type_reference);
                }
            }
        }
        None
    }

    fn data_field_type_reference(
        &self,
        data: &DataDefinition,
        field_name: &str,
    ) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
        for member in self.program.data_members(data) {
            if let DataMember::Field(field) = member
                && field.name.as_str() == field_name
            {
                return Some(field.type_reference);
            }
        }
        None
    }

    /// If a cell holds a `Ref`, return the referenced cell (so field access on a `&mut`
    /// parameter reaches the aliased place). Otherwise the cell itself.
    pub(super) fn deref_cell(&self, cell: Cell) -> Cell {
        let inner = match &*cell.borrow() {
            Value::Ref(target) => Some(target.clone()),
            _ => None,
        };
        inner.unwrap_or(cell)
    }

    /// Evaluate an index expression to a `usize` element index.
    pub(super) fn eval_index(
        &mut self,
        index: ExpressionHandle,
        frame: &Frame,
    ) -> EvalResult<usize> {
        let value = self.eval_expression(index, frame)?;
        let raw = value
            .as_int()
            .ok_or_else(|| Halt::Trap("array index is not an integer".to_owned()))?;
        usize::try_from(raw).map_err(|_| Halt::Trap(format!("array index {raw} out of range")))
    }

    /// Resolve one element CELL of an `Array` place (sharing the same allocation, so a write
    /// through the returned cell aliases the array element).
    pub(super) fn element_cell(&self, container: &Cell, index: usize) -> EvalResult<Cell> {
        let container = self.deref_cell(container.clone());
        let borrowed = container.borrow();
        match &*borrowed {
            Value::Array(elements) => elements
                .get(index)
                .cloned()
                .ok_or_else(|| Halt::Trap(format!("array index {index} out of bounds"))),
            other => trap(format!("cannot index {other:?}")),
        }
    }

    /// Evaluate a subslice `collection[start..end]` into an `Array` view that SHARES the
    /// collection's element cells (so writes through the subslice alias the original). A
    /// missing start defaults to 0; a missing end to the length; `end_inclusive` extends by
    /// one.
    pub(super) fn eval_subslice(
        &mut self,
        collection: ExpressionHandle,
        range: &psi_typed_trees::expression::TableRangeExpression,
        frame: &Frame,
    ) -> EvalResult<Value> {
        // A nested subslice base (`sub[1..][1..]`) is not a place — the inner
        // range-indexed expression produces a VIEW value. Evaluate it as a value
        // (recursing through this function) and slice the resulting window;
        // element cells stay shared, matching the fat-descriptor model where a
        // subslice only offsets the pointer.
        let nested_view = if let ExpressionNode::Indexed(inner) =
            self.program.expression_table.expression(collection).clone()
            && matches!(
                self.program.expression_table.expression(inner.index),
                ExpressionNode::Range(_)
            ) {
            Some(self.eval_expression(collection, frame)?)
        } else {
            None
        };
        let elements = match nested_view {
            Some(Value::Array(elements)) => elements,
            Some(other) => return trap(format!("cannot subslice {other:?}")),
            None => {
                let collection_cell = self.resolve_place(collection, frame)?;
                match &*self.deref_cell(collection_cell).borrow() {
                    Value::Array(elements) => elements.clone(),
                    // A Str-backed slice (a `&[u8] in Path` bound to a string
                    // literal) subslices into a byte view: expose each byte as an
                    // Int cell so the shared range logic + the `Array` host-arg arm
                    // (eval_fs_bytes) handle `path[a..b]` uniformly.
                    Value::Str(text) => text
                        .borrow()
                        .iter()
                        .map(|byte| self.allocate_cell(Value::Int(i64::from(*byte))))
                        .collect::<EvalResult<Vec<_>>>()?,
                    other => return trap(format!("cannot subslice {other:?}")),
                }
            }
        };
        let len = elements.len();
        let start = if range.start.is_valid() {
            self.eval_index(range.start, frame)?
        } else {
            0
        };
        let mut end = if range.end.is_valid() {
            self.eval_index(range.end, frame)?
        } else {
            len
        };
        if range.end_inclusive {
            end = end.saturating_add(1);
        }
        let end = end.min(len);
        let start = start.min(end);
        Ok(Value::Array(elements[start..end].to_vec()))
    }
}
