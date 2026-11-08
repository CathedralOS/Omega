/*
    Program lookup, type metadata, record and byte views, array windows, and
    place resolution for the checked-tree interpreter.

    We resolve authored names and projections to their underlying cells or
    encoded reference views without advancing control-flow state.
*/

pub(crate) mod array_windows {
    use crate::interpreter::evaluator::{
        Cell, EvalResult, Evaluator, ExpressionHandle, Frame, Halt, TypeReferenceHandle, Value,
        trap, unsupported,
    };
    impl Evaluator<'_> {
        pub(crate) fn capture_array_window(
            &mut self,
            target: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Option<(Cell, usize, usize, TypeReferenceHandle)>> {
            let Some(indexed) = self.program.expression_table.indexed_node(target) else {
                return Ok(None);
            };
            let Some(range) = self.program.expression_table.range_node(indexed.index) else {
                return Ok(None);
            };
            let collection = indexed.collection;
            let range = *range;
            let element_type = self
                .expression_type_reference(collection, frame)
                .and_then(|collection_type| self.collection_element_type(collection_type))
                .ok_or_else(|| {
                    Halt::Unsupported("array window lost its element destination".to_owned())
                })?;
            let target = self.resolve_place(collection, frame)?;
            let target = self.deref_cell(target);
            let target_length = match &*target.borrow() {
                Value::Array(elements) => elements.len(),
                Value::Str(text) => text.borrow().len(),
                _ => return unsupported("array window requires array or byte-carrier storage"),
            };
            let start = if range.start.is_valid() {
                self.eval_index(range.start, frame)?
            } else {
                0
            };
            let end = if range.end.is_valid() {
                self.eval_index(range.end, frame)?
            } else {
                target_length
            };
            let end = if range.end_inclusive {
                end.checked_add(1)
                    .ok_or_else(|| Halt::Trap("inclusive array window bound overflow".to_owned()))?
            } else {
                end
            };
            if start > end || end > target_length {
                return trap("array window replacement is out of bounds");
            }
            Ok(Some((target, start, end, element_type)))
        }

        pub(crate) fn write_array_window(
            &self,
            target: Cell,
            start: usize,
            end: usize,
            value: Value,
        ) -> EvalResult<()> {
            let Value::Array(source) = value else {
                return unsupported("array window replacement is not an array value");
            };
            let replacements = source
                .iter()
                .map(|cell| cell.borrow().clone())
                .collect::<Vec<_>>();
            if replacements.len() != end - start {
                return trap("array window replacement has a different element count");
            }
            match &*target.borrow() {
                Value::Array(targets) => {
                    for (target, value) in targets[start..end].iter().zip(replacements) {
                        *target.borrow_mut() = value;
                    }
                }
                Value::Str(text) => {
                    let bytes = replacements
                        .into_iter()
                        .map(|value| {
                            value
                                .as_int()
                                .and_then(|integer| u8::try_from(integer).ok())
                                .ok_or_else(|| {
                                    Halt::Trap("byte window replacement is not a byte".to_owned())
                                })
                        })
                        .collect::<EvalResult<Vec<_>>>()?;
                    for (position, byte) in (start..end).zip(bytes) {
                        text.write_byte(position, byte).map_err(|_| {
                            Halt::Trap("byte window replacement is out of bounds".to_owned())
                        })?;
                    }
                }
                _ => return unsupported("array window lost its backing storage"),
            }
            Ok(())
        }
    }
}

pub(crate) mod names_recasts_and_places {
    use crate::interpreter::UnavailableStaticInput::Binding;
    use crate::interpreter::evaluator::{
        ArithmeticDomain, Cell, DataDefinition, DataMember, EvalResult, Evaluator,
        ExpressionHandle, ExpressionNode, FloatMeaning, FloatSemantics, Frame, Halt, HashSet,
        MutableRecordRecastWrite, PrimitiveType, Reference, ReferenceProjection,
        ReferenceProjectionStep, ReferenceStorage, SemanticFloatFormat, SymbolHandle,
        TableNamePath, TypeReferenceHandle, TypeReferenceNode, Value, apply_arithmetic_domain,
        expression_table, interpreter_f32_from_bits, interpreter_f32_to_bits, trap,
        type_reference_table, unsupported,
    };
    use language_semantics::const_value::boolean_literal_spelling;
    use language_semantics::declaration_selection::CollectionMeasure;
    use typed_trees::types::TypeReferenceNode::Constrained;
    use typed_trees::types::TypeReferenceNode::FixedArray;
    impl<'program> Evaluator<'program> {
        pub(crate) fn field_cell(&self, container: &Cell, field: &str) -> EvalResult<Cell> {
            let container = self.deref_cell(container.clone());
            let borrowed = container.borrow();
            match &*borrowed {
                Value::Struct {
                    fields, type_name, ..
                } => fields
                    .get(field)
                    .cloned()
                    .ok_or_else(|| Halt::Trap(format!("no field `{field}` on `{type_name}`"))),
                Value::Enum {
                    variant_name,
                    payload,
                    ..
                } => payload
                    .iter()
                    .find(|(name, _)| **name == *field)
                    .map(|(_, cell)| cell.clone())
                    .ok_or_else(|| {
                        Halt::Trap(format!(
                            "case `{variant_name}` carries no payload field `{field}`"
                        ))
                    }),
                Value::Array(elements)
                    if CollectionMeasure::from_authored_spelling(field)
                        == Some(CollectionMeasure::Length) =>
                {
                    self.allocate_cell(Value::Int(elements.len() as i64))
                }
                Value::Str(text)
                    if CollectionMeasure::from_authored_spelling(field)
                        == Some(CollectionMeasure::Length) =>
                {
                    self.allocate_cell(Value::Int(text.borrow().len() as i64))
                }
                other => trap(format!("cannot read field `{field}` of {other:?}")),
            }
        }

        pub(crate) fn eval_name(
            &mut self,
            path: &TableNamePath,
            frame: &Frame,
        ) -> EvalResult<Value> {
            let members = expression_table(self.program).name_path_members(path.members);
            if let [name] = members
                && let Some(value) = boolean_literal_spelling(name.as_str())
            {
                return Ok(Value::Bool(value));
            }
            if let Some(enum_value) = self.enum_value_from_path(path)? {
                return Ok(enum_value);
            }
            let cell = self.resolve_name_place(path, frame)?;
            let value = cell.borrow().clone();
            self.observe_reference_value(value)
        }

        pub(crate) fn read_reference(&self, reference: &Reference) -> EvalResult<Value> {
            match &reference.storage {
                None => Ok(reference.cell.borrow().clone()),
                Some(ReferenceStorage::Direct { target, .. }) => {
                    self.eval_recast(reference.cell.borrow().clone(), Some(*target))
                }
                Some(ReferenceStorage::ByteRegion {
                    cells,
                    offset,
                    target,
                }) => self.assemble_scalar_byte_region(cells, *offset, *target),
                Some(ReferenceStorage::AggregateByteRegion { cells, projection }) => {
                    self.read_record_view_projection(cells, *projection)
                }
                Some(ReferenceStorage::AggregateTyped {
                    source,
                    source_type,
                    projection,
                }) => {
                    let cells = self.snapshot_typed_value_bytes(source, *source_type)?;
                    self.read_record_view_projection(&cells, *projection)
                }
            }
        }

        pub(crate) fn observe_reference_value(&self, value: Value) -> EvalResult<Value> {
            match value {
                Value::Ref(reference) => self.read_reference(&reference),
                other => Ok(other),
            }
        }

        pub(crate) fn capture_reference_view(
            &mut self,
            initializer: ExpressionHandle,
            mut destination: TypeReferenceHandle,
            frame: &mut Frame,
        ) -> EvalResult<Option<(Cell, ReferenceStorage)>> {
            loop {
                match type_reference_table(self.program).type_reference(destination) {
                    TypeReferenceNode::Constrained { base_type, .. } => destination = *base_type,
                    TypeReferenceNode::Reference { .. } => break,
                    _ => return Ok(None),
                }
            }
            if let Some(reference) = self.reference_recast_initializer(initializer, frame)? {
                return Ok(Some(reference));
            }
            let Some((mut recast, path)) = self.reference_storage_path(initializer, frame)? else {
                return Ok(None);
            };
            if let Some(projection) = self.project_record_reference(&recast, &path, frame)? {
                let source = Self::retain_record_reference_projection(&mut recast, projection)?;
                return Ok(Some((source, recast)));
            }
            if !path.is_empty() {
                return trap("scalar reference view has a nonempty structural projection");
            }
            let source = self.resolve_place(initializer, frame)?;
            Ok(Some((self.deref_cell(source), recast)))
        }

        pub(crate) fn reference_recast_initializer(
            &mut self,
            initializer: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Option<(Cell, ReferenceStorage)>> {
            let cast_handle = match self.program.expression_table.expression(initializer) {
                ExpressionNode::Borrow(inner) => inner.target,
                ExpressionNode::Cast(cast) if cast.form.is_recast() => initializer,
                _ => return Ok(None),
            };
            let Some(cast) = self.program.expression_table.cast_node(cast_handle) else {
                return Ok(None);
            };
            if !cast.form.is_recast() {
                return Ok(None);
            }
            let target = self.cast_target_primitive(cast.target_type);
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
            if let ExpressionNode::Indexed(indexed) = expression_table(self.program)
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
                        Some(target) => ReferenceStorage::ByteRegion {
                            cells,
                            offset,
                            target,
                        },
                        None => ReferenceStorage::AggregateByteRegion {
                            cells,
                            projection: ReferenceProjection {
                                offset,
                                type_reference: cast.target_type,
                                stored_integer: None,
                            },
                        },
                    };
                    return Ok(Some((source_cell, recast)));
                }
            }
            if target.is_none() {
                if let Some((mut storage, path)) =
                    self.reference_storage_path(source_handle, frame)?
                    && let Some(mut projection) =
                        self.project_record_reference(&storage, &path, frame)?
                {
                    projection.type_reference = cast.target_type;
                    projection.stored_integer = None;
                    let source =
                        Self::retain_record_reference_projection(&mut storage, projection)?;
                    return Ok(Some((source, storage)));
                }
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
                        ReferenceStorage::AggregateTyped {
                            source,
                            source_type,
                            projection: ReferenceProjection {
                                offset: 0,
                                type_reference: cast.target_type,
                                stored_integer: None,
                            },
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
                ReferenceStorage::Direct { source, target },
            )))
        }

        pub(crate) fn reference_storage_path(
            &mut self,
            handle: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Option<(ReferenceStorage, Vec<ReferenceProjectionStep>)>> {
            match self.program.expression_table.expression(handle).clone() {
                ExpressionNode::Borrow(inner) => {
                    if let Some((_, recast)) = self.reference_recast_initializer(handle, frame)? {
                        return Ok(Some((recast, Vec::new())));
                    }
                    self.reference_storage_path(inner.target, frame)
                }
                ExpressionNode::Cast(cast) if cast.form.is_recast() => Ok(self
                    .reference_recast_initializer(handle, frame)?
                    .map(|(_, recast)| (recast, Vec::new()))),
                ExpressionNode::Name(path) => {
                    let members = expression_table(self.program).name_path_members(path.members);
                    if members.is_empty() {
                        return Ok(None);
                    }
                    let Some(mut cell) = frame.local_cell(path.head_symbol) else {
                        return Ok(None);
                    };
                    for position in 1..=members.len() {
                        let storage = match &*cell.borrow() {
                            Value::Ref(reference) => reference.storage.clone(),
                            _ => None,
                        };
                        if let Some(storage) = storage {
                            return Ok(Some((
                                storage,
                                members[position..]
                                    .iter()
                                    .map(|member| {
                                        ReferenceProjectionStep::Field(member.as_str().to_owned())
                                    })
                                    .collect(),
                            )));
                        }
                        if let Some(member) = members.get(position) {
                            cell = self.field_cell(&cell, member.as_str())?;
                        }
                    }
                    Ok(None)
                }
                ExpressionNode::Member(member) => {
                    let Some((recast, mut path)) =
                        self.reference_storage_path(member.receiver, frame)?
                    else {
                        return Ok(None);
                    };
                    path.push(ReferenceProjectionStep::Field(
                        member.member.as_str().to_owned(),
                    ));
                    Ok(Some((recast, path)))
                }
                ExpressionNode::Indexed(indexed) => {
                    let Some((recast, mut path)) =
                        self.reference_storage_path(indexed.collection, frame)?
                    else {
                        return Ok(None);
                    };
                    path.push(ReferenceProjectionStep::Index(indexed.index));
                    Ok(Some((recast, path)))
                }
                _ => Ok(None),
            }
        }

        pub(crate) fn read_mutable_record_recast_target(
            &mut self,
            handle: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Option<Value>> {
            let Some((recast, path)) = self.reference_storage_path(handle, frame)? else {
                return Ok(None);
            };
            if matches!(path.as_slice(), [ReferenceProjectionStep::Field(field)]
            if CollectionMeasure::from_authored_spelling(field) == Some(CollectionMeasure::Length))
                && let Some((projection, region_length)) =
                    self.record_reference_geometry(&recast)?
                && self.declared_type_is_slice(projection.type_reference)
                && let Some(element_type) = self.collection_element_type(projection.type_reference)
                && let Some((stride, _)) =
                    self.record_view_type_layout(element_type, &mut HashSet::new())
            {
                let remaining = region_length.saturating_sub(projection.offset);
                if stride == 0 || !remaining.is_multiple_of(stride) {
                    return trap("slice recast bytes do not tile the element layout");
                }
                return Ok(Some(Value::Int((remaining / stride) as i64)));
            }
            let Some(projection) = self.project_record_reference(&recast, &path, frame)? else {
                return Ok(None);
            };
            let cells = match recast {
                ReferenceStorage::AggregateByteRegion { cells, .. } => cells,
                ReferenceStorage::AggregateTyped {
                    source,
                    source_type,
                    ..
                } => self.snapshot_typed_value_bytes(&source, source_type)?,
                _ => return Ok(None),
            };
            self.read_record_view_projection(&cells, projection)
                .map(Some)
        }

        fn project_record_reference(
            &mut self,
            recast: &ReferenceStorage,
            path: &[ReferenceProjectionStep],
            frame: &mut Frame,
        ) -> EvalResult<Option<ReferenceProjection>> {
            let Some((retained, region_length)) = self.record_reference_geometry(recast)? else {
                return Ok(None);
            };
            if path.is_empty() {
                return Ok(Some(retained));
            }
            if retained.stored_integer.is_some() {
                return trap("stored-integer reference cannot project structural children");
            }
            let projection = self
                .record_view_type_projection(
                    retained.type_reference,
                    path,
                    retained.offset,
                    region_length,
                    frame,
                    None,
                )?
                .ok_or_else(|| {
                    Halt::Trap(format!(
                        "cannot project `{}` through captured reference view `{}`",
                        Self::mutable_record_projection_display(path),
                        self.program.display_type_reference(retained.type_reference),
                    ))
                })?;
            let extent = projection
                .stored_integer
                .map(|integer| usize::from(integer.stored_width_bits / 8))
                .or_else(|| {
                    self.record_view_type_layout(projection.type_reference, &mut HashSet::new())
                        .map(|(size, _)| size)
                })
                .ok_or_else(|| {
                    Halt::Trap("reference projection has no physical extent".to_owned())
                })?;
            if projection
                .offset
                .checked_add(extent)
                .is_none_or(|end| end > region_length)
            {
                return trap("reference projection exceeds its original backing region");
            }
            Ok(Some(projection))
        }

        fn retain_record_reference_projection(
            storage: &mut ReferenceStorage,
            projection: ReferenceProjection,
        ) -> EvalResult<Cell> {
            match storage {
                ReferenceStorage::AggregateByteRegion {
                    cells,
                    projection: retained,
                } => {
                    *retained = projection;
                    cells.get(projection.offset).cloned().ok_or_else(|| {
                        Halt::Trap(
                            "reference projection starts outside its original byte region"
                                .to_owned(),
                        )
                    })
                }
                ReferenceStorage::AggregateTyped {
                    source,
                    projection: retained,
                    ..
                } => {
                    *retained = projection;
                    Ok(source.clone())
                }
                _ => trap("record projection lost its original backing"),
            }
        }

        fn record_reference_geometry(
            &self,
            recast: &ReferenceStorage,
        ) -> EvalResult<Option<(ReferenceProjection, usize)>> {
            let geometry = match recast {
                ReferenceStorage::AggregateByteRegion { cells, projection } => {
                    (*projection, cells.len())
                }
                ReferenceStorage::AggregateTyped {
                    source_type,
                    projection,
                    ..
                } => (
                    *projection,
                    self.record_view_type_layout(*source_type, &mut HashSet::new())
                        .ok_or_else(|| {
                            Halt::Trap("typed reference backing has no fixed layout".to_owned())
                        })?
                        .0,
                ),
                _ => return Ok(None),
            };
            Ok(Some(geometry))
        }

        fn read_record_view_projection(
            &self,
            cells: &[Cell],
            projection: ReferenceProjection,
        ) -> EvalResult<Value> {
            if let Some(integer) = projection.stored_integer {
                let primitive = self
                    .program
                    .primitive_type_reference(projection.type_reference)
                    .ok_or_else(|| {
                        Halt::Trap("stored-integer projection is not scalar".to_owned())
                    })?;
                return self.assemble_stored_integer_byte_region(
                    cells,
                    projection.offset,
                    primitive,
                    integer.stored_width_bits,
                    integer.interpretation,
                );
            }
            self.assemble_record_view_type(projection.type_reference, cells, projection.offset)?
                .ok_or_else(|| {
                    Halt::Trap(format!(
                        "cannot assemble captured reference view `{}`",
                        self.program
                            .display_type_reference(projection.type_reference)
                    ))
                })
        }

        pub(crate) fn prepare_mutable_record_recast_write(
            &mut self,
            handle: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Option<MutableRecordRecastWrite>> {
            let Some((recast, path)) = self.reference_storage_path(handle, frame)? else {
                return Ok(None);
            };
            let Some(projection) = self.project_record_reference(&recast, &path, frame)? else {
                return Ok(None);
            };
            Ok(Some(MutableRecordRecastWrite { recast, projection }))
        }

        pub(crate) fn write_mutable_record_recast_target(
            &mut self,
            write: MutableRecordRecastWrite,
            value: Value,
        ) -> EvalResult<()> {
            let (cells, typed_source) = match write.recast {
                ReferenceStorage::AggregateByteRegion { cells, .. } => (cells, None),
                ReferenceStorage::AggregateTyped {
                    source,
                    source_type,
                    ..
                } => (
                    self.snapshot_typed_value_bytes(&source, source_type)?,
                    Some((source, source_type)),
                ),
                _ => return trap("scalar recast reached the aggregate write seam"),
            };
            let projection = write.projection;
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
            Ok(())
        }

        fn mutable_record_projection_display(path: &[ReferenceProjectionStep]) -> String {
            let mut rendered = String::new();
            for step in path {
                match step {
                    ReferenceProjectionStep::Field(field) => {
                        if !rendered.is_empty() {
                            rendered.push('.');
                        }
                        rendered.push_str(field);
                    }
                    ReferenceProjectionStep::Index(_) => rendered.push_str("[..]"),
                }
            }
            rendered
        }

        pub(crate) fn enum_value_from_path(
            &self,
            path: &TableNamePath,
        ) -> EvalResult<Option<Value>> {
            if !path.symbol.is_valid()
                || self.program.symbols.get(path.symbol).kind != symbols::SymbolKind::Variant
            {
                return Ok(None);
            }
            let owner = self.program.symbols.get(path.symbol).parent;
            let Some(data) = self.program.data_definition_by_symbol(owner) else {
                return Ok(None);
            };
            let Some(variant_name) =
                self.program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) if variant.symbol == path.symbol => {
                            Some(variant.name.as_str())
                        }
                        _ => None,
                    })
            else {
                return Ok(None);
            };
            let common: Vec<(crate::value::ValueName, Cell)> = self
                .program
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field),
                    _ => None,
                })
                .map(|field| {
                    Ok((
                        field.name.as_str().into(),
                        self.allocate_cell(self.default_for_type(field.type_reference)?)?,
                    ))
                })
                .collect::<EvalResult<_>>()?;
            Ok(Some(Value::Enum {
                type_symbol: data.symbol,
                variant_name: variant_name.into(),
                payload: common,
            }))
        }

        pub(crate) fn resolve_place(
            &mut self,
            handle: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Cell> {
            match self.program.expression_table.expression(handle).clone() {
                ExpressionNode::SavedSubject(_) => self.eval_read_cell(handle, frame),
                ExpressionNode::Name(path) => self.resolve_name_place(&path, frame),
                ExpressionNode::Member(member) => {
                    let receiver = self.resolve_place(member.receiver, frame)?;
                    self.field_cell(&receiver, member.member.as_str())
                }
                ExpressionNode::Indexed(indexed) => {
                    if let Some(range) = self.program.expression_table.range_node(indexed.index) {
                        let range = *range;
                        self.tick()?;
                        let view = self.eval_subslice(indexed.collection, &range, frame)?;
                        return self.allocate_cell(view);
                    }
                    let collection = self.resolve_place(indexed.collection, frame)?;
                    let index = self.eval_index(indexed.index, frame)?;
                    self.element_cell(&collection, index)
                }
                ExpressionNode::Borrow(inner) => self.resolve_place(inner.target, frame),
                other => match self.eval_expression(handle, frame)? {
                    value @ Value::Ref(_) => self.allocate_cell(value),
                    _ => unsupported(format!("place expression not supported: {other:?}")),
                },
            }
        }

        fn resolve_name_place(&mut self, path: &TableNamePath, frame: &Frame) -> EvalResult<Cell> {
            let members = expression_table(self.program)
                .name_path_members(path.members)
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect::<Vec<_>>();
            if members.is_empty() {
                return trap("empty name path");
            }

            let head = members[0].as_str();
            let mut cell = if let Some(local) = frame.local_cell(path.head_symbol) {
                local
            } else if path.head_symbol.is_valid()
                && self.program.symbols.get(path.head_symbol).kind
                    == symbols::SymbolKind::TypeParameter
            {
                return Err(Halt::StaticInputUnavailable(Binding {
                    symbol: path.head_symbol,
                    display: head.to_owned(),
                }));
            } else if self.is_declaration_namespace(path.head_symbol) {
                return trap(format!("`{head}` names a declaration, not storage"));
            } else {
                return trap(format!(
                    "place root `{head}` has no bound local or parameter"
                ));
            };

            for member in &members[1..] {
                cell = self.deref_cell(cell);
                cell = self.field_cell(&cell, member)?;
            }
            Ok(cell)
        }

        pub(crate) fn declared_type_is_fixed_array(
            &self,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            if !type_reference.is_valid() {
                return false;
            }
            match type_reference_table(self.program).type_reference(type_reference) {
                FixedArray { .. } => true,
                Constrained { base_type, .. } => self.declared_type_is_fixed_array(*base_type),
                _ => false,
            }
        }

        fn fixed_array_element_type(
            &self,
            type_reference: typed_trees::types::TypeReferenceHandle,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            if !type_reference.is_valid() {
                return None;
            }
            match type_reference_table(self.program).type_reference(type_reference) {
                FixedArray { element_type, .. } => Some(*element_type),
                Constrained { base_type, .. } => self.fixed_array_element_type(*base_type),
                _ => None,
            }
        }

        pub(crate) fn assignment_target_coercion(
            &mut self,
            handle: ExpressionHandle,
            frame: &mut Frame,
        ) -> Option<(typed_trees::types::PrimitiveType, ArithmeticDomain)> {
            if let ExpressionNode::Indexed(indexed) =
                self.program.expression_table.expression(handle).clone()
            {
                let array_type = self.expression_type_reference(indexed.collection, frame)?;
                let element_type = self.fixed_array_element_type(array_type)?;
                let primitive = self.program.primitive_type_reference(element_type)?;
                let domain = self
                    .program
                    .arithmetic_domain_for_type_reference(array_type);
                return Some((primitive, domain));
            }
            let type_reference = self.expression_type_reference(handle, frame)?;
            let primitive = self.program.primitive_type_reference(type_reference)?;
            let domain = self
                .program
                .arithmetic_domain_for_type_reference(type_reference);
            Some((primitive, domain))
        }

        pub(crate) fn coerce_scalar_with(
            &self,
            value: Value,
            primitive: typed_trees::types::PrimitiveType,
            domain: ArithmeticDomain,
        ) -> EvalResult<Value> {
            match &value {
                Value::UndeterminedNaN(format) => {
                    let expected = match format {
                        numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                        numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                    };
                    if primitive != expected {
                        return trap(
                            "undetermined NaN cannot change its landed carrier without a conversion",
                        );
                    }
                    Ok(value)
                }
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

        pub(crate) fn coerce_scalar_value(
            &self,
            value: Value,
            type_reference: typed_trees::types::TypeReferenceHandle,
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

        pub(crate) fn field_type_reference(
            &self,
            type_symbol: SymbolHandle,
            field_name: &str,
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            if let Some(data) = self.program.data_definition_by_symbol(type_symbol) {
                return self.data_field_type_reference(data, field_name);
            }
            if let Some(machine) = self.program.machine(type_symbol) {
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
        ) -> Option<typed_trees::types::TypeReferenceHandle> {
            for member in self.program.data_members(data) {
                if let Some(field) = member.as_field()
                    && field.name.as_str() == field_name
                {
                    return Some(field.type_reference);
                }
            }
            None
        }

        pub(crate) fn deref_cell(&self, cell: Cell) -> Cell {
            let inner = match &*cell.borrow() {
                Value::Ref(reference) => Some(reference.cell.clone()),
                _ => None,
            };
            inner.unwrap_or(cell)
        }

        pub(crate) fn eval_index(
            &mut self,
            index: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<usize> {
            let value = self.eval_expression(index, frame)?;
            let raw = value
                .as_int()
                .ok_or_else(|| Halt::Trap("array index is not an integer".to_owned()))?;
            usize::try_from(raw).map_err(|_| Halt::Trap(format!("array index {raw} out of range")))
        }

        pub(crate) fn element_cell(&self, container: &Cell, index: usize) -> EvalResult<Cell> {
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

        pub(crate) fn eval_subslice(
            &mut self,
            collection: ExpressionHandle,
            range: &typed_trees::expression::TableRangeExpression,
            frame: &mut Frame,
        ) -> EvalResult<Value> {
            let collection = self.resolve_place(collection, frame)?;
            let collection = self.deref_cell(collection).borrow().clone();
            let length = match &collection {
                Value::Array(elements) => elements.len(),
                Value::Str(text) => text.borrow().len(),
                other => return trap(format!("cannot subslice {other:?}")),
            };
            let start = if range.start.is_valid() {
                self.eval_index(range.start, frame)?
            } else {
                0
            };
            let end = if range.end.is_valid() {
                self.eval_index(range.end, frame)?
            } else {
                length
            };
            let end = if range.end_inclusive {
                end.checked_add(1)
                    .ok_or_else(|| Halt::Trap("inclusive subslice bound overflow".to_owned()))?
            } else {
                end
            };
            if start > end || end > length {
                return trap("subslice range is out of bounds");
            }
            match collection {
                Value::Array(elements) => Ok(Value::Array(elements[start..end].to_vec())),
                Value::Str(text) => text
                    .subslice(start, end)
                    .map(Value::Str)
                    .map_err(Halt::Trap),
                other => trap(format!("cannot subslice {other:?}")),
            }
        }
    }
}

pub(crate) mod program_lookup {
    use crate::interpreter::evaluator::{DataDefinition, Evaluator, Machine, State};
    impl<'program> Evaluator<'program> {
        pub(crate) fn find_machine_by_name(&self, name: &str) -> Option<&'program Machine> {
            let machine = self.program.realized_machine_named(name)?;
            self.find_machine_by_symbol(machine.symbol)
        }

        pub(crate) fn find_machine_by_symbol(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Option<&'program Machine> {
            let selected = self
                .selected_build_time_machine_bodies
                .iter()
                .find(|selected| selected.family == symbol)
                .map_or(symbol, |selected| selected.body);
            self.program.machine(selected)
        }

        pub(crate) fn find_state(&self, machine: &Machine, name: &str) -> Option<&'program State> {
            self.program
                .machine_states(machine)
                .iter()
                .find(|state| state.name.as_str() == name)
        }

        pub(crate) fn find_state_by_symbol(
            &self,
            machine: &Machine,
            symbol: symbols::SymbolHandle,
        ) -> Option<&'program State> {
            if symbol == symbols::SymbolHandle::invalid() {
                return None;
            }
            self.program.state_in_machine(machine, symbol)
        }

        pub(crate) fn find_data_by_name(&self, name: &str) -> Option<&'program DataDefinition> {
            self.program.data_definition_by_name_str(name)
        }
    }
}

pub(crate) mod record_views {
    use crate::interpreter::evaluator::{
        Cell, EvalResult, Evaluator, FixedArrayLength, Frame, Halt, HashSet, PrimitiveType,
        ReferenceProjection, ReferenceProjectionStep, TypeReferenceHandle, TypeReferenceNode,
        Value, interpreter_f32_from_bits, interpreter_f32_to_bits, trap, type_reference_table,
        wrap_to_width,
    };
    use layout_plans::IntegerInterpretation::Signed;
    use layout_plans::IntegerInterpretation::Unsigned;
    impl<'program> Evaluator<'program> {
        pub(crate) fn assemble_scalar_byte_region(
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
                let byte = cell.borrow().as_int().ok_or_else(|| {
                    Halt::Trap("byte-region read requires concrete integer bytes".to_owned())
                })? as u64
                    & 0xFF;
                bits |= byte << (8 * byte_index);
            }
            let assembled = match target {
                PrimitiveType::F32 => Value::Float(interpreter_f32_from_bits(bits as u32)),
                PrimitiveType::F64 => Value::Float(f64::from_bits(bits)),
                integer => Value::Int(wrap_to_width(bits as i64, integer)),
            };
            Ok(assembled)
        }

        pub(crate) fn write_scalar_byte_region(
            &self,
            cells: &[Cell],
            offset: usize,
            target: PrimitiveType,
            value: Value,
        ) -> EvalResult<()> {
            if matches!(value, Value::UndeterminedNaN(_)) {
                return trap("undetermined NaN has no representation for a byte-region write");
            }
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

        pub(crate) fn write_stored_integer_byte_region(
            &self,
            cells: &[Cell],
            offset: usize,
            target_type: TypeReferenceHandle,
            stored_width_bits: u16,
            interpretation: layout_plans::IntegerInterpretation,
            value: Value,
        ) -> EvalResult<()> {
            if stored_width_bits == 0
                || stored_width_bits > 64
                || !stored_width_bits.is_multiple_of(8)
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
            let integer = value.as_int().ok_or_else(|| {
                Halt::Trap("stored-integer recast write is not integer".to_owned())
            })?;
            let bit_count = u32::from(stored_width_bits);
            let fits = match interpretation {
                Signed => {
                    let magnitude = 1_i128 << (bit_count - 1);
                    i128::from(integer) >= -magnitude && i128::from(integer) < magnitude
                }
                Unsigned => i128::from(integer) >= 0 && i128::from(integer) < (1_i128 << bit_count),
            };
            if !fits {
                return trap(format!(
                    "stored-integer recast write value {integer} does not fit {stored_width_bits}-bit {} storage",
                    match interpretation {
                        Signed => "signed",
                        Unsigned => "unsigned",
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

        fn assemble_record_view_inner(
            &self,
            type_reference: TypeReferenceHandle,
            cells: &[Cell],
            base_offset: usize,
            visiting: &mut HashSet<typed_trees::type_identity::TypeIdentityId>,
        ) -> EvalResult<Option<Value>> {
            let identity = self.program.normalized_type_identity(type_reference);
            if !visiting.insert(identity) {
                return Ok(None);
            }
            let Some(data) = self.program.closed_data_definition(type_reference) else {
                visiting.remove(&identity);
                return Ok(None);
            };
            let mut field_specs: Vec<(String, TypeReferenceHandle, usize, usize)> = Vec::new();
            let mut field_symbols = Vec::new();
            for member in self.program.data_members(data) {
                let Some(field) = member.as_field() else {
                    visiting.remove(&identity);
                    return Ok(None);
                };
                if field.relevance.is_erased() {
                    continue;
                }
                let Some((size, align)) =
                    self.record_view_type_layout(field.type_reference, visiting)
                else {
                    visiting.remove(&identity);
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

            let application = self.program.plan_laid_layout_for_data(data);
            if application.is_none()
                && self
                    .program
                    .layout_storage_arguments(type_reference)
                    .is_some()
            {
                visiting.remove(&identity);
                return trap(
                    "record view requires the exact admitted fixed representation geometry",
                );
            }
            let plan = match application {
                Some(application) => match application.invariant_geometry() {
                    Some(geometry) => Some(geometry),
                    None => {
                        visiting.remove(&identity);
                        return Ok(None);
                    }
                },
                None => None,
            };
            let offsets = if let Some(plan) = plan {
                if plan.offsets.len() != field_specs.len()
                    || application
                        .is_none_or(|application| application.field_symbols != field_symbols)
                {
                    visiting.remove(&identity);
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
                        visiting.remove(&identity);
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
                    let Some(primitive) = self.program.primitive_type_reference(type_reference)
                    else {
                        visiting.remove(&identity);
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
                    visiting.remove(&identity);
                    return Ok(None);
                };
                field_values.insert(name.into(), self.allocate_cell(value)?);
            }
            visiting.remove(&identity);
            Ok(Some(Value::Struct {
                type_symbol,
                type_name: data.name.as_str().into(),
                fields: field_values,
            }))
        }

        pub(crate) fn assemble_record_view_type(
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

        pub(crate) fn assemble_stored_integer_byte_region(
            &self,
            cells: &[Cell],
            offset: usize,
            target: PrimitiveType,
            stored_width_bits: u16,
            interpretation: layout_plans::IntegerInterpretation,
        ) -> EvalResult<Value> {
            if stored_width_bits == 0
                || stored_width_bits > 64
                || !stored_width_bits.is_multiple_of(8)
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
                let byte = cell.borrow().as_int().ok_or_else(|| {
                    Halt::Trap("stored-integer view requires concrete integer bytes".to_owned())
                })? as u64
                    & 0xFF;
                bits |= byte << (8 * byte_index);
            }
            if matches!(interpretation, Signed) && stored_width_bits < 64 {
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
            visiting: &mut HashSet<typed_trees::type_identity::TypeIdentityId>,
            outer_stride: Option<usize>,
        ) -> EvalResult<Option<Value>> {
            if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
                return self
                    .assemble_scalar_byte_region(cells, base_offset, primitive)
                    .map(Some);
            }
            match type_reference_table(self.program).type_reference(type_reference) {
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
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                    self.assemble_record_view_inner(type_reference, cells, base_offset, visiting)
                }
                _ => Ok(None),
            }
        }

        pub(crate) fn record_view_type_layout(
            &self,
            type_reference: TypeReferenceHandle,
            visiting: &mut HashSet<typed_trees::type_identity::TypeIdentityId>,
        ) -> Option<(usize, usize)> {
            if let Some(primitive) = self.program.primitive_type_reference(type_reference) {
                let size = primitive.scalar_byte_size()?;
                return Some((size, size));
            }
            match type_reference_table(self.program).type_reference(type_reference) {
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
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                    self.record_view_data_layout(type_reference, visiting)
                }
                _ => None,
            }
        }

        fn record_view_data_layout(
            &self,
            type_reference: TypeReferenceHandle,
            visiting: &mut HashSet<typed_trees::type_identity::TypeIdentityId>,
        ) -> Option<(usize, usize)> {
            let identity = self.program.normalized_type_identity(type_reference);
            if !visiting.insert(identity) {
                return None;
            }
            let data = self.program.closed_data_definition(type_reference)?;
            let mut field_layouts = Vec::new();
            let mut field_symbols = Vec::new();
            for member in self.program.data_members(data) {
                let Some(field) = member.as_field() else {
                    visiting.remove(&identity);
                    return None;
                };
                if field.relevance.is_erased() {
                    continue;
                }
                let Some(layout) = self.record_view_type_layout(field.type_reference, visiting)
                else {
                    visiting.remove(&identity);
                    return None;
                };
                field_layouts.push(layout);
                field_symbols.push(field.symbol);
            }
            if self
                .program
                .layout_storage_arguments(type_reference)
                .is_some()
                && self.program.plan_laid_layout_for_data(data).is_none()
            {
                visiting.remove(&identity);
                return None;
            }
            let result = if let Some(application) = self.program.plan_laid_layout_for_data(data) {
                let plan = application.invariant_geometry()?;
                (plan.offsets.len() == field_layouts.len()
                    && application.field_symbols == field_symbols)
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
            visiting.remove(&identity);
            result
        }

        fn record_view_fields(
            &self,
            type_reference: TypeReferenceHandle,
        ) -> Option<
            Vec<(
                String,
                TypeReferenceHandle,
                usize,
                Option<typed_trees::PlanLaidIntegerField>,
                Option<typed_trees::PlanLaidRepeatedField>,
            )>,
        > {
            let data = self.program.closed_data_definition(type_reference)?;
            let mut fields = Vec::new();
            let mut layouts = Vec::new();
            let mut field_symbols = Vec::new();
            for member in self.program.data_members(data) {
                let field = member.as_field()?;
                if field.relevance.is_erased() {
                    continue;
                }
                let layout =
                    self.record_view_type_layout(field.type_reference, &mut HashSet::new())?;
                fields.push((field.name.as_str().to_owned(), field.type_reference));
                layouts.push(layout);
                field_symbols.push(field.symbol);
            }
            if self
                .program
                .layout_storage_arguments(type_reference)
                .is_some()
                && self.program.plan_laid_layout_for_data(data).is_none()
            {
                return None;
            }
            let (offsets, integer_fields, repeated_fields) = if let Some(application) =
                self.program.plan_laid_layout_for_data(data)
            {
                let plan = application.invariant_geometry()?;
                if plan.offsets.len() != fields.len() || application.field_symbols != field_symbols
                {
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
            type_reference: TypeReferenceHandle,
            path: &[ReferenceProjectionStep],
            base_offset: usize,
            region_len: usize,
            frame: &mut Frame,
        ) -> EvalResult<Option<ReferenceProjection>> {
            let type_name = self.program.display_type_reference(type_reference);
            let Some((ReferenceProjectionStep::Field(field_name), rest)) = path.split_first()
            else {
                return Ok(None);
            };
            let (_, field_type, field_offset, stored_integer, repeated_field) = self
                .record_view_fields(type_reference)
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
                return Ok(rest.is_empty().then_some(ReferenceProjection {
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

        pub(crate) fn record_view_type_projection(
            &mut self,
            type_reference: TypeReferenceHandle,
            path: &[ReferenceProjectionStep],
            base_offset: usize,
            region_len: usize,
            frame: &mut Frame,
            outer_stride: Option<usize>,
        ) -> EvalResult<Option<ReferenceProjection>> {
            if path.is_empty() {
                return Ok(Some(ReferenceProjection {
                    offset: base_offset,
                    type_reference,
                    stored_integer: None,
                }));
            }
            let (step, rest) = path.split_first().expect("nonempty projection path");
            match step {
                ReferenceProjectionStep::Field(_) => {
                    let mut record_type = type_reference;
                    while let TypeReferenceNode::Constrained { base_type, .. }
                    | TypeReferenceNode::Reference {
                        referee: base_type, ..
                    } = type_reference_table(self.program).type_reference(record_type)
                    {
                        record_type = *base_type;
                    }
                    if self.program.closed_data_definition(record_type).is_none() {
                        return Ok(None);
                    }
                    self.record_view_projection(record_type, path, base_offset, region_len, frame)
                }
                ReferenceProjectionStep::Index(index_handle) => {
                    let node = type_reference_table(self.program)
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
                        .ok_or_else(|| {
                            Halt::Trap("record-view array offset overflow".to_owned())
                        })?;
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
            type_reference: TypeReferenceHandle,
            cells: &[Cell],
            base_offset: usize,
            value: Value,
        ) -> EvalResult<()> {
            let type_name = self.program.display_type_reference(type_reference);
            let Value::Struct { fields, .. } = value else {
                return trap(format!(
                    "mutable record recast `{type_name}` requires a record value"
                ));
            };
            let Some(field_specs) = self.record_view_fields(type_reference) else {
                return trap(format!(
                    "cannot lay out mutable record recast `{type_name}`"
                ));
            };
            for (name, type_reference, field_offset, stored_integer, repeated_field) in field_specs
            {
                let Some(field_cell) = fields.get(name.as_str()) else {
                    return trap(format!(
                        "record value written through `{type_name}` has no field `{name}`"
                    ));
                };
                let offset = base_offset.checked_add(field_offset).ok_or_else(|| {
                    Halt::Trap("mutable record recast offset overflow".to_owned())
                })?;
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

        pub(crate) fn declared_type_is_slice(&self, type_reference: TypeReferenceHandle) -> bool {
            if !type_reference.is_valid() {
                return false;
            }
            match type_reference_table(self.program).type_reference(type_reference) {
                TypeReferenceNode::Slice { .. } => true,
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::Reference {
                    referee: base_type, ..
                } => self.declared_type_is_slice(*base_type),
                _ => false,
            }
        }

        pub(crate) fn snapshot_typed_value_bytes(
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

        pub(crate) fn commit_typed_value_bytes(
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

        pub(crate) fn write_record_view_type(
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
            match type_reference_table(self.program).type_reference(type_reference) {
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
                            .ok_or_else(|| {
                                Halt::Trap("mutable slice offset overflow".to_owned())
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
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                    self.write_record_view(type_reference, cells, base_offset, value)
                }
                _ => trap("mutable record projection is not a fixed-layout value"),
            }
        }

        pub(crate) fn eval_recast(
            &self,
            value: Value,
            target: Option<PrimitiveType>,
        ) -> EvalResult<Value> {
            let Some(target) = target else {
                return Ok(value);
            };
            let value = match value {
                Value::Ref(reference) => self.read_reference(&reference)?,
                other => other,
            };
            if matches!(value, Value::UndeterminedNaN(_)) {
                return trap("undetermined NaN has no representation for a raw-bit recast");
            }
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
                        Value::Float(f) => match integer.scalar_byte_size() {
                            Some(4) => interpreter_f32_to_bits(f) as i64,
                            _ => f.to_bits() as i64,
                        },
                        other => other.as_int().ok_or_else(|| {
                            Halt::Trap("recast to integer of non-scalar".to_owned())
                        })?,
                    };
                    Ok(Value::Int(wrap_to_width(raw, integer)))
                }
            }
        }
    }
}

pub(crate) mod type_metadata {
    use crate::interpreter::evaluator::{
        ArithmeticDomain, BinaryOperator, EvalResult, EvaluatedArgument, Evaluator,
        ExpressionHandle, ExpressionNode, FixedArrayLength, Frame, PrimitiveType,
        TypeReferenceHandle, TypeReferenceNode, Value, expression_table,
        integer_primitive_byte_width, primitive_is_unsigned64, trap, type_reference_table,
    };
    impl<'program> Evaluator<'program> {
        pub(crate) fn eval_state_argument(
            &mut self,
            argument: ExpressionHandle,
            destination: TypeReferenceHandle,
            frame: &mut Frame,
        ) -> EvalResult<EvaluatedArgument> {
            let cell = if self.declared_type_is_reference(destination) {
                let value = self.eval_expression_at_type(argument, destination, frame)?;
                self.allocate_cell(value)?
            } else if let Some(value) = self.anonymous_integer_landing_value(
                argument,
                self.program.primitive_type_reference(destination),
            )? {
                self.allocate_cell(value)?
            } else if matches!(
                self.program.expression_table.expression(argument),
                ExpressionNode::ArrayLiteral(_)
            ) {
                let value = self.eval_expression_at_type(argument, destination, frame)?;
                self.allocate_cell(value)?
            } else {
                self.eval_argument(argument, frame)?
            };
            let value = cell.borrow().clone();
            let value = self.capture_value_operand(value, destination)?;
            *cell.borrow_mut() = value;
            Ok(EvaluatedArgument::plain(cell))
        }

        pub(crate) fn declared_type_is_reference(
            &self,
            mut destination: TypeReferenceHandle,
        ) -> bool {
            loop {
                match type_reference_table(self.program).type_reference(destination) {
                    TypeReferenceNode::Constrained { base_type, .. } => destination = *base_type,
                    TypeReferenceNode::Reference { .. } => return true,
                    _ => return false,
                }
            }
        }

        pub(crate) fn expression_type_reference(
            &self,
            expression: ExpressionHandle,
            frame: &Frame,
        ) -> Option<TypeReferenceHandle> {
            match self.program.expression_table.expression(expression) {
                ExpressionNode::SavedSubject(saved) => {
                    let subject = self.program.saved_value_source(saved)?;
                    let machine = self.program.machine_by_symbol(frame.machine_symbol)?;
                    let state = self.program.state_in_machine(machine, frame.state_symbol)?;
                    validation::expression_result_type_reference(
                        self.program,
                        machine,
                        state,
                        subject,
                    )
                }
                ExpressionNode::Call(call) if call.target_symbol.is_valid() => {
                    self.program.machines().iter().find_map(|machine| {
                        self.program
                            .state_in_machine(machine, call.target_symbol)
                            .map(|state| state.return_type)
                    })
                }
                ExpressionNode::ArrayLiteral(_) => {
                    validation::declared_constant_array_type(self.program, expression)
                }
                ExpressionNode::Borrow(inner) => {
                    self.expression_type_reference(inner.target, frame)
                }
                ExpressionNode::Cast(cast) => Some(cast.target_type),
                ExpressionNode::Indexed(indexed) => {
                    let collection = self.expression_type_reference(indexed.collection, frame)?;
                    self.collection_element_type(collection)
                }
                ExpressionNode::Member(member) => {
                    let receiver = self.expression_type_reference(member.receiver, frame)?;
                    self.projected_member_type(receiver, member.member.as_str())
                }
                ExpressionNode::Name(path) => {
                    let members = expression_table(self.program).name_path_members(path.members);
                    let (_, rest) = members.split_first()?;
                    let mut current = frame.local_type(path.head_symbol)?;
                    for member in rest {
                        current = self.projected_member_type(current, member.as_str())?;
                    }
                    Some(current)
                }
                _ => None,
            }
        }

        fn projected_member_type(
            &self,
            mut type_reference: TypeReferenceHandle,
            member_name: &str,
        ) -> Option<TypeReferenceHandle> {
            loop {
                match type_reference_table(self.program).type_reference(type_reference) {
                    TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                    TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                    TypeReferenceNode::Named { symbol, .. } => {
                        return self.field_type_reference(*symbol, member_name);
                    }
                    _ => return None,
                }
            }
        }

        pub(crate) fn collection_element_type(
            &self,
            mut type_reference: TypeReferenceHandle,
        ) -> Option<TypeReferenceHandle> {
            loop {
                match type_reference_table(self.program).type_reference(type_reference) {
                    TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                    TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
                    TypeReferenceNode::FixedArray { element_type, .. }
                    | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
                    _ => return None,
                }
            }
        }

        pub(crate) fn eval_recast_to_type(
            &self,
            value: Value,
            target_type: TypeReferenceHandle,
        ) -> EvalResult<Value> {
            let target = type_reference_table(self.program)
                .type_reference(target_type)
                .clone();
            match target {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => self.eval_recast_to_type(value, referee),
                TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } => {
                    let value = match value {
                        Value::Ref(reference) => self.read_reference(&reference)?,
                        other => other,
                    };
                    let Value::Array(elements) = value else {
                        return trap("fixed-array recast source is not an array");
                    };
                    if elements.len() != length {
                        return trap(format!(
                            "fixed-array recast expected {length} elements, found {}",
                            elements.len()
                        ));
                    }
                    let mut recast = Vec::with_capacity(elements.len());
                    for element in elements {
                        let value = element.borrow().clone();
                        let value = self.eval_recast_to_type(value, element_type)?;
                        recast.push(self.allocate_cell(value)?);
                    }
                    Ok(Value::Array(recast))
                }
                TypeReferenceNode::Slice { element_type } => {
                    let value = match value {
                        Value::Ref(reference) => self.read_reference(&reference)?,
                        other => other,
                    };
                    let Value::Array(elements) = value else {
                        return trap("slice recast source is not an array or slice");
                    };
                    let mut recast = Vec::with_capacity(elements.len());
                    for element in elements {
                        let value = element.borrow().clone();
                        let value = self.eval_recast_to_type(value, element_type)?;
                        recast.push(self.allocate_cell(value)?);
                    }
                    Ok(Value::Array(recast))
                }
                _ => self.eval_recast(value, self.cast_target_primitive(target_type)),
            }
        }

        pub(crate) fn expression_is_unsigned64(
            &self,
            expression: checked_trees::expression::ExpressionHandle,
            frame: &Frame,
        ) -> bool {
            primitive_is_unsigned64(
                self.expression_scalar_type(expression, frame)
                    .map(|(primitive, _)| primitive),
            )
        }

        pub(crate) fn expression_scalar_type(
            &self,
            expression: checked_trees::expression::ExpressionHandle,
            frame: &Frame,
        ) -> Option<(PrimitiveType, ArithmeticDomain)> {
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    let mut result = None;
                    for arm in self.program.expression_table.match_arms(dispatch.arms) {
                        if let Some(arm_type) = self.expression_scalar_type(arm.value, frame) {
                            if result.is_some_and(|result| result != arm_type) {
                                return None;
                            }
                            result = Some(arm_type);
                        }
                    }
                    result
                }
                ExpressionNode::Cast(cast) => {
                    Some((self.cast_target_primitive(cast.target_type)?, cast.domain))
                }
                ExpressionNode::Borrow(inner) => self.expression_scalar_type(inner.target, frame),
                ExpressionNode::Unary(unary) => self.expression_scalar_type(unary.operand, frame),
                ExpressionNode::Integer(literal) => literal.landing().map(|landing| {
                    let primitive = PrimitiveType::from_landed_integer(landing.landed_type);
                    (primitive, landing.domain)
                }),
                ExpressionNode::Indexed(_) | ExpressionNode::Member(_) => {
                    let reference = self.expression_type_reference(expression, frame)?;
                    Some((
                        self.program.primitive_type_reference(reference)?,
                        self.program.arithmetic_domain_for_type_reference(reference),
                    ))
                }
                ExpressionNode::UndeterminedNaN(format) => Some((
                    match format {
                        numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                        numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                    },
                    ArithmeticDomain::Exact,
                )),
                ExpressionNode::Float(literal) => literal.landing().map(|format| {
                    (
                        match format {
                            numerics::literals::FloatFormat::F32 => PrimitiveType::F32,
                            numerics::literals::FloatFormat::F64 => PrimitiveType::F64,
                        },
                        ArithmeticDomain::Exact,
                    )
                }),
                ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                    ) =>
                {
                    self.expression_scalar_type(binary.left, frame)
                }
                ExpressionNode::Binary(binary) => {
                    let left = self.expression_scalar_type(binary.left, frame);
                    let right = self.expression_scalar_type(binary.right, frame);
                    match (left, right) {
                        (Some(left), Some(right)) => {
                            let left_width = integer_primitive_byte_width(left.0).unwrap_or(8);
                            let right_width = integer_primitive_byte_width(right.0).unwrap_or(8);
                            Some(if right_width > left_width {
                                right
                            } else if left_width > right_width || left.1 != ArithmeticDomain::Exact
                            {
                                left
                            } else {
                                right
                            })
                        }
                        (left, right) => left.or(right),
                    }
                }
                ExpressionNode::Name(path) => {
                    let members = expression_table(self.program).name_path_members(path.members);
                    if members.len() == 1 {
                        return frame.local_scalar(path.head_symbol);
                    }
                    let reference = self.expression_type_reference(expression, frame)?;
                    Some((
                        self.program.primitive_type_reference(reference)?,
                        self.program.arithmetic_domain_for_type_reference(reference),
                    ))
                }
                ExpressionNode::Call(call) if call.target_symbol.is_valid() => {
                    let state = self
                        .program
                        .machines()
                        .iter()
                        .flat_map(|machine| self.program.machine_states(machine))
                        .find(|state| state.symbol == call.target_symbol)?;
                    let primitive = self.program.primitive_type_reference(state.return_type)?;
                    let domain = self
                        .program
                        .arithmetic_domain_for_type_reference(state.return_type);
                    Some((primitive, domain))
                }
                _ => None,
            }
        }
    }
}

pub(crate) mod value_projections {
    use crate::interpreter::evaluator::{
        Cell, EvalResult, Evaluator, ExpressionHandle, ExpressionNode, Frame, Halt, Value,
        expression_table, unsupported,
    };
    use language_semantics::const_value::boolean_literal_spelling;
    use typed_trees::operator::builtin_index_read_is_spelled;
    impl Evaluator<'_> {
        pub(crate) fn eval_read_cell(
            &mut self,
            expression: ExpressionHandle,
            frame: &mut Frame,
        ) -> EvalResult<Cell> {
            match self.program.expression_table.expression(expression).clone() {
                ExpressionNode::SavedSubject(saved) => frame
                    .match_subjects
                    .iter()
                    .rev()
                    .find(|(capture, _)| *capture == saved.capture)
                    .map(|(_, cell)| cell.clone())
                    .ok_or_else(|| {
                        Halt::Unsupported(
                            "saved subject has no active evaluated capture".to_owned(),
                        )
                    }),
                ExpressionNode::Name(path) => {
                    let members = expression_table(self.program).name_path_members(path.members);
                    if members.len() > 1
                        && let Some(value) =
                            self.read_mutable_record_recast_target(expression, frame)?
                    {
                        return self.allocate_cell(value);
                    }
                    if matches!(members, [name] if boolean_literal_spelling(name.as_str()).is_some())
                    {
                        let value = self.eval_name(&path, frame)?;
                        return self.allocate_cell(value);
                    }
                    if let Some(value) = self.enum_value_from_path(&path)? {
                        return self.allocate_cell(value);
                    }
                    self.resolve_place(expression, frame)
                }
                ExpressionNode::Member(member) => {
                    if let Some(value) =
                        self.read_mutable_record_recast_target(expression, frame)?
                    {
                        return self.allocate_cell(value);
                    }
                    if member.case_variant.is_some()
                        && let Some(observed) =
                            frame.guard_call_result(self.program, member.receiver)
                    {
                        let receiver = self.allocate_cell(observed)?;
                        return self.field_cell(&receiver, member.member.as_str());
                    }
                    let receiver = self.eval_read_cell(member.receiver, frame)?;
                    self.field_cell(&receiver, member.member.as_str())
                }
                ExpressionNode::Indexed(indexed) => {
                    if let Some(value) =
                        self.read_mutable_record_recast_target(expression, frame)?
                    {
                        return self.allocate_cell(value);
                    }
                    if let Some(range) = self.program.expression_table.range_node(indexed.index) {
                        let range = *range;
                        let value = self.eval_subslice(indexed.collection, &range, frame)?;
                        return self.allocate_cell(value);
                    }
                    self.require_builtin_index(expression, &indexed, frame)?;
                    let collection = self.eval_read_cell(indexed.collection, frame)?;
                    let collection = self.deref_cell(collection);
                    let index = self.eval_index(indexed.index, frame)?;
                    if let Value::Str(text) = &*collection.borrow() {
                        let byte = text.borrow().get(index).copied().ok_or_else(|| {
                            Halt::Trap(format!("string index {index} out of bounds"))
                        })?;
                        return self.allocate_cell(Value::Int(i64::from(byte)));
                    }
                    self.element_cell(&collection, index)
                }
                _ => {
                    let value = self.eval_expression(expression, frame)?;
                    self.allocate_cell(value)
                }
            }
        }

        fn require_builtin_index(
            &self,
            expression: ExpressionHandle,
            indexed: &typed_trees::expression::TableIndexedExpression,
            frame: &Frame,
        ) -> EvalResult<()> {
            let operands = [
                self.expression_type_reference(indexed.collection, frame),
                self.expression_type_reference(indexed.index, frame),
            ];
            if !builtin_index_read_is_spelled(
                self.program,
                frame.machine_symbol,
                expression,
                &operands,
            ) {
                return unsupported("array value projection has no exact builtin indexing meaning");
            }
            Ok(())
        }
    }
}
