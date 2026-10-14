//! Record projections bind a declared field once, then resolve the current local place
//! at each evaluation site. They materialize an ordinary scalar binding before
//! later effects; neither a synthetic parameter nor a delayed return load may
//! stand in for that observation. Stage 04 retains the root type and field identity
//! together with explicit stored-reference crossings; lowering consumes that plan.
//! Reads through references use PrimitiveScalarRead because they observe a value
//! without naming a canonical owned leaf. Terminal independently checks custody.
//! A normalized projection can observe an earlier call operand's exact captured
//! occurrence. The selected operand prefix supplies that home; copying a subtree
//! rebases the declared field path to its snapshot. The observation neither
//! repeats the producer nor disposes the enclosing call's temporary. Other
//! authored reads of the original place retain their observation-time behavior.
//! The same leaf declaration can occur below several children of one root.
//! Bind the complete checked subject to its canonical carrier path, rather than
//! conflating reads by final field identity or projected type alone.
//! A runtime-indexed record over bytes keeps that original array home. Each
//! scalar observation uses its declaration-derived byte offset and the ordinary
//! byte-region re-view, whose individual reads re-prove the entire observed
//! field's bounds. It does not project a record field below a `u8` element or
//! establish a copied record to stand in for the borrowed storage.
//! Literal and runtime byte coordinates use the same read path. Recast
//! normalization supplies nested field offsets and physical integer widths;
//! an `IntegerAt` read decodes that width before widening to its semantic
//! carrier. Recomputing a flat scalar layout here loses both relationships.
//! A record-as-record recast (`&place as &View`, shared or mutable) erases the
//! same way: the checked judgment proved identical geometry, so a view field
//! reads the source leaf at its offset under the source's own declaration
//! (`validation::recast_record_source_leaf`). Terminal then verifies an
//! ordinary read of a field the source declares, with the source's facts. An
//! equal-width integer leaf of the other sign reads its declared carrier and
//! replays the view's modular image; any other carrier change refuses.
//! A silent loan local's captured place is its stage 04 `CheckedPlaceLoanPlan`
//! row: its root, root type and authored segments resolve here against the
//! structural type catalog, never against the loan's initializer.
//! Stores use the same captured source and normalized geometry. A loan local
//! is not a second storage home; write permission and concrete encoding fit
//! remain separate obligations at the emitting operation.
use super::super::{
    PrimitiveType, StructuralFieldId, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeId, StructuralTypeShape, terminal_scalar_type,
};
use super::{
    CheckedScalarComputationKind, CheckedTrees, LoweringError, ScalarType, storage, unsupported,
};
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, LoweredElementPathSegment, PrimitiveReadSource,
};
pub(crate) use crate::expression_preparation::bindings::place_observations::LoanedSource;
use crate::expression_preparation::bindings::place_observations::{
    ByteOverlayField, ByteOverlaySource, ObservationKind, PlaceObservation,
    PlaceObservationCapture, PrimitiveObservation, byte_overlay_read,
};
use crate::scalar_lowering::branch_states::structural_values;
use checked_trees::CheckedUnitStructuralArgumentPlan;
use checked_trees::data::DataMember;
use checked_trees::types::TypeReferenceNode;

impl super::Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn structural_field(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        receiver_expression: typed_trees::expression::ExpressionHandle,
        receiver: checked_trees::CheckedStructuralValueHandle,
        subject: &CheckedUnitStructuralArgumentPlan,
        field: symbols::SymbolHandle,
        primitive: PrimitiveType,
        input_types: &[super::QualifiedScalarType],
        target: usize,
        site: &super::Site<'_>,
    ) -> Result<usize, LoweringError> {
        let (machine, state) = crate::declarations::state_declaration(self.checked, site.state)?;
        let mut completed = None;
        let established = self
            .established_values
            .iter()
            .rev()
            .find(|value| match value {
                super::EstablishedValue::Produced { root, .. } => {
                    receiver.is_valid() && *root == receiver
                        || receiver_expression.is_valid()
                            && receiver_expression
                                == self
                                    .checked
                                    .facts
                                    .values
                                    .structural_values
                                    .nodes
                                    .get(*root)
                                    .expression
                }
                super::EstablishedValue::Copied { expression, .. } => {
                    receiver_expression.is_valid() && receiver_expression == *expression
                }
            })
            .cloned();
        let expression = if let Some(established) = established {
            // This is an observation of a prior operand, not establishment or
            // disposal. The owning call retains its normal cleanup frontier.
            match established {
                super::EstablishedValue::Produced { place, .. } if receiver.is_valid() => {
                    completed_observation(
                        self.fields,
                        site.bindings,
                        subject,
                        field,
                        primitive,
                        place,
                    )?
                }
                super::EstablishedValue::Produced {
                    root,
                    place,
                    structural_type,
                } => {
                    let value = self.checked.facts.values.structural_values.nodes.get(root);
                    let original = match &value.kind {
                        checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace {
                            source,
                            ..
                        }
                        | checked_trees::CheckedStructuralValueKind::Place(source) => source,
                        _ => {
                            return unsupported(
                                "captured field observation lost its established source",
                            );
                        }
                    };
                    let home = site
                        .bindings
                        .borrowed_structural_home(original, self.checked.state_parameters(state))?;
                    let root_type = site.bindings.structural_argument_type(&home).ok_or(
                        LoweringError::Unsupported(
                            "captured field observation lost its established carrier",
                        ),
                    )?;
                    let mut source =
                        crate::emission::operation_emission::calls::PreparedStructuralArgument::from_static(
                            home, Some(root_type),
                        )?;
                    source.path.extend(
                        crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
                            identities, &original.path, root_type, self.structural_types,
                        )?,
                    );
                    copied_observation(
                        self.fields,
                        site.bindings,
                        self.structural_types,
                        subject,
                        field,
                        primitive,
                        place,
                        structural_type,
                        &source,
                    )?
                }
                super::EstablishedValue::Copied {
                    place,
                    structural_type,
                    source,
                    ..
                } => copied_observation(
                    self.fields,
                    site.bindings,
                    self.structural_types,
                    subject,
                    field,
                    primitive,
                    place,
                    structural_type,
                    &source,
                )?,
            }
        } else if receiver.is_valid() {
            let values = &self.checked.facts.values.structural_values;
            let root = values
                .root_for_expression(
                    site.state,
                    site.statement,
                    values.nodes.get(receiver).expression,
                )
                .ok_or(LoweringError::Unsupported(
                    "computed receiver lost its complete value root",
                ))?;
            // Observation does not dispose its receiver. Source permission
            // production retains this exact expression owner and its normal
            // death's ordered residual/hook plan. The completed storage below
            // is exported with the scalar expansion for that cleanup to use.
            let prepared = structural_values::prepare_value(
                identities,
                self.checked,
                self.qualifications,
                site.state,
                facts::PlaceRoot::Expression(values.nodes.get(receiver).expression),
                site.statement,
                receiver,
                values.calls_for_root(receiver).unwrap_or(&[]),
                root.type_reference,
                site.bindings,
                input_types,
                self.structural_types,
                receiver_storage(self.fields, subject, field)?,
            )?;
            let expression = completed_observation(
                self.fields,
                site.bindings,
                subject,
                field,
                primitive,
                prepared.place,
            )?;
            completed = Some(prepared);
            expression
        } else {
            observation(
                self.fields,
                site.bindings,
                machine,
                self.checked.state_parameters(state),
                subject,
                field,
                primitive,
            )?
        };
        let mut arguments = super::parameters(input_types);
        arguments.push(super::parameter(
            input_types.len(),
            terminal_scalar_type(primitive)?.into(),
        ));
        let observation = self.push(crate::scalar_lowering::branch_states::prepared::LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: input_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: vec![crate::emission::operation_emission::LoweredScalarBinding::Expression(expression)],
            terminator: crate::scalar_lowering::branch_states::prepared::LoweredScalarBranchTerminator::jump(target, arguments),
        });
        match completed {
            Some(value) => value.finish(
                identities,
                site.state,
                site.bindings,
                input_types,
                observation,
                self,
            ),
            None => Ok(observation),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Binding {
    symbol: symbols::SymbolHandle,
    subject: CheckedUnitStructuralArgumentPlan,
    receiver: Option<structural_values::Storage>,
    /// The established place this read resolves through when the subject is a
    /// shared `&`-record loan local rather than an established place of its
    /// own. The captured place supplies the source and the canonical prefix
    /// already leads `path`; `None` for ordinary established-local subjects.
    loan_source: Option<LoanedSource>,
    /// The referee record and exact selected field offset when the loan's
    /// captured prefix ends on a byte element. Its observation reads that
    /// region instead of projecting a record field below the byte carrier.
    byte_overlay: Option<ByteOverlayField>,
    /// `path` respelled in the runtime segment vocabulary, populated only for
    /// a byte-carrier loan read.
    carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    /// Exact declared path for a float observation or stored-reference crossing.
    primitive_path: Vec<terminal_psi::StructuralPathSegment>,
    /// The declared root beneath which this observation's path is resolved.
    observation_root_type: semantic_vocabulary::StructuralTypeId,
    /// The checked coordinate of the loan's one non-literal index segment,
    /// when the captured path selects an element by runtime value.
    runtime_index: Option<checked_trees::CheckedRuntimeIndex>,
    /// `true` when the runtime index steps through an element view (live
    /// extent); `false` for a fixed array (literal extent on the type).
    view_container: bool,
    /// The element path to the scalar leaf: every segment below the runtime
    /// index plus the leaf's own field projection.
    element_path: Vec<terminal_psi::StructuralPathSegment>,
    /// Element-view observations retain their separate invariant contract.
    bounded_leaf: bool,
    /// The declared scalar field, or `None` when the path's last literal
    /// element is the scalar itself and the read is a primitive path read.
    field: Option<StructuralFieldId>,
    scalar_type: ScalarType,
    /// The carrier the observed leaf declares. It equals `scalar_type` except
    /// through a typed record view whose field re-views an equal-width
    /// integer leaf of another sign; the read then observes the leaf at this
    /// carrier and replays the view's modular image.
    leaf_scalar_type: ScalarType,
}

/// A shared recast view of an established record as another record type.
/// The checked recast judgment proved identical geometry, so each of the
/// view's fields names the source leaf at the same offset and width; the read
/// resolves that source leaf under the source's own declaration instead of
/// projecting a field the source does not declare.
struct TypedRecordOverlay {
    source: typed_trees::types::TypeReferenceHandle,
    referee: typed_trees::types::TypeReferenceHandle,
}

/// The established place an observation local borrows from: the captured
/// root join, the canonical prefix to the referent, the referee's
/// structural declaration at that prefix, and — when the prefix ends on a
/// `u8` element (`&self.buf[0] as &T`) — the referee record the byte region
/// overlays.
struct CapturedPlaceSubject {
    access: checked_trees::CheckedStructuralAccess,
    source: LoanedSource,
    path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    terminal: StructuralTypeId,
    primitive: Option<PrimitiveObservation>,
    byte_overlay: Option<ByteOverlaySource>,
    typed_overlay: Option<TypedRecordOverlay>,
    /// The checked coordinate of the loan's non-literal index, when the
    /// captured path selects an element by runtime value.
    runtime_index: Option<checked_trees::CheckedRuntimeIndex>,
    /// `true` when the runtime index steps through an element view.
    view_container: bool,
    /// Terminal segments below the runtime index, inside the element.
    element_suffix: Vec<terminal_psi::StructuralPathSegment>,
    /// `true` when the loan's formation admission proved its byte window;
    /// stores through it carry that decision to their emitted obligations.
    decided_byte_view_store: bool,
}

/// The silent place loans the plan records for `state`: the single-state
/// body's roster, or the composed state's.
fn planned_place_loans<'plan>(
    checked: &'plan CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
) -> &'plan [checked_trees::CheckedPlaceLoanPlan] {
    let plans = &checked.facts.flow.terminal_unit_effects;
    if let Some(plan) = plans
        .for_machine(machine)
        .filter(|plan| plan.state == state)
    {
        return &plan.place_loans;
    }
    plans
        .composed_for_machine(machine)
        .and_then(|plan| plan.state(state))
        .map_or(&[], |row| row.place_loans.as_slice())
}

/// The captured place a silent loan local names, when `symbol` is one.
fn loan_subject(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    symbol: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<Option<CapturedPlaceSubject>, LoweringError> {
    match planned_place_loans(checked, machine, state)
        .iter()
        .find(|loan| loan.symbol == symbol)
    {
        Some(loan) => captured_place_subject(identities, checked, state, loan, types),
        None => Ok(None),
    }
}

/// Each silent loan of a primitive or record referee observes its captured
/// place. An unsupported capture is retained, not refused: only a use of the
/// loan owns that failure.
pub(crate) fn prepare_place_observations(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> std::sync::Arc<[PlaceObservation]> {
    let mut observations = Vec::new();
    for loan in planned_place_loans(checked, machine, state) {
        let capture = (|| {
            let Some(subject) = captured_place_subject(identities, checked, state, loan, types)?
            else {
                return Ok(None);
            };
            let observation = if let Some(primitive) = subject.primitive {
                ObservationKind::Primitive(primitive)
            } else if let Some(overlay) = subject.byte_overlay {
                if subject.view_container || !subject.element_suffix.is_empty() {
                    return Ok(None);
                }
                let Some(layout) =
                    validation::recast_scalar_layout(&checked.typed, overlay.referee)
                else {
                    return unsupported("record byte observation has no normalized scalar layout");
                };
                ObservationKind::ByteRecord { overlay, layout }
            } else {
                ObservationKind::Structural
            };
            Ok(Some(PlaceObservationCapture {
                coordinate: None,
                source: subject.source,
                carrier_path: subject.carrier_path,
                runtime_index: subject.runtime_index,
                observation,
            }))
        })();
        let capture = match capture {
            Ok(Some(capture)) => Ok(capture),
            Ok(None) => Err(LoweringError::Unsupported(
                "place loan has no scalar or record observation",
            )),
            Err(error) => Err(error),
        };
        observations.push(PlaceObservation {
            statement_index: loan.statement_index,
            symbol: loan.symbol,
            loan: loan.clone(),
            selectors: Vec::new(),
            capture,
        });
    }
    observations.into()
}

/// The type identity a captured root or referee resolves under. A type naming
/// the machine itself denotes the attached data record its states operate on.
fn carrier_identity(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Result<String, LoweringError> {
    match checked.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } if *symbol == machine.symbol => {
            let identity = checked
                .normalized_nominal_type_identity(machine.attached_data_symbol)
                .ok_or(LoweringError::Unsupported(
                    "record loan's attached carrier has no identity",
                ))?;
            Ok(checked.type_identity_text(identity).unwrap_or_default())
        }
        _ => Ok(checked.normalized_type_identity_text(reference)),
    }
}

/// A silent loan names no established place of its own, so a member read or
/// store through it resolves the storage the loan captured plus the canonical
/// path from that root to the referee. `None` means the loan's referee has no
/// observation here; `Some` carries the captured source, the canonical prefix
/// and the declared type it ends on, so the ordinary subject walk continues
/// there.
fn captured_place_subject(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    loan: &checked_trees::CheckedPlaceLoanPlan,
    types: &[StructuralTypeDeclaration],
) -> Result<Option<CapturedPlaceSubject>, LoweringError> {
    let referee = loan.referee;
    if checked.primitive_type_reference(referee).is_none()
        && !matches!(
            checked.type_reference_table.type_reference(referee),
            TypeReferenceNode::Named { .. }
        )
    {
        return Ok(None);
    }
    let (owner, _) = crate::declarations::state_declaration(checked, state)?;
    let source = match loan.root {
        checked_trees::CheckedPlaceLoanRoot::StructuralParameter { parameter_index } => {
            LoanedSource::Parameter(parameter_index)
        }
        checked_trees::CheckedPlaceLoanRoot::Local { symbol } => LoanedSource::Local(symbol),
        checked_trees::CheckedPlaceLoanRoot::ScalarParameter { .. } => {
            return unsupported("record loan captured a non-structural parameter");
        }
    };
    let root_identity = carrier_identity(checked, owner, loan.root_type)?;
    let mut declaration =
        terminal_psi::structural_type_by_identity(types, identities.intern(&root_identity)).ok_or(
            LoweringError::Unsupported("record loan's captured root has no structural namespace"),
        )?;
    let mut path = Vec::new();
    let mut carrier_path = Vec::new();
    let mut runtime_index = None;
    let mut view_container = false;
    let mut runtime_split = None;
    // Whether a literal fixed-array index ends the prefix (`a[i]` cannot
    // extend into a `u8` element's members, so a byte-carrier prefix always
    // ends there).
    let mut literal_index_tail = false;
    let mut primitive_field = None;
    let segment_count = loan.path.len();
    for (segment_ordinal, segment) in loan.path.iter().enumerate() {
        match segment {
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                let Some(fields) = declaration.shape.record_fields() else {
                    return unsupported("record loan's captured path left its record namespace");
                };
                let mut matching = fields
                    .iter()
                    .filter(|field| field.identity == identities.intern(&identity.spelling()));
                let selected = matching.next().ok_or(LoweringError::Unsupported(
                    "record loan's captured field is absent",
                ))?;
                if matching.next().is_some() || selected.relevance.is_erased() {
                    return unsupported("record loan's captured field is ambiguous or erased");
                }
                // Refined and bounded scalar declarations keep their original
                // field identity so the ordinary read retains its restrictions.
                // Atomic cells are a distinct access family, never plain loans.
                if let Some(scalar_type) = selected.field_type.scalar_type()
                    && !matches!(selected.field_type, StructuralFieldType::Atomic(_))
                {
                    if checked.primitive_type_reference(referee).is_none() {
                        return unsupported(
                            "record observation cannot replace a scalar storage endpoint",
                        );
                    }
                    if segment_ordinal + 1 != segment_count {
                        return unsupported("primitive observation continues below a scalar field");
                    }
                    primitive_field = Some((selected.id, scalar_type));
                    carrier_path.push(terminal_psi::StructuralPathSegment::Field(
                        selected.identity.clone(),
                    ));
                    literal_index_tail = false;
                    break;
                }
                let Some(nested) = selected.field_type.as_structural() else {
                    return unsupported("record loan's captured field is not structural");
                };
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    selected.id,
                ));
                carrier_path.push(terminal_psi::StructuralPathSegment::Field(
                    selected.identity.clone(),
                ));
                literal_index_tail = false;
                declaration = terminal_psi::find_structural_type(types, nested).ok_or(
                    LoweringError::Unsupported("record loan's captured type is absent"),
                )?;
            }
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                let index = *index;
                // An element-view root carries no literal extent: the index's
                // bound is the view's live length, proven by the loan's own
                // captured obligations rather than replayed here.
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, length } => {
                        if index >= *length {
                            return unsupported("record loan's captured index is out of bounds");
                        }
                        literal_index_tail = true;
                        *element
                    }
                    StructuralTypeShape::ElementView { element } => {
                        literal_index_tail = false;
                        *element
                    }
                    _ => {
                        return unsupported(
                            "record loan's captured index left its array namespace",
                        );
                    }
                };
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index));
                carrier_path.push(terminal_psi::StructuralPathSegment::FixedIndex(index));
                declaration = terminal_psi::find_structural_type(types, element).ok_or(
                    LoweringError::Unsupported("record loan's captured element type is absent"),
                )?;
            }
            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
                if runtime_index.is_some() {
                    return unsupported("record loan selects at most one runtime element");
                }
                // Neither container shape carries its bound on the segment —
                // an element view's bound is its live extent and a fixed
                // array's is its declared length; Terminal re-proves both.
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, .. } => {
                        view_container = false;
                        *element
                    }
                    StructuralTypeShape::ElementView { element } => {
                        view_container = true;
                        *element
                    }
                    _ => {
                        return unsupported("record loan's runtime index left its array namespace");
                    }
                };
                runtime_index = Some(selector.clone());
                runtime_split = Some(path.len());
                literal_index_tail = false;
                declaration = terminal_psi::find_structural_type(types, element).ok_or(
                    LoweringError::Unsupported("record loan's captured element type is absent"),
                )?;
            }
            _ => {
                return unsupported(
                    "record loan's captured place selects a case, a byte window or a stored reference",
                );
            }
        }
    }
    // The u8 element is the backing coordinate, not the record namespace.
    // Resolve fields under the referee, then rejoin their recast geometry.
    // Static footprint checks and per-byte Terminal obligations below keep
    // this namespace selection from granting access outside the region.
    let mut terminal = declaration.id;
    let mut byte_overlay = None;
    let referee_identity = carrier_identity(checked, owner, referee)?;
    if (literal_index_tail || runtime_index.is_some())
        && matches!(
            &declaration.shape,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 8
        )
        && let Some(overlay) = types.iter().find(|candidate| {
            candidate.identity == identities.intern(&referee_identity)
                && matches!(candidate.shape, StructuralTypeShape::Record { .. })
        })
    {
        terminal = overlay.id;
        byte_overlay = Some(ByteOverlaySource { referee });
    }
    // A record-as-record recast keeps the source declaration as the
    // namespace; each read rejoins its view field to the source leaf. A
    // mutable view reads the same way: the loan only adds write authority.
    let typed_overlay = (loan.recast_source.is_valid()
        && byte_overlay.is_none()
        && runtime_index.is_none()
        && declaration.identity != identities.intern(&referee_identity)
        && matches!(declaration.shape, StructuralTypeShape::Record { .. }))
    .then_some(TypedRecordOverlay {
        source: loan.recast_source,
        referee,
    });
    // Segments below the runtime index walk inside the selected element;
    // they leave both path vocabularies so the emitted read can splice the
    // selector in where the canonical spelling has no runtime segment.
    let (carrier, element_suffix) = match runtime_split {
        Some(split) => (path.split_off(split), carrier_path.split_off(split)),
        None => (Vec::new(), Vec::new()),
    };
    let primitive = checked
        .primitive_type_reference(referee)
        .map(|viewed| {
            let (field, scalar_type) = match primitive_field {
                Some((field, scalar_type)) => (Some(field), scalar_type),
                None => match declaration.shape {
                    StructuralTypeShape::PrimitiveScalar(scalar_type) => (None, scalar_type),
                    _ => {
                        return unsupported("primitive observation has no declared scalar storage");
                    }
                },
            };
            Ok(PrimitiveObservation {
                scalar_type,
                viewed_type: terminal_scalar_type(viewed)?,
                field,
                path: if runtime_split.is_some() {
                    carrier.clone()
                } else {
                    path.clone()
                },
                element_path: element_suffix.clone(),
                view_container,
            })
        })
        .transpose()?;
    Ok(Some(CapturedPlaceSubject {
        primitive,
        access: loan.access,
        source,
        path,
        carrier_path,
        terminal,
        byte_overlay,
        typed_overlay,
        runtime_index,
        view_container,
        element_suffix,
        decided_byte_view_store: loan.decided_byte_view_store,
    }))
}

pub(crate) struct LoanedStore {
    pub symbol: symbols::SymbolHandle,
    pub source: LoanedSource,
    pub path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub byte_overlay: Option<ByteOverlayField>,
    /// `true` when the loan's formation admission proved its byte window —
    /// the recast's `index + view footprint <= region length` — so the byte
    /// store's emitted obligations are source-checked rather than re-proven.
    pub decided_byte_view_store: bool,
}

/// Rejoin a field store to the same captured place used by observations.
/// This resolves geometry, not live backing custody; the operation frame must
/// still establish exclusive access to the returned root before emitting it.
pub(crate) fn loaned_store(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    root: checked_trees::CheckedStoreRoot,
    carrier_path: &[checked_trees::CheckedUnitStructuralPathSegment],
    field_identity: &checked_trees::CheckedMemberIdentity,
    primitive_type: checked_trees::types::PrimitiveType,
    types: &[StructuralTypeDeclaration],
) -> Result<Option<LoanedStore>, LoweringError> {
    let checked_trees::CheckedStoreRoot::Local { symbol } = root else {
        return Ok(None);
    };
    let Some(loan) = loan_subject(identities, checked, machine, state, symbol, types)? else {
        return Ok(None);
    };
    if loan.access != checked_trees::CheckedStructuralAccess::MutableBorrow {
        return unsupported("record store requires a mutable captured loan");
    }
    let field = crate::emission::structural_scalar_store::lower_structural_scalar_store_local(
        identities,
        primitive_type,
        carrier_path,
        field_identity,
        loan.terminal,
        types,
    )?;
    let mut path = captured_store_path(identities, &loan)?;
    let byte_overlay = if let Some(overlay) = loan.byte_overlay {
        // Byte-region admission requires a recursively fact-free referee.
        // The encoding-fit proof is not a semantic field-range proof: if that
        // admission ever broadens, retain rather than silently drop the latter.
        if field.requires_range_obligation {
            return unsupported(
                "byte record store requires separate semantic field-range evidence",
            );
        }
        // Source replay has already authenticated the complete authored
        // destination against this checked path. Resolve normalized geometry
        // from that path directly; reconstructing a second field recognizer
        // here would discard the runtime selectors the store retained.
        let mut scalar_path = carrier_path.to_vec();
        scalar_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field_identity.clone(),
        ));
        Some(overlay.scalar_path(checked, &scalar_path, terminal_scalar_type(primitive_type)?)?)
    } else {
        path.extend(carrier_path.iter().cloned());
        None
    };
    Ok(Some(LoanedStore {
        symbol,
        source: loan.source,
        path,
        byte_overlay,
        decided_byte_view_store: loan.decided_byte_view_store,
    }))
}

/// Primitive array elements and record fields use the same captured backing.
/// The full leaf path is geometry only; source replay separately rejoins the
/// authored assignment, and emission still proves each stored byte's bound.
pub(crate) fn loaned_primitive_store(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    destination: &checked_trees::CheckedStoreRoot,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
    types: &[StructuralTypeDeclaration],
) -> Result<Option<LoanedStore>, LoweringError> {
    let checked_trees::CheckedStoreRoot::Local { symbol } = destination else {
        return Ok(None);
    };
    let Some(loan) = loan_subject(identities, checked, machine, state, *symbol, types)? else {
        return Ok(None);
    };
    let Some(overlay) = &loan.byte_overlay else {
        return Ok(None);
    };
    if loan.access != checked_trees::CheckedStructuralAccess::MutableBorrow {
        return unsupported("record store requires a mutable captured loan");
    }
    let (_, scalar_type) = crate::emission::primitive_store::lower_destination_path(
        identities,
        loan.terminal,
        path,
        types,
    )?;
    Ok(Some(LoanedStore {
        symbol: *symbol,
        source: loan.source,
        path: captured_store_path(identities, &loan)?,
        byte_overlay: Some(overlay.scalar_path(checked, path, scalar_type)?),
        decided_byte_view_store: loan.decided_byte_view_store,
    }))
}

fn captured_store_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    loan: &CapturedPlaceSubject,
) -> Result<Vec<checked_trees::CheckedUnitStructuralPathSegment>, LoweringError> {
    let mut path = Vec::new();
    for segment in &loan.carrier_path {
        path.push(checked_path_segment(identities, segment)?);
    }
    if let Some(selector) = &loan.runtime_index {
        // Byte overlays resolve this authored coordinate through their
        // formation-time snapshot. Other record loans still require an
        // immutable coordinate; neither path may reload a mutable selector.
        if loan.byte_overlay.is_none()
            && !matches!(
                selector,
                checked_trees::CheckedRuntimeIndex::Parameter { .. }
                    | checked_trees::CheckedRuntimeIndex::Local { .. }
            )
        {
            return unsupported("record store requires an immutable captured selector");
        }
        path.push(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(selector.clone()));
    }
    for segment in &loan.element_suffix {
        path.push(checked_path_segment(identities, segment)?);
    }
    Ok(path)
}

fn checked_path_segment(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    segment: &terminal_psi::StructuralPathSegment,
) -> Result<checked_trees::CheckedUnitStructuralPathSegment, LoweringError> {
    use checked_trees::CheckedUnitStructuralPathSegment as Checked;
    match segment {
        terminal_psi::StructuralPathSegment::Field(field) => Ok(Checked::Field(
            checked_trees::CheckedMemberIdentity::of_spelling(
                identities.text(*field).unwrap_or_default(),
            ),
        )),
        terminal_psi::StructuralPathSegment::FixedIndex(index) => Ok(Checked::FixedIndex(*index)),
        _ => unsupported("record loan captured an unsupported structural segment"),
    }
}

/// Bind every record field read the `machines`' computations perform.
/// `plan_machine` names the Unit plan being lowered: a fused roster's states
/// all carry their silent loans in that one plan, whichever machine declares
/// them.
pub(crate) fn prepare(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan_machine: symbols::SymbolHandle,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Binding>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut bindings: Vec<Binding> = Vec::new();
    for root in plans.machine_roots(machines) {
        for handle in super::reachable_nodes(checked, &[root.root])? {
            let node = plans.nodes.get(handle);
            let CheckedScalarComputationKind::StructuralField {
                source_type,
                field_identity,
                receiver_value,
                subject,
                field,
                ..
            } = &node.kind
            else {
                continue;
            };
            if bindings
                .iter()
                .any(|binding| binding.symbol == *field && binding.subject == *subject)
            {
                continue;
            }
            let identity = field_identity.clone();
            let loan = match subject.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } => loan_subject(identities, checked, plan_machine, root.state, symbol, types)?,
                _ => None,
            };
            let (
                loan_source,
                byte_overlay,
                typed_overlay,
                carrier_path,
                mut path,
                mut declaration,
                runtime_index,
                view_container,
                element_suffix,
            ) = match loan {
                Some(subject) => (
                    Some(subject.source),
                    subject.byte_overlay,
                    subject.typed_overlay,
                    subject.carrier_path,
                    subject.path,
                    terminal_psi::find_structural_type(&types, subject.terminal).ok_or(
                        LoweringError::Unsupported("record read's loaned carrier type is absent"),
                    )?,
                    subject.runtime_index,
                    subject.view_container,
                    subject.element_suffix,
                ),
                None => {
                    // Storage shares the owner's carrier namespace, not its
                    // qualified theory. The computation retains source_type;
                    // this lookup neither forgets nor establishes membership.
                    let root_identity = validation::structural_type_identity(
                        &checked.typed,
                        *source_type,
                        &[],
                        &[],
                    );
                    let mut declarations = types.iter().filter(|declaration| {
                        declaration.identity
                            == identities.intern(
                                &checked
                                    .type_identity_text(root_identity)
                                    .unwrap_or_default(),
                            )
                    });
                    let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                        "record read has no structural namespace",
                    ))?;
                    if declarations.next().is_some() {
                        return unsupported("record read has ambiguous structural namespaces");
                    }
                    (
                        None,
                        None,
                        None,
                        Vec::new(),
                        Vec::new(),
                        declaration,
                        None,
                        false,
                        Vec::new(),
                    )
                }
            };
            let (walk, identity, leaf_primitive) = match &typed_overlay {
                Some(overlay) => record_view_source_walk(checked, overlay, subject, *field)?,
                None => (
                    subject.path.clone(),
                    identity.spelling(),
                    node.primitive_type,
                ),
            };
            let observation_root_type = declaration.id;
            let mut primitive_path = carrier_path.clone();
            let mut selected_case_fields = None;
            for segment in &walk {
                let identity = match segment {
                    checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => identity,
                    checked_trees::CheckedUnitStructuralPathSegment::Case(identity) => {
                        let (StructuralTypeShape::Sum { cases }
                        | StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
                        else {
                            return unsupported("field observation case left its sum namespace");
                        };
                        let mut cases = cases.iter().filter(|case| {
                            case.identity == identities.intern(&identity.spelling())
                        });
                        let case = cases.next().ok_or(LoweringError::Unsupported(
                            "field observation selected an absent case",
                        ))?;
                        if cases.next().is_some() || selected_case_fields.is_some() {
                            return unsupported("field observation has an ambiguous case path");
                        }
                        path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                            case.id,
                        ));
                        primitive_path.push(terminal_psi::StructuralPathSegment::Case(case.id));
                        selected_case_fields = Some(case.fields.as_slice());
                        continue;
                    }
                    checked_trees::CheckedUnitStructuralPathSegment::Referent => {
                        let StructuralTypeShape::Reference { referent, .. } = declaration.shape
                        else {
                            return unsupported(
                                "record read referent has no reference declaration",
                            );
                        };
                        declaration = terminal_psi::find_structural_type(types, referent).ok_or(
                            LoweringError::Unsupported("record read referent type is absent"),
                        )?;
                        primitive_path.push(terminal_psi::StructuralPathSegment::Referent);
                        continue;
                    }
                    // A literal element of a fixed array of records selects
                    // that element's record as the next carrier.
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                        let StructuralTypeShape::FixedArray { element, length } = declaration.shape
                        else {
                            return unsupported("record read element left its array namespace");
                        };
                        if *index >= length {
                            return unsupported("record read element is outside its array");
                        }
                        path.push(
                            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(*index),
                        );
                        primitive_path
                            .push(terminal_psi::StructuralPathSegment::FixedIndex(*index));
                        declaration = terminal_psi::find_structural_type(types, element).ok_or(
                            LoweringError::Unsupported("record read element type is absent"),
                        )?;
                        continue;
                    }
                    _ => {
                        return unsupported(
                            "record read requires declared record field projections",
                        );
                    }
                };
                let fields = match selected_case_fields.take() {
                    Some(fields) => fields,
                    None => declaration
                        .shape
                        .direct_fields()
                        .ok_or(LoweringError::Unsupported(
                            "record read carrier path left its record namespace",
                        ))?,
                };
                let mut matching = fields
                    .iter()
                    .filter(|field| field.identity == identities.intern(&identity.spelling()));
                let selected = matching.next().ok_or(LoweringError::Unsupported(
                    "record read carrier field is absent",
                ))?;
                let Some(nested) = selected.field_type.as_structural() else {
                    return unsupported("record read carrier field is not structural");
                };
                if matching.next().is_some() || selected.relevance.is_erased() {
                    return unsupported("record read carrier field is ambiguous or erased");
                }
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    selected.id,
                ));
                primitive_path.push(terminal_psi::StructuralPathSegment::Field(
                    selected.identity.clone(),
                ));
                declaration = terminal_psi::find_structural_type(types, nested).ok_or(
                    LoweringError::Unsupported("record read carrier type is absent"),
                )?;
            }
            // A mixed carrier's common field reads like a record's; its
            // payload fields arrive through the `Case` step above.
            let scalar_type = terminal_scalar_type(node.primitive_type)?;
            let leaf_scalar_type = terminal_scalar_type(leaf_primitive)?;
            // A zero field means the walk's last literal element already
            // selected the scalar: its declaration is that primitive carrier.
            if !field.is_valid() {
                if typed_overlay.is_some()
                    || !matches!(declaration.shape,
                        StructuralTypeShape::PrimitiveScalar(actual) if actual == leaf_scalar_type)
                {
                    return unsupported("element observation does not end at its scalar element");
                }
                bindings.push(Binding {
                    symbol: *field,
                    subject: subject.clone(),
                    receiver: if receiver_value.is_valid() {
                        Some(structural_values::Storage::reserve(
                            identities,
                            checked,
                            *receiver_value,
                            types,
                            next_place,
                        )?)
                    } else {
                        None
                    },
                    loan_source,
                    byte_overlay: None,
                    carrier_path,
                    path,
                    runtime_index,
                    view_container,
                    element_path: element_suffix,
                    bounded_leaf: false,
                    primitive_path,
                    observation_root_type,
                    field: None,
                    scalar_type,
                    leaf_scalar_type,
                });
                continue;
            }
            let fields = match selected_case_fields {
                Some(fields) => fields,
                None => declaration
                    .shape
                    .direct_fields()
                    .ok_or(LoweringError::Unsupported(
                        "record read requires a record namespace",
                    ))?,
            };
            let retained = terminal_psi::field_by_identity(&fields, identities.intern(&identity))
                .ok_or(LoweringError::Unsupported(
                "record read has no declared field identity",
            ))?;
            let bounded_leaf =
                terminal_semantics::scalar_field_requires_evidence(&retained.field_type);
            primitive_path.push(terminal_psi::StructuralPathSegment::Field(
                retained.identity.clone(),
            ));
            // The selected field owns either scalar storage or a descriptor.
            // Follow declared readable crossings at the endpoint just as the
            // preceding path does; moving/replacing the descriptor is a
            // different operation and must not be inferred from this read.
            if let Some(mut selected) = retained.field_type.as_structural() {
                let mut crossings = 0;
                loop {
                    let selected_type = terminal_psi::find_structural_type(types, selected).ok_or(
                        LoweringError::Unsupported("scalar observation endpoint type is absent"),
                    )?;
                    match selected_type.shape {
                        StructuralTypeShape::Reference { referent, access } => {
                            if !matches!(
                                access,
                                terminal_psi::StructuralAccess::SharedBorrow
                                    | terminal_psi::StructuralAccess::MutableBorrow
                            ) || crossings >= types.len()
                            {
                                return unsupported("scalar observation endpoint is not readable");
                            }
                            primitive_path.push(terminal_psi::StructuralPathSegment::Referent);
                            selected = referent;
                            crossings += 1;
                        }
                        StructuralTypeShape::PrimitiveScalar(actual)
                            if actual == leaf_scalar_type && crossings != 0 =>
                        {
                            break;
                        }
                        _ => {
                            return unsupported(
                                "scalar observation endpoint is not a referenced primitive of its scalar type",
                            );
                        }
                    }
                }
            }
            let byte_overlay = byte_overlay
                .map(|overlay| overlay.field(checked, &subject.path, *field, scalar_type))
                .transpose()?;
            let mut element_path = element_suffix;
            if runtime_index.is_some() {
                // The capture selects the receiver; the checked observation
                // continues from it, including every stored-reference crossing.
                element_path.extend_from_slice(&primitive_path[carrier_path.len()..]);
            }
            bindings.push(Binding {
                symbol: *field,
                subject: subject.clone(),
                receiver: if receiver_value.is_valid() {
                    Some(structural_values::Storage::reserve(
                        identities,
                        checked,
                        *receiver_value,
                        types,
                        next_place,
                    )?)
                } else {
                    None
                },
                loan_source,
                byte_overlay,
                carrier_path,
                path,
                runtime_index,
                view_container,
                element_path,
                bounded_leaf,
                primitive_path,
                observation_root_type,
                field: Some(retained.id),
                scalar_type,
                leaf_scalar_type,
            });
        }
    }
    Ok(bindings)
}

pub(super) fn receiver_storage<'a>(
    fields: &'a [Binding],
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
) -> Result<&'a structural_values::Storage, LoweringError> {
    fields
        .iter()
        .find(|binding| binding.symbol == field && binding.subject == *subject)
        .and_then(|binding| binding.receiver.as_ref())
        .ok_or(LoweringError::Unsupported(
            "computed field receiver has no reserved storage",
        ))
}

/// The source-side walk for a read through a typed record view: the canonical
/// path segments from the source record to the leaf's declaring record, the
/// leaf's own field identity, and the leaf's declared carrier. Only a direct
/// scalar field of the view is admitted. The leaf keeps its source carrier;
/// the ordinary representation-view replay emits the carrier relation after
/// the source observation has been lowered.
fn record_view_source_walk(
    checked: &CheckedTrees,
    overlay: &TypedRecordOverlay,
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
) -> Result<
    (
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        String,
        PrimitiveType,
    ),
    LoweringError,
> {
    if !subject.path.is_empty() {
        return unsupported("record view read selects below a direct view field");
    }
    let leaf = validation::recast_record_source_leaf(
        &checked.typed,
        overlay.source,
        overlay.referee,
        field,
    )
    .ok_or(LoweringError::Unsupported(
        "record view field has no single source leaf",
    ))?;
    let (last, prefix) = leaf.path.split_last().ok_or(LoweringError::Unsupported(
        "record view source leaf has an empty path",
    ))?;
    let mut walk = Vec::new();
    for segment in prefix {
        walk.push(match segment {
            validation::RecastLeafSegment::Field(symbol) => {
                checked_trees::CheckedUnitStructuralPathSegment::Field(field_path_identity(
                    checked, *symbol,
                )?)
            }
            validation::RecastLeafSegment::Element(index) => {
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).map_err(|_| {
                        LoweringError::Unsupported("record view source element exceeds u64")
                    })?,
                )
            }
        });
    }
    let validation::RecastLeafSegment::Field(last) = last else {
        return unsupported("record view source leaf is not a field");
    };
    Ok((
        walk,
        field_path_identity(checked, *last)?.spelling(),
        leaf.primitive,
    ))
}

/// The canonical path identity of one declared record field symbol.
pub(crate) fn field_path_identity(
    checked: &CheckedTrees,
    field: symbols::SymbolHandle,
) -> Result<checked_trees::CheckedMemberIdentity, LoweringError> {
    let owner_symbol = checked.symbols.get(field).parent;
    checked
        .data_definitions()
        .iter()
        .filter(|owner| owner.symbol == owner_symbol)
        .flat_map(|owner| checked.data_members(owner))
        .find_map(|member| match member {
            DataMember::Field(candidate) if candidate.symbol == field => {
                Some(checked_trees::CheckedMemberIdentity::of_member(
                    candidate.identity,
                    candidate.name.clone(),
                ))
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "record view source field has no declaration",
        ))
}

/// A local loan's referee can be owned by no signature, result or operation.
/// Retain its concrete storage shape independently of scalar kind: primitive
/// representation views need the same geometry as record field observations.
/// This roots only authored local loans, not every stored pointer's referent;
/// catalog traversal still stops at reference edges, keeping growing recursive
/// applications finite. Geometry grants neither admission nor a storage home.
pub(crate) fn type_roots(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<typed_trees::type_identity::TypeIdentityId>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut types = Vec::new();
    // A loan used only by primitive or array reads has no field computation.
    // Its declared type still owns the observation geometry.
    if let Some(owner) = checked.machine(machine) {
        for state in checked.machine_states(owner) {
            for local in checked.facts.state_locals.locals(state.symbol) {
                let TypeReferenceNode::Reference { referee, .. } = checked
                    .type_reference_table
                    .type_reference(local.declared_type)
                else {
                    continue;
                };
                // The plan registry's identity rule looks through a
                // qualification: a loan of an element of `[E; N] in Trapping`
                // lends plain `E` storage.
                let identity =
                    validation::structural_type_identity(&checked.typed, *referee, &[], &[]);
                if !types.contains(&identity) {
                    types.push(identity);
                }
            }
        }
    }
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine)
    {
        for handle in super::reachable_nodes(checked, &[root.root])? {
            let CheckedScalarComputationKind::StructuralField { source_type, .. } =
                &plans.nodes.get(handle).kind
            else {
                continue;
            };
            let identity =
                validation::structural_type_identity(&checked.typed, *source_type, &[], &[]);
            if !types.contains(&identity) {
                types.push(identity);
            }
        }
    }
    Ok(types)
}

pub(super) fn observation(
    fields: &[Binding],
    bindings: &storage::ScalarBindings,
    machine: &checked_trees::machine::Machine,
    parameters: &[checked_trees::signature::StateParameter],
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let binding = field_binding(fields, subject, field, primitive)?;
    let source = observation_source(binding, bindings, machine, parameters, subject)?;
    observe(binding, bindings, source, None)
}

pub(super) fn completed_observation(
    fields: &[Binding],
    bindings: &storage::ScalarBindings,
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
    source: semantic_vocabulary::PlaceId,
) -> Result<LoweredDirectExpression, LoweringError> {
    let binding = field_binding(fields, subject, field, primitive)?;
    observe(
        binding,
        bindings,
        terminal_psi::StructuralArgument {
            place: source,
            path: Vec::new(),
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
        None,
    )
}

/// A copied operand owns a subtree of the original observation root. Retain
/// the same declared leaf while making its path relative to that snapshot.
#[allow(clippy::too_many_arguments)]
fn copied_observation(
    fields: &[Binding],
    bindings: &storage::ScalarBindings,
    types: &[terminal_psi::StructuralTypeDeclaration],
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
    place: semantic_vocabulary::PlaceId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    source: &crate::emission::operation_emission::calls::PreparedStructuralArgument,
) -> Result<LoweredDirectExpression, LoweringError> {
    use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
    if !terminal_psi::StructuralAccess::SharedBorrow.admits(source.access) {
        return unsupported("captured field observation requires readable source custody");
    }
    let mut binding = field_binding(fields, subject, field, primitive)?.clone();
    let prefix = source
        .path
        .iter()
        .map(|segment| match segment {
            PreparedStructuralPathSegment::Static(segment) => Ok(segment.clone()),
            _ => Err(LoweringError::Unsupported(
                "captured field observation requires its evaluated projection path",
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root_type = source.root_type.ok_or(LoweringError::Unsupported(
        "captured field observation lost its source carrier",
    ))?;
    let (canonical_prefix, selected_type) =
        terminal_semantics::canonical_static_projection(types.iter(), root_type, &prefix).ok_or(
            LoweringError::Unsupported("captured field observation lost its source projection"),
        )?;
    if selected_type.id != structural_type || root_type != binding.observation_root_type {
        return unsupported("captured field observation changes its operand carrier");
    }
    binding.primitive_path = binding
        .primitive_path
        .strip_prefix(prefix.as_slice())
        .ok_or(LoweringError::Unsupported(
            "captured field observation lost its relative path",
        ))?
        .to_vec();
    binding.path = binding
        .path
        .strip_prefix(canonical_prefix.as_slice())
        .ok_or(LoweringError::Unsupported(
            "captured field observation lost its relative canonical path",
        ))?
        .to_vec();
    binding.observation_root_type = structural_type;
    observe(
        &binding,
        bindings,
        terminal_psi::StructuralArgument {
            place,
            path: Vec::new(),
            access: terminal_psi::StructuralAccess::SharedBorrow,
        },
        Some(structural_type),
    )
}

fn field_binding<'a>(
    fields: &'a [Binding],
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> Result<&'a Binding, LoweringError> {
    if subject.access == checked_trees::CheckedStructuralAccess::WriteOnlyBorrow {
        return unsupported("field observation requires readable source custody");
    }
    let scalar_type = terminal_scalar_type(primitive)?;
    let mut matching = fields.iter().filter(|binding| {
        binding.symbol == field && binding.subject == *subject && binding.scalar_type == scalar_type
    });
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "record read lost its field binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("record read has ambiguous field bindings");
    }
    Ok(binding)
}

fn observation_source(
    binding: &Binding,
    bindings: &storage::ScalarBindings,
    _machine: &checked_trees::machine::Machine,
    parameters: &[checked_trees::signature::StateParameter],
    subject: &CheckedUnitStructuralArgumentPlan,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    // The ordinary shared-argument join already resolves an exact live local
    // without transferring it. Field replay admits only StructuralLocal sources;
    // use that same current storage mapping instead of a parallel read map. A
    // shared `&`-record loan local never enters that roster — it mints no place
    // — so the read resolves the captured parameter's place directly and lets
    // the emitted canonical path name the referee below it.
    Ok(match binding.loan_source {
        Some(LoanedSource::Parameter(parameter_index)) => bindings.borrowed_structural_home(
            &CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                },
                path: Vec::new(),
                type_identity: subject.type_identity.clone(),
                access: subject.access,
            },
            parameters,
        )?,
        Some(LoanedSource::Local(symbol)) => bindings.borrowed_structural_home(
            &CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                },
                path: Vec::new(),
                type_identity: subject.type_identity.clone(),
                access: subject.access,
            },
            parameters,
        )?,
        None => bindings.borrowed_structural_home(subject, parameters)?,
    })
}

fn observe(
    binding: &Binding,
    bindings: &storage::ScalarBindings,
    home: terminal_psi::StructuralArgument,
    captured_type: Option<semantic_vocabulary::StructuralTypeId>,
) -> Result<LoweredDirectExpression, LoweringError> {
    if !terminal_psi::StructuralAccess::SharedBorrow.admits(home.access) {
        return unsupported("field observation requires readable source custody");
    }
    let source = home.place;
    let scalar_type = binding.scalar_type;
    let primitive_path =
        if captured_type.is_none() && binding.loan_source.is_none() && binding.receiver.is_none() {
            bindings.structural_observation_path(
                &home,
                binding.observation_root_type,
                &binding.primitive_path,
            )?
        } else {
            let mut path = home.path.clone();
            path.extend_from_slice(&binding.primitive_path);
            path
        };
    let mut carrier_path = home.path.clone();
    carrier_path.extend_from_slice(&binding.carrier_path);
    let crosses_reference = primitive_path.contains(&terminal_psi::StructuralPathSegment::Referent);
    let canonical_prefix = if home.path.is_empty() {
        Some(Vec::new())
    } else {
        bindings.structural_place_type(source).and_then(|root| {
            terminal_semantics::canonical_static_projection(
                bindings.structural_declarations().iter(),
                root,
                &home.path,
            )
            .map(|(path, _)| path)
        })
    };
    let canonical_path = canonical_prefix.map(|mut path| {
        path.extend_from_slice(&binding.path);
        path
    });

    if let Some((terminal_psi::StructuralPathSegment::Field(_), carrier)) =
        primitive_path.split_last()
        && let Some((terminal_psi::StructuralPathSegment::Case(case), prefix)) =
            carrier.split_last()
        && let Some(field) = binding.field
        && let Some(payload) =
            bindings.established_case_payload(source, prefix, *case, field, scalar_type)?
    {
        return Ok(payload);
    }
    if binding.leaf_scalar_type != scalar_type
        && (binding.runtime_index.is_some() || binding.byte_overlay.is_some())
    {
        return unsupported("record view carrier change needs a static source leaf");
    }
    if binding.runtime_index.is_some() {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
            binding.subject.source
        else {
            return unsupported("record observation lost its captured loan identity");
        };
        let index = bindings.observation_coordinate(symbol)?;
        if let Some(overlay) = &binding.byte_overlay {
            if binding.view_container {
                return unsupported("record byte overlay requires fixed-array backing");
            }
            return byte_overlay_read(
                source,
                &carrier_path,
                index,
                overlay.offset,
                overlay.stored_type,
                binding.scalar_type,
            );
        }
        // The loan freezes its selector at formation. Re-evaluating a field
        // selector here would silently retarget the reference after a write.
        let scalar_type = binding.scalar_type;
        return Ok(if binding.view_container {
            if !carrier_path.is_empty() || binding.bounded_leaf {
                return unsupported(
                    "record loan's runtime element read needs a scalar leaf at the view root",
                );
            }
            LoweredDirectExpression::ElementViewRead {
                source,
                index: Box::new(index),
                path: binding
                    .element_path
                    .iter()
                    .cloned()
                    .map(LoweredElementPathSegment::Static)
                    .collect(),
                scalar_type,
                bound_decided: false,
            }
        } else {
            LoweredDirectExpression::IndexedPrimitiveRead {
                source,
                path: carrier_path.clone(),
                index: Box::new(index),
                extent: None,
                element_path: binding
                    .element_path
                    .iter()
                    .cloned()
                    .map(LoweredElementPathSegment::Static)
                    .collect(),
                scalar_type,
                bound_decided: false,
            }
        });
    }
    if let Some(overlay) = &binding.byte_overlay {
        let Some((terminal_psi::StructuralPathSegment::FixedIndex(index), prefix)) =
            carrier_path.split_last()
        else {
            return unsupported("record byte overlay has no captured byte coordinate");
        };
        return byte_overlay_read(
            source,
            prefix,
            LoweredDirectExpression::IntegerLiteral {
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*index)),
                scalar_type: terminal_scalar_type(PrimitiveType::U64)?,
            },
            overlay.offset,
            overlay.stored_type,
            scalar_type,
        );
    }
    // Pure expressions and computations observe the same selected payload
    // carrier. Keep direct scalar snapshots above this preparation; nested
    // reads retain their exact relative path after the common carrier copy.
    // Pointer crossings remain observation-time reads of their live referent,
    // not a canonical owned-carrier copy.
    let mut read_source = PrimitiveReadSource::Place(source);
    let mut field_path = canonical_path.clone();
    let mut read_path = primitive_path.clone();
    if !crosses_reference && let Some(path) = &canonical_path {
        let prepared =
            crate::expression_preparation::bindings::structural_fields::prepare_case_observation(
                source,
                bindings
                    .structural_place_type(source)
                    .or(captured_type)
                    .unwrap_or(binding.observation_root_type),
                path,
                bindings.structural_declarations(),
            )?;
        read_path = primitive_path
            .get(prepared.captured_prefix_length..)
            .ok_or(LoweringError::Unsupported(
                "case observation lost its relative primitive path",
            ))?
            .to_vec();
        read_source = prepared.source;
        field_path = Some(prepared.path);
    }
    let leaf_type = binding.leaf_scalar_type;
    let field = binding.field;
    let leaf = match leaf_type {
        _ if crosses_reference || canonical_path.is_none() || field.is_none() => {
            LoweredDirectExpression::PrimitiveRead {
                source: read_source,
                path: read_path,
                scalar_type: leaf_type,
            }
        }
        ScalarType::Boolean => LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::StructuralField {
                source: read_source,
                path: field_path.ok_or(LoweringError::Unsupported(
                    "record field observation lost its canonical source path",
                ))?,
                field: field.ok_or(LoweringError::Unsupported(
                    "record field observation lost its declared field",
                ))?,
            }),
        },
        ScalarType::Integer(_) => LoweredDirectExpression::StructuralField {
            source: read_source,
            path: field_path.ok_or(LoweringError::Unsupported(
                "record field observation lost its canonical source path",
            ))?,
            field: field.ok_or(LoweringError::Unsupported(
                "record field observation lost its declared field",
            ))?,
            scalar_type: leaf_type,
        },
        ScalarType::IeeeFloat(_) => LoweredDirectExpression::PrimitiveRead {
            source: read_source,
            path: read_path,
            scalar_type: leaf_type,
        },
    };
    // Every record view observes its leaf at the declared source carrier,
    // then uses the same carrier relation as a whole scalar re-view.
    if leaf_type == scalar_type {
        Ok(leaf)
    } else {
        crate::expression_preparation::representation_view::replay(leaf, scalar_type)
    }
}
