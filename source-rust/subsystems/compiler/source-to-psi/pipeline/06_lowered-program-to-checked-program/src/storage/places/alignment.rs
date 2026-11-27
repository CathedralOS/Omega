//! Name an alignment observation by the storage whose address it measures.

use super::*;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Static, ScalarReadProjection as Projection,
};

impl Storage<'_> {
    pub(super) fn bind_alignment(
        &self,
        tables: &MachineTables,
        actual: &Lineage,
        path: &[Projection],
        ty: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        self.alignment_term(tables, actual, path, ty)
            .map_err(crate::Unbound::Refused)?
            .ok_or(crate::Unbound::Unproved(
                "the observed alignment has no stable correspondence to this storage",
            ))
    }

    /// Reborrowing a field starts with the geometry of the formal. Even the
    /// same address can yield different observations if enclosing storage
    /// imposes a smaller cap (for example, a packed record field).
    pub(super) fn alignment_at_call(
        &self,
        tables: &MachineTables,
        actual: &Lineage,
        formal: &terminal_psi::StructuralParameterDeclaration,
        path: &[Projection],
        ty: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        if formal.access == StructuralAccess::Owned {
            return Err(crate::Unbound::Unproved(
                "an owned parameter gets its own storage at the call",
            ));
        }
        let concerns = Construct::Machine;
        let same_type = match self
            .shape(actual, concerns)
            .map_err(crate::Unbound::Refused)?
        {
            Shape::Typed(current) => {
                current == formal.structural_type
                    || self.reads_as_lent_view(current, formal.structural_type)
            }
            Shape::Scalar(current) => matches!(
                self.declaration(formal.structural_type, concerns).map_err(crate::Unbound::Refused)?.shape,
                StructuralTypeShape::PrimitiveScalar(carrier) if carrier == current
            ),
            _ => false,
        };
        if !same_type {
            return Err(crate::Unbound::Unproved(
                "the borrowed alignment subject needs the same carrier at the call",
            ));
        }
        let term = self.bind_alignment(tables, actual, path, ty)?;
        let ScalarTerm::StructuralRead { path: observed, .. } = &term else {
            return Err(crate::Unbound::Unsupported(
                "alignment needs a storage observation",
            ));
        };
        let root = self
            .root_type(actual.root, concerns)
            .map_err(crate::Unbound::Refused)?;
        let (_, observed) = observed
            .split_last()
            .expect("alignment is a terminal observation");
        let actual_path = self.static_alignment_path(root, observed);
        if !actual.segments.is_empty() {
            let mut geometry =
                terminal_semantics::ReferenceStorageGeometry::new(self.types.iter(), |id| {
                    Err(terminal_semantics::StorageGeometryError::UnresolvedExtent(
                        id,
                    ))
                });
            let actual_cap = actual_path
                .as_deref()
                .and_then(|path| geometry.location_alignment_limit(root, path));
            let formal_cap = self
                .static_alignment_path(formal.structural_type, path)
                .and_then(|path| geometry.location_alignment_limit(formal.structural_type, &path));
            if !actual_cap
                .zip(formal_cap)
                .is_some_and(|(left, right)| left == right)
            {
                return Err(crate::Unbound::Unproved(
                    "the projected actual and formal need corresponding alignment guarantees",
                ));
            }
        }
        Ok(actual_path
            .as_deref()
            .and_then(|path| self.known_alignment_term(actual.root, path, ty))
            .unwrap_or(term))
    }

    fn static_alignment_path(
        &self,
        root: StructuralTypeId,
        path: &[Projection],
    ) -> Option<Vec<StructuralPathSegment>> {
        let steps = path
            .iter()
            .map(|step| match step {
                Projection::Static(step) => Some(*step),
                Projection::Referent => Some(Static::Referent),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        terminal_semantics::canonical_structural_storage_path(self.types.iter(), root, &steps)
    }

    pub(super) fn known_alignment_term(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
        ty: ScalarType,
    ) -> Option<ScalarTerm> {
        let alignment =
            terminal_semantics::known_storage_alignment(self.types, self.lowered, place, path)?;
        Some(ScalarTerm::Integer {
            scalar_type: ty.as_integer()?,
            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(alignment)),
        })
    }

    /// Writes to inline bytes preserve their location. A shared input loan
    /// keeps a descriptor's backing fixed when a projection crosses it.
    pub(super) fn alignment_term(
        &self,
        tables: &MachineTables,
        actual: &Lineage,
        path: &[Projection],
        ty: ScalarType,
    ) -> Checked<Option<ScalarTerm>> {
        let concerns = Construct::Machine;
        let kind = self.root_kind(actual.root, concerns)?;
        let (input_access, producer) = match kind {
            StructuralPlaceKind::Parameter { .. } => (
                terminal_psi::StructuralParameterDeclaration::find_by_place(
                    &self.lowered.structural_parameters,
                    actual.root,
                )
                .map(|input| input.access),
                None,
            ),
            // A new iteration can establish the same local name at a new location.
            StructuralPlaceKind::OperationResult { producer, .. } => {
                let (block, _) = self.graph.locate(ProgramPoint::AfterOperation(producer))?;
                if self.graph.in_cycle(block)? {
                    return Ok(None);
                }
                (None, Some(producer))
            }
            StructuralPlaceKind::BlockParameter { block, .. } if !self.graph.in_cycle(block)? => {
                (None, None)
            }
            _ => return Ok(None),
        };

        let mut observed = Vec::with_capacity(actual.segments.len() + path.len() + 1);
        for segment in &actual.segments {
            observed.push(match *segment {
                Segment::Field(field) => Static::Field(field).into(),
                Segment::Case(case) => Static::Case(case).into(),
                Segment::FixedIndex(index) => Static::FixedIndex(index).into(),
                Segment::Referent => Projection::Referent,
                Segment::RuntimeIndex { selector } => {
                    let scalar_type = tables
                        .values
                        .checked(selector, self.machine, concerns)?
                        .scalar_type;
                    Projection::Index(Box::new(ScalarTerm::value(selector, scalar_type)))
                }
                _ => return Ok(None),
            });
        }
        observed.extend_from_slice(path);
        observed.push(Projection::Alignment);
        let root_type = self.root_type(actual.root, concerns)?;
        let mut view_elements = Vec::new();
        let mut references = Vec::new();
        let scalar_type = terminal_semantics::structural_read_type_with_crossings(
            self.types.iter(),
            root_type,
            &observed,
            |position, extent| {
                if extent.is_none() {
                    view_elements.push(position);
                }
            },
            |position, access| references.push((position, access)),
        );
        if scalar_type != Some(ty) {
            return Ok(None);
        }
        let crossings = view_elements.len() + references.len();
        if crossings != 0 && input_access != Some(StructuralAccess::SharedBorrow) {
            // A write anywhere that may reach this mutable input can replace a
            // descriptor. If the first descriptor leads into exclusive storage,
            // bytes beyond that crossing may also contain a replaceable descriptor.
            let behind_shared_reference = references.first().is_some_and(|(position, access)| {
                *access == StructuralAccess::SharedBorrow
                    && view_elements.first().is_none_or(|first| position < first)
            });
            let replaceable = match producer {
                // The producer establishes this descriptor once. Releasing or
                // moving it ends that value's lifetime without supplying a new
                // backing. A later initialization is a separate logged store.
                Some(producer) => self.writes_reaching_root(actual.root).any(|write| {
                    write.operation != producer && !matches!(write.stored, Stored::Vacated)
                }),
                None => {
                    input_access != Some(StructuralAccess::MutableBorrow)
                        || self.storage_ever_written(actual.root)
                }
            };
            if (crossings > 1 && !behind_shared_reference) || replaceable {
                return Ok(None);
            }
        }
        // Element views start a new geometry region at the selected element.
        // A captured literal zero therefore has the same cap as a first-element
        // clause. Inline arrays retain their original selector and its geometry.
        for position in view_elements {
            if let Projection::Index(index) = &observed[position]
                && let Some(index) = index_literal(self, tables, index)
                && let Some(index) = match index {
                    semantic_vocabulary::IntegerValue::Unsigned(n) => u64::try_from(n).ok(),
                    semantic_vocabulary::IntegerValue::Signed(n) => u64::try_from(n).ok(),
                }
            {
                observed[position] = Static::FixedIndex(index).into();
            }
        }
        Ok(ScalarTerm::structural_read(actual.root, observed, ty).ok())
    }
}

impl AtCall<'_, '_> {
    pub(super) fn bind_call_alignment(
        &mut self,
        actual: &Lineage,
        formal: &terminal_psi::StructuralParameterDeclaration,
        location: &[Projection],
        ty: ScalarType,
    ) -> Result<ScalarTerm, crate::Unbound> {
        // Lending a shared view captures its descriptor. Indexing that copy
        // still measures the backing of the original descriptor.
        if actual.segments.is_empty()
            && matches!(
                location.first(),
                Some(Projection::Index(_) | Projection::Static(Static::FixedIndex(_)))
            )
            && is_shared_view(
                &self
                    .storage
                    .declaration(formal.structural_type, self.concerns)
                    .map_err(crate::Unbound::Refused)?
                    .shape,
            )
            && let Reached::Input(source) = self.reaching_whole_root(actual.root)?
            && source != actual.root
        {
            let term = self.storage.alignment_at_call(
                self.tables,
                &Lineage {
                    root: source,
                    segments: Vec::new(),
                },
                formal,
                location,
                ty,
            );
            if !matches!(term, Err(crate::Unbound::Unproved(_))) {
                return term;
            }
        }
        self.storage
            .alignment_at_call(self.tables, actual, formal, location, ty)
    }
}
