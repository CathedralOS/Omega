//! Prepare address geometry against the carrier actually published by its
//! dominating operation. Logical local loans retain their checked root; stored
//! values retain physical paths. A reference descriptor and a referent-presented
//! alias can therefore supply the same loan without replaying initializer syntax.
//! Geometry does not grant permission: source occurrence, access and live-loan
//! checks still precede this join, and Terminal validates its concrete endpoint.

use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralPathSegment};
use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::ValueDeclaration;
use terminal_psi::{StructuralAccess, StructuralPathSegment};

/// The one call structural-argument construction: every lowering route
/// resolves an argument's home and path, then mints its `StructuralArgument`
/// here so call argument shape has a single spelling.
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

/// The Terminal access a checked structural access lowers to: the two
/// vocabularies correspond one to one. A lane that admits only some
/// accesses matches the checked access itself.
pub(crate) fn terminal_access(access: CheckedStructuralAccess) -> StructuralAccess {
    match access {
        CheckedStructuralAccess::Owned => StructuralAccess::Owned,
        CheckedStructuralAccess::SharedBorrow => StructuralAccess::SharedBorrow,
        CheckedStructuralAccess::MutableBorrow => StructuralAccess::MutableBorrow,
        CheckedStructuralAccess::WriteOnlyBorrow => StructuralAccess::WriteOnlyBorrow,
    }
}

/// Address presentation can attenuate an established loan, never widen it.
/// This does not establish ownership, live backing, bounds or qualifications.
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

/// Lower a checked static projection. A `RuntimeIndex` has no context-free
/// lowering: its Terminal segment names the evaluated index value and an
/// obligation the carrying operation owns, so the emitter that evaluates the
/// index lowers it. Claims, discards, qualifications and residuals are static
/// by construction; a runtime segment reaching this function is refused.
pub(crate) fn lower_structural_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<Vec<StructuralPathSegment>, LoweringError> {
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::AllElements => Err(LoweringError::Unsupported(
                "universal qualification is not an operand path",
            )),
            CheckedUnitStructuralPathSegment::Case(_) => Err(LoweringError::Unsupported(
                "a case projection requires its exact source type during preparation",
            )),
            CheckedUnitStructuralPathSegment::RepresentationView { .. } => {
                Err(LoweringError::Unsupported(
                    "a returned view origin requires its checked declaration and formation codec",
                ))
            }
            CheckedUnitStructuralPathSegment::Referent => Ok(StructuralPathSegment::Referent),
            CheckedUnitStructuralPathSegment::Field(identity) => Ok(StructuralPathSegment::Field(
                identities.intern(&identity.spelling()),
            )),
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                Ok(StructuralPathSegment::FixedIndex(*index))
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex { .. }
            | CheckedUnitStructuralPathSegment::ComputedIndex(_) => {
                Err(LoweringError::Unsupported(
                    "a runtime-index projection is lowered by the emitter that evaluates its index",
                ))
            }
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                Ok(StructuralPathSegment::FixedByteRange {
                    start: *start,
                    end: *end,
                })
            }
        })
        .collect()
}

/// A static projection is resolved against its original source declaration
/// during preparation. Only runtime selector coordinates wait for the emitting
/// call's scalar frame and bound-obligation namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreparedStructuralPathSegment {
    Static(StructuralPathSegment),
    /// A dynamic extent is observed before selecting its backing, even for a literal index.
    ViewExtent {
        bytes: bool,
    },
    RuntimeIndex(checked_trees::CheckedRuntimeIndex),
    ComputedIndex(checked_trees::CheckedScalarComputationHandle),
    /// Execute this exact authored child while retaining its fixed geometry.
    EvaluatedFixedIndex {
        index: u64,
        computation: checked_trees::CheckedScalarComputationHandle,
    },
    /// A silent alias keeps its declaration-time selector in original storage.
    CapturedIndex(semantic_vocabulary::PlaceId),
}

/// Typed preparation may insert view markers or consume a logical reference
/// hop, but preserves the exact index roster. Join positions through that
/// roster and verify the fixed value before adding executable bookkeeping.
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
    let children = operands
        .span(evaluations)
        .ok_or(LoweringError::Unsupported(
            "selector projection has a stale computation child span",
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
            .ok_or(LoweringError::Unsupported(
                "selector child roster exceeds its projection suffix",
            ))?;
    for (ordinal, computation) in children.iter().enumerate() {
        let index_ordinal = ordinal + inherited;
        let position = checked_indexes[index_ordinal];
        let target = &mut prepared[prepared_indexes[index_ordinal]];
        match checked_path[position] {
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                if *target
                    != PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(
                        index,
                    ))
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
                    && *target == PreparedStructuralPathSegment::ComputedIndex(expected) => {}
            _ => return unsupported("selector child lost its exact dynamic coordinate"),
        }
    }
    Ok(())
}

/// The actual endpoint of a prepared projection, including inline carriers
/// that have no separate structural declaration.
#[derive(Debug)]
enum PreparedArgumentTip<'a> {
    Declaration(&'a terminal_psi::StructuralTypeDeclaration),
    Inline(&'a terminal_psi::StructuralFieldType),
    ByteElement,
    ByteWindow,
}

/// The coordinate system in which a checked projection was produced. A
/// completed value already names its published carrier; a logical local loan
/// instead names the backing that its carrier presents. Neither choice grants
/// access: the consuming operation retains its separately checked custody.
#[derive(Clone, Copy)]
pub(crate) enum ProjectionRoot {
    Published(semantic_vocabulary::StructuralTypeId),
    Borrowed {
        published: semantic_vocabulary::StructuralTypeId,
        logical: typed_trees::type_identity::TypeIdentityId,
        /// Root crossings actually spelled by this checked producer before its
        /// logical suffix. Physical publication emits its own typed crossings;
        /// only this explicit prefix may be consumed, never a nested field hop.
        spelled_reference_hops: u32,
    },
    /// The origin producer explicitly retained formal-entry reference steps.
    /// The published borrowed parameter already denotes that storage; no
    /// nested pointer step may be erased by this root-only adaptation.
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
    let expected = identities.intern(&checked.type_identity_text(endpoint).unwrap_or_default());
    let tip = match tip {
        PreparedArgumentTip::Declaration(declaration) => Some(declaration),
        _ => None,
    };
    complete_reference_endpoint(tip, expected, access, types, |segment| {
        path.push(PreparedStructuralPathSegment::Static(segment));
    })?;
    Ok(path)
}

/// Complete an emitted edge against its receiving declaration, independently
/// of which operation materialized the incoming value. Copying a reference
/// copies its descriptor; lending its referent still needs an explicit crossing.
/// Already referent-presented parameters and owned descriptor transfers retain
/// their paths. Admission separately checks the endpoint, live loan and bounds.
pub(crate) fn complete_emitted_argument_path(
    root: semantic_vocabulary::StructuralTypeId,
    path: &mut Vec<StructuralPathSegment>,
    endpoint: semantic_vocabulary::StructuralTypeId,
    access: StructuralAccess,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let expected = terminal_psi::find_structural_type(types, endpoint).ok_or(
        LoweringError::Unsupported("state argument lost its receiving declaration"),
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

/// A content observation reads the endpoint of the actual descriptor chain,
/// rather than transporting a reference value. Keep this separate from call
/// endpoint completion: a receiving reference declaration may stop that walk
/// before its referent, and owned transfers must never cross a reference.
pub(crate) fn complete_read_observation_path(
    root: semantic_vocabulary::StructuralTypeId,
    path: &mut Vec<StructuralPathSegment>,
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<semantic_vocabulary::StructuralTypeId, LoweringError> {
    let tip = terminal_semantics::runtime_structural_path_tip(types.iter(), root, path).ok_or(
        LoweringError::Unsupported("content observation lost its exact carrier"),
    )?;
    let endpoint = follow_reference_endpoint(
        Some(tip),
        |_| false,
        StructuralAccess::SharedBorrow,
        types,
        |segment| path.push(segment),
    )?
    .ok_or(LoweringError::Unsupported(
        "content observation lost its reference endpoint",
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
    // A call lends its declared endpoint. A stored reference is a carrier for
    // that endpoint; preserve every crossing instead of passing its wrapper.
    // Owned transport keeps the descriptor itself and never performs this read.
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
        if !structural_access_can_supply(supplied, access) || crossed.contains(&declaration.id) {
            return Err(LoweringError::Unsupported(
                "borrowed argument crosses an unreadable or cyclic reference",
            ));
        }
        crossed.push(declaration.id);
        tip = Some(terminal_psi::find_structural_type(types, referent).ok_or(
            LoweringError::Unsupported("borrowed argument reference lost its referent declaration"),
        )?);
        append(StructuralPathSegment::Referent);
    }
    Ok(tip)
}

fn prepare_structural_argument_projection<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    projection: &[CheckedUnitStructuralPathSegment],
    published_root: semantic_vocabulary::StructuralTypeId,
    types: &'a [terminal_psi::StructuralTypeDeclaration],
) -> Result<(Vec<PreparedStructuralPathSegment>, PreparedArgumentTip<'a>), LoweringError> {
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
    )?;
    if !projection.is_empty() {
        retain_selector_evaluations(
            projection,
            argument.projection_evaluations,
            &checked.facts.values.scalar_computations.operands,
            &mut path,
        )?;
    }
    Ok((path, tip))
}

fn prepare_root_projection<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    projection: &[CheckedUnitStructuralPathSegment],
    root: ProjectionRoot,
    requested: StructuralAccess,
    types: &'a [terminal_psi::StructuralTypeDeclaration],
) -> Result<(Vec<PreparedStructuralPathSegment>, PreparedArgumentTip<'a>), LoweringError> {
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
                    return unsupported("reference origin lost its declared entry crossing");
                };
                remainder = suffix;
            }
            return prepare_argument_projection(identities, remainder, published, types);
        }
    };
    if !identity.is_valid() {
        return Err(LoweringError::Unsupported(
            "borrowed projection has no logical root declaration",
        ));
    }
    let mut roots = types.iter().filter(|row| {
        row.identity == identities.intern(&checked.type_identity_text(identity).unwrap_or_default())
    });
    let logical_root = roots.next().ok_or(LoweringError::Unsupported(
        "borrowed argument lost its checked logical root",
    ))?;
    if roots.next().is_some() {
        return Err(LoweringError::Unsupported(
            "borrowed argument logical root is empty or ambiguous",
        ));
    }
    if requested == StructuralAccess::Owned {
        return Err(LoweringError::Unsupported(
            "logical borrowed argument cannot transfer its carrier",
        ));
    }
    let mut remainder = projection;
    for _ in 0..spelled_reference_hops {
        let Some((CheckedUnitStructuralPathSegment::Referent, suffix)) = remainder.split_first()
        else {
            return unsupported("borrowed projection lost its producer-spelled root crossing");
        };
        remainder = suffix;
    }
    let mut prepared = Vec::new();
    let mut carrier_id = published_root;
    // A path cannot cross more distinct declarations than the canonical type
    // catalog holds. This also fails closed on recursive reference-only cycles,
    // without a spelling/depth limit or a second structural graph.
    let mut remaining = types.len();
    while carrier_id != logical_root.id {
        remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
            "borrowed argument has a cyclic reference presentation",
        ))?;
        let mut carriers = types.iter().filter(|row| row.id == carrier_id);
        let carrier = carriers.next().ok_or(LoweringError::Unsupported(
            "borrowed argument lost its published carrier",
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
                    || access.is_some_and(|access| !structural_access_can_supply(access, requested))
                {
                    return unsupported("borrowed argument changes its descriptor presentation");
                }
                // Descriptor presentation is implicit; it emits no pointer
                // crossing. Its element/count rules remain in the strict suffix.
                carrier_id = logical_root.id;
            }
            _ => return unsupported("borrowed argument changes its published root"),
        }
    }
    let (suffix, tip) = prepare_argument_projection(identities, remainder, logical_root.id, types)?;
    prepared.extend(suffix);
    Ok((prepared, tip))
}

/// Resolve the path against its original source declaration. Only runtime
/// selector values and their bound obligations remain for the emitting call.
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
        return Err(LoweringError::Unsupported(
            "element-view write does not select a descriptor",
        ));
    }
    Ok(path)
}

fn prepare_argument_projection<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    path: &[CheckedUnitStructuralPathSegment],
    root: semantic_vocabulary::StructuralTypeId,
    types: &'a [terminal_psi::StructuralTypeDeclaration],
) -> Result<(Vec<PreparedStructuralPathSegment>, PreparedArgumentTip<'a>), LoweringError> {
    use PreparedArgumentTip as Tip;
    use terminal_psi::{ByteSequenceCarrier, StructuralFieldType, StructuralTypeShape};
    let declaration = |id| {
        let mut matches = types.iter().filter(|declaration| declaration.id == id);
        let found = matches.next().ok_or(LoweringError::Unsupported(
            "argument path lost its source type",
        ))?;
        if matches.next().is_some() {
            return Err(LoweringError::Unsupported(
                "argument path source type is ambiguous",
            ));
        }
        Ok(found)
    };
    let mut tip = Tip::Declaration(declaration(root)?);
    let mut selected_case: Option<&terminal_psi::StructuralCaseDeclaration> = None;
    let mut prepared = Vec::with_capacity(path.len());
    for step in path {
        if selected_case.is_some() && !matches!(step, CheckedUnitStructuralPathSegment::Field(_)) {
            return Err(LoweringError::Unsupported(
                "argument case must select its payload field",
            ));
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
            Tip::ByteElement | Tip::ByteWindow => None,
        };
        if matches!(
            step,
            CheckedUnitStructuralPathSegment::FixedIndex(_)
                | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                | CheckedUnitStructuralPathSegment::ComputedIndex(_)
        ) {
            let byte_view = matches!(bytes, Some(ByteSequenceCarrier::BorrowedView { .. }));
            if byte_view || matches!(shape, Some(StructuralTypeShape::ElementView { .. })) {
                prepared.push(PreparedStructuralPathSegment::ViewExtent { bytes: byte_view });
            }
        }
        let lowered = match step {
            CheckedUnitStructuralPathSegment::AllElements => {
                return Err(LoweringError::Unsupported(
                    "universal qualification is not an executable projection",
                ));
            }
            CheckedUnitStructuralPathSegment::RepresentationView { .. } => {
                return Err(LoweringError::Unsupported(
                    "a returned view origin is not an executable argument projection",
                ));
            }
            CheckedUnitStructuralPathSegment::Case(identity) => {
                let cases = match shape {
                    Some(
                        StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. },
                    ) => cases,
                    _ => {
                        return Err(LoweringError::Unsupported(
                            "argument case requires its exact source sum",
                        ));
                    }
                };
                let mut matches = cases
                    .iter()
                    .filter(|case| case.identity == identities.intern(&identity.spelling()));
                let case = matches.next().ok_or(LoweringError::Unsupported(
                    "argument case belongs to another source type",
                ))?;
                if matches.next().is_some() {
                    return Err(LoweringError::Unsupported(
                        "argument case identity is ambiguous",
                    ));
                }
                selected_case = Some(case);
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Case(case.id))
            }
            CheckedUnitStructuralPathSegment::Field(identity) => {
                let fields = match selected_case.take() {
                    Some(case) => &case.fields,
                    None => match shape {
                        Some(terminal_psi::record_or_mixed_shape_pattern!(fields)) => fields,
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "argument field requires its source record or selected case",
                            ));
                        }
                    },
                };
                let mut matches = fields
                    .iter()
                    .filter(|field| field.identity == identities.intern(&identity.spelling()));
                let field = matches.next().ok_or(LoweringError::Unsupported(
                    "argument field belongs to another declaration",
                ))?;
                if matches.next().is_some() || field.relevance.is_erased() {
                    return Err(LoweringError::Unsupported(
                        "argument field is ambiguous or erased",
                    ));
                }
                tip = match field.field_type {
                    StructuralFieldType::Structural(child) => Tip::Declaration(declaration(child)?),
                    _ => Tip::Inline(&field.field_type),
                };
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Field(
                    identities.intern(&identity.spelling()),
                ))
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                tip = match shape {
                    Some(StructuralTypeShape::FixedArray { element, length }) => {
                        if index >= length {
                            return Err(LoweringError::Unsupported(
                                "argument index exceeds its array",
                            ));
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
                            return Err(LoweringError::Unsupported(
                                "argument index has no element storage",
                            ));
                        }
                    },
                };
                PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(*index))
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
                        return Err(LoweringError::Unsupported(
                            "argument selector has no element storage",
                        ));
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
                    return Err(LoweringError::Unsupported(
                        "argument referent requires a reference carrier",
                    ));
                };
                tip = Tip::Declaration(declaration(*referent)?);
                PreparedStructuralPathSegment::Static(StructuralPathSegment::Referent)
            }
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                let capacity = match shape {
                    Some(StructuralTypeShape::FixedArray { element, length })
                        if declaration(*element)?.shape
                            == StructuralTypeShape::PrimitiveScalar(
                                semantic_vocabulary::ScalarType::Integer(
                                    semantic_vocabulary::IntegerType::new(
                                        semantic_vocabulary::IntegerSign::Unsigned,
                                        8,
                                    )
                                    .expect("u8 is a supported integer carrier"),
                                ),
                            ) =>
                    {
                        Some(*length)
                    }
                    _ => match bytes {
                        Some(
                            ByteSequenceCarrier::BoundedOwned { capacity }
                            | ByteSequenceCarrier::BoundedView { capacity, .. },
                        ) => Some(capacity),
                        Some(ByteSequenceCarrier::BorrowedView { .. }) => None,
                        _ => {
                            return Err(LoweringError::Unsupported(
                                "argument window has no byte storage",
                            ));
                        }
                    },
                };
                if start > end || capacity.is_some_and(|capacity| *end > capacity) {
                    return Err(LoweringError::Unsupported(
                        "argument window exceeds its byte storage",
                    ));
                }
                tip = Tip::ByteWindow;
                PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedByteRange {
                    start: *start,
                    end: *end,
                })
            }
        };
        prepared.push(lowered);
    }
    if selected_case.is_some() {
        return Err(LoweringError::Unsupported(
            "argument case has no payload field",
        ));
    }
    Ok((prepared, tip))
}

pub(crate) fn prepare_unscoped_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    path: &[CheckedUnitStructuralPathSegment],
) -> Result<Vec<PreparedStructuralPathSegment>, LoweringError> {
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::RuntimeIndex(index) => {
                Ok(PreparedStructuralPathSegment::RuntimeIndex(index.clone()))
            }
            CheckedUnitStructuralPathSegment::ComputedIndex(root) => {
                Ok(PreparedStructuralPathSegment::ComputedIndex(*root))
            }
            segment => Ok(PreparedStructuralPathSegment::Static(
                lower_structural_path(identities, std::slice::from_ref(segment))?.remove(0),
            )),
        })
        .collect()
}

/// Custody paths are static, but still need their declared root to resolve a
/// case identity. Use the ordinary typed preparation without a runtime frame;
/// a selector requiring evaluation or a bound obligation remains rejected.
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

/// Retain a returned loan's declaration-relative interpretations. This recipe
/// describes possible backing, not an executable address: selectors are already
/// fixed by the result interface and each view rejoins its checked declaration.
/// Actual formation and result custody authenticate the recipe independently.
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
                CheckedOrigin::Element => unsupported("returned origin lost its collection step"),
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let (prefix, _) =
            prepare_root_projection(identities, checked, &prefix, coordinate, access, types)?;
        let prefix = emit_argument_path(&prefix, None, &mut std::iter::empty(), None)?;
        let prefix = terminal_semantics::reference_source_storage_path(types.iter(), root, &prefix);
        let prefix = prefix
            .iter()
            .cloned()
            .map(Origin::Exact)
            .collect::<Vec<_>>();
        root = terminal_semantics::reference_origin_type(types.iter(), root, &prefix).ok_or(
            LoweringError::Unsupported("returned origin has no typed prefix"),
        )?;
        output.extend(prefix);
        let Some(position) = next_projection else {
            return Ok(output);
        };
        let projection = match &path[position] {
            CheckedOrigin::Element => Origin::Element,
            CheckedOrigin::Exact(CheckedUnitStructuralPathSegment::FixedIndex(index)) => {
                Origin::Exact(StructuralPathSegment::FixedIndex(*index))
            }
            CheckedOrigin::Exact(CheckedUnitStructuralPathSegment::RepresentationView {
                type_identity,
                storage,
            }) => {
                let checked_trees::CheckedReferenceStorage::View { stored_scalar } = storage else {
                    return unsupported("returned view has no formation codec");
                };
                let declaration = crate::terminal_identities::structural_declaration_by_identity(
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
                .ok_or(LoweringError::Unsupported(
                    "returned view has an incompatible storage codec",
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
        .ok_or(LoweringError::Unsupported(
            "returned origin has an invalid typed projection",
        ))?;
        output.push(projection);
        path = &path[position + 1..];
        // Entry presentation belongs to the root, never to a later element,
        // view or nested reference prefix.
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

/// A checked argument path can carry a runtime selector only when the
/// operand lowering owns the index's evaluation and obligation: a dense
/// scalar parameter or value-binding local of the calling state is already
/// in the caller's operand frame, and a `StructuralField` selector consumes
/// a field snapshot already evaluated at its authored operand position.
/// Unit calls mint these reads in
/// their operation frame; scalar computations carry them through their ordered
/// operand blocks. The call allocates each selector's bound obligation
/// from the emission context (the same counter `allocate_requirement`
/// serves). An `AssignmentIndex` segment belongs to a store destination, not
/// a call argument, so it is refused here.
pub(crate) fn lower_argument_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    path: &[CheckedUnitStructuralPathSegment],
    caller_scalar_values: Option<&[ValueDeclaration]>,
    selector_values: &mut dyn Iterator<Item = ValueId>,
    calls: Option<&mut CallEmissionContext<'_>>,
) -> Result<Vec<terminal_psi::StructuralPathSegment>, LoweringError> {
    emit_argument_path(
        &prepare_unscoped_path(identities, path)?,
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
                selector_values.next().ok_or(LoweringError::Unsupported(
                    "fixed selector lost its authored evaluation",
                ))?;
                Ok(StructuralPathSegment::FixedIndex(*index))
            }
            PreparedStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::Parameter { position }
                | checked_trees::CheckedRuntimeIndex::Local { position },
            ) => {
                let (Some(values), Some(calls)) = (caller_scalar_values, calls.as_deref_mut())
                else {
                    return Err(LoweringError::Unsupported(
                        "runtime index argument path has no caller scalar frame",
                    ));
                };
                let index = values
                    .get(usize::try_from(*position).map_err(|_| {
                        LoweringError::Unsupported("runtime index position exceeds usize")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "runtime index selector position has no caller scalar",
                    ))?;
                if !matches!(
                    index.scalar_type,
                    ScalarType::Integer(integer) if !integer.is_address()
                ) {
                    return Err(LoweringError::Unsupported(
                        "runtime index selector is not an integer operand",
                    ));
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
                    return Err(LoweringError::Unsupported(
                        "runtime index argument path has no caller scalar frame",
                    ));
                };
                let index = selector_values.next().ok_or(LoweringError::Unsupported(
                    "field-selector runtime index has no evaluated operand",
                ))?;
                calls.allocate_requirement().map(|obligation| {
                    terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation }
                })
            }
            PreparedStructuralPathSegment::RuntimeIndex(_) => Err(LoweringError::Unsupported(
                "an assignment-index selector is owned by its store destination",
            )),
            PreparedStructuralPathSegment::CapturedIndex(_) => Err(LoweringError::Unsupported(
                "captured loan selector requires its operand emission frame",
            )),
            PreparedStructuralPathSegment::ViewExtent { .. } => Err(LoweringError::Unsupported(
                "a view element projection requires its emitting operand frame",
            )),
            PreparedStructuralPathSegment::Static(segment) => Ok(segment.clone()),
        })
        .collect()
}

/// Emit the projection's real extent and literal selector operands in the same
/// frame as a computed selector. Preparation resolves geometry; this owner
/// allocates the operation's bound obligation, never infers it from the type.
pub(crate) fn emit_captured_argument_path(
    path: &[PreparedStructuralPathSegment],
    source: semantic_vocabulary::PlaceId,
    caller_values: Option<&[ValueDeclaration]>,
    selectors: &mut dyn Iterator<Item = ValueId>,
    mut calls: Option<&mut CallEmissionContext<'_>>,
    emission: Option<(
        &mut u64,
        &mut crate::emission::operation_emission::buffer::OperationBuffer,
    )>,
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
                return Err(LoweringError::Unsupported(
                    "captured loan selector has no emitting frame",
                ));
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
                .ok_or(LoweringError::Unsupported(
                    "captured loan selector has no obligation frame",
                ))?
                .allocate_requirement()?;
            output.push(StructuralPathSegment::RuntimeIndex { index, obligation });
            dynamic_extent = false;
            continue;
        }
        if let PreparedStructuralPathSegment::ViewExtent { bytes } = segment {
            // Observe the descriptor at this exact prefix, before indexing its
            // backing. Projected descriptors remain in their original storage;
            // the length emitter deliberately does not cache them by root.
            let Some((next_value, operations)) = emission.as_mut() else {
                return Err(LoweringError::Unsupported(
                    "view projection has no operand emission frame",
                ));
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
            PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(index)) => {
                Some(*index)
            }
            PreparedStructuralPathSegment::EvaluatedFixedIndex { index, .. } => {
                selectors.next().ok_or(LoweringError::Unsupported(
                    "fixed selector lost its authored evaluation",
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
                .ok_or(LoweringError::Unsupported(
                    "view projection has no obligation frame",
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
