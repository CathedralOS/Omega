use crate::interpreter::evaluator::{
    ArithmeticDomain, Cell, DataDefinition, DataMember, EvalResult, Evaluator, ExpressionHandle,
    ExpressionNode, FloatMeaning, FloatSemantics, Frame, Halt, HashSet, MutableRecordRecastWrite,
    PrimitiveType, Reference, ReferenceProjection, ReferenceProjectionStep, ReferenceStorage,
    SemanticFloatFormat, SymbolHandle, TableNamePath, TypeReferenceHandle, TypeReferenceNode,
    Value, apply_arithmetic_domain, interpreter_f32_from_bits, interpreter_f32_to_bits, trap,
    unsupported,
};
use language_semantics::const_value::boolean_literal_spelling;
use language_semantics::declaration_selection::CollectionMeasure;
impl<'program> Evaluator<'program> {
    pub(in crate::interpreter::evaluator) fn field_cell(
        &self,
        container: &Cell,
        field: &str,
    ) -> EvalResult<Cell> {
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
                .find(|(name, _)| **name == *field)
                .map(|(_, cell)| cell.clone())
                .ok_or_else(|| {
                    Halt::Trap(format!(
                        "case `{variant_name}` carries no payload field `{field}`"
                    ))
                }),
            // `slice.len` / `array.len` in member form produces a fresh length.
            Value::Array(elements)
                if CollectionMeasure::from_authored_spelling(field)
                    == Some(CollectionMeasure::Length) =>
            {
                self.allocate_cell(Value::Int(elements.len() as i64))
            }
            // A text literal flowing into a `&[u8] in Utf8` parameter observes
            // its UTF-8 byte count, matching the native `<literal>.len` fold.
            Value::Str(text)
                if CollectionMeasure::from_authored_spelling(field)
                    == Some(CollectionMeasure::Length) =>
            {
                self.allocate_cell(Value::Int(text.borrow().len() as i64))
            }
            other => trap(format!("cannot read field `{field}` of {other:?}")),
        }
    }

    pub(in crate::interpreter::evaluator) fn eval_name(
        &mut self,
        path: &TableNamePath,
        frame: &Frame,
    ) -> EvalResult<Value> {
        // The boolean keywords `true`/`false` can arrive as single-member name paths in
        // value/transition position (the parser does not always fold them to a literal).
        let members = self
            .program
            .expression_table
            .name_path_members(path.members);
        if let [name] = members
            && let Some(value) = boolean_literal_spelling(name.as_str())
        {
            return Ok(Value::Bool(value));
        }
        // An enum value reference (`CellId::R02` / `Command::Look`) resolves to an Enum.
        if let Some(enum_value) = self.enum_value_from_path(path)? {
            return Ok(enum_value);
        }
        let cell = self.resolve_name_place(path, frame)?;
        let value = cell.borrow().clone();
        // A `Ref` read through a name dereferences transparently (param of `&mut T`).
        self.observe_reference_value(value)
    }

    /// Read the selected endpoint at its retained representation. No lexical
    /// binding lookup is needed, so returned and aggregate-stored references
    /// use the same observation as parameters and locals.
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

    pub(in crate::interpreter::evaluator) fn observe_reference_value(
        &self,
        value: Value,
    ) -> EvalResult<Value> {
        match value {
            Value::Ref(reference) => self.read_reference(&reference),
            other => Ok(other),
        }
    }

    /// Capture one checked reference endpoint for locals and call operands.
    /// View projections retain their original backing and physical codec;
    /// selectors are evaluated here once, not in the borrowing callee.
    pub(in crate::interpreter::evaluator) fn capture_reference_view(
        &mut self,
        initializer: ExpressionHandle,
        mut destination: TypeReferenceHandle,
        frame: &mut Frame,
    ) -> EvalResult<Option<(Cell, ReferenceStorage)>> {
        loop {
            match self
                .program
                .type_reference_table
                .type_reference(destination)
            {
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

    /// Retain the original cells and captured coordinates of an explicit reference
    /// recast, whether shared or mutable. The checked reference type owns
    /// access; this descriptor owns representation and backing identity.
    pub(in crate::interpreter::evaluator) fn reference_recast_initializer(
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
            if let Some((mut storage, path)) = self.reference_storage_path(source_handle, frame)?
                && let Some(mut projection) =
                    self.project_record_reference(&storage, &path, frame)?
            {
                // A re-view changes observation geometry, not backing kind.
                // Keep the captured cells or typed owner rather than treating
                // the byte cell of an existing view as a new record value.
                projection.type_reference = cast.target_type;
                projection.stored_integer = None;
                let source = Self::retain_record_reference_projection(&mut storage, projection)?;
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

    /// Find the captured reference at the selected storage place, retaining
    /// any remaining projection. References in locals, parameters and record
    /// fields carry the same descriptor. Normalize typed Name and Member paths.
    pub(in crate::interpreter::evaluator) fn reference_storage_path(
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
                let members = self
                    .program
                    .expression_table
                    .name_path_members(path.members);
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

    pub(in crate::interpreter::evaluator) fn read_mutable_record_recast_target(
        &mut self,
        handle: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        let Some((recast, path)) = self.reference_storage_path(handle, frame)? else {
            return Ok(None);
        };
        if matches!(path.as_slice(), [ReferenceProjectionStep::Field(field)]
            if CollectionMeasure::from_authored_spelling(field) == Some(CollectionMeasure::Length))
            && let Some((projection, region_length)) = self.record_reference_geometry(&recast)?
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

    /// Projection starts at the captured endpoint, not the cast's authored root.
    /// An empty path preserves a stored-integer codec instead of replacing it
    /// with the logical scalar width when the reference is forwarded.
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
            .ok_or_else(|| Halt::Trap("reference projection has no physical extent".to_owned()))?;
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
                        "reference projection starts outside its original byte region".to_owned(),
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
                .ok_or_else(|| Halt::Trap("stored-integer projection is not scalar".to_owned()))?;
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

    pub(in crate::interpreter::evaluator) fn prepare_mutable_record_recast_write(
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

    pub(in crate::interpreter::evaluator) fn write_mutable_record_recast_target(
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

    /// `Type::Variant` paths whose head is an enum/data symbol with a matching variant.
    pub(in crate::interpreter::evaluator) fn enum_value_from_path(
        &self,
        path: &TableNamePath,
    ) -> EvalResult<Option<Value>> {
        // Resolution has already selected the declaration. Display names may
        // include an importing package's alias, and same-leaf data declarations
        // are not interchangeable during evaluation.
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
        Ok(Some(
            // MIXED shapes: a bare payload-less case still carries the COMMON
            // fields, zero-initialized (scalar-only by validation, so the
            // primitive default is the zero value). Pure sums add nothing.
            Value::Enum {
                type_symbol: data.symbol,
                variant_name: variant_name.into(),
                payload: common,
            },
        ))
    }

    // ---- place resolution ---------------------------------------------------

    /// Resolve an lvalue expression to its storage cell (for assignment / `&mut`).
    pub(in crate::interpreter::evaluator) fn resolve_place(
        &mut self,
        handle: ExpressionHandle,
        frame: &mut Frame,
    ) -> EvalResult<Cell> {
        match self.program.expression_table.expression(handle).clone() {
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
            // A place can be ROOTED at a computed expression: `pick(bag).field`,
            // `view(values)[i]`, a `match` arm that returns `&mut`. Evaluating it
            // once produces a `Ref` whose referent cell IS the place; callers
            // deref it like any name-resolved reference cell. A non-reference
            // result is a temporary with no storage and still fails closed.
            other => match self.eval_expression(handle, frame)? {
                value @ Value::Ref(_) => self.allocate_cell(value),
                _ => unsupported(format!("place expression not supported: {other:?}")),
            },
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

        // Storage belongs to an explicit local or parameter. Debug spelling
        // cannot select the activation record or manufacture an implicit field.
        let head = members[0].as_str();
        let mut cell = if let Some(local) = frame.local_cell(path.head_symbol) {
            local
        } else if self.is_declaration_namespace(path.head_symbol) {
            // A head resolved to a declaration (`Worker` in `Worker::write`)
            // names no storage. Reading it as an implicit field would let a
            // same-spelled field of the current instance stand in for it.
            return trap(format!("`{head}` names a declaration, not storage"));
        } else {
            return trap(format!(
                "place root `{head}` has no bound local or parameter"
            ));
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
    pub(in crate::interpreter::evaluator) fn declared_type_is_fixed_array(
        &self,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        if !type_reference.is_valid() {
            return false;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            typed_trees::types::TypeReferenceNode::FixedArray { .. } => true,
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                self.declared_type_is_fixed_array(*base_type)
            }
            _ => false,
        }
    }

    /// The ELEMENT type of an owned fixed array `[T; N]` -- seeing THROUGH a
    /// domain `Constrained` wrapper (`[u8; N] in Wrapping`). `None` for a slice,
    /// scalar, or invalid reference. Used to wrap an array-element store to the
    /// element's width/domain (the field-store truncation, for `arr[i] = v`).
    fn fixed_array_element_type(
        &self,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if !type_reference.is_valid() {
            return None;
        }
        match self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
                Some(*element_type)
            }
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
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
    pub(in crate::interpreter::evaluator) fn assignment_target_coercion(
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
            // The arithmetic domain lives on the ARRAY (`[T;N] in D`), not the
            // bare element type, so read it from the array reference.
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

    /// The CORE value-landing coercion: coerce a stored SCALAR to an already-
    /// resolved (primitive, arithmetic-domain), matching the native store into
    /// that typed slot -- the decision-17 truncate/clamp/trap for an integer, f32
    /// rounding for a float. A non-scalar value (Struct, Array, Ref, ...) passes
    /// through unchanged. Every interpreter value-landing seam funnels here.
    pub(in crate::interpreter::evaluator) fn coerce_scalar_with(
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

    /// Coerce a stored SCALAR to a declared TYPE reference (resolves its primitive
    /// and domain, then [`coerce_scalar_with`]). A non-primitive type passes through.
    /// Used where a value lands in a typed slot with the type in hand: struct/case
    /// literal FIELD init + the LocalData store (the type carries its own domain).
    pub(in crate::interpreter::evaluator) fn coerce_scalar_value(
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

    /// Declared type reference of `field_name` on the data record or machine
    /// identified by `type_symbol`. A machine instance's struct carries the
    /// MACHINE's symbol while its fields come from the attached data (plus the
    /// machine-owned cells), so both field sources are searched. The caller
    /// derives the primitive type and arithmetic domain from the reference.
    pub(in crate::interpreter::evaluator) fn field_type_reference(
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

    /// If a cell holds a `Ref`, return the referenced cell (so field access on a `&mut`
    /// parameter reaches the aliased place). Otherwise the cell itself.
    pub(in crate::interpreter::evaluator) fn deref_cell(&self, cell: Cell) -> Cell {
        let inner = match &*cell.borrow() {
            Value::Ref(reference) => Some(reference.cell.clone()),
            _ => None,
        };
        inner.unwrap_or(cell)
    }

    /// Evaluate an index expression to a `usize` element index.
    pub(in crate::interpreter::evaluator) fn eval_index(
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

    /// Resolve one element CELL of an `Array` place (sharing the same allocation, so a write
    /// through the returned cell aliases the array element).
    pub(in crate::interpreter::evaluator) fn element_cell(
        &self,
        container: &Cell,
        index: usize,
    ) -> EvalResult<Cell> {
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

    /// Evaluate a subslice `collection[start..end]` into a view sharing the
    /// collection's element cells or packed bytes. A
    /// missing start defaults to 0; a missing end to the length; `end_inclusive` extends by
    /// one.
    pub(in crate::interpreter::evaluator) fn eval_subslice(
        &mut self,
        collection: ExpressionHandle,
        range: &typed_trees::expression::TableRangeExpression,
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        // Resolve nested windows recursively without replaying their selectors.
        // Clone only the view descriptor; its element cells or bytes stay shared.
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
