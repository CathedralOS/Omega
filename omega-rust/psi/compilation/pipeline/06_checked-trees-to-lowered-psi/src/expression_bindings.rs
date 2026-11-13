/*
Storage binding resolution, structural path projection, owned argument preparation, and structural
field custody for source-bound expressions during `expression_preparation`.
*/

pub(crate) use crate::expression_preparation::*;

pub(crate) mod bindings {

    use super::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
        CheckedTrees, LoweringError, PlaceId, PrimitiveType, ScalarType, StructuralAccess,
        StructuralArgument, StructuralParameterDeclaration, StructuralTypeDeclaration,
        StructuralTypeId, terminal_scalar_type, unsupported,
    };
    use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
    use crate::emission::operation_emission::calls::PreparedStructuralArgument;
    use crate::emission::operation_emission::expressions::LoweredDirectExpression;
    use crate::emission::operation_emission::expressions::PrimitiveReadSource;
    use crate::expression_preparation::bindings::place_observations::PlaceObservation;
    use crate::shared::checked_facts::value_facts;
    use terminal_psi::find_structural_type;

    pub(crate) mod published_types {
        use super::{LoweringError, PlaceId, StructuralTypeId};
        use crate::emission::operation_emission::buffer::OperationBuffer;

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) struct PublishedStructuralTypes {
            homes: Vec<(PlaceId, StructuralTypeId)>,
        }

        impl PublishedStructuralTypes {
            pub(crate) fn from_rows(
                rows: impl IntoIterator<Item = (PlaceId, StructuralTypeId)>,
            ) -> Result<Self, LoweringError> {
                let mut homes = rows.into_iter().collect::<Vec<_>>();
                homes.sort_unstable_by_key(|(place, _)| *place);
                for pair in homes.windows(2) {
                    if pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1 {
                        return Err(LoweringError::ConflictingStructuralPlaceType {
                            place: pair[0].0,
                            first: pair[0].1,
                            second: pair[1].1,
                        });
                    }
                }
                homes.dedup();
                Ok(Self { homes })
            }

            pub(crate) fn homes(&self) -> impl Iterator<Item = (PlaceId, StructuralTypeId)> + '_ {
                self.homes.iter().copied()
            }

            pub(crate) fn get(&self, place: PlaceId) -> Option<StructuralTypeId> {
                self.homes
                    .binary_search_by_key(&place, |(place, _)| *place)
                    .ok()
                    .map(|index| self.homes[index].1)
            }

            pub(crate) fn root_type(
                &self,
                argument: &terminal_psi::StructuralArgument,
            ) -> Result<StructuralTypeId, LoweringError> {
                self.get(argument.place)
                    .ok_or_else(|| LoweringError::MissingCrashArgumentSource {
                        argument: argument.clone(),
                    })
            }

            fn retain(
                &mut self,
                place: PlaceId,
                carrier: StructuralTypeId,
            ) -> Result<(), LoweringError> {
                match self.homes.binary_search_by_key(&place, |(place, _)| *place) {
                    Ok(index) if self.homes[index].1 != carrier => {
                        Err(LoweringError::ConflictingStructuralPlaceType {
                            place,
                            first: self.homes[index].1,
                            second: carrier,
                        })
                    }
                    Ok(_) => Ok(()),
                    Err(index) => {
                        self.homes.insert(index, (place, carrier));
                        Ok(())
                    }
                }
            }

            pub(crate) fn with_completed(
                mut self,
                operations: &OperationBuffer,
            ) -> Result<Self, LoweringError> {
                for result in operations
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.structural())
                {
                    self.retain(result.place, result.structural_type)?;
                }
                for (_, binding) in &operations.structural_values {
                    if binding.path().is_empty() {
                        self.retain(binding.place(), binding.structural_type())?;
                    }
                }
                for (place, carrier) in &operations.literal_types {
                    self.retain(*place, *carrier)?;
                }
                for (place, carrier) in &operations.captured_place_types {
                    self.retain(*place, *carrier)?;
                }
                Ok(self)
            }
        }
    }
    pub(crate) use published_types::PublishedStructuralTypes;
    pub(crate) mod formal_observations {
        use super::{
            AuthoredScalarBinding, LoweringError, ScalarBindings, ScalarType, StructuralArgument,
            StructuralTypeId, unsupported,
        };
        use crate::emission::structural_observations::explicit_path;
        use symbols::SymbolHandle;

        #[derive(Clone)]
        pub(crate) struct CapturedStructuralFormal {
            pub(crate) source_position: u32,
            pub(crate) symbol: SymbolHandle,
            pub(crate) argument: StructuralArgument,
            pub(crate) structural_type: StructuralTypeId,
        }

        impl ScalarBindings {
            pub(crate) fn capture_formal_observations(
                &self,
                scalar_formals: &[(usize, ScalarType)],
                structural_formals: &[CapturedStructuralFormal],
            ) -> Result<Self, LoweringError> {
                for (position, formal) in structural_formals.iter().enumerate() {
                    if !formal.symbol.is_valid()
                        || structural_formals[..position].iter().any(|earlier| {
                            earlier.symbol == formal.symbol
                                || earlier.source_position == formal.source_position
                        })
                        || self.structural_argument_type(&formal.argument)
                            != Some(formal.structural_type)
                    {
                        return unsupported(
                            "captured formal observation lost its exact typed actual",
                        );
                    }
                }
                let mut frame = self.clone();
                frame.authored = scalar_formals
                    .iter()
                    .map(|(value_position, scalar_type)| {
                        Some(AuthoredScalarBinding {
                            value_position: *value_position,
                            scalar_type: *scalar_type,
                            storage_symbol: SymbolHandle::invalid(),
                        })
                    })
                    .collect();
                frame.immutable = frame
                    .authored
                    .iter()
                    .map(|binding| binding.map(|row| row.value_position))
                    .collect();
                frame.parameter_count = scalar_formals.len();
                frame.primitive_storage.clear();
                frame.scalar_parameter_homes.clear();
                frame.captured_structural_formals = structural_formals.to_vec();
                Ok(frame)
            }

            pub(crate) fn captured_storage_home(
                &self,
                root: checked_trees::CheckedStorageRoot,
            ) -> Result<Option<(StructuralArgument, StructuralTypeId)>, LoweringError> {
                let mut observations = self
                    .captured_storage_observations
                    .iter()
                    .filter(|(candidate, _, _)| *candidate == root);
                if let Some((_, argument, structural_type)) = observations.next() {
                    if observations.next().is_some() {
                        return unsupported("storage observation has conflicting captured homes");
                    }
                    return Ok(Some((argument.clone(), *structural_type)));
                }
                use checked_trees::CheckedStorageRoot as Root;
                if self.captured_structural_formals.is_empty() {
                    return Ok(None);
                }
                let formal = match root {
                    Root::Parameter { index } => Some(
                        self.captured_structural_formals
                            .iter()
                            .find(|formal| formal.source_position == index)
                            .ok_or(LoweringError::Unsupported(
                                "captured storage observation lost its authored formal coordinate",
                            ))?,
                    ),
                    Root::OwnedLocal { symbol } | Root::ViewLocal { symbol } => self
                        .captured_structural_formals
                        .iter()
                        .find(|formal| formal.symbol == symbol),
                };
                Ok(formal.map(|formal| (formal.argument.clone(), formal.structural_type)))
            }

            pub(crate) fn captured_field_observation(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                position: u32,
                path: &[checked_trees::CheckedStructuralPredicatePathSegment],
                scalar_type: ScalarType,
            ) -> Result<
                Option<crate::emission::operation_emission::expressions::LoweredDirectExpression>,
                LoweringError,
            > {
                let Some((source, structural_type)) =
                    self.captured_storage_home(checked_trees::CheckedStorageRoot::Parameter {
                        index: position,
                    })?
                else {
                    return Ok(None);
                };
                if !matches!(
                    source.access,
                    terminal_psi::StructuralAccess::Owned
                        | terminal_psi::reading_borrow_pair_pattern!()
                ) {
                    return unsupported("captured field observation requires readable access");
                }
                let declarations = self.structural_fields.declarations();
                let projection = explicit_path(identities, structural_type, path, declarations)?;
                if projection.scalar_type() != Some(scalar_type) {
                    return unsupported(
                        "captured field observation changes its declared scalar type",
                    );
                }
                let prepared = super::structural_paths::prepare_argument_path(
                    identities,
                    &projection.segments,
                    structural_type,
                    declarations,
                )?;
                if let Some(field) = projection.scalar_field
                    && let Some(mut suffix) = projection.canonical
                {
                    let prefix = if source.path.is_empty() {
                        Some(Vec::new())
                    } else {
                        self.structural_place_type(source.place).and_then(|root| {
                            terminal_semantics::canonical_static_projection(
                                declarations.iter(),
                                root,
                                &source.path,
                            )
                            .map(|(path, _)| path)
                        })
                    };
                    if let Some(mut prefix) = prefix {
                        suffix.pop();
                        prefix.extend(suffix);
                        return Ok(Some(super::structural_fields::canonical_field_read(
                            source.place.into(),
                            prefix,
                            field.id,
                            scalar_type,
                        )));
                    }
                }
                let mut physical_path = source.path;
                for segment in prepared {
                    let super::structural_paths::PreparedStructuralPathSegment::Static(segment) =
                        segment
                    else {
                        return unsupported(
                            "captured field observation requires its retained runtime selectors",
                        );
                    };
                    physical_path.push(segment);
                }
                Ok(Some(crate::emission::operation_emission::expressions::LoweredDirectExpression::PrimitiveRead {
                    qualifications: Default::default(), source: source.place.into(), path: physical_path, scalar_type,
                }))
            }

            pub(crate) fn captured_formal_home(
                &self,
                argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            ) -> Result<Option<StructuralArgument>, LoweringError> {
                use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;
                let parameter_index = match argument.source {
                    Source::Parameter { parameter_index }
                    | Source::ByteSequenceSubslice {
                        root:
                            checked_trees::CheckedStorageRoot::Parameter {
                                index: parameter_index,
                            },
                        ..
                    }
                    | Source::ElementViewSubslice {
                        root:
                            checked_trees::CheckedStorageRoot::Parameter {
                                index: parameter_index,
                            },
                        ..
                    } => Some(parameter_index),
                    _ => None,
                };
                if let Some(index) = parameter_index {
                    if self.captured_structural_formals.is_empty() {
                        return Ok(None);
                    }
                    return self
                        .captured_structural_formals
                        .get(index as usize)
                        .map(|formal| Some(formal.argument.clone()))
                        .ok_or(LoweringError::Unsupported(
                            "captured formal observation lost its dense structural coordinate",
                        ));
                }
                let symbol = argument
                    .structural_storage_root()
                    .and_then(|root| root.as_symbol());
                Ok(symbol.and_then(|symbol| {
                    self.captured_structural_formals
                        .iter()
                        .find(|formal| formal.symbol == symbol)
                        .map(|formal| formal.argument.clone())
                }))
            }
        }
    }
    pub(crate) mod owned_arguments {

        use super::{
            LoweringError, ScalarBindings, StructuralAccess, StructuralArgument,
            StructuralLocalBinding, structural_paths, unsupported, valid_structural_root,
        };

        impl ScalarBindings {
            pub(crate) fn owned_argument(
                &self,
                identities: &mut terminal_psi::TerminalIdentityTableBuilder,
                argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            ) -> Result<StructuralArgument, LoweringError> {
                let mut source = self.owned_argument_home(argument)?;
                let selected_root =
                    self.structural_argument_type(&source)
                        .ok_or(LoweringError::Unsupported(
                            "owned argument lost its published carrier",
                        ))?;
                source
                    .path
                    .extend(structural_paths::lower_static_argument_path(
                        identities,
                        &argument.path,
                        selected_root,
                        self.structural_fields.declarations(),
                    )?);
                Ok(source)
            }

            pub(crate) fn owned_argument_home(
                &self,
                argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            ) -> Result<StructuralArgument, LoweringError> {
                if let Some(captured) = self.captured_formal_home(argument)? {
                    if argument.access != checked_trees::CheckedStructuralAccess::Owned
                        || captured.access != StructuralAccess::Owned
                    {
                        return unsupported("captured owned operand changes its source custody");
                    }
                    return Ok(captured);
                }
                let root = argument.structural_storage_root();
                let ordinal = argument.source_structural_result_binding_ordinal();
                if root.is_some() || ordinal.is_some() {
                    let mut locals = self.structural_locals.iter().filter(|row| {
                        root.is_some_and(|root| row.root == root)
                            || ordinal
                                .is_some_and(|ordinal| row.result_binding_ordinal == Some(ordinal))
                    });
                    let StructuralLocalBinding {
                        argument: source, ..
                    } = locals.next().ok_or(LoweringError::Unsupported(
                        "computed owned structural operand lost its established local",
                    ))?;
                    if root.is_some_and(|root| !valid_structural_root(root))
                        || locals.next().is_some()
                        || argument.access != checked_trees::CheckedStructuralAccess::Owned
                        || source.access != StructuralAccess::Owned
                    {
                        return unsupported(
                            "computed owned structural operand changes its source custody",
                        );
                    }
                    return Ok(source.clone());
                }
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } = argument.source
                else {
                    return unsupported("computed owned operand requires an existing parameter");
                };
                let (_, parameter) = self
                    .structural_parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "computed owned operand lost its source parameter",
                    ))?;
                if argument.access != checked_trees::CheckedStructuralAccess::Owned
                    || parameter.access != StructuralAccess::Owned
                {
                    return unsupported("computed owned operand changes its source custody");
                }
                Ok(StructuralArgument {
                    place: parameter.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                })
            }
        }
    }
    pub(crate) use formal_observations::CapturedStructuralFormal;
    pub(crate) mod primitive_reads {

        use super::{
            LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralLocalBinding,
            StructuralTypeDeclaration, primitive_storage_place, unsupported,
        };
        use crate::expression_preparation::representation_view::replay;
        use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

        pub(crate) fn primitive_storage_read(
            storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
            locals: &[StructuralLocalBinding],
            declarations: &[StructuralTypeDeclaration],
            symbol: symbols::SymbolHandle,
            scalar_type: ScalarType,
        ) -> Result<(PlaceId, Vec<StructuralPathSegment>), LoweringError> {
            let (place, path, declared_type) =
                primitive_storage_source(storage, locals, declarations, symbol)?;
            if declared_type != scalar_type {
                return unsupported("primitive storage read changes its declared endpoint type");
            }
            Ok((place, path))
        }

        pub(crate) fn primitive_storage_source(
            storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
            locals: &[StructuralLocalBinding],
            declarations: &[StructuralTypeDeclaration],
            symbol: symbols::SymbolHandle,
        ) -> Result<(PlaceId, Vec<StructuralPathSegment>, ScalarType), LoweringError> {
            if !symbol.is_valid() {
                return unsupported("primitive storage read has no source identity");
            }
            let mut matching = locals
                .iter()
                .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol));
            let local = matching.next();
            if matching.next().is_some()
                || (local.is_some() && storage.iter().any(|row| row.0 == symbol))
            {
                return unsupported("primitive storage read has ambiguous established storage");
            }
            let Some(local) = local else {
                let mut rows = storage.iter().filter(|row| row.0 == symbol);
                let scalar_type = rows
                    .next()
                    .ok_or(LoweringError::Unsupported(
                        "primitive storage read has no initialized place",
                    ))?
                    .2;
                if rows.next().is_some() {
                    return unsupported("primitive storage read identity is missing or duplicated");
                }
                return Ok((
                    primitive_storage_place(storage, symbol, scalar_type)?,
                    Vec::new(),
                    scalar_type,
                ));
            };
            if local.argument.access == StructuralAccess::WriteOnlyBorrow
                || !local.argument.path.is_empty()
            {
                return unsupported(
                    "primitive storage read changes its established carrier custody",
                );
            }
            let declaration = |identity| {
                let mut rows = declarations.iter().filter(|row| row.id == identity);
                let result = rows.next();
                if rows.next().is_some() { None } else { result }
            };
            let root = declaration(local.structural_type).ok_or(LoweringError::Unsupported(
                "primitive storage read has no unique carrier declaration",
            ))?;
            let (observed, path) = match root.shape {
                StructuralTypeShape::Reference { referent, access } => {
                    if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
                        return unsupported(
                            "primitive storage read crosses an unreadable reference",
                        );
                    }
                    (
                        declaration(referent).ok_or(LoweringError::Unsupported(
                            "primitive storage read has no unique referent declaration",
                        ))?,
                        vec![StructuralPathSegment::Referent],
                    )
                }
                _ => (root, Vec::new()),
            };
            let StructuralTypeShape::PrimitiveScalar(scalar_type) = observed.shape else {
                return unsupported("primitive storage read has no declared scalar endpoint");
            };
            Ok((local.argument.place, path, scalar_type))
        }

        pub(crate) fn captured_observation(
            bindings: &super::ScalarBindings,
            symbol: symbols::SymbolHandle,
            scalar_type: ScalarType,
        ) -> Result<
            Option<crate::emission::operation_emission::expressions::LoweredDirectExpression>,
            LoweringError,
        > {
            use super::place_observations::ObservationKind;
            use crate::emission::operation_emission::expressions::LoweredDirectExpression as Direct;
            let mut matching = bindings
                .place_observations
                .iter()
                .filter(|loan| loan.symbol == symbol);
            let Some(loan) = matching.next() else {
                return Ok(None);
            };
            if !symbol.is_valid() || matching.next().is_some() {
                return unsupported("primitive observation has ambiguous captured storage");
            }
            let capture = loan.capture.as_ref().map_err(Clone::clone)?;
            let ObservationKind::Primitive(observation) = &capture.observation else {
                return Ok(None);
            };
            if observation.viewed_type != scalar_type
                || bindings
                    .structural_locals
                    .iter()
                    .any(|row| row.root == facts::PlaceRoot::Symbol(symbol))
                || bindings.primitive_storage.iter().any(|row| row.0 == symbol)
            {
                return unsupported(
                    "primitive observation changes its captured view or carrier identity",
                );
            }
            let source = bindings.observation_backing(capture.source)?;
            let read = if capture.runtime_index.is_some() {
                let index = Box::new(bindings.observation_coordinate(symbol)?);
                if observation.view_container {
                    return unsupported(
                        "captured primitive observation requires fixed-array storage",
                    );
                }
                Direct::IndexedPrimitiveRead {
                    qualifications: Default::default(),
                    source,
                    path: capture.carrier_path.clone(),
                    index,
                    extent: None,
                    element_path: observation.element_path.iter().cloned().map(
                        crate::emission::operation_emission::expressions::LoweredElementPathSegment::Static,
                    ).collect(),
                    scalar_type: observation.scalar_type,
                    bound_decided: false,
                }
            } else if let Some(field) = observation.field {
                match observation.scalar_type {
                    ScalarType::Boolean => Direct::Boolean {
                        expression: Box::new(crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression::StructuralField {
                            qualifications: Default::default(),
                            source: source.into(),
                            path: observation.path.clone(),
                            field,
                        }),
                    },
                    ScalarType::Integer(_) | ScalarType::Callable(_) => Direct::StructuralField {
                        qualifications: Default::default(),
                        source: source.into(),
                        path: observation.path.clone(),
                        field,
                        scalar_type: observation.scalar_type,
                    },
                    ScalarType::IeeeFloat(_) => Direct::PrimitiveRead {
                        qualifications: Default::default(),
                        source: source.into(),
                        path: capture.carrier_path.clone(),
                        scalar_type: observation.scalar_type,
                    },
                }
            } else if observation.scalar_type != observation.viewed_type
                && matches!(
                    capture.carrier_path.last(),
                    Some(StructuralPathSegment::FixedIndex(_))
                )
            {
                let Some((StructuralPathSegment::FixedIndex(index), path)) =
                    capture.carrier_path.split_last()
                else {
                    unreachable!()
                };
                Direct::IndexedPrimitiveRead {
                    qualifications: Default::default(),
                    source,
                    path: path.to_vec(),
                    index: Box::new(Direct::IntegerLiteral {
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                        scalar_type: super::terminal_scalar_type(
                            typed_trees::types::PrimitiveType::U64,
                        )?,
                    }),
                    extent: None,
                    element_path: Vec::new(),
                    scalar_type: observation.scalar_type,
                    bound_decided: true,
                }
            } else {
                Direct::PrimitiveRead {
                    qualifications: Default::default(),
                    source: source.into(),
                    path: capture.carrier_path.clone(),
                    scalar_type: observation.scalar_type,
                }
            };
            Ok(Some(
                if observation.scalar_type == observation.viewed_type {
                    read
                } else {
                    replay(read, observation.viewed_type)?
                },
            ))
        }
    }
    pub(crate) use primitive_reads::primitive_storage_read;
    pub(crate) mod place_observations {
        use super::super::{
            CheckedTrees, LoweringError, PrimitiveType, ScalarType, terminal_scalar_type,
            unsupported,
        };
        use crate::emission::operation_emission::expressions::LoweredDirectExpression;

        #[derive(Clone)]
        pub(crate) struct ByteOverlayField {
            pub offset: u64,
            pub stored_type: ScalarType,
            pub indices: Vec<ByteOverlayIndex>,
        }

        #[derive(Clone)]
        pub(crate) struct ByteOverlayIndex {
            pub selector: checked_trees::CheckedRuntimeIndex,
            pub stride: u64,
            pub extent: u64,
        }

        #[derive(Clone)]
        pub(crate) struct ByteOverlaySource {
            pub(crate) referee: typed_trees::types::TypeReferenceHandle,
        }

        impl ByteOverlaySource {
            pub(crate) fn field(
                &self,
                checked: &CheckedTrees,
                path: &[checked_trees::CheckedUnitStructuralPathSegment],
                field: symbols::SymbolHandle,
                scalar_type: ScalarType,
            ) -> Result<ByteOverlayField, LoweringError> {
                let projection =
                    validation::recast_scalar_projection(&checked.typed, self.referee, path, field)
                        .ok_or(LoweringError::Unsupported(
                            "record byte overlay lost its checked projection geometry",
                        ))?;
                self.projection(projection, scalar_type)
            }

            pub(crate) fn scalar_path(
                &self,
                checked: &CheckedTrees,
                path: &[checked_trees::CheckedUnitStructuralPathSegment],
                scalar_type: ScalarType,
            ) -> Result<ByteOverlayField, LoweringError> {
                let geometry =
                    validation::recast_scalar_path_geometry(&checked.typed, self.referee, path)
                        .ok_or(LoweringError::Unsupported(
                            "record byte overlay lost its checked projection geometry",
                        ))?;
                self.geometry(geometry, path, scalar_type)
            }

            pub(crate) fn geometry(
                &self,
                geometry: validation::RecastScalarGeometry,
                path: &[checked_trees::CheckedUnitStructuralPathSegment],
                scalar_type: ScalarType,
            ) -> Result<ByteOverlayField, LoweringError> {
                let mut field = self.projection(geometry.projection, scalar_type)?;
                let selectors = path.iter().filter_map(|segment| match segment {
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
                        Some(selector)
                    }
                    _ => None,
                });
                for (index, selector) in geometry.indices.into_iter().zip(selectors) {
                    field.indices.push(ByteOverlayIndex {
                        selector: selector.clone(),
                        stride: u64::try_from(index.stride).map_err(|_| {
                            LoweringError::Unsupported("record array stride exceeds u64")
                        })?,
                        extent: u64::try_from(index.extent).map_err(|_| {
                            LoweringError::Unsupported("record array extent exceeds u64")
                        })?,
                    });
                }
                Ok(field)
            }

            pub(crate) fn projection(
                &self,
                projection: validation::RecastScalarProjection,
                scalar_type: ScalarType,
            ) -> Result<ByteOverlayField, LoweringError> {
                Ok(ByteOverlayField {
                    offset: u64::try_from(projection.offset).map_err(|_| {
                        LoweringError::Unsupported("record byte overlay offset exceeds u64")
                    })?,
                    stored_type: projection
                        .stored_integer
                        .map(ScalarType::Integer)
                        .unwrap_or(scalar_type),
                    indices: Vec::new(),
                })
            }
        }

        #[derive(Clone, Copy)]
        pub(crate) enum LoanedSource {
            Parameter(u32),
            Local(symbols::SymbolHandle),
        }

        #[derive(Clone)]
        pub(crate) struct PlaceObservation {
            pub(crate) statement_index: u32,
            pub(crate) symbol: symbols::SymbolHandle,
            pub(crate) loan: checked_trees::CheckedPlaceLoanPlan,
            pub(crate) selectors: Vec<semantic_vocabulary::PlaceId>,
            pub(crate) capture: Result<PlaceObservationCapture, LoweringError>,
        }

        #[derive(Clone)]
        pub(crate) struct PlaceObservationCapture {
            pub(crate) coordinate: Option<semantic_vocabulary::PlaceId>,
            pub(crate) source: LoanedSource,
            pub(crate) carrier_path: Vec<terminal_psi::StructuralPathSegment>,
            pub(crate) runtime_index: Option<checked_trees::CheckedRuntimeIndex>,
            pub(crate) observation: ObservationKind,
        }

        #[derive(Clone)]
        pub(crate) enum ObservationKind {
            Structural,
            ByteRecord {
                overlay: ByteOverlaySource,
                layout: validation::RecastScalarLayout,
            },
            Primitive(PrimitiveObservation),
        }

        #[derive(Clone)]
        pub(crate) struct PrimitiveObservation {
            pub(crate) scalar_type: ScalarType,
            pub(crate) viewed_type: ScalarType,
            pub(crate) field: Option<semantic_vocabulary::StructuralFieldId>,
            pub(crate) path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
            pub(crate) element_path: Vec<terminal_psi::StructuralPathSegment>,
            pub(crate) view_container: bool,
        }

        pub(crate) fn byte_record_observation(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            loans: &[PlaceObservation],
            bindings: &super::ScalarBindings,
            proven_indexes: &[checked_trees::expression::ExpressionHandle],
            symbol: symbols::SymbolHandle,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            index: &checked_trees::CheckedScalarExpression,
            element_path: &[checked_trees::CheckedScalarReadProjection],
            primitive: PrimitiveType,
        ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
            use checked_trees::{
                CheckedScalarExpression, CheckedUnitStructuralPathSegment as Projection,
            };
            let mut matching = loans.iter().filter(|loan| loan.symbol == symbol);
            let Some(loan) = matching.next() else {
                return Ok(None);
            };
            if !symbol.is_valid() || matching.next().is_some() {
                return unsupported("record byte observation has ambiguous loan identity");
            }
            let loan = loan.capture.as_ref().map_err(Clone::clone)?;
            let ObservationKind::ByteRecord { overlay, layout } = &loan.observation else {
                return Ok(None);
            };
            let mut selectors = Vec::new();
            let mut leaf_path = Vec::new();
            append_observation_path(identities, bindings, path, &mut leaf_path, &mut selectors)?;
            if let CheckedScalarExpression::IntegerLiteral { literal } = index {
                leaf_path.push(Projection::FixedIndex(literal.value_u64().ok_or(
                    LoweringError::Unsupported(
                        "record byte observation has an invalid array index",
                    ),
                )?));
            } else {
                append_coordinate(
                    bindings.expression(identities, proven_indexes, index)?,
                    &mut leaf_path,
                    &mut selectors,
                )?;
            }
            for step in element_path {
                match step {
                    checked_trees::CheckedScalarReadProjection::Static(segment) => {
                        append_observation_path(
                            identities,
                            bindings,
                            std::slice::from_ref(segment),
                            &mut leaf_path,
                            &mut selectors,
                        )?
                    }
                    checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                        append_coordinate(
                            bindings.expression(
                                identities,
                                proven_indexes,
                                &selector.expression,
                            )?,
                            &mut leaf_path,
                            &mut selectors,
                        )?
                    }
                }
            }
            let scalar_type = terminal_scalar_type(primitive)?;
            let geometry =
                layout
                    .scalar_path_geometry(&leaf_path)
                    .ok_or(LoweringError::Unsupported(
                        "record byte observation lost its normalized scalar leaf",
                    ))?;
            let projection = overlay.geometry(geometry, &leaf_path, scalar_type)?;
            let source = bindings.observation_backing(loan.source)?;
            let (prefix, coordinate) = match &loan.runtime_index {
                Some(_) => (
                    loan.carrier_path.as_slice(),
                    bindings.observation_coordinate(symbol)?,
                ),
                None => {
                    let Some((terminal_psi::StructuralPathSegment::FixedIndex(index), prefix)) =
                        loan.carrier_path.split_last()
                    else {
                        return unsupported(
                            "record byte observation lost its captured byte coordinate",
                        );
                    };
                    (
                        prefix,
                        LoweredDirectExpression::IntegerLiteral {
                            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                            scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                        },
                    )
                }
            };
            let mut coordinate = coordinate;
            for (ordinal, (index, selector)) in projection.indices.iter().zip(selectors).enumerate()
            {
                if index.selector
                    != (checked_trees::CheckedRuntimeIndex::AssignmentIndex {
                        depth: u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("record byte coordinate ordinal overflow")
                        })?,
                    })
                {
                    return unsupported(
                        "record byte observation reordered its runtime coordinates",
                    );
                }
                let selector = bounded_coordinate(selector, index.extent)?;
                let scaled = integer_coordinate_binary(
                    crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactMultiply,
                    index.extent.checked_mul(index.stride).is_some(),
                    selector,
                    coordinate_literal(index.stride),
                );
                coordinate = integer_coordinate_binary(
                    crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactAdd,
                    true,
                    coordinate,
                    scaled,
                );
            }
            byte_overlay_read(
                source,
                prefix,
                coordinate,
                projection.offset,
                projection.stored_type,
                scalar_type,
            )
            .map(Some)
        }

        pub(crate) fn byte_overlay_read(
            source: semantic_vocabulary::PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
            index: LoweredDirectExpression,
            offset: u64,
            stored_type: ScalarType,
            scalar_type: ScalarType,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let index_type = index.scalar_type();
            let index = if offset == 0 {
                index
            } else {
                LoweredDirectExpression::IntegerBinary {
                    kind: crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactAdd,
                    scalar_type: index_type,
                    left: Box::new(index),
                    right: Box::new(LoweredDirectExpression::IntegerLiteral {
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(offset)),
                        scalar_type: index_type,
                    }),
                    formation_decided: false,
                }
            };
            let byte_type = terminal_scalar_type(PrimitiveType::U8)?;
            let first_byte = LoweredDirectExpression::IndexedPrimitiveRead {
                qualifications: Default::default(),
                source,
                path: path.to_vec(),
                index: Box::new(index),
                extent: None,
                element_path: Vec::new(),
                scalar_type: byte_type,
                bound_decided: false,
            };
            let stored = if stored_type == byte_type {
                first_byte
            } else {
                crate::expression_preparation::representation_view::replay(first_byte, stored_type)?
            };
            if stored_type == scalar_type {
                return Ok(stored);
            }
            match (stored_type, scalar_type) {
                (ScalarType::Integer(source_type), ScalarType::Integer(target_type))
                    if source_type.can_widen_to(target_type) =>
                {
                    Ok(LoweredDirectExpression::IntegerWiden {
                        scalar_type,
                        operand: Box::new(stored),
                    })
                }
                _ => unsupported("record byte overlay cannot decode its stored integer carrier"),
            }
        }

        pub(crate) fn bounded_coordinate(
            selector: LoweredDirectExpression,
            extent: u64,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactSubtract;
            let maximum = extent.checked_sub(1).ok_or(LoweringError::Unsupported(
                "record byte observation selects an empty array",
            ))?;
            let selector =
                crate::expression_preparation::prepare_expression::exact_u64_index(selector)?;
            Ok(integer_coordinate_binary(
                ExactSubtract,
                true,
                coordinate_literal(maximum),
                integer_coordinate_binary(
                    ExactSubtract,
                    false,
                    coordinate_literal(maximum),
                    selector,
                ),
            ))
        }

        fn coordinate_literal(value: u64) -> LoweredDirectExpression {
            LoweredDirectExpression::IntegerLiteral {
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(value)),
                scalar_type: ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .expect("u64 is an admitted scalar carrier"),
                ),
            }
        }

        fn integer_coordinate_binary(
            kind: crate::emission::operation_emission::integer::LoweredIntegerBinaryKind,
            formation_decided: bool,
            left: LoweredDirectExpression,
            right: LoweredDirectExpression,
        ) -> LoweredDirectExpression {
            LoweredDirectExpression::IntegerBinary {
                kind,
                scalar_type: left.scalar_type(),
                left: Box::new(left),
                right: Box::new(right),
                formation_decided,
            }
        }

        fn append_coordinate(
            selector: LoweredDirectExpression,
            path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
            selectors: &mut Vec<LoweredDirectExpression>,
        ) -> Result<(), LoweringError> {
            let depth = u32::try_from(selectors.len()).map_err(|_| {
                LoweringError::Unsupported("record byte observation has too many coordinates")
            })?;
            path.push(
                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                ),
            );
            selectors.push(selector);
            Ok(())
        }

        fn append_observation_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            bindings: &super::ScalarBindings,
            supplied: &[checked_trees::CheckedStructuralPredicatePathSegment],
            path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
            selectors: &mut Vec<LoweredDirectExpression>,
        ) -> Result<(), LoweringError> {
            use checked_trees::{
                CheckedRuntimeIndex, CheckedStructuralPredicatePathSegment as Predicate,
                CheckedUnitStructuralPathSegment as Projection,
            };
            for segment in supplied {
                match segment {
                    Predicate::Referent => path.push(Projection::Referent),
                    Predicate::Field(field) => path.push(Projection::Field(field.clone())),
                    Predicate::FixedIndex(index) => path.push(Projection::FixedIndex(*index)),
                    Predicate::RuntimeIndex(index) => {
                        let selector = match index {
                            CheckedRuntimeIndex::StorageRead { symbol } => {
                                bindings.runtime_index_storage(*symbol)?
                            }
                            CheckedRuntimeIndex::Parameter { position } => {
                                bindings.runtime_index_parameter(*position)?
                            }
                            CheckedRuntimeIndex::Local { position } => {
                                bindings.runtime_index_local(*position)?
                            }
                            CheckedRuntimeIndex::StructuralField(field) => {
                                bindings.runtime_index_field(identities, field)?
                            }
                            CheckedRuntimeIndex::AssignmentIndex { .. } => {
                                return unsupported(
                                    "record byte observation has an assignment-only coordinate",
                                );
                            }
                        };
                        append_coordinate(selector, path, selectors)?;
                    }
                    Predicate::Case(_) => {
                        return unsupported(
                            "record byte observation cannot reinterpret a case payload",
                        );
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) mod structural_cases {

        use super::super::{StructuralTypeId, StructuralTypeShape};
        use super::{
            LoweringError, PlaceId, StructuralAccess, StructuralParameterDeclaration,
            StructuralTypeDeclaration, unsupported,
        };
        use crate::emission::structural_observations::explicit_path;
        use crate::shared::structural_types::unique_declaration;

        #[derive(Clone)]
        pub(crate) struct StructuralCaseBinding {
            source_position: u32,
            source: PlaceId,
            structural_type: StructuralTypeId,
            declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
        }

        impl StructuralCaseBinding {
            pub(crate) fn collect(
                parameters: &[(u32, StructuralParameterDeclaration)],
                types: &[StructuralTypeDeclaration],
            ) -> Vec<Self> {
                let declarations: std::sync::Arc<[StructuralTypeDeclaration]> = types.into();
                let mut bindings = Vec::new();
                for (position, parameter) in parameters {
                    if parameter.access == StructuralAccess::WriteOnlyBorrow {
                        continue;
                    }
                    bindings.push(Self {
                        source_position: *position,
                        source: parameter.place,
                        structural_type: parameter.structural_type,
                        declarations: std::sync::Arc::clone(&declarations),
                    });
                }
                bindings
            }
        }

        pub(crate) fn resolve_at(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            declarations: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            steps: &[checked_trees::CheckedUnitStructuralPathSegment],
            identity: &checked_trees::CheckedMemberIdentity,
        ) -> Result<
            (
                Vec<terminal_psi::StructuralPathSegment>,
                semantic_vocabulary::StructuralCaseId,
            ),
            LoweringError,
        > {
            let path = prepare_case_path(identities, declarations, structural_type, steps)?;
            resolve_emitted_path(identities, declarations, structural_type, path, identity)
        }

        pub(crate) fn resolve_observation_at(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            declarations: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            steps: &[checked_trees::CheckedUnitStructuralPathSegment],
            identity: &checked_trees::CheckedMemberIdentity,
        ) -> Result<
            (
                Vec<terminal_psi::StructuralPathSegment>,
                semantic_vocabulary::StructuralCaseId,
            ),
            LoweringError,
        > {
            let mut path = prepare_case_path(identities, declarations, structural_type, steps)?;
            super::structural_paths::complete_read_observation_path(
                structural_type,
                &mut path,
                declarations,
            )?;
            resolve_emitted_path(identities, declarations, structural_type, path, identity)
        }

        fn prepare_case_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            declarations: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            steps: &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
            let prepared = super::structural_paths::prepare_argument_path(
                identities,
                steps,
                structural_type,
                declarations,
            )?;
            super::structural_paths::emit_argument_path(
                &prepared,
                None,
                &mut std::iter::empty(),
                None,
            )
        }

        fn resolve_emitted_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            declarations: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            identity: &checked_trees::CheckedMemberIdentity,
        ) -> Result<
            (
                Vec<terminal_psi::StructuralPathSegment>,
                semantic_vocabulary::StructuralCaseId,
            ),
            LoweringError,
        > {
            for (position, segment) in path.iter().enumerate() {
                if *segment != terminal_psi::StructuralPathSegment::Referent {
                    continue;
                }
                let reference = terminal_semantics::runtime_structural_path_tip(
                    declarations.iter(),
                    structural_type,
                    &path[..position],
                )
                .ok_or(LoweringError::Unsupported(
                    "case observation lost its exact reference carrier",
                ))?;
                if !matches!(
                    reference.shape,
                    StructuralTypeShape::Reference {
                        access: terminal_psi::reading_borrow_pair_pattern!(),
                        ..
                    }
                ) {
                    return unsupported("case observation requires a readable reference crossing");
                }
            }
            let sum = terminal_semantics::runtime_structural_path_tip(
                declarations.iter(),
                structural_type,
                &path,
            )
            .ok_or(LoweringError::Unsupported(
                "case observation has no exact declared carrier",
            ))?;
            Ok((
                path,
                selected_case(identities, declarations, sum.id, identity)?,
            ))
        }

        pub(crate) fn resolve(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            bindings: &[StructuralCaseBinding],
            subject: &checked_trees::CheckedStructuralParameterField,
            identity: &checked_trees::CheckedMemberIdentity,
        ) -> Result<
            (
                PlaceId,
                Vec<terminal_psi::StructuralPathSegment>,
                semantic_vocabulary::StructuralCaseId,
            ),
            LoweringError,
        > {
            let mut matching = bindings
                .iter()
                .filter(|binding| binding.source_position == subject.parameter_position);
            let binding = matching.next().ok_or(LoweringError::Unsupported(
                "case observation has no exact readable source and case binding",
            ))?;
            if matching.next().is_some() {
                return unsupported("case observation has ambiguous source bindings");
            }
            let (sum, path) = prepare_observation_path(
                identities,
                &binding.declarations,
                binding.structural_type,
                &subject.path,
            )?;
            Ok((
                binding.source,
                path,
                selected_case(identities, &binding.declarations, sum, identity)?,
            ))
        }

        pub(crate) fn resolve_runtime_element(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            declarations: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            source: PlaceId,
            prefix: &[checked_trees::CheckedStructuralPredicatePathSegment],
            element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            identity: &checked_trees::CheckedMemberIdentity,
        ) -> Result<
            (
                PlaceId,
                Vec<terminal_psi::StructuralPathSegment>,
                Vec<terminal_psi::StructuralPathSegment>,
                semantic_vocabulary::StructuralCaseId,
                bool,
            ),
            LoweringError,
        > {
            let (array, prefix) =
                prepare_observation_path(identities, declarations, structural_type, prefix)?;
            let (element, observes_element_view_length) = match &unique_declaration(
                declarations,
                array,
                "case observation lost its exact carrier type",
                "case observation has ambiguous carrier declarations",
            )?
            .shape
            {
                StructuralTypeShape::FixedArray { element, .. } => (*element, false),
                StructuralTypeShape::ElementView { element, .. } if prefix.is_empty() => {
                    (*element, true)
                }
                _ => {
                    return unsupported(
                        "runtime case observation requires an array or whole element view",
                    );
                }
            };
            let (sum, element_path) =
                prepare_observation_path(identities, declarations, element, element_path)?;
            let case = selected_case(identities, declarations, sum, identity)?;
            Ok((
                source,
                prefix,
                element_path,
                case,
                observes_element_view_length,
            ))
        }

        fn prepare_observation_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            declarations: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            steps: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Result<(StructuralTypeId, Vec<terminal_psi::StructuralPathSegment>), LoweringError>
        {
            let observation = explicit_path(identities, structural_type, steps, declarations)?;
            let selected = observation.declaration.ok_or(LoweringError::Unsupported(
                "case observation requires a structural carrier endpoint",
            ))?;
            let prepared = super::structural_paths::prepare_argument_path(
                identities,
                &observation.segments,
                structural_type,
                declarations,
            )?;
            let path = super::structural_paths::emit_argument_path(
                &prepared,
                None,
                &mut std::iter::empty(),
                None,
            )?;
            Ok((selected.id, path))
        }

        fn selected_case(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            declarations: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
            identity: &checked_trees::CheckedMemberIdentity,
        ) -> Result<semantic_vocabulary::StructuralCaseId, LoweringError> {
            let cases = match &unique_declaration(
                declarations,
                structural_type,
                "case observation lost its exact carrier type",
                "case observation has ambiguous carrier declarations",
            )?
            .shape
            {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => return unsupported("case observation requires an exact selected sum"),
            };
            let mut selected = cases
                .iter()
                .filter(|case| case.identity == identities.intern(&identity.spelling()));
            let case = selected.next().ok_or(LoweringError::Unsupported(
                "case observation lost its exact selected case",
            ))?;
            if selected.next().is_some() {
                return unsupported("case observation has ambiguous selected cases");
            }
            Ok(case.id)
        }

        pub(crate) fn sum_cases(
            bindings: &[StructuralCaseBinding],
            source: PlaceId,
        ) -> Option<&[terminal_psi::StructuralCaseDeclaration]> {
            let binding = bindings.iter().find(|binding| binding.source == source)?;
            let declaration =
                terminal_psi::find_structural_type(&binding.declarations, binding.structural_type)?;
            match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    Some(cases.as_slice())
                }
                _ => None,
            }
        }
    }
    pub(crate) mod structural_fields {

        use super::super::{
            CheckedUnitStructuralPathSegment, StructuralFieldId, StructuralFieldType,
            StructuralTypeId, StructuralTypeShape,
        };
        use super::{
            LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralParameterDeclaration,
            StructuralTypeDeclaration, unsupported,
        };
        use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
        use crate::emission::operation_emission::expressions::LoweredDirectExpression;
        use crate::emission::operation_emission::expressions::LoweredElementPathSegment;
        use crate::emission::scalar_types::terminal_scalar_type;
        use semantic_vocabulary::StructuralCaseId;

        pub(crate) mod case_observation {

            use semantic_vocabulary::CanonicalStructuralPathSegment;

            use super::{
                LoweringError, PlaceId, StructuralTypeDeclaration, StructuralTypeId, unsupported,
            };
            use crate::emission::operation_emission::expressions::PrimitiveReadSource;

            pub(crate) struct PreparedCaseObservation {
                pub(crate) source: PrimitiveReadSource,
                pub(crate) path: Vec<CanonicalStructuralPathSegment>,
                pub(crate) captured_prefix_length: usize,
            }

            pub(crate) fn prepare(
                source: PlaceId,
                root_type: StructuralTypeId,
                carrier_path: &[CanonicalStructuralPathSegment],
                declarations: &[StructuralTypeDeclaration],
            ) -> Result<PreparedCaseObservation, LoweringError> {
                let first_case = carrier_path
                    .iter()
                    .position(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)));
                let Some(case_position) =
                    first_case.filter(|position| position + 1 < carrier_path.len())
                else {
                    return Ok(PreparedCaseObservation {
                        source: source.into(),
                        path: carrier_path.to_vec(),
                        captured_prefix_length: 0,
                    });
                };
                if !matches!(
                    carrier_path[case_position + 1],
                    CanonicalStructuralPathSegment::Field(_)
                ) {
                    return unsupported("case observation requires its declared payload field");
                }
                let captured_prefix_length = case_position + 2;
                let captured_path = &carrier_path[..captured_prefix_length];
                let structural_type = terminal_semantics::canonical_case_leaf_type(
                    declarations.iter(),
                    root_type,
                    captured_path,
                )
                .ok_or(LoweringError::Unsupported(
                    "case observation lost its exact structural payload carrier",
                ))?;
                if !terminal_semantics::is_copyable_carrier(declarations.iter(), structural_type) {
                    return Ok(PreparedCaseObservation {
                        source: source.into(),
                        path: carrier_path.to_vec(),
                        captured_prefix_length: 0,
                    });
                }
                Ok(PreparedCaseObservation {
                    source: PrimitiveReadSource::CaseLeaf {
                        source,
                        path: captured_path.to_vec(),
                        structural_type,
                    },
                    path: carrier_path[captured_prefix_length..].to_vec(),
                    captured_prefix_length,
                })
            }

            impl PreparedCaseObservation {
                pub(crate) fn rebase_observation(
                    self,
                    root_type: StructuralTypeId,
                    declarations: &[StructuralTypeDeclaration],
                    observation: &mut super::LoweredDirectExpression,
                ) -> Result<(), LoweringError> {
                    use super::{LoweredBooleanReturnExpression, LoweredDirectExpression};
                    let observation_type = match &self.source {
                        PrimitiveReadSource::Place(_) => root_type,
                        PrimitiveReadSource::CaseLeaf {
                            structural_type, ..
                        } => *structural_type,
                    };
                    let runtime_prefix = terminal_semantics::canonical_structural_storage_path(
                        declarations.iter(),
                        observation_type,
                        &self.path,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "case observation lost its declared storage prefix",
                    ))?;
                    match observation {
                        LoweredDirectExpression::PrimitiveRead { source, path, .. } => {
                            *source = self.source;
                            drop(path.splice(..0, runtime_prefix));
                        }
                        LoweredDirectExpression::StructuralField { source, path, .. } => {
                            *source = self.source;
                            drop(path.splice(..0, self.path));
                        }
                        LoweredDirectExpression::Boolean { expression } => {
                            match expression.as_mut() {
                                LoweredBooleanReturnExpression::PrimitiveRead {
                                    source,
                                    path,
                                    ..
                                } => {
                                    *source = self.source;
                                    drop(path.splice(..0, runtime_prefix));
                                }
                                LoweredBooleanReturnExpression::StructuralField {
                                    source,
                                    path,
                                    ..
                                } => {
                                    *source = self.source;
                                    drop(path.splice(..0, self.path));
                                }
                                _ => {
                                    return unsupported(
                                        "case payload did not resolve to a Boolean observation",
                                    );
                                }
                            }
                        }
                        _ => {
                            return unsupported(
                                "case payload did not resolve to a field observation",
                            );
                        }
                    }
                    Ok(())
                }
            }
        }
        use crate::emission::structural_observations as observation_paths;
        pub(crate) use case_observation::prepare as prepare_case_observation;

        #[derive(Clone)]
        pub(crate) struct StructuralScalarFieldBinding {
            source_position: u32,
            source: PlaceId,
            access: StructuralAccess,
            structural_type: StructuralTypeId,
            declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
            case_payloads: Vec<EstablishedCasePayload>,
        }

        #[derive(Clone, Default)]
        pub(crate) struct StructuralFieldBindings {
            roots: Vec<StructuralScalarFieldBinding>,
            parameter_homes: Vec<StructuralParameterHome>,
            established_payloads: Vec<EstablishedCasePayload>,
            declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
        }

        #[derive(Clone)]
        struct StructuralParameterHome {
            source_position: u32,
            source: PlaceId,
            access: StructuralAccess,
            structural_type: StructuralTypeId,
        }

        impl std::ops::Deref for StructuralFieldBindings {
            type Target = [StructuralScalarFieldBinding];
            fn deref(&self) -> &Self::Target {
                &self.roots
            }
        }
        impl std::ops::DerefMut for StructuralFieldBindings {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.roots
            }
        }

        impl StructuralFieldBindings {
            pub(crate) fn clear_parameters(&mut self) {
                self.roots.clear();
                self.parameter_homes.clear();
            }

            pub(crate) fn adopt_declarations(
                &mut self,
                structural_type: StructuralTypeId,
                declarations: &[StructuralTypeDeclaration],
            ) {
                if !self
                    .declarations
                    .iter()
                    .any(|row| row.id == structural_type)
                {
                    self.declarations = declarations.into();
                }
            }

            /// The field binding of a whole established home, with this
            /// frame's declarations.
            pub(crate) fn binding_at(
                &self,
                home: &terminal_psi::StructuralArgument,
                structural_type: StructuralTypeId,
            ) -> Result<StructuralScalarFieldBinding, LoweringError> {
                // Psi 05 forms a rooted field observation only over a whole
                // local, so a projected home is a broken plan.
                if !home.path.is_empty() {
                    return Err(LoweringError::MissingDependencyRosterEntry {
                        identity_kind: "structural local place",
                        role: "field observation at a local's place",
                    });
                }
                Ok(StructuralScalarFieldBinding {
                    source_position: 0,
                    source: home.place,
                    access: home.access,
                    structural_type,
                    declarations: self.declarations.clone(),
                    case_payloads: Vec::new(),
                })
            }

            pub(crate) fn storage_home(
                &self,
                root: checked_trees::CheckedStorageRoot,
                structural_locals: &[super::StructuralLocalBinding],
                view_locals: &[super::view_locals::ViewLocalBinding],
            ) -> Result<(terminal_psi::StructuralArgument, StructuralTypeId), LoweringError>
            {
                let candidates: Vec<_> = match root {
                    checked_trees::CheckedStorageRoot::Parameter { index } => self
                        .parameter_homes
                        .iter()
                        .filter(|binding| binding.source_position == index)
                        .map(|binding| {
                            (
                                terminal_psi::StructuralArgument {
                                    place: binding.source,
                                    path: Vec::new(),
                                    access: binding.access,
                                },
                                binding.structural_type,
                            )
                        })
                        .collect(),
                    checked_trees::CheckedStorageRoot::ViewLocal { symbol }
                    | checked_trees::CheckedStorageRoot::OwnedLocal { symbol } => {
                        let owned = structural_locals
                            .iter()
                            .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol))
                            .map(|local| (local.argument.clone(), local.structural_type))
                            .collect::<Vec<_>>();
                        if owned.is_empty()
                            && matches!(root, checked_trees::CheckedStorageRoot::ViewLocal { .. })
                        {
                            view_locals
                                .iter()
                                .filter(|local| local.symbol == symbol)
                                .map(|local| {
                                    (
                                        terminal_psi::StructuralArgument {
                                            place: local.place,
                                            path: Vec::new(),
                                            access: local.access,
                                        },
                                        local.structural_type,
                                    )
                                })
                                .collect()
                        } else {
                            owned
                        }
                    }
                };
                match candidates.as_slice() {
                    [home] => Ok(home.clone()),
                    _ => Err(LoweringError::ObservationHomeCountMismatch {
                        root,
                        matching_homes: candidates.len(),
                        parameter_positions: self
                            .parameter_homes
                            .iter()
                            .map(|home| home.source_position)
                            .collect(),
                        local_symbols: structural_locals
                            .iter()
                            .filter_map(|home| home.root.as_symbol())
                            .collect(),
                        view_symbols: view_locals.iter().map(|home| home.symbol).collect(),
                    }),
                }
            }

            pub(crate) fn established_payload(
                &self,
                source: PlaceId,
                path: &[terminal_psi::StructuralPathSegment],
                case: StructuralCaseId,
                field: StructuralFieldId,
            ) -> Result<Option<usize>, LoweringError> {
                let mut rows = self.established_payloads.iter().filter(|row| {
                    row.source == source
                        && row.path == path
                        && row.case == case
                        && row.field == field
                });
                let position = rows.next().map(|row| row.position);
                if rows.next().is_some() {
                    return unsupported("case payload has duplicate established values");
                }
                Ok(position)
            }

            pub(crate) fn declarations(&self) -> &[StructuralTypeDeclaration] {
                &self.declarations
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct EstablishedCasePayload {
            pub(crate) source: PlaceId,
            pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
            pub(crate) case: StructuralCaseId,
            pub(crate) field: StructuralFieldId,
            pub(crate) position: usize,
        }

        pub(crate) enum CasePayloadRead {
            Observation { expression: LoweredDirectExpression },
            Established { position: usize },
        }

        pub(crate) fn establish_case_payloads(
            fields: &mut StructuralFieldBindings,
            established: &[EstablishedCasePayload],
        ) {
            fields.established_payloads.retain(|prior| {
                !established
                    .iter()
                    .any(|row| row.source == prior.source && row.path == prior.path)
            });
            fields.established_payloads.extend_from_slice(established);
            for field in &mut fields.roots {
                let rows = fields
                    .established_payloads
                    .iter()
                    .filter(|row| row.source == field.source)
                    .cloned()
                    .collect::<Vec<_>>();
                if !rows.is_empty() {
                    field.case_payloads = rows;
                }
            }
        }

        impl StructuralScalarFieldBinding {
            pub(crate) fn collect(
                parameters: &[(u32, StructuralParameterDeclaration)],
                types: &[StructuralTypeDeclaration],
            ) -> StructuralFieldBindings {
                let declarations: std::sync::Arc<[StructuralTypeDeclaration]> = types.into();
                let mut bindings = Vec::new();
                for (position, parameter) in parameters {
                    if !matches!(
                        parameter.access,
                        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
                    ) {
                        continue;
                    }
                    bindings.push(StructuralScalarFieldBinding {
                        source_position: *position,
                        source: parameter.place,
                        access: parameter.access,
                        structural_type: parameter.structural_type,
                        declarations: std::sync::Arc::clone(&declarations),
                        case_payloads: Vec::new(),
                    });
                }
                StructuralFieldBindings {
                    roots: bindings,
                    parameter_homes: parameters
                        .iter()
                        .map(|(position, parameter)| StructuralParameterHome {
                            source_position: *position,
                            source: parameter.place,
                            access: parameter.access,
                            structural_type: parameter.structural_type,
                        })
                        .collect(),
                    established_payloads: Vec::new(),
                    declarations,
                }
            }
        }

        pub(crate) fn resolve(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            fields: &[StructuralScalarFieldBinding],
            position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            scalar_type: ScalarType,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let observation = resolve_typed(identities, fields, position, path)?;
            if observation.scalar_type() != scalar_type {
                return unsupported("runtime field observation changes its declared scalar type");
            }
            Ok(observation)
        }

        pub(crate) fn resolve_typed(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            fields: &[StructuralScalarFieldBinding],
            position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let mut matching = fields
                .iter()
                .filter(|field| field.source_position == position);
            let binding = matching.next().ok_or(LoweringError::Unsupported(
                "runtime field observation has no exact readable binding",
            ))?;
            if matching.next().is_some() {
                return unsupported("runtime field observation has ambiguous bindings");
            }
            resolve_at(identities, binding, path)
        }

        /*
            Observe one scalar field below a readable home. The home decides
            only the root place: a formal's collected binding, or the
            established place of the root a field observation names.
        */
        pub(crate) fn resolve_at(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            binding: &StructuralScalarFieldBinding,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Result<LoweredDirectExpression, LoweringError> {
            use super::structural_paths::PreparedStructuralPathSegment as Prepared;

            let projection = observation_paths::explicit_path(
                identities,
                binding.structural_type,
                path,
                &binding.declarations,
            )?;
            let scalar_type = projection.scalar_type().ok_or(LoweringError::Unsupported(
                "runtime observation requires a declared scalar endpoint",
            ))?;
            if projection.scalar_field.is_none()
                && path.iter().any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedStructuralPredicatePathSegment::Case(_)
                    )
                })
            {
                let (source, path) =
                    resolve_case_primitive(identities, binding, projection, scalar_type)?;
                return Ok(LoweredDirectExpression::PrimitiveRead {
                    qualifications: Default::default(),
                    source,
                    path,
                    scalar_type,
                });
            }
            let prepared = super::structural_paths::prepare_argument_path(
                identities,
                &projection.segments,
                binding.structural_type,
                &binding.declarations,
            )?;
            if prepared
                .iter()
                .any(|segment| matches!(segment, Prepared::ViewExtent { .. }))
            {
                return descriptor_observation(binding, &prepared, scalar_type);
            }
            let physical_path = prepared
                .into_iter()
                .map(|step| match step {
                    Prepared::Static(step) => Ok(step),
                    _ => unsupported("static observation requires its captured selector"),
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;

            if let Some(field) = projection.scalar_field {
                if let Some((terminal_psi::StructuralPathSegment::Field(_), carrier)) =
                    physical_path.split_last()
                    && let Some((terminal_psi::StructuralPathSegment::Case(case), prefix)) =
                        carrier.split_last()
                    && let Some(row) = binding.case_payloads.iter().find(|row| {
                        row.path == prefix && row.case == *case && row.field == field.id
                    })
                {
                    return Ok(LoweredDirectExpression::Parameter {
                        position: row.position,
                        scalar_type,
                    });
                }
                if let Some(mut canonical) = projection.canonical {
                    canonical.pop();
                    return Ok(canonical_field_read(
                        binding.source.into(),
                        canonical,
                        field.id,
                        scalar_type,
                    ));
                }
            }
            Ok(LoweredDirectExpression::PrimitiveRead {
                qualifications: Default::default(),
                source: binding.source.into(),
                path: physical_path,
                scalar_type,
            })
        }

        fn descriptor_observation(
            binding: &StructuralScalarFieldBinding,
            prepared: &[super::structural_paths::PreparedStructuralPathSegment],
            scalar_type: ScalarType,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            use super::structural_paths::PreparedStructuralPathSegment as Prepared;
            use crate::emission::operation_emission::expressions::LoweredElementPathSegment as Segment;

            let count_type = terminal_scalar_type(typed_trees::types::PrimitiveType::U64)?;
            let mut path = Vec::with_capacity(prepared.len());
            let mut extent = None;
            for segment in prepared {
                match segment {
                    Prepared::ViewExtent { bytes } if extent.is_none() => {
                        extent = Some(if *bytes {
                            LoweredDirectExpression::ByteSequenceLength {
                                source: binding.source,
                                path: path.clone(),
                                access: StructuralAccess::SharedBorrow,
                                scalar_type: count_type,
                            }
                        } else {
                            LoweredDirectExpression::ElementViewLength {
                                source: binding.source,
                                path: path.clone(),
                                access: StructuralAccess::SharedBorrow,
                                scalar_type: count_type,
                            }
                        });
                    }
                    Prepared::Static(terminal_psi::StructuralPathSegment::FixedIndex(index))
                        if extent.is_some() =>
                    {
                        path.push(Segment::RuntimeIndex {
                            selector: LoweredDirectExpression::IntegerLiteral {
                                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                                    *index,
                                )),
                                scalar_type: count_type,
                            },
                            extent: extent.take(),
                            bound_decided: false,
                        });
                    }
                    Prepared::Static(segment) if extent.is_none() => {
                        path.push(Segment::Static(segment.clone()));
                    }
                    _ => {
                        return unsupported(
                            "descriptor observation requires its retained selector",
                        );
                    }
                }
            }
            if extent.is_some() {
                return unsupported("descriptor observation lost its element selector");
            }
            let selectors = path
                .iter()
                .filter(|segment| matches!(segment, Segment::RuntimeIndex { .. }))
                .count();
            if selectors
                > crate::emission::operation_emission::expressions::TERMINAL_NESTED_SELECTOR_LIMIT
            {
                return unsupported(
                    "descriptor observation exceeds the runtime selector namespace",
                );
            }
            let first = path
                .iter()
                .position(|segment| matches!(segment, Segment::RuntimeIndex { .. }))
                .ok_or(LoweringError::Unsupported(
                    "descriptor observation has no element selector",
                ))?;
            let element_path = path.split_off(first + 1);
            let Some(Segment::RuntimeIndex {
                selector,
                extent,
                bound_decided,
            }) = path.pop()
            else {
                return unsupported("descriptor observation lost its prepared selector");
            };
            let prefix = path
                .into_iter()
                .map(|segment| match segment {
                    Segment::Static(segment) => Ok(segment),
                    _ => {
                        unsupported("descriptor observation prefix contains an unprepared selector")
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(LoweredDirectExpression::IndexedPrimitiveRead {
                qualifications: Default::default(),
                source: binding.source,
                path: prefix,
                index: Box::new(selector),
                extent: extent.map(Box::new),
                element_path,
                scalar_type,
                bound_decided,
            })
        }

        pub(crate) fn canonical_field_read(
            source: crate::emission::operation_emission::expressions::PrimitiveReadSource,
            path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
            field: StructuralFieldId,
            scalar_type: ScalarType,
        ) -> LoweredDirectExpression {
            match scalar_type {
                ScalarType::Boolean => LoweredDirectExpression::Boolean {
                    expression: Box::new(LoweredBooleanReturnExpression::StructuralField {
                        qualifications: Default::default(),
                        source,
                        path,
                        field,
                    }),
                },
                _ => LoweredDirectExpression::StructuralField {
                    qualifications: Default::default(),
                    source,
                    path,
                    field,
                    scalar_type,
                },
            }
        }

        pub(crate) fn boolean_observation(
            observation: LoweredDirectExpression,
        ) -> LoweredBooleanReturnExpression {
            match observation {
                LoweredDirectExpression::Boolean { expression } => *expression,
                LoweredDirectExpression::PrimitiveRead {
                    qualifications,
                    source,
                    path,
                    scalar_type: ScalarType::Boolean,
                } => LoweredBooleanReturnExpression::PrimitiveRead {
                    qualifications,
                    source,
                    path,
                },
                operand => LoweredBooleanReturnExpression::ScalarObservation {
                    operand: Box::new(operand),
                },
            }
        }

        pub(crate) fn resolve_case_payload(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            fields: &[StructuralScalarFieldBinding],
            position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            scalar_type: ScalarType,
        ) -> Result<CasePayloadRead, LoweringError> {
            let mut matching = fields
                .iter()
                .filter(|field| field.source_position == position);
            let binding = matching.next().ok_or(LoweringError::Unsupported(
                "case payload observation has no exact readable binding",
            ))?;
            if matching.next().is_some() {
                return unsupported("case payload observation has ambiguous bindings");
            }
            let Some((
                checked_trees::CheckedStructuralPredicatePathSegment::Case(case_identity),
                tail,
            )) = path.split_first()
            else {
                return unsupported(
                    "case payload observation requires one case and one payload field",
                );
            };
            let mut declarations = binding
                .declarations
                .iter()
                .filter(|declaration| declaration.id == binding.structural_type);
            let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                "case payload observation lost its sum declaration",
            ))?;
            if declarations.next().is_some() {
                return unsupported("case payload observation has ambiguous sum declarations");
            }
            let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                &declaration.shape
            else {
                return unsupported("case payload observation requires a sum root");
            };
            let mut selected = cases
                .iter()
                .filter(|case| case.identity == identities.intern(&case_identity.spelling()));
            let case = selected.next().ok_or(LoweringError::Unsupported(
                "case payload observation lost its declared case",
            ))?;
            if selected.next().is_some() {
                return unsupported("case payload observation has an erased or ambiguous payload");
            }
            let field = match tail {
                [checked_trees::CheckedStructuralPredicatePathSegment::Field(field_identity)] => {
                    let mut payload = case.fields.iter().filter(|field| {
                        field.identity == identities.intern(&field_identity.spelling())
                    });
                    let field = payload.next().ok_or(LoweringError::Unsupported(
                        "case payload observation lost its declared payload field",
                    ))?;
                    if payload.next().is_some() || field.relevance.is_erased() {
                        return unsupported(
                            "case payload observation has an erased or ambiguous payload",
                        );
                    }
                    let declared =
                        field
                            .field_type
                            .scalar_type()
                            .ok_or(LoweringError::Unsupported(
                                "case payload observation requires a scalar payload",
                            ))?;
                    if declared != scalar_type {
                        return unsupported(
                            "case payload observation changes its declared scalar type",
                        );
                    }
                    field.id
                }
                [
                    checked_trees::CheckedStructuralPredicatePathSegment::Field(_),
                    projection @ ..,
                ] if !projection.is_empty() => {
                    let mut payload = plan_case_payload_leaf(identities, fields, position, path)
                        .ok_or(LoweringError::Unsupported(
                            "case payload observation has no exact scalar projection",
                        ))?;
                    if payload.observation.scalar_type() != scalar_type {
                        return unsupported(
                            "case payload observation changes its declared scalar type",
                        );
                    }
                    prepare_case_observation(
                        payload.source,
                        binding.structural_type,
                        &[
                            semantic_vocabulary::CanonicalStructuralPathSegment::Case(payload.case),
                            semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                payload.member,
                            ),
                        ],
                        &binding.declarations,
                    )?
                    .rebase_observation(
                        binding.structural_type,
                        &binding.declarations,
                        &mut payload.observation,
                    )?;
                    return Ok(CasePayloadRead::Observation {
                        expression: payload.observation,
                    });
                }
                _ => {
                    return unsupported(
                        "case payload observation requires one case and one payload field",
                    );
                }
            };
            if let Some(row) = binding
                .case_payloads
                .iter()
                .find(|row| row.path.is_empty() && row.case == case.id && row.field == field)
            {
                return Ok(CasePayloadRead::Established {
                    position: row.position,
                });
            }
            Ok(CasePayloadRead::Observation {
                expression: canonical_field_read(
                    binding.source.into(),
                    vec![semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                        case.id,
                    )],
                    field,
                    scalar_type,
                ),
            })
        }

        #[derive(Debug, Clone)]
        pub(crate) struct PlannedCasePayloadLeaf {
            pub(crate) source: PlaceId,
            pub(crate) case: StructuralCaseId,
            pub(crate) member: StructuralFieldId,
            pub(crate) observation: LoweredDirectExpression,
        }

        pub(crate) fn plan_case_payload_leaf(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            fields: &[StructuralScalarFieldBinding],
            position: u32,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Option<PlannedCasePayloadLeaf> {
            let [
                checked_trees::CheckedStructuralPredicatePathSegment::Case(case_identity),
                checked_trees::CheckedStructuralPredicatePathSegment::Field(member_identity),
                projection @ ..,
            ] = path
            else {
                return None;
            };
            if projection.is_empty() {
                return None;
            }
            let mut matching = fields
                .iter()
                .filter(|field| field.source_position == position);
            let binding = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            let mut declarations = binding
                .declarations
                .iter()
                .filter(|declaration| declaration.id == binding.structural_type);
            let declaration = declarations.next()?;
            if declarations.next().is_some() {
                return None;
            }
            let cases = declaration.shape.sum_cases()?;
            let mut selected = cases
                .iter()
                .filter(|case| case.identity == identities.intern(&case_identity.spelling()));
            let case = selected.next()?;
            if selected.next().is_some() {
                return None;
            }
            let mut members = case
                .fields
                .iter()
                .filter(|field| field.identity == identities.intern(&member_identity.spelling()));
            let member = members.next()?;
            if members.next().is_some() || member.relevance.is_erased() {
                return None;
            }
            let member_type = member.field_type.as_structural()?;
            let projected = StructuralScalarFieldBinding {
                source_position: position,
                source: binding.source,
                access: binding.access,
                structural_type: member_type,
                declarations: std::sync::Arc::clone(&binding.declarations),
                case_payloads: Vec::new(),
            };
            let observation = resolve_typed(
                identities,
                std::slice::from_ref(&projected),
                position,
                projection,
            )
            .ok()?;
            Some(PlannedCasePayloadLeaf {
                source: binding.source,
                case: case.id,
                member: member.id,
                observation,
            })
        }

        pub(crate) fn prepare_sequence_observation(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            structural_type: StructuralTypeId,
            declarations: &[StructuralTypeDeclaration],
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Result<
            Option<(
                Vec<super::structural_paths::PreparedStructuralPathSegment>,
                Option<StructuralFieldId>,
                super::view_locals::ViewCarrier,
            )>,
            LoweringError,
        > {
            let projection =
                observation_paths::explicit_path(identities, structural_type, path, declarations)?;
            let (field, carrier) = if let Some(field) = projection.scalar_field {
                if !matches!(
                    &field.field_type,
                    StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            | terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    ) | StructuralFieldType::PredicateByteSequence {
                        carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            | terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                        ..
                    }
                ) {
                    return Ok(None);
                }
                (Some(field), super::view_locals::ViewCarrier::Bytes)
            } else {
                let declaration = projection.declaration.ok_or(LoweringError::Unsupported(
                    "length observation lost its sequence declaration",
                ))?;
                let Some(carrier) =
                    super::view_locals::ViewCarrier::observation_of(declaration.id, declarations)
                else {
                    return Ok(None);
                };
                (None, carrier)
            };
            let mut steps = super::structural_paths::prepare_argument_path(
                identities,
                &projection.segments,
                structural_type,
                declarations,
            )?;
            if let Some(field) = field
                && steps.pop()
                    != Some(
                        super::structural_paths::PreparedStructuralPathSegment::Static(
                            terminal_psi::StructuralPathSegment::Field(field.identity),
                        ),
                    )
            {
                return unsupported("byte length observation lost its field endpoint");
            }
            Ok(Some((steps, field.map(|field| field.id), carrier)))
        }

        pub(crate) fn sequence_length(
            home: &terminal_psi::StructuralArgument,
            carrier: super::view_locals::ViewCarrier,
            scalar_type: ScalarType,
        ) -> LoweredDirectExpression {
            let path = home
                .path
                .iter()
                .cloned()
                .map(LoweredElementPathSegment::Static)
                .collect();
            match carrier {
                super::view_locals::ViewCarrier::Bytes
                | super::view_locals::ViewCarrier::FixedBytes => {
                    LoweredDirectExpression::ByteSequenceLength {
                        source: home.place,
                        path,
                        access: StructuralAccess::SharedBorrow,
                        scalar_type,
                    }
                }
                super::view_locals::ViewCarrier::Elements { .. } => {
                    LoweredDirectExpression::ElementViewLength {
                        source: home.place,
                        path,
                        access: StructuralAccess::SharedBorrow,
                        scalar_type,
                    }
                }
            }
        }

        pub(crate) struct IndexedElementObservation {
            pub(crate) source: PlaceId,
            pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
            pub(crate) element_path: Vec<super::structural_paths::PreparedStructuralPathSegment>,
            pub(crate) scalar_type: ScalarType,
            pub(crate) extent: Option<Box<LoweredDirectExpression>>,
        }

        pub(crate) fn resolve_indexed_element(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            fields: &StructuralFieldBindings,
            home: &terminal_psi::StructuralArgument,
            structural_type: StructuralTypeId,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            element_path: &[checked_trees::CheckedStructuralPredicatePathSegment],
        ) -> Result<IndexedElementObservation, LoweringError> {
            let projection = observation_paths::explicit_path(
                identities,
                structural_type,
                path,
                fields.declarations(),
            )?;
            let declaration = projection.declaration.ok_or(LoweringError::Unsupported(
                "indexed observation lost its collection carrier",
            ))?;
            let (element_path, scalar_type, carrier) = match &declaration.shape {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element, .. } => {
                    let element_projection = observation_paths::explicit_path(
                        identities,
                        *element,
                        element_path,
                        fields.declarations(),
                    )?;
                    let scalar_type =
                        crate::emission::primitive_store::indexed_element_scalar_type(
                            identities,
                            *element,
                            &element_projection.segments,
                            fields.declarations(),
                        )?;
                    (
                        super::structural_paths::prepare_argument_path(
                            identities,
                            &element_projection.segments,
                            *element,
                            fields.declarations(),
                        )?,
                        scalar_type,
                        super::view_locals::ViewCarrier::of(declaration.id, fields.declarations()),
                    )
                }
                StructuralTypeShape::ByteSequence(_) if element_path.is_empty() => (
                    Vec::new(),
                    terminal_scalar_type(typed_trees::types::PrimitiveType::U8)?,
                    Some(super::view_locals::ViewCarrier::Bytes),
                ),
                _ => {
                    return unsupported(
                        "indexed observation requires a scalar collection endpoint",
                    );
                }
            };
            let path = super::structural_paths::prepare_argument_path(
                identities,
                &projection.segments,
                structural_type,
                fields.declarations(),
            )?
            .into_iter()
            .map(|step| match step {
                super::structural_paths::PreparedStructuralPathSegment::Static(step) => Ok(step),
                _ => unsupported("indexed collection prefix requires static projections"),
            })
            .collect::<Result<Vec<_>, _>>()?;
            let path = home.path.iter().cloned().chain(path).collect::<Vec<_>>();
            let extent = carrier
                .map(|carrier| {
                    Ok::<_, LoweringError>(Box::new(sequence_length(
                        &terminal_psi::StructuralArgument {
                            place: home.place,
                            path: path.clone(),
                            access: home.access,
                        },
                        carrier,
                        terminal_scalar_type(typed_trees::types::PrimitiveType::U64)?,
                    )))
                })
                .transpose()?;
            Ok(IndexedElementObservation {
                source: home.place,
                path,
                element_path,
                scalar_type,
                extent,
            })
        }

        fn resolve_case_primitive(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            binding: &StructuralScalarFieldBinding,
            projection: observation_paths::ObservationPath<'_>,
            scalar_type: ScalarType,
        ) -> Result<
            (
                crate::emission::operation_emission::expressions::PrimitiveReadSource,
                Vec<terminal_psi::StructuralPathSegment>,
            ),
            LoweringError,
        > {
            use semantic_vocabulary::CanonicalStructuralPathSegment as Canonical;

            let Some(mut canonical) = projection.canonical else {
                return unsupported(
                    "case primitive observation requires an inline canonical carrier",
                );
            };
            let Some(Canonical::FixedIndex(index)) = canonical.pop() else {
                return unsupported(
                    "case primitive observation requires an indexed primitive carrier",
                );
            };
            let Some(structural_type) = terminal_semantics::canonical_case_leaf_type(
                binding.declarations.iter(),
                binding.structural_type,
                &canonical,
            ) else {
                return unsupported("primitive observation lost its canonical carrier");
            };
            let (path, declared) = crate::emission::primitive_store::lower_path(
                identities,
                structural_type,
                &[CheckedUnitStructuralPathSegment::FixedIndex(index)],
                &binding.declarations,
            )?;
            if declared != scalar_type {
                return unsupported("primitive observation changes its declared scalar type");
            }
            Ok((
                crate::emission::operation_emission::expressions::PrimitiveReadSource::CaseLeaf {
                    source: binding.source,
                    path: canonical,
                    structural_type,
                },
                path,
            ))
        }
    }
    pub(crate) mod structural_paths {

        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::emission::operation_emission::calls::CallEmissionContext;
        use crate::shared::lowering_error::{LoweringError, unsupported};

        macro_rules! unsupported_error {
            ($message:literal) => {
                LoweringError::Unsupported($message)
            };
        }

        use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralPathSegment};
        use semantic_vocabulary::{ScalarType, ValueId};
        use terminal_psi::ValueDeclaration;
        use terminal_psi::{StructuralAccess, StructuralPathSegment};

        pub(crate) fn structural_argument(
            place: semantic_vocabulary::PlaceId,
            path: Vec<StructuralPathSegment>,
            access: StructuralAccess,
        ) -> terminal_psi::StructuralArgument {
            terminal_psi::StructuralArgument {
                place,
                path,
                access,
            }
        }

        pub(crate) fn terminal_access(access: CheckedStructuralAccess) -> StructuralAccess {
            match access {
                CheckedStructuralAccess::Owned => StructuralAccess::Owned,
                CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
                CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
                CheckedStructuralAccess::WriteOnlyBorrow => StructuralAccess::WriteOnlyBorrow,
            }
        }

        pub(crate) fn structural_access_can_supply(
            source: StructuralAccess,
            presented: StructuralAccess,
        ) -> bool {
            match source {
                StructuralAccess::Owned => true,
                StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow => matches!(
                    presented,
                    StructuralAccess::SharedBorrow
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                ),
                StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
            }
        }

        // Place loans already name admitted storage without a descriptor.
        // Mark only their present local-name shell; later crossings stay intact.
        pub(crate) fn retain_place_loan_presentation(
            checked: &checked_trees::CheckedTrees,
            loan: &checked_trees::CheckedPlaceLoanPlan,
            argument: &mut checked_trees::CheckedUnitStructuralArgumentPlan,
        ) {
            use checked_trees::CheckedUnitStructuralArgumentSourcePlan as Source;

            if let Source::StructuralLocal { symbol } = argument.source
                && symbol == loan.symbol
                && matches!(
                    argument.path.first(),
                    Some(CheckedUnitStructuralPathSegment::Referent)
                )
            {
                argument.source = Source::StructuralLocalBorrow {
                    symbol,
                    root_type_identity:
                        crate::shared::terminal_identities::stored_structural_identity(
                            checked,
                            loan.referee,
                        ),
                    root_reference_hops: 1,
                };
            }
        }

        pub(crate) fn lower_structural_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            path: &[CheckedUnitStructuralPathSegment],
        ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
            path.iter()
            .map(|segment| match segment {
                CheckedUnitStructuralPathSegment::AllElements => {
                    unsupported("universal qualification is not an operand path")
                }
                CheckedUnitStructuralPathSegment::Case(_) => unsupported(
                    "a case projection requires its exact source type during preparation",
                ),
                CheckedUnitStructuralPathSegment::RepresentationView { .. } => unsupported(
                    "a returned view origin requires its checked declaration and formation codec",
                ),
                CheckedUnitStructuralPathSegment::Referent => Ok(StructuralPathSegment::Referent),
                CheckedUnitStructuralPathSegment::Field(identity) => Ok(
                    StructuralPathSegment::Field(identities.intern(&identity.spelling())),
                ),
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    Ok(StructuralPathSegment::FixedIndex(*index))
                }
                CheckedUnitStructuralPathSegment::RuntimeIndex { .. }
                | CheckedUnitStructuralPathSegment::ComputedIndex(_) => unsupported(
                    "a runtime-index projection is lowered by the emitter that evaluates its index",
                ),
                CheckedUnitStructuralPathSegment::FixedRange { start, end } => {
                    Ok(StructuralPathSegment::FixedRange {
                        start: *start,
                        end: *end,
                    })
                }
            })
            .collect()
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum PreparedStructuralPathSegment {
            Static(StructuralPathSegment),
            ViewExtent {
                bytes: bool,
            },
            RuntimeIndex(checked_trees::CheckedRuntimeIndex),
            ComputedIndex(checked_trees::CheckedScalarComputationHandle),
            EvaluatedFixedIndex {
                index: u64,
                computation: checked_trees::CheckedScalarComputationHandle,
            },
            CapturedIndex(semantic_vocabulary::PlaceId),
        }

        pub(crate) fn retain_selector_evaluations(
            checked_path: &[CheckedUnitStructuralPathSegment],
            evaluations: arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
            operands: &arena::Arena<checked_trees::CheckedScalarComputationHandle>,
            prepared: &mut [PreparedStructuralPathSegment],
        ) -> Result<(), LoweringError> {
            if evaluations.is_empty() {
                return Ok(());
            }
            if !checked_trees::CheckedUnitStructuralArgumentPlan::projection_evaluations_match(
                checked_path,
                evaluations,
                operands,
            ) {
                return unsupported("selector child roster changes its checked projection");
            }
            let children = operands.span(evaluations).ok_or(unsupported_error!(
                "selector projection has a stale computation child span"
            ))?;
            let checked_indexes = checked_path
                .iter()
                .enumerate()
                .filter_map(|(position, segment)| {
                    matches!(
                        segment,
                        CheckedUnitStructuralPathSegment::FixedIndex(_)
                            | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                            | CheckedUnitStructuralPathSegment::ComputedIndex(_)
                    )
                    .then_some(position)
                })
                .collect::<Vec<_>>();
            let prepared_indexes = prepared
                .iter()
                .enumerate()
                .filter_map(|(position, segment)| {
                    matches!(
                        segment,
                        PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(_))
                            | PreparedStructuralPathSegment::RuntimeIndex(_)
                            | PreparedStructuralPathSegment::ComputedIndex(_)
                            | PreparedStructuralPathSegment::CapturedIndex(_)
                    )
                    .then_some(position)
                })
                .collect::<Vec<_>>();
            if checked_indexes.len() != prepared_indexes.len() {
                return unsupported("prepared projection changes its checked index roster");
            }
            let inherited =
                checked_indexes
                    .len()
                    .checked_sub(children.len())
                    .ok_or(unsupported_error!(
                        "selector child roster exceeds its projection suffix"
                    ))?;
            for (ordinal, computation) in children.iter().enumerate() {
                let index_ordinal = ordinal + inherited;
                let position = checked_indexes[index_ordinal];
                let target = &mut prepared[prepared_indexes[index_ordinal]];
                match checked_path[position] {
                    CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        if *target
                            != PreparedStructuralPathSegment::Static(
                                StructuralPathSegment::FixedIndex(index),
                            )
                        {
                            return unsupported(
                                "selector evaluation changes its prepared fixed coordinate",
                            );
                        }
                        *target = PreparedStructuralPathSegment::EvaluatedFixedIndex {
                            index,
                            computation: *computation,
                        };
                    }
                    CheckedUnitStructuralPathSegment::ComputedIndex(expected)
                        if expected == *computation
                            && *target
                                == PreparedStructuralPathSegment::ComputedIndex(expected) => {}
                    _ => return unsupported("selector child lost its exact dynamic coordinate"),
                }
            }
            Ok(())
        }

        #[derive(Debug)]
        enum PreparedArgumentTip<'a> {
            Declaration(&'a terminal_psi::StructuralTypeDeclaration),
            Inline(&'a terminal_psi::StructuralFieldType),
            ByteElement,
            CollectionWindow,
        }

        #[derive(Clone, Copy)]
        pub(crate) enum ProjectionRoot {
            Published(semantic_vocabulary::StructuralTypeId),
            Borrowed {
                published: semantic_vocabulary::StructuralTypeId,
                logical: typed_trees::type_identity::TypeIdentityId,
                spelled_reference_hops: u32,
            },
            BorrowedEntry {
                published: semantic_vocabulary::StructuralTypeId,
                access: StructuralAccess,
                reference_hops: u32,
            },
        }

        /// Join a logical loan root to its exact published selected carrier, preserving
        /// any physical prefix retained by the binding's caller. Physical argument paths
        /// use the strict walker unchanged; it must never implicitly dereference a
        /// descriptor value or a nested reference field.
        #[track_caller]
        pub(crate) fn prepare_structural_argument_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            published_root: semantic_vocabulary::StructuralTypeId,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
            prepare_structural_argument_projection(
                identities,
                checked,
                argument,
                &argument.path,
                published_root,
                types,
            )
            .map(|(path, _)| path)
        }

        /// A final call addresses the completed value. Its range producer has already
        /// consumed the original collection path; ordinary storage joins above still
        /// require that path when they bind the original source.
        #[track_caller]
        pub(crate) fn prepare_call_argument_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            published_root: semantic_vocabulary::StructuralTypeId,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
            let (path, tip) = prepare_structural_argument_projection(
                identities,
                checked,
                argument,
                argument.call_projection(),
                published_root,
                types,
            )?;
            complete_lent_path(
                identities,
                checked,
                path,
                tip,
                argument.type_identity,
                terminal_access(argument.access),
                types,
            )
        }

        /// Complete a checked projection in its producer's coordinate system. Physical
        /// paths stay strict; logical loans retain their explicit carrier adaptation.
        /// The endpoint is a declaration view, not a fabricated invocation operand.
        #[track_caller]
        pub(crate) fn prepare_projection_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            projection: &[CheckedUnitStructuralPathSegment],
            root: ProjectionRoot,
            endpoint: typed_trees::type_identity::TypeIdentityId,
            access: StructuralAccess,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
            let (path, tip) =
                prepare_root_projection(identities, checked, projection, root, access, types)?;
            complete_lent_path(identities, checked, path, tip, endpoint, access, types)
        }

        fn complete_lent_path<'a>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            mut path: Vec<PreparedStructuralPathSegment>,
            tip: PreparedArgumentTip<'a>,
            endpoint: typed_trees::type_identity::TypeIdentityId,
            access: StructuralAccess,
            types: &'a [terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
            let expected =
                identities.intern(&checked.type_identity_text(endpoint).unwrap_or_default());
            let tip = match tip {
                PreparedArgumentTip::Declaration(declaration) => Some(declaration),
                _ => None,
            };
            complete_reference_endpoint(tip, expected, access, types, |segment| {
                path.push(PreparedStructuralPathSegment::Static(segment));
            })?;
            Ok(path)
        }

        pub(crate) fn complete_emitted_argument_path(
            root: semantic_vocabulary::StructuralTypeId,
            path: &mut Vec<StructuralPathSegment>,
            endpoint: semantic_vocabulary::StructuralTypeId,
            access: StructuralAccess,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<(), LoweringError> {
            let expected = terminal_psi::find_structural_type(types, endpoint).ok_or(
                unsupported_error!("state argument lost its receiving declaration"),
            )?;
            let tip = terminal_semantics::runtime_structural_path_tip(types.iter(), root, path);
            complete_reference_endpoint(tip, expected.identity, access, types, |segment| {
                path.push(segment);
            })
        }

        fn complete_reference_endpoint<'a>(
            tip: Option<&'a terminal_psi::StructuralTypeDeclaration>,
            expected: terminal_psi::TerminalIdentityIndex,
            access: StructuralAccess,
            types: &'a [terminal_psi::StructuralTypeDeclaration],
            append: impl FnMut(StructuralPathSegment),
        ) -> Result<(), LoweringError> {
            follow_reference_endpoint(
                tip,
                |declaration| declaration.identity == expected,
                access,
                types,
                append,
            )?;
            Ok(())
        }

        pub(crate) fn complete_read_observation_path(
            root: semantic_vocabulary::StructuralTypeId,
            path: &mut Vec<StructuralPathSegment>,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError> {
            let tip = terminal_semantics::runtime_structural_path_tip(types.iter(), root, path)
                .ok_or(unsupported_error!(
                    "content observation lost its exact carrier"
                ))?;
            let endpoint = follow_reference_endpoint(
                Some(tip),
                |_| false,
                StructuralAccess::SharedBorrow,
                types,
                |segment| path.push(segment),
            )?
            .ok_or(unsupported_error!(
                "content observation lost its reference endpoint"
            ))?;
            Ok(endpoint.id)
        }

        fn follow_reference_endpoint<'a>(
            mut tip: Option<&'a terminal_psi::StructuralTypeDeclaration>,
            is_endpoint: impl Fn(&terminal_psi::StructuralTypeDeclaration) -> bool,
            access: StructuralAccess,
            types: &'a [terminal_psi::StructuralTypeDeclaration],
            mut append: impl FnMut(StructuralPathSegment),
        ) -> Result<Option<&'a terminal_psi::StructuralTypeDeclaration>, LoweringError> {
            if access == StructuralAccess::Owned {
                return Ok(tip);
            }
            let mut crossed = Vec::new();
            while let Some(declaration) = tip {
                if is_endpoint(declaration) {
                    break;
                }
                let terminal_psi::StructuralTypeShape::Reference {
                    referent,
                    access: supplied,
                } = declaration.shape
                else {
                    break;
                };
                if !structural_access_can_supply(supplied, access)
                    || crossed.contains(&declaration.id)
                {
                    return unsupported(
                        "borrowed argument crosses an unreadable or cyclic reference",
                    );
                }
                crossed.push(declaration.id);
                tip = Some(terminal_psi::find_structural_type(types, referent).ok_or(
                    unsupported_error!("borrowed argument reference lost its referent declaration"),
                )?);
                append(StructuralPathSegment::Referent);
            }
            Ok(tip)
        }

        #[track_caller]
        fn prepare_structural_argument_projection<'a>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            projection: &[CheckedUnitStructuralPathSegment],
            published_root: semantic_vocabulary::StructuralTypeId,
            types: &'a [terminal_psi::StructuralTypeDeclaration],
        ) -> Result<(Vec<PreparedStructuralPathSegment>, PreparedArgumentTip<'a>), LoweringError>
        {
            let root = match argument.borrowed_root_type_identity() {
                Some(logical) => ProjectionRoot::Borrowed {
                    published: published_root,
                    logical,
                    spelled_reference_hops: argument.borrowed_root_reference_hops(),
                },
                None => ProjectionRoot::Published(published_root),
            };
            let (mut path, tip) = prepare_root_projection(
                identities,
                checked,
                projection,
                root,
                terminal_access(argument.access),
                types,
            )
            .map_err(|error| match error {
                LoweringError::InvalidArgumentProjection(mut failure) => {
                    failure.source_root = argument.structural_storage_root();
                    failure.source_parameter = argument.source_parameter_index();
                    failure.source_result = argument.source_structural_result_binding_ordinal();
                    failure.published_root = Some(published_root);
                    failure.argument_type = Some(argument.type_identity);
                    failure.logical_root = argument.borrowed_root_type_identity();
                    failure.root_reference_hops = argument.borrowed_root_reference_hops();
                    failure.access = Some(argument.access);
                    LoweringError::InvalidArgumentProjection(failure)
                }
                error => error,
            })?;
            if !projection.is_empty() {
                retain_selector_evaluations(
                    projection,
                    argument.projection_evaluations,
                    &checked.facts.values.scalar_computations.operands,
                    &mut path,
                )?;
            }
            retain_proven_selector_coordinates(checked, &mut path);
            Ok((path, tip))
        }

        pub(crate) fn retain_proven_selector_coordinates(
            checked: &checked_trees::CheckedTrees,
            path: &mut [PreparedStructuralPathSegment],
        ) {
            let computations = &checked.facts.values.scalar_computations;
            for segment in path {
                let PreparedStructuralPathSegment::ComputedIndex(computation) = *segment else {
                    continue;
                };
                if !computations.nodes.is_valid(computation) {
                    continue;
                }
                let selector = computations.nodes.get(computation).authored_root;
                if let Some(index) = checked.facts.selector_coordinate(selector) {
                    *segment =
                        PreparedStructuralPathSegment::EvaluatedFixedIndex { index, computation };
                }
            }
        }

        #[track_caller]
        fn prepare_root_projection<'a>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            projection: &[CheckedUnitStructuralPathSegment],
            root: ProjectionRoot,
            requested: StructuralAccess,
            types: &'a [terminal_psi::StructuralTypeDeclaration],
        ) -> Result<(Vec<PreparedStructuralPathSegment>, PreparedArgumentTip<'a>), LoweringError>
        {
            let (published_root, identity, spelled_reference_hops) = match root {
                ProjectionRoot::Published(published) => {
                    return prepare_argument_projection(identities, projection, published, types);
                }
                ProjectionRoot::Borrowed {
                    published,
                    logical,
                    spelled_reference_hops,
                } => (published, logical, spelled_reference_hops),
                ProjectionRoot::BorrowedEntry {
                    published,
                    access,
                    reference_hops,
                } => {
                    if access == StructuralAccess::Owned
                        || requested == StructuralAccess::Owned
                        || !structural_access_can_supply(access, requested)
                    {
                        return unsupported("reference origin entry changes its borrowed access");
                    }
                    let mut remainder = projection;
                    for _ in 0..reference_hops {
                        let Some((CheckedUnitStructuralPathSegment::Referent, suffix)) =
                            remainder.split_first()
                        else {
                            return unsupported(
                                "reference origin lost its declared entry crossing",
                            );
                        };
                        remainder = suffix;
                    }
                    return prepare_argument_projection(identities, remainder, published, types);
                }
            };
            if !identity.is_valid() {
                return unsupported("borrowed projection has no logical root declaration");
            }
            let mut roots = types.iter().filter(|row| {
                row.identity
                    == identities.intern(&checked.type_identity_text(identity).unwrap_or_default())
            });
            let logical_root = roots.next().ok_or(unsupported_error!(
                "borrowed argument lost its checked logical root"
            ))?;
            if roots.next().is_some() {
                return unsupported("borrowed argument logical root is empty or ambiguous");
            }
            if requested == StructuralAccess::Owned {
                return unsupported("logical borrowed argument cannot transfer its carrier");
            }
            let mut remainder = projection;
            for _ in 0..spelled_reference_hops {
                let Some((CheckedUnitStructuralPathSegment::Referent, suffix)) =
                    remainder.split_first()
                else {
                    return unsupported(
                        "borrowed projection lost its producer-spelled root crossing",
                    );
                };
                remainder = suffix;
            }
            let mut prepared = Vec::new();
            let mut carrier_id = published_root;
            let mut remaining = types.len();
            while carrier_id != logical_root.id {
                remaining = remaining.checked_sub(1).ok_or(unsupported_error!(
                    "borrowed argument has a cyclic reference presentation"
                ))?;
                let mut carriers = types.iter().filter(|row| row.id == carrier_id);
                let carrier = carriers.next().ok_or(unsupported_error!(
                    "borrowed argument lost its published carrier"
                ))?;
                if carriers.next().is_some() {
                    return unsupported("borrowed argument published carrier is ambiguous");
                }
                match &carrier.shape {
                    terminal_psi::StructuralTypeShape::Reference { referent, access } => {
                        if !structural_access_can_supply(*access, requested) {
                            return unsupported(
                                "borrowed argument crosses an incompatible reference access",
                            );
                        }
                        prepared.push(PreparedStructuralPathSegment::Static(
                            StructuralPathSegment::Referent,
                        ));
                        carrier_id = *referent;
                    }
                    terminal_psi::StructuralTypeShape::ElementView { access, .. }
                    | terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access },
                    )
                    | terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                    ) => {
                        if terminal_semantics::borrowed_referent_presentation(
                            types.iter(),
                            carrier.id,
                            logical_root.id,
                        ) != Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
                            || access.is_some_and(|access| {
                                !structural_access_can_supply(access, requested)
                            })
                        {
                            return unsupported(
                                "borrowed argument changes its descriptor presentation",
                            );
                        }
                        carrier_id = logical_root.id;
                    }
                    _ => return unsupported("borrowed argument changes its published root"),
                }
            }
            let (suffix, tip) =
                prepare_argument_projection(identities, remainder, logical_root.id, types)?;
            prepared.extend(suffix);
            Ok((prepared, tip))
        }

        /// Resolve the path against its original source declaration. Only runtime
        /// selector values and their bound obligations remain for the emitting call.
        #[track_caller]
        pub(crate) fn prepare_argument_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            path: &[CheckedUnitStructuralPathSegment],
            root: semantic_vocabulary::StructuralTypeId,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
            prepare_argument_projection(identities, path, root, types).map(|(path, _)| path)
        }

        /// Resolve an indexed store's descriptor through the same typed projection
        /// as executable arguments. The caller retains only a conservative proof
        /// footprint; the indexed writer still evaluates the actual selector path.
        #[track_caller]
        pub(crate) fn prepare_element_view_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            path: &[CheckedUnitStructuralPathSegment],
            root: semantic_vocabulary::StructuralTypeId,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
            let (path, tip) = prepare_argument_projection(identities, path, root, types)?;
            if !matches!(tip, PreparedArgumentTip::Declaration(row)
                if matches!(row.shape, terminal_psi::StructuralTypeShape::ElementView { .. }))
            {
                return unsupported("element-view write does not select a descriptor");
            }
            Ok(path)
        }

        #[track_caller]
        fn prepare_argument_projection<'a>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            path: &[CheckedUnitStructuralPathSegment],
            root: semantic_vocabulary::StructuralTypeId,
            types: &'a [terminal_psi::StructuralTypeDeclaration],
        ) -> Result<(Vec<PreparedStructuralPathSegment>, PreparedArgumentTip<'a>), LoweringError>
        {
            use PreparedArgumentTip as Tip;
            use terminal_psi::{ByteSequenceCarrier, StructuralFieldType, StructuralTypeShape};
            let declaration = |id| {
                let mut matches = types.iter().filter(|declaration| declaration.id == id);
                let found = matches
                    .next()
                    .ok_or(unsupported_error!("argument path lost its source type"))?;
                if matches.next().is_some() {
                    return unsupported("argument path source type is ambiguous");
                }
                Ok(found)
            };
            let mut tip = Tip::Declaration(declaration(root)?);
            let mut selected_case: Option<&terminal_psi::StructuralCaseDeclaration> = None;
            let mut prepared = Vec::with_capacity(path.len());
            for (step_index, step) in path.iter().enumerate() {
                if selected_case.is_some()
                    && !matches!(step, CheckedUnitStructuralPathSegment::Field(_))
                {
                    return unsupported("argument case must select its payload field");
                }
                let shape = match tip {
                    Tip::Declaration(declaration) => Some(&declaration.shape),
                    _ => None,
                };
                let bytes = match tip {
                    Tip::Declaration(declaration) => match declaration.shape {
                        StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                        _ => None,
                    },
                    Tip::Inline(field) => field.byte_sequence_carrier(),
                    Tip::ByteElement | Tip::CollectionWindow => None,
                };
                if matches!(
                    step,
                    CheckedUnitStructuralPathSegment::FixedIndex(_)
                        | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                        | CheckedUnitStructuralPathSegment::ComputedIndex(_)
                ) {
                    let byte_view = matches!(bytes, Some(ByteSequenceCarrier::BorrowedView { .. }));
                    if byte_view || matches!(shape, Some(StructuralTypeShape::ElementView { .. })) {
                        prepared
                            .push(PreparedStructuralPathSegment::ViewExtent { bytes: byte_view });
                    }
                }
                let lowered = match step {
                    CheckedUnitStructuralPathSegment::AllElements => {
                        return unsupported(
                            "universal qualification is not an executable projection",
                        );
                    }
                    CheckedUnitStructuralPathSegment::RepresentationView { .. } => {
                        return unsupported(
                            "a returned view origin is not an executable argument projection",
                        );
                    }
                    CheckedUnitStructuralPathSegment::Case(identity) => {
                        let cases = match shape {
                            Some(
                                StructuralTypeShape::Sum { cases }
                                | StructuralTypeShape::Mixed { cases, .. },
                            ) => cases,
                            _ => {
                                let (selected_type, selected_carrier) = match tip {
                                    Tip::Declaration(declaration) => (
                                        Some(declaration.id),
                                        match &declaration.shape {
                                            StructuralTypeShape::Reference { .. } => {
                                                "reference descriptor"
                                            }
                                            StructuralTypeShape::PrimitiveScalar(_) => {
                                                "primitive scalar"
                                            }
                                            StructuralTypeShape::ByteSequence(_) => "byte sequence",
                                            StructuralTypeShape::ElementView { .. } => {
                                                "element view"
                                            }
                                            StructuralTypeShape::Record { .. } => "record",
                                            StructuralTypeShape::FixedArray { .. } => "fixed array",
                                            StructuralTypeShape::Sum { .. } => "sum",
                                            StructuralTypeShape::Mixed { .. } => {
                                                "record with cases"
                                            }
                                            StructuralTypeShape::AtomicCell(_) => "atomic cell",
                                        },
                                    ),
                                    Tip::Inline(_) => (None, "inline field"),
                                    Tip::ByteElement => (None, "byte element"),
                                    Tip::CollectionWindow => (None, "collection window"),
                                };
                                let site = std::panic::Location::caller();
                                return Err(LoweringError::InvalidArgumentProjection(Box::new(
                                    crate::shared::lowering_error::ArgumentProjectionFailure {
                                        reason: "argument case requires its exact source sum",
                                        machine: None,
                                        root,
                                        selected_type,
                                        selected_carrier,
                                        failed_step: step_index,
                                        segments: path.iter().take(32).cloned().collect(),
                                        segment_count: path.len(),
                                        source_root: None,
                                        source_parameter: None,
                                        source_result: None,
                                        published_root: None,
                                        argument_type: None,
                                        logical_root: None,
                                        root_reference_hops: 0,
                                        access: None,
                                        operation_file: site.file(),
                                        operation_line: site.line(),
                                    },
                                )));
                            }
                        };
                        let mut matches = cases.iter().filter(|case| {
                            case.identity == identities.intern(&identity.spelling())
                        });
                        let case = matches.next().ok_or(unsupported_error!(
                            "argument case belongs to another source type"
                        ))?;
                        if matches.next().is_some() {
                            return unsupported("argument case identity is ambiguous");
                        }
                        selected_case = Some(case);
                        PreparedStructuralPathSegment::Static(StructuralPathSegment::Case(case.id))
                    }
                    CheckedUnitStructuralPathSegment::Field(identity) => {
                        let fields = match selected_case.take() {
                            Some(case) => &case.fields,
                            None => match shape {
                                Some(terminal_psi::record_or_mixed_shape_pattern!(fields)) => {
                                    fields
                                }
                                _ => {
                                    return unsupported(
                                        "argument field requires its source record or selected case",
                                    );
                                }
                            },
                        };
                        let mut matches = fields.iter().filter(|field| {
                            field.identity == identities.intern(&identity.spelling())
                        });
                        let field = matches.next().ok_or(unsupported_error!(
                            "argument field belongs to another declaration"
                        ))?;
                        if matches.next().is_some() || field.relevance.is_erased() {
                            return unsupported("argument field is ambiguous or erased");
                        }
                        tip = match field.field_type {
                            StructuralFieldType::Structural(child) => {
                                Tip::Declaration(declaration(child)?)
                            }
                            _ => Tip::Inline(&field.field_type),
                        };
                        PreparedStructuralPathSegment::Static(StructuralPathSegment::Field(
                            identities.intern(&identity.spelling()),
                        ))
                    }
                    CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        tip = match shape {
                            Some(StructuralTypeShape::FixedArray {
                                element,
                                length: terminal_psi::ArrayExtent::Literal(length),
                            }) => {
                                if index >= length {
                                    return unsupported("argument index exceeds its array");
                                }
                                Tip::Declaration(declaration(*element)?)
                            }
                            Some(StructuralTypeShape::ElementView { element, .. }) => {
                                Tip::Declaration(declaration(*element)?)
                            }
                            _ => match bytes {
                                Some(
                                    ByteSequenceCarrier::BoundedOwned { capacity }
                                    | ByteSequenceCarrier::BoundedView { capacity, .. },
                                ) if *index < capacity => Tip::ByteElement,
                                Some(ByteSequenceCarrier::BorrowedView { .. }) => Tip::ByteElement,
                                _ => {
                                    return unsupported("argument index has no element storage");
                                }
                            },
                        };
                        PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(
                            *index,
                        ))
                    }
                    CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                    | CheckedUnitStructuralPathSegment::ComputedIndex(_) => {
                        tip = match shape {
                            Some(
                                StructuralTypeShape::FixedArray { element, .. }
                                | StructuralTypeShape::ElementView { element, .. },
                            ) => Tip::Declaration(declaration(*element)?),
                            _ if bytes.is_some() => Tip::ByteElement,
                            _ => {
                                return unsupported("argument selector has no element storage");
                            }
                        };
                        match step {
                            CheckedUnitStructuralPathSegment::RuntimeIndex(index) => {
                                PreparedStructuralPathSegment::RuntimeIndex(index.clone())
                            }
                            CheckedUnitStructuralPathSegment::ComputedIndex(root) => {
                                PreparedStructuralPathSegment::ComputedIndex(*root)
                            }
                            _ => unreachable!("matched evaluated structural selector"),
                        }
                    }
                    CheckedUnitStructuralPathSegment::Referent => {
                        let Some(StructuralTypeShape::Reference { referent, .. }) = shape else {
                            return unsupported("argument referent requires a reference carrier");
                        };
                        tip = Tip::Declaration(declaration(*referent)?);
                        PreparedStructuralPathSegment::Static(StructuralPathSegment::Referent)
                    }
                    CheckedUnitStructuralPathSegment::FixedRange { start, end } => {
                        let capacity = match shape {
                            Some(StructuralTypeShape::FixedArray {
                                length: terminal_psi::ArrayExtent::Literal(length),
                                ..
                            }) => Some(*length),
                            _ => match bytes {
                                Some(
                                    ByteSequenceCarrier::BoundedOwned { capacity }
                                    | ByteSequenceCarrier::BoundedView { capacity, .. },
                                ) => Some(capacity),
                                Some(ByteSequenceCarrier::BorrowedView { .. }) => None,
                                _ => {
                                    return unsupported(
                                        "argument window has no collection storage",
                                    );
                                }
                            },
                        };
                        if start > end || capacity.is_some_and(|capacity| *end > capacity) {
                            return unsupported("argument window exceeds its collection storage");
                        }
                        tip = Tip::CollectionWindow;
                        PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedRange {
                            start: *start,
                            end: *end,
                        })
                    }
                };
                prepared.push(lowered);
            }
            if selected_case.is_some() {
                return unsupported("argument case has no payload field");
            }
            Ok((prepared, tip))
        }

        pub(crate) fn lower_static_argument_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            path: &[CheckedUnitStructuralPathSegment],
            root: semantic_vocabulary::StructuralTypeId,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
            lower_typed_argument_path(
                identities,
                path,
                root,
                types,
                None,
                &mut std::iter::empty(),
                None,
            )
        }

        pub(crate) fn lower_reference_origin_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            mut path: &[checked_trees::CheckedReferenceOriginProjection],
            coordinate: ProjectionRoot,
            access: StructuralAccess,
            types: &[terminal_psi::StructuralTypeDeclaration],
        ) -> Result<Vec<terminal_psi::ReferenceOriginProjection>, LoweringError> {
            use checked_trees::CheckedReferenceOriginProjection as CheckedOrigin;
            use terminal_psi::ReferenceOriginProjection as Origin;
            let mut output = Vec::with_capacity(path.len());
            let mut coordinate = coordinate;
            let mut root = match coordinate {
                ProjectionRoot::Published(root)
                | ProjectionRoot::Borrowed {
                    published: root, ..
                }
                | ProjectionRoot::BorrowedEntry {
                    published: root, ..
                } => root,
            };
            loop {
                let next_projection = path.iter().position(|projection| {
                    matches!(
                        projection,
                        CheckedOrigin::Element
                            | CheckedOrigin::Exact(
                                CheckedUnitStructuralPathSegment::RepresentationView { .. }
                                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
                            )
                    )
                });
                let prefix_length = next_projection.unwrap_or(path.len());
                let prefix = path[..prefix_length]
                    .iter()
                    .map(|projection| match projection {
                        CheckedOrigin::Exact(segment) => Ok(segment.clone()),
                        CheckedOrigin::Element => {
                            unsupported("returned origin lost its collection step")
                        }
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?;
                let (prefix, _) = prepare_root_projection(
                    identities, checked, &prefix, coordinate, access, types,
                )?;
                let prefix = emit_argument_path(&prefix, None, &mut std::iter::empty(), None)?;
                let prefix =
                    terminal_semantics::reference_source_storage_path(types.iter(), root, &prefix);
                let prefix = prefix
                    .iter()
                    .cloned()
                    .map(Origin::Exact)
                    .collect::<Vec<_>>();
                root = terminal_semantics::reference_origin_type(types.iter(), root, &prefix)
                    .ok_or(unsupported_error!("returned origin has no typed prefix"))?;
                output.extend(prefix);
                let Some(position) = next_projection else {
                    return Ok(output);
                };
                let projection = match &path[position] {
                    CheckedOrigin::Element => Origin::Element,
                    CheckedOrigin::Exact(CheckedUnitStructuralPathSegment::FixedIndex(index)) => {
                        Origin::Exact(StructuralPathSegment::FixedIndex(*index))
                    }
                    CheckedOrigin::Exact(
                        CheckedUnitStructuralPathSegment::RepresentationView {
                            type_identity,
                            storage,
                        },
                    ) => {
                        let checked_trees::CheckedReferenceStorage::View { stored_scalar } =
                            storage
                        else {
                            return unsupported("returned view has no formation codec");
                        };
                        let declaration =
                            crate::shared::terminal_identities::structural_declaration_by_identity(
                                types,
                                identities,
                                checked,
                                *type_identity,
                                "returned view lost its checked declaration",
                            )?;
                        terminal_semantics::representation_view_declaration(
                            types.iter(),
                            declaration.id,
                            *stored_scalar,
                        )
                        .ok_or(unsupported_error!(
                            "returned view has an incompatible storage codec"
                        ))?;
                        Origin::Exact(StructuralPathSegment::RepresentationView {
                            structural_type: declaration.id,
                            stored_scalar: *stored_scalar,
                        })
                    }
                    _ => return unsupported("returned origin has an unexpected projection"),
                };
                root = terminal_semantics::reference_origin_type(
                    types.iter(),
                    root,
                    std::slice::from_ref(&projection),
                )
                .ok_or(unsupported_error!(
                    "returned origin has an invalid typed projection"
                ))?;
                output.push(projection);
                path = &path[position + 1..];
                coordinate = ProjectionRoot::Published(root);
            }
        }

        pub(crate) fn lower_typed_argument_path(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            path: &[CheckedUnitStructuralPathSegment],
            root: semantic_vocabulary::StructuralTypeId,
            types: &[terminal_psi::StructuralTypeDeclaration],
            caller_scalar_values: Option<&[ValueDeclaration]>,
            selector_values: &mut dyn Iterator<Item = ValueId>,
            calls: Option<&mut CallEmissionContext<'_>>,
        ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
            emit_argument_path(
                &prepare_argument_path(identities, path, root, types)?,
                caller_scalar_values,
                selector_values,
                calls,
            )
        }

        pub(crate) fn emit_argument_path(
            path: &[PreparedStructuralPathSegment],
            caller_scalar_values: Option<&[ValueDeclaration]>,
            selector_values: &mut dyn Iterator<Item = ValueId>,
            calls: Option<&mut CallEmissionContext<'_>>,
        ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
            let mut calls = calls;
            path.iter()
                .map(|segment| match segment {
                    PreparedStructuralPathSegment::EvaluatedFixedIndex { index, .. } => {
                        selector_values.next().ok_or(unsupported_error!(
                            "fixed selector lost its authored evaluation"
                        ))?;
                        Ok(StructuralPathSegment::FixedIndex(*index))
                    }
                    PreparedStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::Parameter { position }
                        | checked_trees::CheckedRuntimeIndex::Local { position },
                    ) => {
                        let (Some(values), Some(calls)) =
                            (caller_scalar_values, calls.as_deref_mut())
                        else {
                            return unsupported(
                                "runtime index argument path has no caller scalar frame",
                            );
                        };
                        let index = values
                            .get(usize::try_from(*position).map_err(|_| {
                                unsupported_error!("runtime index position exceeds usize")
                            })?)
                            .ok_or(unsupported_error!(
                                "runtime index selector position has no caller scalar"
                            ))?;
                        if !matches!(
                            index.scalar_type,
                            ScalarType::Integer(integer) if !integer.is_address()
                        ) {
                            return unsupported("runtime index selector is not an integer operand");
                        }
                        calls.allocate_requirement().map(|obligation| {
                            terminal_psi::StructuralPathSegment::RuntimeIndex {
                                index: index.id,
                                obligation,
                            }
                        })
                    }
                    PreparedStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::StructuralField(_)
                        | checked_trees::CheckedRuntimeIndex::StorageRead { .. },
                    )
                    | PreparedStructuralPathSegment::ComputedIndex(_) => {
                        let Some(calls) = calls.as_deref_mut() else {
                            return unsupported(
                                "runtime index argument path has no caller scalar frame",
                            );
                        };
                        let index = selector_values.next().ok_or(unsupported_error!(
                            "field-selector runtime index has no evaluated operand"
                        ))?;
                        calls.allocate_requirement().map(|obligation| {
                            terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation }
                        })
                    }
                    PreparedStructuralPathSegment::RuntimeIndex(_) => unsupported(
                        "an assignment-index selector is owned by its store destination",
                    ),
                    PreparedStructuralPathSegment::CapturedIndex(_) => {
                        unsupported("captured loan selector requires its operand emission frame")
                    }
                    PreparedStructuralPathSegment::ViewExtent { .. } => {
                        unsupported("a view element projection requires its emitting operand frame")
                    }
                    PreparedStructuralPathSegment::Static(segment) => Ok(segment.clone()),
                })
                .collect()
        }

        pub(crate) fn emit_captured_argument_path(
            path: &[PreparedStructuralPathSegment],
            source: semantic_vocabulary::PlaceId,
            caller_values: Option<&[ValueDeclaration]>,
            selectors: &mut dyn Iterator<Item = ValueId>,
            mut calls: Option<&mut CallEmissionContext<'_>>,
            emission: Option<(&mut u64, &mut OperationBuffer)>,
        ) -> Result<Vec<StructuralPathSegment>, LoweringError> {
            use crate::emission::operation_emission::expressions::{
                emit_byte_length, emit_element_length, emit_scalar_leaf,
            };
            let mut emission = emission;
            let mut output = Vec::new();
            let mut dynamic_extent = false;
            for segment in path {
                if let PreparedStructuralPathSegment::CapturedIndex(place) = segment {
                    let Some((next, operations)) = emission.as_mut() else {
                        return unsupported("captured loan selector has no emitting frame");
                    };
                    let scalar = ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .expect("u64"),
                    );
                    let index = emit_scalar_leaf(
                        terminal_psi::OperationKind::PrimitiveScalarRead {
                            source: *place,
                            path: Vec::new(),
                        },
                        scalar,
                        next,
                        operations,
                    );
                    let obligation = calls
                        .as_deref_mut()
                        .ok_or(unsupported_error!(
                            "captured loan selector has no obligation frame"
                        ))?
                        .allocate_requirement()?;
                    output.push(StructuralPathSegment::RuntimeIndex { index, obligation });
                    dynamic_extent = false;
                    continue;
                }
                if let PreparedStructuralPathSegment::ViewExtent { bytes } = segment {
                    let Some((next_value, operations)) = emission.as_mut() else {
                        return unsupported("view projection has no operand emission frame");
                    };
                    if *bytes {
                        emit_byte_length(
                            structural_argument(
                                source,
                                output.clone(),
                                terminal_psi::StructuralAccess::SharedBorrow,
                            ),
                            next_value,
                            operations,
                        );
                    } else {
                        emit_element_length(
                            structural_argument(
                                source,
                                output.clone(),
                                terminal_psi::StructuralAccess::SharedBorrow,
                            ),
                            next_value,
                            operations,
                        );
                    }
                    dynamic_extent = true;
                    continue;
                }
                let fixed_index = match segment {
                    PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(
                        index,
                    )) => Some(*index),
                    PreparedStructuralPathSegment::EvaluatedFixedIndex { index, .. } => {
                        selectors.next().ok_or(unsupported_error!(
                            "fixed selector lost its authored evaluation"
                        ))?;
                        Some(*index)
                    }
                    _ => None,
                };
                if dynamic_extent && let Some(index) = fixed_index {
                    let Some((next_value, operations)) = emission.as_mut() else {
                        unreachable!("extent required emission frame")
                    };
                    let scalar_type = ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .expect("u64"),
                    );
                    let index = emit_scalar_leaf(
                        terminal_psi::OperationKind::IntegerConstant {
                            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(index)),
                        },
                        scalar_type,
                        next_value,
                        operations,
                    );
                    let obligation = calls
                        .as_deref_mut()
                        .ok_or(unsupported_error!(
                            "view projection has no obligation frame"
                        ))?
                        .allocate_requirement()?;
                    output.push(StructuralPathSegment::RuntimeIndex { index, obligation });
                } else if let Some(index) = fixed_index {
                    output.push(StructuralPathSegment::FixedIndex(index));
                } else {
                    output.extend(emit_argument_path(
                        std::slice::from_ref(segment),
                        caller_values,
                        selectors,
                        calls.as_deref_mut(),
                    )?);
                }
                dynamic_extent = false;
            }
            Ok(output)
        }
    }
    pub(crate) mod view_locals {
        use super::{LoweringError, PlaceId, ScalarType, StructuralTypeId, unsupported};
        use terminal_psi::{
            ByteSequenceCarrier, StructuralTypeDeclaration, StructuralTypeShape,
            find_structural_type,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ViewCarrier {
            Bytes,
            FixedBytes,
            Elements { element: Option<ScalarType> },
        }

        impl ViewCarrier {
            pub(crate) fn observation_of(
                structural_type: StructuralTypeId,
                types: &[StructuralTypeDeclaration],
            ) -> Option<Self> {
                Self::of(structural_type, types).or_else(|| {
                    let declaration = find_structural_type(types, structural_type)?;
                    (matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                        && terminal_semantics::fixed_byte_extent(types.iter(), structural_type)
                            .is_some())
                    .then_some(Self::FixedBytes)
                })
            }

            pub(crate) fn of(
                structural_type: StructuralTypeId,
                types: &[StructuralTypeDeclaration],
            ) -> Option<Self> {
                let declaration = find_structural_type(types, structural_type)?;
                match declaration.shape {
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::byte_sequence_view_carrier_pattern!()
                        | ByteSequenceCarrier::BoundedOwned { .. },
                    ) => Some(Self::Bytes),
                    StructuralTypeShape::ElementView { element, .. } => Some(Self::Elements {
                        element: find_structural_type(types, element).and_then(|declaration| {
                            match declaration.shape {
                                StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                                    Some(scalar_type)
                                }
                                _ => None,
                            }
                        }),
                    }),
                    _ => None,
                }
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ViewLocalBinding {
            pub(crate) symbol: symbols::SymbolHandle,
            pub(crate) place: PlaceId,
            pub(crate) structural_type: StructuralTypeId,
            pub(crate) carrier: ViewCarrier,
            pub(crate) access: terminal_psi::StructuralAccess,
        }

        pub(crate) fn resolve(
            view_locals: &[ViewLocalBinding],
            symbol: symbols::SymbolHandle,
        ) -> Result<ViewLocalBinding, LoweringError> {
            let mut matching = view_locals.iter().filter(|local| local.symbol == symbol);
            let Some(local) = matching.next() else {
                return unsupported("view observation names no established view local");
            };
            if !symbol.is_valid() || matching.next().is_some() {
                return unsupported("view local identity is missing or duplicated");
            }
            Ok(*local)
        }
    }
    pub(crate) use structural_fields::{StructuralFieldBindings, StructuralScalarFieldBinding};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum LoanAccess {
        Shared,
        Mutable,
    }

    impl LoanAccess {
        pub(crate) fn structural_access(self) -> StructuralAccess {
            match self {
                Self::Shared => StructuralAccess::SharedBorrow,
                Self::Mutable => StructuralAccess::MutableBorrow,
            }
        }
    }

    #[derive(Clone, Debug)]
    pub(crate) struct StructuralLocalBinding {
        pub(crate) result_binding_ordinal: Option<u32>,
        pub(crate) root: facts::PlaceRoot,
        pub(crate) argument: StructuralArgument,
        pub(crate) structural_type: StructuralTypeId,
    }

    #[track_caller]
    pub(crate) fn retain_completed_structural_homes(
        locals: &mut Vec<StructuralLocalBinding>,
        homes: &[StructuralLocalBinding],
    ) -> Result<(), LoweringError> {
        for home in homes {
            if let Some(existing) = locals.iter_mut().find(|local| local.root == home.root) {
                if existing.argument != home.argument
                    || existing.structural_type != home.structural_type
                {
                    let producer = std::panic::Location::caller();
                    return Err(LoweringError::ConflictingStructuralValueHome {
                        root: home.root,
                        established: existing.argument.clone(),
                        established_type: existing.structural_type,
                        requested: home.argument.clone(),
                        requested_type: home.structural_type,
                        producer_file: producer.file(),
                        producer_line: producer.line(),
                    });
                }
                if let Some(ordinal) = home.result_binding_ordinal {
                    if existing
                        .result_binding_ordinal
                        .is_some_and(|existing| existing != ordinal)
                    {
                        return unsupported("computed storage has conflicting result aliases");
                    }
                    existing.result_binding_ordinal = Some(ordinal);
                }
            } else {
                locals.push(home.clone());
            }
        }
        Ok(())
    }

    pub(crate) fn structural_local_by_root(
        locals: &[StructuralLocalBinding],
        root: facts::PlaceRoot,
    ) -> Option<&StructuralLocalBinding> {
        locals.iter().find(|local| local.root == root)
    }

    fn valid_structural_root(root: facts::PlaceRoot) -> bool {
        match root {
            facts::PlaceRoot::Symbol(symbol) => symbol.is_valid(),
            facts::PlaceRoot::Expression(expression) => expression.is_valid(),
            _ => false,
        }
    }

    pub(crate) fn owned_structural_home(
        root: facts::PlaceRoot,
        source_parameters: &[checked_trees::signature::StateParameter],
        parameters: &[(u32, StructuralParameterDeclaration)],
        locals: &[StructuralLocalBinding],
    ) -> Result<(StructuralArgument, StructuralTypeId), LoweringError> {
        let mut bindings = locals.iter().filter(|binding| binding.root == root);
        let home = if let Some(binding) = bindings.next() {
            if bindings.next().is_some() {
                return unsupported("owned storage has duplicate source bindings");
            }
            (binding.argument.clone(), binding.structural_type)
        } else {
            let position = source_parameters
                .iter()
                .position(|parameter| Some(parameter.symbol) == root.as_symbol())
                .ok_or(LoweringError::Unsupported(
                    "owned storage has no source binding",
                ))?;
            let (_, parameter) = parameters
                .iter()
                .find(|(source, _)| *source as usize == position)
                .ok_or(LoweringError::Unsupported(
                    "owned storage lost its structural parameter",
                ))?;
            (
                StructuralArgument {
                    place: parameter.place,
                    path: Vec::new(),
                    access: parameter.access,
                },
                parameter.structural_type,
            )
        };
        if home.0.access != StructuralAccess::Owned {
            return unsupported("loan completion cannot establish borrowed ingress custody");
        }
        Ok(home)
    }

    fn declared_structural_types<'a>(
        parameters: &'a [(u32, StructuralParameterDeclaration)],
        locals: &'a [StructuralLocalBinding],
        completed: &'a [terminal_psi::StructuralPlaceDeclaration],
    ) -> impl Iterator<Item = (PlaceId, StructuralTypeId)> + 'a {
        parameters
            .iter()
            .map(|(_, parameter)| (parameter.place, parameter.structural_type))
            .chain(
                locals
                    .iter()
                    .filter(|local| local.argument.path.is_empty())
                    .map(|local| (local.argument.place, local.structural_type)),
            )
            .chain(completed.iter().filter_map(|declaration| {
                declaration
                    .kind
                    .produced_structural_type()
                    .map(|carrier| (declaration.id, carrier))
            }))
    }

    pub(crate) fn published_structural_type(
        parameters: &[(u32, StructuralParameterDeclaration)],
        locals: &[StructuralLocalBinding],
        completed: &[terminal_psi::StructuralPlaceDeclaration],
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        let mut types = declared_structural_types(parameters, locals, completed)
            .filter_map(|(candidate, carrier)| (candidate == place).then_some(carrier));
        let carrier = types.next()?;
        types
            .all(|candidate| candidate == carrier)
            .then_some(carrier)
    }

    pub(crate) struct ResolvedBorrowedArgument {
        pub(crate) source: StructuralArgument,
        pub(crate) argument: checked_trees::CheckedUnitStructuralArgumentPlan,
        captured: Vec<PlaceId>,
        pub(crate) root_type_identity: Option<typed_trees::type_identity::TypeIdentityId>,
    }

    impl ResolvedBorrowedArgument {
        pub(crate) fn prepare_path(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            root: StructuralTypeId,
            types: &[StructuralTypeDeclaration],
        ) -> Result<Vec<structural_paths::PreparedStructuralPathSegment>, LoweringError> {
            let mut path = structural_paths::prepare_structural_argument_path(
                identities,
                checked,
                &self.argument,
                root,
                types,
            )?;
            self.retain_captured_coordinates(&mut path)?;
            Ok(path)
        }

        pub(crate) fn retain_captured_coordinates(
            &self,
            path: &mut [structural_paths::PreparedStructuralPathSegment],
        ) -> Result<(), LoweringError> {
            let mut coordinates = self.captured.iter().copied();
            for segment in path {
                if matches!(
                    segment,
                    structural_paths::PreparedStructuralPathSegment::RuntimeIndex(_)
                ) && let Some(place) = coordinates.next()
                {
                    *segment =
                        structural_paths::PreparedStructuralPathSegment::CapturedIndex(place);
                }
            }
            if coordinates.next().is_some() {
                return unsupported("captured loan selectors exceed its checked prefix");
            }
            Ok(())
        }

        pub(crate) fn captured_coordinate(&self) -> Option<PlaceId> {
            self.captured.first().copied()
        }
    }

    #[derive(Clone, Copy)]
    struct AuthoredScalarBinding {
        value_position: usize,
        scalar_type: ScalarType,
        storage_symbol: symbols::SymbolHandle,
    }

    #[derive(Clone)]
    pub(crate) struct ScalarBindings {
        callables: std::sync::Arc<crate::callable_catalog::PreparedCallables>,
        immutable: Vec<Option<usize>>,
        authored: Vec<Option<AuthoredScalarBinding>>,
        parameter_count: usize,
        primitive_storage: Vec<(symbols::SymbolHandle, PlaceId, ScalarType)>,
        scalar_parameter_homes: Vec<(u32, PlaceId, ScalarType, StructuralAccess)>,
        structural_parameters: Vec<(u32, StructuralParameterDeclaration)>,
        captured_structural_formals: Vec<CapturedStructuralFormal>,
        captured_storage_observations: Vec<(
            checked_trees::CheckedStorageRoot,
            StructuralArgument,
            StructuralTypeId,
        )>,
        captured_place_declarations: Vec<terminal_psi::StructuralPlaceDeclaration>,
        physical_root_types: Vec<(PlaceId, StructuralTypeId)>,
        structural_locals: Vec<StructuralLocalBinding>,
        structural_fields: StructuralFieldBindings,
        structural_cases: Vec<structural_cases::StructuralCaseBinding>,
        element_views: std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
        view_locals: Vec<view_locals::ViewLocalBinding>,
        place_observations: std::sync::Arc<[PlaceObservation]>,
    }

    impl ScalarBindings {
        pub(crate) fn with_callables(
            mut self,
            callables: std::sync::Arc<crate::callable_catalog::PreparedCallables>,
        ) -> Self {
            self.callables = callables;
            self
        }

        pub(crate) fn with_callables_from(mut self, source: &Self) -> Self {
            self.callables = std::sync::Arc::clone(&source.callables);
            self
        }

        pub(crate) fn callables(&self) -> &crate::callable_catalog::PreparedCallables {
            &self.callables
        }

        pub(crate) fn scalar_type(
            &self,
            scalar: checked_trees::CheckedScalarType,
        ) -> Result<ScalarType, LoweringError> {
            self.callables.types.scalar_type(scalar)
        }

        pub(crate) fn with_completed_storage_observation(
            mut self,
            root: checked_trees::CheckedStorageRoot,
            argument: StructuralArgument,
            structural_type: StructuralTypeId,
        ) -> Self {
            self.captured_storage_observations
                .retain(|(candidate, _, _)| *candidate != root);
            self.captured_storage_observations
                .push((root, argument, structural_type));
            self
        }

        pub(crate) fn storage_observation_home(
            &self,
            root: checked_trees::CheckedStorageRoot,
        ) -> Result<(terminal_psi::StructuralArgument, StructuralTypeId), LoweringError> {
            let home = self
                .captured_storage_home(root)?
                .map(Ok)
                .unwrap_or_else(|| {
                    self.structural_fields.storage_home(
                        root,
                        &self.structural_locals,
                        &self.view_locals,
                    )
                })?;
            if !matches!(
                home.0.access,
                StructuralAccess::Owned | terminal_psi::reading_borrow_pair_pattern!()
            ) {
                return unsupported("storage observation requires readable access");
            }
            Ok(home)
        }

        pub(crate) fn owned_structural_home(
            &self,
            root: facts::PlaceRoot,
            source_parameters: &[checked_trees::signature::StateParameter],
        ) -> Result<(StructuralArgument, StructuralTypeId), LoweringError> {
            owned_structural_home(
                root,
                source_parameters,
                &self.structural_parameters,
                &self.structural_locals,
            )
        }
        fn scalar_parameter_home_row(
            &self,
            index: u32,
        ) -> Option<&(u32, PlaceId, ScalarType, StructuralAccess)> {
            self.scalar_parameter_homes
                .iter()
                .find(|row| row.0 == index)
        }

        fn published_borrowed_home(
            &self,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            parameters: &[checked_trees::signature::StateParameter],
        ) -> Result<StructuralArgument, LoweringError> {
            let access = structural_paths::terminal_access(argument.access);
            if access == StructuralAccess::Owned {
                return unsupported("computed argument requires borrowed access");
            }
            let supports_borrow = |source| access.admits(source);
            if let Some(captured) = self.captured_formal_home(argument)? {
                if !supports_borrow(captured.access) {
                    return unsupported("captured formal observation widens source access");
                }
                return Ok(StructuralArgument {
                    access: structural_paths::terminal_access(argument.access),
                    ..captured
                });
            }
            let storage_root = argument
                .structural_storage_root()
                .or(match argument.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                    symbol,
                } => Some(facts::PlaceRoot::Symbol(symbol)),
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root:
                        checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
                        | checked_trees::CheckedStorageRoot::ViewLocal { symbol },
                    ..
                }
                | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    root:
                        checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
                        | checked_trees::CheckedStorageRoot::ViewLocal { symbol },
                    ..
                } => Some(facts::PlaceRoot::Symbol(symbol)),
                _ => None,
            });
            let result_ordinal = argument.source_structural_result_binding_ordinal();
            let place = match argument.source {
                _ if storage_root.is_some() || result_ordinal.is_some() => {
                    let mut locals = self.structural_locals.iter().filter(|row| {
                        storage_root.is_some_and(|root| row.root == root)
                            || result_ordinal
                                .is_some_and(|ordinal| row.result_binding_ordinal == Some(ordinal))
                    });
                    match locals.next() {
                        Some(source) => {
                            if storage_root.is_some_and(|root| !valid_structural_root(root)) {
                                return unsupported(
                                    "computed borrow has an invalid local storage root",
                                );
                            }
                            if locals.next().is_some() {
                                return unsupported(
                                    "computed borrow has ambiguous local storage homes",
                                );
                            }
                            if !supports_borrow(source.argument.access) {
                                return unsupported(
                                    "computed borrow widens its local storage access",
                                );
                            }
                            if self.structural_argument_type(&source.argument)
                                != Some(source.structural_type)
                            {
                                return unsupported(
                                    "computed borrow disagrees with its published storage type",
                                );
                            }
                            source.argument.clone()
                        }
                        None => {
                            let Some(symbol) = storage_root.and_then(|root| root.as_symbol())
                            else {
                                return unsupported(
                                    "computed borrow lost its established expression",
                                );
                            };
                            if self.view_locals.iter().any(|local| local.symbol == symbol) {
                                let local = view_locals::resolve(&self.view_locals, symbol)?;
                                if !supports_borrow(local.access) {
                                    return unsupported(
                                        "computed borrow widens its view local custody",
                                    );
                                }
                                return Ok(StructuralArgument::whole(
                                    local.place,
                                    structural_paths::terminal_access(argument.access),
                                ));
                            }
                            let position = typed_trees::signature::state_parameter_position(
                                parameters, symbol,
                            )
                            .ok_or(LoweringError::Unsupported(
                                "computed borrow lost its established local",
                            ))?;
                            let (_, source) = self
                                .structural_parameters
                                .iter()
                                .find(|(place_position, _)| *place_position as usize == position)
                                .ok_or(LoweringError::Unsupported(
                                    "computed borrow lost its parameter",
                                ))?;
                            if !supports_borrow(source.access) {
                                return unsupported("computed borrow widens its parameter custody");
                            }
                            StructuralArgument {
                                place: source.place,
                                path: Vec::new(),
                                access: source.access,
                            }
                        }
                    }
                }
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                }
                | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root:
                        checked_trees::CheckedStorageRoot::Parameter {
                            index: parameter_index,
                        },
                    ..
                }
                | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    root:
                        checked_trees::CheckedStorageRoot::Parameter {
                            index: parameter_index,
                        },
                    ..
                } => {
                    let (_, source) = self
                        .structural_parameters
                        .get(parameter_index as usize)
                        .ok_or(LoweringError::Unsupported(
                            "computed borrow lost its parameter",
                        ))?;
                    if !supports_borrow(source.access) {
                        return unsupported("computed borrow widens its parameter custody");
                    }
                    StructuralArgument {
                        place: source.place,
                        path: Vec::new(),
                        access: source.access,
                    }
                }
                _ => return unsupported("computed borrow has no established source"),
            };
            Ok(StructuralArgument {
                access: structural_paths::terminal_access(argument.access),
                ..place
            })
        }

        pub(crate) fn resolve_borrowed_argument(
            &self,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            parameters: &[checked_trees::signature::StateParameter],
        ) -> Result<ResolvedBorrowedArgument, LoweringError> {
            let mut argument = argument.clone();
            let mut captured = Vec::new();
            let mut root_type_identity = None;
            let mut visited = Vec::new();
            while let Some(symbol) = argument
                .structural_storage_root()
                .and_then(|root| root.as_symbol())
            {
                let mut aliases = self
                    .place_observations
                    .iter()
                    .filter(|loan| loan.symbol == symbol);
                let Some(alias) = aliases.next() else { break };
                if aliases.next().is_some() || visited.contains(&symbol) {
                    return unsupported("place loan has duplicate or cyclic origin");
                }
                visited.push(symbol);
                structural_paths::retain_place_loan_presentation(
                    checked,
                    &alias.loan,
                    &mut argument,
                );
                if !structural_paths::structural_access_can_supply(
                    structural_paths::terminal_access(alias.loan.access),
                    structural_paths::terminal_access(argument.access),
                ) {
                    return unsupported("place loan presentation widens its checked access");
                }
                let count = alias
                    .loan
                    .path
                    .iter()
                    .filter(|segment| {
                        matches!(
                            segment,
                            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                        )
                    })
                    .count();
                if alias.selectors.len() != count {
                    return unsupported("place loan selectors were not captured at formation");
                }
                let omitted_crossing = usize::from(argument.borrowed_root_reference_hops() > 0);
                let mut prefix = alias.loan.path.clone();
                prefix.extend(argument.path.iter().skip(omitted_crossing).cloned());
                argument.path = prefix;
                let mut coordinates = alias.selectors.clone();
                coordinates.extend(captured);
                captured = coordinates;
                root_type_identity = Some(
                    crate::shared::terminal_identities::stored_structural_identity(
                        checked,
                        alias.loan.root_type,
                    ),
                );
                argument.source = match alias.loan.root {
                    checked_trees::CheckedPlaceLoanRoot::StructuralParameter { parameter_index } => {
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index,
                        }
                    }
                    checked_trees::CheckedPlaceLoanRoot::Local { symbol } => {
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                            symbol,
                            root_type_identity: checked.normalized_type_identity(alias.loan.root_type),
                            root_reference_hops: 0,
                        }
                    }
                    checked_trees::CheckedPlaceLoanRoot::ScalarParameter { .. } => {
                        return unsupported("structural place loan requires its original scalar home");
                    }
                };
            }
            let source = self.published_borrowed_home(&argument, parameters)?;
            Ok(ResolvedBorrowedArgument {
                source,
                argument,
                captured,
                root_type_identity,
            })
        }

        pub(crate) fn prepared_borrowed_argument(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            parameters: &[checked_trees::signature::StateParameter],
            types: &[StructuralTypeDeclaration],
        ) -> Result<PreparedStructuralArgument, LoweringError> {
            let resolved = self.resolve_borrowed_argument(checked, argument, parameters)?;
            let source = &resolved.source;
            let root = self
                .structural_argument_type(source)
                .ok_or(LoweringError::Unsupported(
                    "borrowed source lost its published selected type",
                ))?;
            let path = source
                .path
                .iter()
                .cloned()
                .map(structural_paths::PreparedStructuralPathSegment::Static)
                .chain(resolved.prepare_path(identities, checked, root, types)?)
                .collect();
            Ok(PreparedStructuralArgument {
                source: source.place.into(),
                root_type: self.structural_place_type(source.place),
                path,
                selector_positions: Vec::new(),
                access: structural_paths::terminal_access(argument.access),
            })
        }

        pub(crate) fn structural_observation_path(
            &self,
            source: &StructuralArgument,
            observed_root: StructuralTypeId,
            path: &[terminal_psi::StructuralPathSegment],
        ) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
            let root = self
                .structural_argument_type(source)
                .ok_or(LoweringError::Unsupported(
                    "structural observation has no established root type",
                ))?;
            let mut selected = source.path.clone();
            if root == observed_root {
                selected.extend_from_slice(path);
                return Ok(selected);
            }
            let declaration = self
                .structural_fields
                .declarations()
                .iter()
                .find(|row| row.id == root);
            if !matches!(declaration.map(|row| &row.shape),
                Some(terminal_psi::StructuralTypeShape::Reference { referent, access })
                    if *referent == observed_root && matches!(access,terminal_psi::reading_borrow_pair_pattern!()))
            {
                return unsupported("structural observation changes its established root type");
            }
            selected.push(terminal_psi::StructuralPathSegment::Referent);
            selected.extend_from_slice(path);
            Ok(selected)
        }

        pub(crate) fn structural_declarations(&self) -> &[StructuralTypeDeclaration] {
            self.structural_fields.declarations()
        }

        pub(crate) fn published_structural_types(
            &self,
        ) -> Result<PublishedStructuralTypes, LoweringError> {
            let physical = PublishedStructuralTypes::from_rows(
                self.physical_root_types
                    .iter()
                    .copied()
                    .chain(declared_structural_types(
                        &self.structural_parameters,
                        &self.structural_locals,
                        &self.captured_place_declarations,
                    )),
            )?;
            let views = PublishedStructuralTypes::from_rows(
                self.view_locals
                    .iter()
                    .filter(|view| physical.get(view.place).is_none())
                    .map(|view| (view.place, view.structural_type)),
            )?;
            PublishedStructuralTypes::from_rows(physical.homes().chain(views.homes()))
        }

        pub(crate) fn structural_place_type(&self, place: PlaceId) -> Option<StructuralTypeId> {
            let mut physical = self
                .physical_root_types
                .iter()
                .copied()
                .chain(declared_structural_types(
                    &self.structural_parameters,
                    &self.structural_locals,
                    &self.captured_place_declarations,
                ))
                .filter_map(|(candidate, carrier)| (candidate == place).then_some(carrier));
            if let Some(first) = physical.next() {
                return physical.all(|other| other == first).then_some(first);
            }
            let mut homes = self.view_locals.iter().filter(|local| local.place == place);
            let first = homes.next()?;
            homes
                .all(|home| home.structural_type == first.structural_type)
                .then_some(first.structural_type)
        }

        pub(crate) fn with_physical_root_types(
            mut self,
            types: impl IntoIterator<Item = (PlaceId, StructuralTypeId)>,
        ) -> Self {
            self.physical_root_types = types.into_iter().collect();
            self
        }

        pub(crate) fn with_completed_place_declarations(
            mut self,
            declarations: impl IntoIterator<Item = terminal_psi::StructuralPlaceDeclaration>,
        ) -> Self {
            self.captured_place_declarations = declarations.into_iter().collect();
            self
        }

        pub(crate) fn structural_argument_type(
            &self,
            source: &StructuralArgument,
        ) -> Option<StructuralTypeId> {
            let root = self.structural_place_type(source.place)?;
            if source.path.is_empty() {
                return Some(root);
            }
            terminal_semantics::runtime_structural_path_tip(
                self.structural_fields.declarations().iter(),
                root,
                &source.path,
            )
            .map(|declaration| declaration.id)
        }

        pub(crate) fn shared_structural_argument(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            parameters: &[checked_trees::signature::StateParameter],
        ) -> Result<StructuralArgument, LoweringError> {
            if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
                return unsupported("computed shared argument changes its access");
            }
            self.borrowed_structural_argument(identities, checked, argument, parameters)
        }

        pub(crate) fn borrowed_structural_argument(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            parameters: &[checked_trees::signature::StateParameter],
        ) -> Result<StructuralArgument, LoweringError> {
            if matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
            ) {
                return self.scalar_parameter_borrow(
                    identities,
                    checked,
                    argument,
                    self.structural_declarations(),
                );
            }
            let resolved = self.resolve_borrowed_argument(checked, argument, parameters)?;
            let mut source = resolved.source.clone();
            let root = self
                .structural_argument_type(&source)
                .ok_or(LoweringError::Unsupported(
                    "borrowed argument lost its published selected type",
                ))?;
            let prepared =
                resolved.prepare_path(identities, checked, root, self.structural_declarations())?;
            source.path.extend(structural_paths::emit_argument_path(
                &prepared,
                None,
                &mut std::iter::empty(),
                None,
            )?);
            Ok(source)
        }

        pub(crate) fn element_case_observation(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            source: &StructuralArgument,
            case: &str,
            captured_coordinate: Option<PlaceId>,
        ) -> Result<LoweredBooleanReturnExpression, LoweringError> {
            let structural_type =
                self.structural_argument_type(source)
                    .ok_or(LoweringError::Unsupported(
                        "case observation lost its established source type",
                    ))?;
            let mut runtime = argument.path.iter().enumerate().filter(|(_, segment)| {
                matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                )
            });
            let Some((
                selector_position,
                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector),
            )) = runtime.next()
            else {
                return unsupported("runtime case observation has no runtime element");
            };
            if runtime.next().is_some() {
                return unsupported("runtime case observation selects more than one element");
            }
            let static_path = |segments: &[checked_trees::CheckedUnitStructuralPathSegment]| {
                segments
                    .iter()
                    .map(|segment| match segment {
                        checked_trees::CheckedUnitStructuralPathSegment::Referent => {
                            Ok(checked_trees::CheckedStructuralPredicatePathSegment::Referent)
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                            Ok(checked_trees::CheckedStructuralPredicatePathSegment::Field(
                                identity.clone(),
                            ))
                        }
                        checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => Ok(
                            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(
                                *index,
                            ),
                        ),
                        checked_trees::CheckedUnitStructuralPathSegment::Case(identity) => {
                            Ok(checked_trees::CheckedStructuralPredicatePathSegment::Case(
                                identity.clone(),
                            ))
                        }
                        _ => unsupported(
                            "runtime case observation has an unsupported static projection",
                        ),
                    })
                    .collect::<Result<Vec<_>, _>>()
            };
            let (place, path, element_path, case, observes_element_view_length) =
                structural_cases::resolve_runtime_element(
                    identities,
                    self.structural_fields.declarations(),
                    structural_type,
                    source.place,
                    &static_path(&argument.path[..selector_position])?,
                    &static_path(&argument.path[selector_position + 1..])?,
                    &checked_trees::CheckedMemberIdentity::of_spelling(case),
                )?;
            let index = if let Some(place) = captured_coordinate {
                // Formation normalizes and checks every selector snapshot as u64.
                LoweredDirectExpression::PrimitiveRead {
                    qualifications: Default::default(),
                    source:
                        crate::emission::operation_emission::expressions::PrimitiveReadSource::Place(
                            place,
                        ),
                    path: Vec::new(),
                    scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
                }
            } else {
                match selector {
                    checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                        self.runtime_index_storage(*symbol)?
                    }
                    checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                        self.runtime_index_parameter(*position)?
                    }
                    checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                        structural_fields::resolve_typed(
                            identities,
                            &self.structural_fields,
                            field.parameter_position,
                            &field.path,
                        )?
                    }
                    checked_trees::CheckedRuntimeIndex::Local { position } => {
                        self.runtime_index_local(*position)?
                    }
                    _ => {
                        return unsupported(
                            "runtime case observation requires a bound scalar selector",
                        );
                    }
                }
            };
            if !matches!(
                index.scalar_type(),
                ScalarType::Integer(integer) if !integer.is_address()
            ) {
                return unsupported("runtime case observation selector is not an integer read");
            }
            if observes_element_view_length && !source.path.is_empty() {
                return unsupported("nested element view requires its established descriptor home");
            }
            let mut selected = source.path.clone();
            selected.extend(path);
            Ok(LoweredBooleanReturnExpression::IndexedCaseMembership {
                source: place,
                observes_element_view_length,
                path: selected,
                index: Box::new(index),
                element_path,
                case,
            })
        }

        pub(crate) fn with_structural_observations(
            mut self,
            types: &[StructuralTypeDeclaration],
        ) -> Self {
            self.structural_fields =
                StructuralScalarFieldBinding::collect(&self.structural_parameters, types);
            self.structural_cases = structural_cases::StructuralCaseBinding::collect(
                &self.structural_parameters,
                types,
            );
            self.element_views = element_views(&self.structural_parameters, types);
            self
        }

        pub(crate) fn with_element_views(
            mut self,
            views: &std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>>,
        ) -> Self {
            self.element_views
                .extend(views.iter().map(|(view, element)| (*view, *element)));
            self
        }

        pub(crate) fn with_resolved_structural_observations(
            mut self,
            fields: &StructuralFieldBindings,
            cases: &[structural_cases::StructuralCaseBinding],
        ) -> Self {
            self.structural_fields = fields.clone();
            self.structural_cases = cases.to_vec();
            self
        }

        pub(crate) fn for_computation_operands(
            offset: usize,
            types: impl IntoIterator<Item = ScalarType>,
        ) -> Self {
            let authored = types
                .into_iter()
                .enumerate()
                .map(|(position, scalar_type)| {
                    Some(AuthoredScalarBinding {
                        value_position: offset + position,
                        scalar_type,
                        storage_symbol: symbols::SymbolHandle::invalid(),
                    })
                })
                .collect::<Vec<_>>();
            Self {
                immutable: authored
                    .iter()
                    .map(|binding| binding.map(|row| row.value_position))
                    .collect(),
                parameter_count: authored.len(),
                authored,
                primitive_storage: Vec::new(),
                scalar_parameter_homes: Vec::new(),
                structural_parameters: Vec::new(),
                captured_structural_formals: Vec::new(),
                captured_storage_observations: Vec::new(),
                captured_place_declarations: Vec::new(),
                physical_root_types: Vec::new(),
                structural_locals: Vec::new(),
                structural_fields: Default::default(),
                structural_cases: Vec::new(),
                element_views: std::collections::BTreeMap::new(),
                view_locals: Vec::new(),
                place_observations: std::sync::Arc::from([]),
                callables: std::sync::Arc::new(
                    crate::callable_catalog::PreparedCallables::default(),
                ),
            }
        }

        pub(crate) fn with_computation_operands(
            mut self,
            offset: usize,
            types: impl IntoIterator<Item = ScalarType>,
        ) -> Self {
            let operands = Self::for_computation_operands(offset, types);
            self.authored = operands.authored;
            self.immutable = operands.immutable;
            self.parameter_count = operands.parameter_count;
            self
        }

        pub(crate) fn new(parameters: impl IntoIterator<Item = ScalarType>) -> Self {
            Self::for_computation_operands(0, parameters)
        }

        pub(crate) fn entry(
            formals: impl IntoIterator<Item = ScalarType>,
            primitive_storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
            parameters: &[(u32, StructuralParameterDeclaration)],
            structural_types: &[StructuralTypeDeclaration],
            place_observations: &std::sync::Arc<[PlaceObservation]>,
        ) -> Self {
            Self::new(formals)
                .with_primitive_storage(primitive_storage)
                .with_structural_parameters(parameters)
                .with_structural_observations(structural_types)
                .with_place_observations(place_observations)
        }

        pub(crate) fn with_structural_parameters(
            mut self,
            parameters: &[(u32, StructuralParameterDeclaration)],
        ) -> Self {
            self.structural_parameters = parameters.to_vec();
            self.captured_structural_formals.clear();
            self.structural_fields.clear_parameters();
            self.structural_cases.clear();
            self
        }

        pub(crate) fn with_primitive_storage(
            mut self,
            storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
        ) -> Self {
            self.primitive_storage = storage.to_vec();
            self
        }

        pub(crate) fn register_scalar_parameter_home(
            &mut self,
            parameter_index: u32,
            place: PlaceId,
            scalar_type: ScalarType,
            access: StructuralAccess,
        ) -> Result<(), LoweringError> {
            if let Some((_, original, original_type, original_access)) =
                self.scalar_parameter_home_row(parameter_index)
            {
                if *original != place || *original_type != scalar_type || *original_access != access
                {
                    return unsupported("scalar ingress changes its original home");
                }
            } else {
                self.scalar_parameter_homes
                    .push((parameter_index, place, scalar_type, access));
            }
            Ok(())
        }

        pub(crate) fn scalar_parameter_home(
            &self,
            parameter_index: u32,
            scalar_type: ScalarType,
        ) -> Result<Option<PlaceId>, LoweringError> {
            match self.scalar_parameter_home_row(parameter_index) {
                Some((_, place, actual, _)) if *actual == scalar_type => Ok(Some(*place)),
                Some(_) => unsupported("scalar ingress home changes its carrier"),
                None => Ok(None),
            }
        }

        pub(crate) fn with_view_locals(mut self, locals: &[view_locals::ViewLocalBinding]) -> Self {
            self.view_locals = locals.to_vec();
            self
        }

        pub(crate) fn with_place_observations(
            mut self,
            loans: &std::sync::Arc<[PlaceObservation]>,
        ) -> Self {
            self.place_observations = loans.clone();
            self
        }

        pub(crate) fn observation_coordinate(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let mut matching = self
                .place_observations
                .iter()
                .filter(|loan| loan.symbol == symbol);
            let loan = matching.next().ok_or(LoweringError::Unsupported(
                "record loan has no captured coordinate",
            ))?;
            if !symbol.is_valid() || matching.next().is_some() {
                return unsupported("record loan coordinate has ambiguous identity");
            }
            let capture = loan.capture.as_ref().map_err(Clone::clone)?;
            let source = capture.coordinate.ok_or(LoweringError::Unsupported(
                "record loan coordinate was not captured at formation",
            ))?;
            Ok(LoweredDirectExpression::PrimitiveRead {
                qualifications: Default::default(),
                source: PrimitiveReadSource::Place(source),
                path: Vec::new(),
                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
            })
        }

        pub(crate) fn observation_backing(
            &self,
            source: crate::expression_preparation::bindings::place_observations::LoanedSource,
        ) -> Result<PlaceId, LoweringError> {
            use crate::expression_preparation::bindings::place_observations::LoanedSource;
            match source {
                LoanedSource::Parameter(position) => {
                    let (_, parameter) = self.structural_parameters.get(position as usize).ok_or(
                        LoweringError::Unsupported(
                            "record byte observation lost its backing parameter",
                        ),
                    )?;
                    Ok(parameter.place)
                }
                LoanedSource::Local(symbol) => {
                    let mut homes = self
                        .primitive_storage
                        .iter()
                        .filter(|(candidate, _, _)| *candidate == symbol)
                        .map(|(_, place, _)| *place)
                        .chain(
                            self.structural_locals
                                .iter()
                                .filter(|local| local.root == facts::PlaceRoot::Symbol(symbol))
                                .map(|local| local.argument.place),
                        );
                    let place = homes.next().ok_or(LoweringError::Unsupported(
                        "place observation lost its live local backing",
                    ))?;
                    if !symbol.is_valid() || homes.next().is_some() {
                        return unsupported("place observation has ambiguous local backing");
                    }
                    Ok(place)
                }
            }
        }

        pub(crate) fn byte_record_observation(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            proven_indexes: &[checked_trees::expression::ExpressionHandle],
            symbol: symbols::SymbolHandle,
            path: &[checked_trees::CheckedStructuralPredicatePathSegment],
            index: &CheckedScalarExpression,
            element_path: &[checked_trees::CheckedScalarReadProjection],
            primitive: PrimitiveType,
        ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
            crate::expression_preparation::bindings::place_observations::byte_record_observation(
                identities,
                &self.place_observations,
                self,
                proven_indexes,
                symbol,
                path,
                index,
                element_path,
                primitive,
            )
        }

        pub(crate) fn with_structural_locals(mut self, locals: &[StructuralLocalBinding]) -> Self {
            self.structural_locals = locals.to_vec();
            self
        }

        /// Bind geometry for a subject capture scheduled before this private
        /// expansion. The emitting owner must establish this exact borrowed home
        /// before any child executes; this row confers no ownership or facts.
        pub(crate) fn with_dispatch_subject(
            mut self,
            alias: checked_trees::expression::ExpressionHandle,
            place: PlaceId,
            structural_type: StructuralTypeId,
        ) -> Result<Self, LoweringError> {
            retain_completed_structural_homes(
                &mut self.structural_locals,
                &[StructuralLocalBinding {
                    result_binding_ordinal: None,
                    root: facts::PlaceRoot::Expression(alias),
                    argument: StructuralArgument::whole(place, StructuralAccess::SharedBorrow),
                    structural_type,
                }],
            )?;
            Ok(self)
        }

        /// Resolve the exact primitive endpoint of an already source-validated loan.
        /// A local reference owns a carrier, not a new primitive home: its retained
        /// `Referent` projection reaches the viewed type while the reference keeps
        /// the original backing identity. The same catalog walk checks parameter
        /// and local projections, including authority at every reference crossing.
        /// Root ownership supplies a temporary loan; it is not the argument access.
        /// Authored parameters resolve symbols absent from the completed local roster;
        /// their source positions remain distinct from dense structural positions.
        /// Keep that owner's home and cleanup, then attenuate at every descriptor
        /// crossing so owning a record cannot widen a shared reference inside it.
        pub(crate) fn primitive_borrow(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            scalar_type: ScalarType,
            structural_types: &[StructuralTypeDeclaration],
            parameters: &[checked_trees::signature::StateParameter],
        ) -> Result<StructuralArgument, LoweringError> {
            let source = self.prepared_primitive_borrow(
                identities,
                checked,
                argument,
                scalar_type,
                structural_types,
                parameters,
            )?;
            Ok(StructuralArgument {
                place: source.source.known_place()?,
                access: source.access,
                path: structural_paths::emit_argument_path(
                    &source.path,
                    None,
                    &mut std::iter::empty(),
                    None,
                )?,
            })
        }

        pub(crate) fn scalar_parameter_borrow(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            structural_types: &[StructuralTypeDeclaration],
        ) -> Result<StructuralArgument, LoweringError> {
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                parameter_index,
            } = argument.source
            else {
                return unsupported("scalar formal borrow names another source");
            };
            let (_, _, scalar_type, _) = self.scalar_parameter_home_row(parameter_index).ok_or(
                LoweringError::Unsupported("borrowed scalar formal has no published home"),
            )?;
            self.primitive_borrow(
                identities,
                checked,
                argument,
                *scalar_type,
                structural_types,
                &[],
            )
        }

        pub(crate) fn prepared_primitive_borrow(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

            checked: &checked_trees::CheckedTrees,
            argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
            scalar_type: ScalarType,
            structural_types: &[StructuralTypeDeclaration],
            parameters: &[checked_trees::signature::StateParameter],
        ) -> Result<PreparedStructuralArgument, LoweringError> {
            let access = structural_paths::terminal_access(argument.access);
            let permits = |granted, requested| {
                matches!(
                    (granted, requested),
                    (
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow
                    ) | (
                        terminal_psi::mutating_access_pair_pattern!(),
                        StructuralAccess::SharedBorrow
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    ) | (
                        StructuralAccess::WriteOnlyBorrow,
                        StructuralAccess::WriteOnlyBorrow
                    )
                )
            };
            if access == StructuralAccess::Owned {
                return unsupported("computed primitive argument cannot transfer ownership");
            }
            match argument.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ScalarParameter {
                    parameter_index,
                } => {
                    if !argument.path.is_empty() {
                        return unsupported(
                            "scalar ingress borrow projects outside its primitive home",
                        );
                    }
                    let place = self
                        .scalar_parameter_home(parameter_index, scalar_type)?
                        .ok_or(LoweringError::Unsupported(
                            "computed primitive ingress has no established original home",
                        ))?;
                    let (_, _, _, granted) =
                        self.scalar_parameter_home_row(parameter_index).ok_or(
                            LoweringError::Unsupported("computed ingress lost its access custody"),
                        )?;
                    if !permits(*granted, access) {
                        return unsupported("computed primitive ingress widens original access");
                    }
                    PreparedStructuralArgument::from_static(
                        StructuralArgument {
                            place,
                            path: Vec::new(),
                            access,
                        },
                        None,
                    )
                }
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                    symbol,
                } => {
                    if !argument.path.is_empty() {
                        return unsupported(
                            "computed primitive storage argument is not a whole referent",
                        );
                    }
                    let (place, path) = primitive_reads::primitive_storage_read(
                        &self.primitive_storage,
                        &self.structural_locals,
                        structural_types,
                        symbol,
                        scalar_type,
                    )?;
                    if !path.is_empty()
                        || self.structural_locals.iter().any(|local| {
                            local.root == facts::PlaceRoot::Symbol(symbol)
                                && local.argument.access != StructuralAccess::Owned
                        })
                    {
                        return unsupported("computed primitive local changes original ownership");
                    }
                    PreparedStructuralArgument::from_static(
                        StructuralArgument {
                            place,
                            path,
                            access,
                        },
                        None,
                    )
                }
                _ => self.prepared_borrowed_argument(
                    identities,
                    checked,
                    argument,
                    parameters,
                    structural_types,
                ),
            }
        }

        pub(crate) fn append(&mut self, scalar_type: ScalarType, position: usize) {
            self.immutable.push(Some(position));
            self.authored.push(Some(AuthoredScalarBinding {
                value_position: position,
                scalar_type,
                storage_symbol: symbols::SymbolHandle::invalid(),
            }));
        }

        pub(crate) fn register_local_storage(
            &mut self,
            ordinal: usize,
            symbol: symbols::SymbolHandle,
            scalar_type: ScalarType,
        ) -> Result<(), LoweringError> {
            let position =
                self.parameter_count
                    .checked_add(ordinal)
                    .ok_or(LoweringError::Unsupported(
                        "addressed scalar local namespace overflow",
                    ))?;
            let binding = self
                .authored
                .get_mut(position)
                .and_then(Option::as_mut)
                .ok_or(LoweringError::Unsupported(
                    "addressed scalar local has no completed binding",
                ))?;
            if !symbol.is_valid()
                || binding.scalar_type != scalar_type
                || (binding.storage_symbol.is_valid() && binding.storage_symbol != symbol)
            {
                return unsupported("addressed scalar local changes its original binding");
            }
            binding.storage_symbol = symbol;
            Ok(())
        }

        pub(crate) fn append_unnamed_result(&mut self, position: usize) {
            self.immutable.push(Some(position));
        }

        pub(crate) fn immutable_local_value(
            &self,
            ordinal: usize,
            scalar_type: ScalarType,
        ) -> Result<usize, LoweringError> {
            let position =
                self.parameter_count
                    .checked_add(ordinal)
                    .ok_or(LoweringError::Unsupported(
                        "immutable local namespace overflow",
                    ))?;
            let binding = self.authored.get(position).copied().flatten().ok_or(
                LoweringError::Unsupported("immutable local has no established scalar value"),
            )?;
            if binding.scalar_type != scalar_type {
                return unsupported("immutable local addressable home changes its scalar type");
            }
            Ok(binding.value_position)
        }

        pub(crate) fn unaddressed_value(&self, position: usize) -> Option<(usize, ScalarType)> {
            let binding = self.authored.get(position).copied().flatten()?;
            (!binding.storage_symbol.is_valid())
                .then_some((binding.value_position, binding.scalar_type))
        }

        pub(crate) fn immutable_position(&self, position: usize) -> Result<usize, LoweringError> {
            self.immutable
                .get(position)
                .copied()
                .flatten()
                .ok_or(LoweringError::Unsupported(
                    "scalar immutable operand is outside the established namespace",
                ))
        }

        fn scalar(&self, expression: &mut CheckedScalarExpression) -> Result<(), LoweringError> {
            match expression {
                CheckedScalarExpression::StructuralSequenceLength { path, .. } => {
                    for step in path {
                        if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) =
                            step
                        {
                            self.scalar(&mut selector.expression)?;
                        }
                    }
                }
                CheckedScalarExpression::StructuralParameterIndexedRead {
                    index,
                    element_path,
                    ..
                } => {
                    self.scalar(&mut index.expression)?;
                    for step in element_path {
                        if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) =
                            step
                        {
                            self.scalar(&mut selector.expression)?;
                        }
                    }
                }
                CheckedScalarExpression::Parameter {
                    position,
                    scalar_type,
                }
                | CheckedScalarExpression::Local {
                    position,
                    scalar_type,
                } => {
                    let binding = self.authored.get(*position).copied().flatten().ok_or(
                        LoweringError::Unsupported("scalar read has no authored binding"),
                    )?;
                    if binding.scalar_type != self.scalar_type(*scalar_type)? {
                        return unsupported(
                            "scalar read changes its established type or callable contract",
                        );
                    }
                    if binding.storage_symbol.is_valid() {
                        *expression = CheckedScalarExpression::StorageRead {
                            symbol: binding.storage_symbol,
                            scalar_type: *scalar_type,
                        };
                    } else {
                        *position = binding.value_position;
                    }
                }
                CheckedScalarExpression::StorageRead { symbol, .. } => {
                    if self
                        .place_observations
                        .iter()
                        .any(|row| row.symbol == *symbol)
                        || self.primitive_storage.iter().any(|row| row.0 == *symbol)
                        || self
                            .structural_locals
                            .iter()
                            .any(|row| row.root == facts::PlaceRoot::Symbol(*symbol))
                    {
                        return Ok(());
                    }
                    return unsupported("scalar storage read names no established storage");
                }
                CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                    self.scalar(left)?;
                    self.scalar(right)?;
                }
                CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
                | CheckedScalarExpression::IntegerWiden { operand, .. }
                | CheckedScalarExpression::RepresentationView { operand, .. }
                | CheckedScalarExpression::IntegerExactCast { operand, .. } => {
                    self.scalar(operand)?
                }
                CheckedScalarExpression::Boolean(expression) => self.boolean(expression)?,
                CheckedScalarExpression::ConstantApplication { .. }
                | CheckedScalarExpression::IntegerLiteral { .. }
                | CheckedScalarExpression::IeeeFloatLiteral { .. }
                | CheckedScalarExpression::ImplementationSelection { .. }
                | CheckedScalarExpression::ErasedParameter { .. }
                | CheckedScalarExpression::StructuralParameterField { .. } => {}
            }
            Ok(())
        }

        fn boolean(&self, expression: &mut CheckedBooleanExpression) -> Result<(), LoweringError> {
            match expression {
                CheckedBooleanExpression::IntegerMathComparison { .. } => {
                    return unsupported(
                        "mathematical contract predicate has no executable binding",
                    );
                }
                CheckedBooleanExpression::Parameter { position }
                | CheckedBooleanExpression::Local { position } => {
                    let binding = self.authored.get(*position).copied().flatten().ok_or(
                        LoweringError::Unsupported("Boolean read has no authored binding"),
                    )?;
                    if binding.storage_symbol.is_valid() {
                        if binding.scalar_type != ScalarType::Boolean {
                            return unsupported("addressed Boolean read changes its scalar type");
                        }
                        *expression = CheckedBooleanExpression::StorageRead {
                            symbol: binding.storage_symbol,
                        };
                    } else {
                        *position = binding.value_position;
                    }
                }
                CheckedBooleanExpression::StorageRead { symbol } => {
                    if self
                        .place_observations
                        .iter()
                        .any(|row| row.symbol == *symbol)
                        || self.primitive_storage.iter().any(|row| row.0 == *symbol)
                        || self
                            .structural_locals
                            .iter()
                            .any(|row| row.root == facts::PlaceRoot::Symbol(*symbol))
                    {
                        return Ok(());
                    }
                    return unsupported("Boolean storage read names no established storage");
                }
                CheckedBooleanExpression::Not(operand) => self.boolean(operand)?,
                facts::checked_boolean_triple_pattern!(left, right, ..) => {
                    self.boolean(left)?;
                    self.boolean(right)?;
                }
                CheckedBooleanExpression::IntegerComparison { left, right, .. }
                | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
                    self.scalar(left)?;
                    self.scalar(right)?;
                }
                CheckedBooleanExpression::ByteSequencePredicate { .. } => {}
                CheckedBooleanExpression::Constant(_)
                | CheckedBooleanExpression::StructuralParameterField { .. }
                | CheckedBooleanExpression::IeeeFloatComparison { .. }
                | CheckedBooleanExpression::StructuralIdentity { .. }
                | CheckedBooleanExpression::ByteSequenceEqual { .. }
                | CheckedBooleanExpression::ErasedParameter { .. }
                | CheckedBooleanExpression::StructuralCaseMembership { .. } => {}
            }
            Ok(())
        }

        pub(crate) fn expression_at(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            state: symbols::SymbolHandle,
            statement: u32,
            role: CheckedScalarExpressionRole,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let Some((_, expression)) = value_facts(checked)
                .scalar_expressions
                .bound_expression_at(state, statement, role)
            else {
                let plans = &checked.facts.values.scalar_expressions;
                let mut source_binding_count = 0usize;
                let mut expression_plan_count = 0usize;
                let mut nearby_bindings = Vec::new();
                let mut nearby_plans = Vec::new();
                for (_, binding) in plans
                    .source_bindings
                    .iter()
                    .filter(|(_, binding)| binding.state == state)
                {
                    if binding.statement_ordinal == statement && binding.role == role {
                        source_binding_count += 1;
                    }
                    if binding.statement_ordinal.abs_diff(statement) <= 1
                        && nearby_bindings.len() < 8
                    {
                        nearby_bindings.push((
                            binding.statement_ordinal,
                            binding.role,
                            binding.expression,
                        ));
                    }
                }
                for expression in plans
                    .expressions
                    .iter()
                    .filter(|expression| expression.state == state)
                {
                    if expression.statement_ordinal == statement && expression.role == role {
                        expression_plan_count += 1;
                    }
                    if expression.statement_ordinal.abs_diff(statement) <= 1
                        && nearby_plans.len() < 8
                    {
                        nearby_plans.push((expression.statement_ordinal, expression.role));
                    }
                }
                let (machine, _) = crate::shared::declarations::state_declaration(checked, state)?;
                return Err(LoweringError::InvalidUnitMachinePlan {
                    machine: checked.symbols.display_path(machine.symbol, "::"),
                    reason: "scalar computation needs one checked expression and one source binding",
                    gap: Some(diagnostics::Diagnostic::error(format!(
                        "scalar expression custody mismatch in state `{}` ({state:?}), statement {statement}, role {role:?}: source_bindings={source_binding_count}, expression_plans={expression_plan_count}; nearby_bindings={nearby_bindings:?}; nearby_plans={nearby_plans:?}",
                        checked.symbols.display_path(state, "::"),
                    ))),
                });
            };
            self.expression(identities, &checked.facts.proven_scalar_indexes, expression)
        }

        pub(crate) fn established_case_payload(
            &self,
            source: PlaceId,
            path: &[terminal_psi::StructuralPathSegment],
            case: semantic_vocabulary::StructuralCaseId,
            field: semantic_vocabulary::StructuralFieldId,
            scalar_type: ScalarType,
        ) -> Result<Option<LoweredDirectExpression>, LoweringError> {
            Ok(self
                .structural_fields
                .established_payload(source, path, case, field)?
                .map(|position| LoweredDirectExpression::Parameter {
                    position,
                    scalar_type,
                }))
        }

        pub(crate) fn runtime_index_field(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

            field: &checked_trees::CheckedStructuralParameterField,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let observation = structural_fields::resolve_typed(
                identities,
                &self.structural_fields,
                field.parameter_position,
                &field.path,
            )?;
            if !matches!(observation.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
            {
                return unsupported("runtime index field selector is not an integer read");
            }
            Ok(observation)
        }

        pub(crate) fn runtime_index_parameter(
            &self,
            position: u32,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            if position as usize >= self.parameter_count {
                return unsupported("runtime selector parameter names a non-parameter scalar");
            }
            self.runtime_index_value(position)
        }

        pub(crate) fn runtime_index_local(
            &self,
            position: u32,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            if (position as usize) < self.parameter_count {
                return unsupported("runtime selector local names a parameter scalar");
            }
            self.runtime_index_value(position)
        }

        pub(crate) fn runtime_index_storage(
            &self,
            symbol: symbols::SymbolHandle,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let (source, path, scalar_type) = primitive_reads::primitive_storage_source(
                &self.primitive_storage,
                &self.structural_locals,
                self.structural_fields.declarations(),
                symbol,
            )?;
            if !matches!(scalar_type, ScalarType::Integer(integer) if !integer.is_address()) {
                return unsupported("storage selector is not an integer read");
            }
            Ok(LoweredDirectExpression::PrimitiveRead {
                qualifications: Default::default(),
                source: source.into(),
                path,
                scalar_type,
            })
        }

        fn runtime_index_value(
            &self,
            position: u32,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let binding = self
                .authored
                .get(position as usize)
                .copied()
                .flatten()
                .ok_or(LoweringError::Unsupported(
                    "runtime selector has no established immutable scalar",
                ))?;
            if !matches!(binding.scalar_type, ScalarType::Integer(integer) if !integer.is_address())
            {
                return unsupported("runtime selector is not an integer");
            }
            if binding.storage_symbol.is_valid() {
                let (source, path) = primitive_storage_read(
                    &self.primitive_storage,
                    &self.structural_locals,
                    self.structural_fields.declarations(),
                    binding.storage_symbol,
                    binding.scalar_type,
                )?;
                Ok(LoweredDirectExpression::PrimitiveRead {
                    qualifications: Default::default(),
                    source: source.into(),
                    path,
                    scalar_type: binding.scalar_type,
                })
            } else {
                Ok(LoweredDirectExpression::Parameter {
                    position: binding.value_position,
                    scalar_type: binding.scalar_type,
                })
            }
        }

        pub(crate) fn expression(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,

            proven_indexes: &[checked_trees::expression::ExpressionHandle],
            expression: &CheckedScalarExpression,
        ) -> Result<LoweredDirectExpression, LoweringError> {
            let mut expression = expression.clone();
            self.scalar(&mut expression)?;
            crate::expression_preparation::prepare_expression::PreparedExpressionContext {
                structural_fields: &self.structural_fields,
                structural_cases: &self.structural_cases,
                primitive_storage: &self.primitive_storage,
                view_locals: &self.view_locals,
                structural_locals: &self.structural_locals,
                scalar_bindings: Some(self),
                proven_indexes,
            }
            .lower_scalar(identities, &expression)
        }
    }

    pub(crate) fn element_views(
        parameters: &[(u32, StructuralParameterDeclaration)],
        types: &[StructuralTypeDeclaration],
    ) -> std::collections::BTreeMap<StructuralTypeId, Option<ScalarType>> {
        parameters
            .iter()
            .filter_map(|(_, parameter)| {
                let shape = find_structural_type(types, parameter.structural_type)?;
                let terminal_psi::StructuralTypeShape::ElementView { element, .. } = shape.shape
                else {
                    return None;
                };
                let element = find_structural_type(types, element)?;
                let scalar_type = match element.shape {
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                        Some(scalar_type)
                    }
                    _ => None,
                };
                Some((parameter.structural_type, scalar_type))
            })
            .collect()
    }

    pub(crate) fn primitive_storage_place(
        storage: &[(symbols::SymbolHandle, PlaceId, ScalarType)],
        symbol: symbols::SymbolHandle,
        scalar_type: ScalarType,
    ) -> Result<PlaceId, LoweringError> {
        let mut matching = storage.iter().filter(|row| row.0 == symbol);
        let Some((_, place, declared_type)) = matching.next() else {
            return unsupported("primitive storage read has no initialized place");
        };
        if !symbol.is_valid() || matching.next().is_some() {
            return unsupported("primitive storage read identity is missing or duplicated");
        }
        if *declared_type != scalar_type {
            return unsupported("primitive storage read changes its declared type");
        }
        Ok(*place)
    }
}
