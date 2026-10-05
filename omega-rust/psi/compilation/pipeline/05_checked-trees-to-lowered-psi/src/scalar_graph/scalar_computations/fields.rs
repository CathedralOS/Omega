//! Record projections bind a declared field once, then resolve the current local place
//! at each evaluation site. They materialize an ordinary scalar binding before
//! later effects; neither a synthetic parameter nor a delayed return load may
//! stand in for that observation. Stage 04 retains the root type and field identity
//! together with explicit stored-reference crossings; lowering consumes that plan.
//! Reads through references use PrimitiveScalarRead because they observe a value
//! without naming a canonical owned leaf. Terminal independently checks custody.
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
use crate::scalar_graph::scalar_graph_lowering::structural_values;
use checked_trees::CheckedUnitStructuralArgumentPlan;
use checked_trees::data::DataMember;
use checked_trees::types::TypeReferenceNode;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

impl super::Expansion<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn structural_field(
        &mut self,
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
        let mut discards = Vec::new();
        let expression = if receiver.is_valid() {
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
            // Disposal is a property of the produced type, not of the field
            // selected from it. Never erase a nominal hook or a reference debt
            // by describing an arbitrary affine record as a trivial discard.
            if !crate::content_classes::plain_owned_with_numeric_constraints(
                self.checked,
                root.type_reference,
            ) {
                return unsupported(
                    "unimplemented: computed receiver disposal requires its nontrivial cleanup plan",
                );
            }
            let prepared = structural_values::prepare_value(
                self.checked,
                self.qualifications,
                site.state,
                site.statement,
                receiver,
                values.calls_for_root(receiver).unwrap_or(&[]),
                root.type_reference,
                site.bindings,
                input_types,
                self.structural_types,
                receiver_storage(self.fields, subject, field)?,
            )?;
            if self.checked.type_multiplicity(root.type_reference)
                == language_semantics::Multiplicity::Affine
            {
                discards.push(prepared.place);
            }
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
        let observation = self.push(crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarBranchState {
            structural_parameters: Vec::new(),
            structural_effects: Vec::new(),
            parameter_types: input_types.to_vec(),
            erased_formal_types: Vec::new(),
            erased_proof_formals: Vec::new(),
            bindings: vec![crate::emission::operation_emission::LoweredScalarBinding::Expression(expression)],
            terminator: crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarBranchTerminator::Jump {
                target, arguments,
                trivial_affine_discards: discards,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
            },
        });
        match completed {
            Some(value) => value.finish(site.state, site.bindings, input_types, observation, self),
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
    field: StructuralFieldId,
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

/// One authored segment of a shared loan's captured place: a declared record
/// member projection, a literal index into a fixed array, or a runtime
/// selector the loan's own obligations bound.
enum LoanedPathSegment {
    Field(String),
    FixedIndex(u64),
    RuntimeIndex(ExpressionHandle),
}

pub(crate) fn prepare_place_observations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<std::sync::Arc<[PlaceObservation]>, LoweringError> {
    let (_, authored) = crate::declarations::state_declaration(checked, state)?;
    let mut loans = Vec::new();
    for (statement_index, statement) in checked
        .statement_table
        .statements(authored.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        let TypeReferenceNode::Reference { referee, .. } = checked
            .type_reference_table
            .type_reference(local.type_reference)
        else {
            continue;
        };
        let statement_index = u32::try_from(statement_index).map_err(|_| {
            LoweringError::Unsupported("place observation statement ordinal overflow")
        })?;
        let primitive = checked.primitive_type_reference(*referee).is_some();
        let operations = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine)
            .map(|plan| plan.operations.as_slice())
            .or_else(|| {
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_for_machine(machine)
                    .and_then(|plan| plan.states.iter().find(|row| row.state == state))
                    .map(|state| state.operations.as_slice())
            });
        let established = operations.is_some_and(|operations| operations.iter().any(|operation| {
            matches!(operation, checked_trees::CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
                if result.statement_index == statement_index)
        }));
        if primitive
            && (established
                || checked
                    .facts
                    .values
                    .structural_values
                    .root_for_expression(state, statement_index, local.initial_value)
                    .is_some())
        {
            // An actual reference carrier owns its capture and observations.
            continue;
        }
        let identity = checked.normalized_type_identity(*referee);
        if !primitive
            && !types.iter().any(|declaration| {
                declaration.identity == identity.as_str()
                    && matches!(declaration.shape, StructuralTypeShape::Record { .. })
            })
        {
            continue;
        }
        let capture = (|| {
            let Some(loan) = captured_place_subject(checked, machine, state, local.symbol, types)?
            else {
                return Ok(None);
            };
            let observation = if let Some(primitive) = loan.primitive {
                ObservationKind::Primitive(primitive)
            } else if let Some(overlay) = loan.byte_overlay {
                if loan.view_container || !loan.element_suffix.is_empty() {
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
                source: loan.source,
                carrier_path: loan.carrier_path,
                runtime_index: loan.runtime_index,
                observation,
            }))
        })();
        let capture = match capture {
            Ok(Some(capture)) => Ok(capture),
            Ok(None) => continue,
            Err(error) => Err(error),
        };
        loans.push(PlaceObservation {
            statement_index,
            symbol: local.symbol,
            capture,
        });
    }
    Ok(loans.into())
}

/// Walk an authored expression back to its root symbol, collecting member and
/// literal-index segments in authored order. Only expression shapes that name
/// an established storage place admit a captured path; a recast peels to the
/// expression it reinterprets.
fn captured_subject_path(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, Vec<LoanedPathSegment>)> {
    match checked.expression_table.expression(expression) {
        ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            captured_subject_path(checked, cast.value)
        }
        ExpressionNode::Name(path) => {
            let mut members = checked
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .iter();
            let root = members.next().copied().filter(|symbol| symbol.is_valid())?;
            Some((
                root,
                members
                    .map(|member| {
                        LoanedPathSegment::Field(checked.symbols.name(*member).to_string())
                    })
                    .collect(),
            ))
        }
        ExpressionNode::Member(member) => {
            let (root, mut segments) = captured_subject_path(checked, member.receiver)?;
            segments.push(LoanedPathSegment::Field(
                checked.symbols.name(member.member_symbol).to_string(),
            ));
            Some((root, segments))
        }
        ExpressionNode::Indexed(indexed) => {
            let (root, mut segments) = captured_subject_path(checked, indexed.collection)?;
            match checked
                .expression_table
                .constant_integer_value(indexed.index)
                .and_then(|index| u64::try_from(index).ok())
            {
                Some(index) => segments.push(LoanedPathSegment::FixedIndex(index)),
                // A non-literal selector keeps its expression: the walk
                // resolves it to the checked coordinate the same expression
                // took on the loan's admission path.
                None => segments.push(LoanedPathSegment::RuntimeIndex(indexed.index)),
            }
            Some((root, segments))
        }
        _ => None,
    }
}

/// The established place an observation local borrows from: the captured
/// root join, the canonical prefix to the referent, the referee's
/// structural declaration at that prefix, and — when the prefix ends on a
/// `u8` element (`&self.buf[0] as &T`) — the referee record the byte region
/// overlays.
struct CapturedPlaceSubject {
    access: language_core::ReferenceAccess,
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
}

/// A captured place observation is a silent loan marker: it mints no established
/// place of its own, so a member read through it must resolve the structural
/// parameter the loan captured plus the canonical path from that root to the
/// referee. `None` means the local is not such a loan; `Some` carries the
/// captured source join, the canonical prefix, and the declared type the
/// prefix ends on so the ordinary subject walk can continue there.
fn captured_place_subject(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    symbol: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<Option<CapturedPlaceSubject>, LoweringError> {
    let (source_machine, source_state) = crate::declarations::state_declaration(checked, state)?;
    if source_machine.symbol != machine {
        return Ok(None);
    }
    let Some(local) = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .iter()
        .find_map(|node| match node {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        })
    else {
        return Ok(None);
    };
    if local.is_mutable {
        return Ok(None);
    }
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return Ok(None);
    };
    if !matches!(
        *access,
        language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
    ) || (checked.primitive_type_reference(*referee).is_none()
        && !matches!(
            checked.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        ))
    {
        return Ok(None);
    }
    let mut initializer = local.initial_value;
    let mut recast = false;
    let target = loop {
        match checked.expression_table.expression(initializer) {
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                recast = true;
                initializer = cast.value;
            }
            ExpressionNode::Borrow(borrow) if borrow.access == *access => break borrow.target,
            _ => return Ok(None),
        }
    };
    let Some((root, mut segments)) = captured_subject_path(checked, target) else {
        return unsupported("record loan captured an unreadable place");
    };
    // An immutable local scalar pinned to a literal is a fixed index for
    // every rule that matters: the checker folds it through the literal-bound
    // gate on the loan's admission path, so the replay folds it through the
    // same literal segment here before the runtime-selector route sees it.
    for segment in segments.iter_mut() {
        if let LoanedPathSegment::RuntimeIndex(expression) = segment
            && let ExpressionNode::Name(name) = checked.expression_table.expression(*expression)
            && name.symbol.is_valid()
            && let Some(index) =
                crate::expression_preparation::source_custody::structural_places::constant_local_index(
                    checked,
                    source_state,
                    name.symbol,
                )
        {
            *segment = LoanedPathSegment::FixedIndex(index);
        }
    }
    let parameters = checked.state_parameters(source_state);
    // A parameter-rooted capture names its declaration. A local-rooted capture keeps
    // that local's established place — the owned record is established storage
    // even though the `&` loan binding itself stays a silent marker.
    let authored_index = parameters
        .iter()
        .position(|parameter| parameter.symbol == root);
    let (source, mut type_reference) = match authored_index {
        Some(authored_index) => {
            // Both unit-plan shapes carry the same authored-position roster:
            // a single-state body keeps it on the machine row, a composed
            // machine keeps one per state.
            let structural_parameters = if let Some(plan) = checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine)
            {
                &plan.structural_parameters
            } else if let Some(plan) = checked
                .facts
                .flow
                .terminal_unit_effects
                .composed_for_machine(machine)
                .and_then(|plan| plan.states.iter().find(|plan| plan.state == state))
            {
                &plan.structural_parameters
            } else {
                return unsupported("record loan's state left the composed plan");
            };
            let Some(parameter_index) = structural_parameters
                .iter()
                .position(|parameter| parameter.position as usize == authored_index)
            else {
                return unsupported("record loan captured a non-structural parameter");
            };
            (
                LoanedSource::Parameter(parameter_index as u32),
                parameters[authored_index].type_reference,
            )
        }
        None => {
            let Some(root_local) = checked
                .statement_table
                .statements(source_state.statement_nodes)
                .iter()
                .find_map(|node| match node {
                    StatementNode::LocalData(local) if local.symbol == root => Some(local),
                    _ => None,
                })
            else {
                return unsupported("record loan captured no declared storage");
            };
            (LoanedSource::Local(root), root_local.type_reference)
        }
    };
    while let TypeReferenceNode::Reference { referee, .. } =
        checked.type_reference_table.type_reference(type_reference)
    {
        type_reference = *referee;
    }
    // The receiver's authored type names the machine itself; its structural
    // carrier is the attached data record the machine's states operate on.
    let root_identity = match checked.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named {
            symbol: named_symbol,
            ..
        } if *named_symbol == source_machine.symbol => format!(
            "named(name({}))",
            checked.symbols.name(source_machine.attached_data_symbol)
        ),
        _ => checked
            .normalized_type_identity(type_reference)
            .into_string(),
    };
    let mut declaration = types
        .iter()
        .find(|declaration| declaration.identity == root_identity)
        .ok_or(LoweringError::Unsupported(
            "record loan's captured root has no structural namespace",
        ))?;
    let mut path = Vec::new();
    let mut carrier_path = Vec::new();
    let mut runtime_index = None;
    let mut view_container = false;
    let mut runtime_split = None;
    // The last literal index step and the array extent it selected inside —
    // set only when that index ends the prefix (`a[i]` cannot extend into a
    // `u8` element's members, so a byte-carrier prefix always ends there).
    let mut tail = None;
    let mut primitive_field = None;
    let segment_count = segments.len();
    for (segment_ordinal, segment) in segments.into_iter().enumerate() {
        match segment {
            LoanedPathSegment::Field(identity) => {
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return unsupported("record loan's captured path left its record namespace");
                };
                let mut matching = fields.iter().filter(|field| field.identity == identity);
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
                    if checked.primitive_type_reference(*referee).is_none() {
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
                    tail = None;
                    break;
                }
                let StructuralFieldType::Structural(nested) = selected.field_type else {
                    return unsupported("record loan's captured field is not structural");
                };
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    selected.id,
                ));
                carrier_path.push(terminal_psi::StructuralPathSegment::Field(
                    selected.identity.clone(),
                ));
                tail = None;
                declaration = types
                    .iter()
                    .find(|declaration| declaration.id == nested)
                    .ok_or(LoweringError::Unsupported(
                        "record loan's captured type is absent",
                    ))?;
            }
            LoanedPathSegment::FixedIndex(index) => {
                // An element-view root carries no literal extent: the index's
                // bound is the view's live length, proven by the loan's own
                // captured obligations rather than replayed here.
                let element = match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, length } => {
                        if index >= *length {
                            return unsupported("record loan's captured index is out of bounds");
                        }
                        tail = Some((index, *length));
                        *element
                    }
                    StructuralTypeShape::ElementView { element } => {
                        tail = None;
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
                declaration = types
                    .iter()
                    .find(|declaration| declaration.id == element)
                    .ok_or(LoweringError::Unsupported(
                        "record loan's captured element type is absent",
                    ))?;
            }
            LoanedPathSegment::RuntimeIndex(expression) => {
                if runtime_index.is_some() {
                    return unsupported("record loan selects at most one runtime element");
                }
                // The selector resolves to the same checked coordinate the
                // admission walk minted: a bare immutable integer parameter,
                // or a pure field chain below an entry parameter. Neither
                // container shape carries its bound on the segment — an
                // element view's bound is its live extent and a fixed
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
                runtime_index = Some(
                    crate::expression_preparation::source_custody::structural_places::runtime_index_selector(
                        checked,
                        source_machine,
                        source_state,
                        expression,
                    )?,
                );
                if validation::expression_result_type_reference(
                    &checked.typed,
                    source_machine,
                    source_state,
                    expression,
                )
                .is_none()
                {
                    return unsupported("record loan's index selector lost its leaf type");
                }
                runtime_split = Some(path.len());
                tail = None;
                declaration = types
                    .iter()
                    .find(|declaration| declaration.id == element)
                    .ok_or(LoweringError::Unsupported(
                        "record loan's captured element type is absent",
                    ))?;
            }
        }
    }
    // The u8 element is the backing coordinate, not the record namespace.
    // Resolve fields under the referee, then rejoin their recast geometry.
    // Static footprint checks and per-byte Terminal obligations below keep
    // this namespace selection from granting access outside the region.
    let mut terminal = declaration.id;
    let mut byte_overlay = None;
    // The referee's authored type names the machine itself under the same
    // remap the captured root already applied.
    let referee_identity = match checked.type_reference_table.type_reference(*referee) {
        TypeReferenceNode::Named {
            symbol: named_symbol,
            ..
        } if *named_symbol == source_machine.symbol => format!(
            "named(name({}))",
            checked.symbols.name(source_machine.attached_data_symbol)
        ),
        _ => checked.normalized_type_identity(*referee).into_string(),
    };
    if (tail.is_some() || runtime_index.is_some())
        && matches!(
            &declaration.shape,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                    && integer.bits() == 8
        )
        && let Some(overlay) = types.iter().find(|candidate| {
            candidate.identity == referee_identity
                && matches!(candidate.shape, StructuralTypeShape::Record { .. })
        })
    {
        terminal = overlay.id;
        byte_overlay = Some(ByteOverlaySource {
            referee: *referee,
            remaining_bytes: tail.and_then(|(index, length)| length.checked_sub(index)),
        });
    }
    // A record-as-record recast keeps the source declaration as the
    // namespace; each read rejoins its view field to the source leaf. A
    // mutable view reads the same way: the loan only adds write authority.
    // The recast may wrap the borrow or sit inside its target
    // (`&place as &View` parses as a borrow of the recast place).
    let recast = recast
        || matches!(
            checked.expression_table.expression(target),
            ExpressionNode::Cast(cast) if cast.form.is_recast()
        );
    let mut typed_overlay = None;
    if recast
        && byte_overlay.is_none()
        && runtime_index.is_none()
        && declaration.identity != referee_identity
        && matches!(declaration.shape, StructuralTypeShape::Record { .. })
    {
        let mut place = target;
        while let ExpressionNode::Cast(cast) = checked.expression_table.expression(place)
            && cast.form.is_recast()
        {
            place = cast.value;
        }
        let source = validation::declared_place_type_raw(
            &checked.typed,
            source_machine,
            Some(source_state),
            place,
        )
        .ok_or(LoweringError::Unsupported(
            "record view lost its source place type",
        ))?;
        typed_overlay = Some(TypedRecordOverlay {
            source,
            referee: *referee,
        });
    }
    // Segments below the runtime index walk inside the selected element;
    // they leave both path vocabularies so the emitted read can splice the
    // selector in where the canonical spelling has no runtime segment.
    let (carrier, element_suffix) = match runtime_split {
        Some(split) => (path.split_off(split), carrier_path.split_off(split)),
        None => (Vec::new(), Vec::new()),
    };
    let primitive = checked
        .primitive_type_reference(*referee)
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
        access: *access,
        source,
        path,
        carrier_path,
        terminal,
        byte_overlay,
        typed_overlay,
        runtime_index,
        view_container,
        element_suffix,
    }))
}

pub(crate) struct LoanedStore {
    pub symbol: symbols::SymbolHandle,
    pub source: LoanedSource,
    pub path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub byte_overlay: Option<ByteOverlayField>,
}

/// Rejoin a field store to the same captured place used by observations.
/// This resolves geometry, not live backing custody; the operation frame must
/// still establish exclusive access to the returned root before emitting it.
pub(crate) fn loaned_store(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
    types: &[StructuralTypeDeclaration],
) -> Result<Option<LoanedStore>, LoweringError> {
    let checked_trees::CheckedStoreRoot::Local { symbol } = store.destination else {
        return Ok(None);
    };
    let Some(loan) = captured_place_subject(checked, machine, state, symbol, types)? else {
        return Ok(None);
    };
    if loan.access != language_core::ReferenceAccess::Mutable {
        return unsupported("record store requires a mutable captured loan");
    }
    let field = crate::emission::structural_scalar_store::lower_structural_scalar_store_local(
        store,
        loan.terminal,
        types,
    )?;
    let mut path = captured_store_path(&loan)?;
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
        let mut scalar_path = store.carrier_path.clone();
        scalar_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            store.field_identity.clone(),
        ));
        Some(overlay.scalar_path(
            checked,
            &scalar_path,
            terminal_scalar_type(store.primitive_type)?,
        )?)
    } else {
        path.extend(store.carrier_path.iter().cloned());
        None
    };
    Ok(Some(LoanedStore {
        symbol,
        source: loan.source,
        path,
        byte_overlay,
    }))
}

/// Primitive array elements and record fields use the same captured backing.
/// The full leaf path is geometry only; source replay separately rejoins the
/// authored assignment, and emission still proves each stored byte's bound.
pub(crate) fn loaned_primitive_store(
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
    let Some(loan) = captured_place_subject(checked, machine, state, *symbol, types)? else {
        return Ok(None);
    };
    let Some(overlay) = &loan.byte_overlay else {
        return Ok(None);
    };
    if loan.access != language_core::ReferenceAccess::Mutable {
        return unsupported("record store requires a mutable captured loan");
    }
    let (_, scalar_type) =
        crate::emission::primitive_store::lower_destination_path(loan.terminal, path, types)?;
    Ok(Some(LoanedStore {
        symbol: *symbol,
        source: loan.source,
        path: captured_store_path(&loan)?,
        byte_overlay: Some(overlay.scalar_path(checked, path, scalar_type)?),
    }))
}

fn captured_store_path(
    loan: &CapturedPlaceSubject,
) -> Result<Vec<checked_trees::CheckedUnitStructuralPathSegment>, LoweringError> {
    let mut path = Vec::new();
    for segment in &loan.carrier_path {
        path.push(checked_path_segment(segment)?);
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
        path.push(checked_path_segment(segment)?);
    }
    Ok(path)
}

fn checked_path_segment(
    segment: &terminal_psi::StructuralPathSegment,
) -> Result<checked_trees::CheckedUnitStructuralPathSegment, LoweringError> {
    use checked_trees::CheckedUnitStructuralPathSegment as Checked;
    match segment {
        terminal_psi::StructuralPathSegment::Field(field) => Ok(Checked::Field(field.clone())),
        terminal_psi::StructuralPathSegment::FixedIndex(index) => Ok(Checked::FixedIndex(*index)),
        _ => unsupported("record loan captured an unsupported structural segment"),
    }
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machines: &[symbols::SymbolHandle],
    types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<Vec<Binding>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut bindings: Vec<Binding> = Vec::new();
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| machines.contains(&root.machine))
    {
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
                } => captured_place_subject(checked, root.machine, root.state, symbol, types)?,
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
                    types
                        .iter()
                        .find(|declaration| declaration.id == subject.terminal)
                        .ok_or(LoweringError::Unsupported(
                            "record read's loaned carrier type is absent",
                        ))?,
                    subject.runtime_index,
                    subject.view_container,
                    subject.element_suffix,
                ),
                None => {
                    let root_identity = checked.normalized_type_identity(*source_type);
                    let mut declarations = types
                        .iter()
                        .filter(|declaration| declaration.identity == root_identity.as_str());
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
                None => (subject.path.clone(), identity, node.primitive_type),
            };
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
                        let mut cases = cases.iter().filter(|case| case.identity == *identity);
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
                        declaration = types
                            .iter()
                            .find(|declaration| declaration.id == referent)
                            .ok_or(LoweringError::Unsupported(
                                "record read referent type is absent",
                            ))?;
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
                        declaration = types
                            .iter()
                            .find(|declaration| declaration.id == element)
                            .ok_or(LoweringError::Unsupported(
                                "record read element type is absent",
                            ))?;
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
                    None => {
                        let StructuralTypeShape::Record { fields } = &declaration.shape else {
                            return unsupported(
                                "record read carrier path left its record namespace",
                            );
                        };
                        fields.as_slice()
                    }
                };
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let selected = matching.next().ok_or(LoweringError::Unsupported(
                    "record read carrier field is absent",
                ))?;
                let StructuralFieldType::Structural(nested) = selected.field_type else {
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
                declaration = types
                    .iter()
                    .find(|declaration| declaration.id == nested)
                    .ok_or(LoweringError::Unsupported(
                        "record read carrier type is absent",
                    ))?;
            }
            let fields = match selected_case_fields {
                Some(fields) => fields,
                None => {
                    let StructuralTypeShape::Record { fields } = &declaration.shape else {
                        return unsupported("record read requires a record namespace");
                    };
                    fields.as_slice()
                }
            };
            let retained = fields
                .iter()
                .find(|field| field.identity == identity)
                .ok_or(LoweringError::Unsupported(
                    "record read has no declared field identity",
                ))?;
            let scalar_type = terminal_scalar_type(node.primitive_type)?;
            let leaf_scalar_type = terminal_scalar_type(leaf_primitive)?;
            let bounded_leaf =
                terminal_semantics::scalar_field_requires_evidence(&retained.field_type);
            primitive_path.push(terminal_psi::StructuralPathSegment::Field(
                retained.identity.clone(),
            ));
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
                        checked,
                        *receiver_value,
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
                field: retained.id,
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
    Ok((walk, field_path_identity(checked, *last)?, leaf.primitive))
}

/// The canonical path identity of one declared record field symbol.
pub(crate) fn field_path_identity(
    checked: &CheckedTrees,
    field: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    let owner_symbol = checked.symbols.get(field).parent;
    checked
        .data_definitions()
        .iter()
        .filter(|owner| owner.symbol == owner_symbol)
        .flat_map(|owner| checked.data_members(owner))
        .find_map(|member| match member {
            DataMember::Field(candidate) if candidate.symbol == field => {
                Some(candidate.path_identity())
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "record view source field has no declaration",
        ))
}

/// A projected loan's referee can be owned by no signature, result or
/// operation. Catalog declared record referees independently of their reads or
/// stores; this supplies geometry, never admission or an owned storage home.
pub(crate) fn type_roots(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<String>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut types = Vec::new();
    // A loan used only by array reads has neither a field computation nor a
    // store. Its declared type still owns the projection geometry.
    if let Some(owner) = checked
        .machines()
        .iter()
        .find(|owner| owner.symbol == machine)
    {
        for state in checked.machine_states(owner) {
            for statement in checked.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                let TypeReferenceNode::Reference { referee, .. } = checked
                    .type_reference_table
                    .type_reference(local.type_reference)
                else {
                    continue;
                };
                let referee_symbol = checked.type_reference_symbol(*referee);
                if !checked
                    .data_definitions()
                    .iter()
                    .any(|definition| definition.symbol == referee_symbol)
                {
                    continue;
                }
                let identity = checked.normalized_type_identity(*referee).into_string();
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
            let identity = checked.normalized_type_identity(*source_type).into_string();
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
    observe(binding, bindings, source)
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
    observe(binding, bindings, source)
}

fn field_binding<'a>(
    fields: &'a [Binding],
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> Result<&'a Binding, LoweringError> {
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
    machine: &checked_trees::machine::Machine,
    parameters: &[checked_trees::signature::StateParameter],
    subject: &CheckedUnitStructuralArgumentPlan,
) -> Result<semantic_vocabulary::PlaceId, LoweringError> {
    // The ordinary shared-argument join already resolves an exact live local
    // without transferring it. Field replay admits only StructuralLocal sources;
    // use that same current storage mapping instead of a parallel read map. A
    // shared `&`-record loan local never enters that roster — it mints no place
    // — so the read resolves the captured parameter's place directly and lets
    // the emitted canonical path name the referee below it.
    Ok(match binding.loan_source {
        Some(LoanedSource::Parameter(parameter_index)) => {
            bindings
                .borrowed_structural_argument(
                    &CheckedUnitStructuralArgumentPlan {
                        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                            parameter_index,
                        },
                        path: Vec::new(),
                        type_identity: subject.type_identity.clone(),
                        access: subject.access,
                    },
                    machine,
                    parameters,
                )?
                .place
        }
        Some(LoanedSource::Local(symbol)) => bindings
            .borrowed_structural_argument(
                &CheckedUnitStructuralArgumentPlan {
                    source:
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                            symbol,
                        },
                    path: Vec::new(),
                    type_identity: subject.type_identity.clone(),
                    access: subject.access,
                },
                machine,
                parameters,
            )?
            .place,
        None => bindings.shared_structural_place(subject, machine, parameters)?,
    })
}

fn observe(
    binding: &Binding,
    bindings: &storage::ScalarBindings,
    source: semantic_vocabulary::PlaceId,
) -> Result<LoweredDirectExpression, LoweringError> {
    let scalar_type = binding.scalar_type;
    let crosses_reference = binding
        .primitive_path
        .contains(&terminal_psi::StructuralPathSegment::Referent);
    if !crosses_reference
        && let [semantic_vocabulary::CanonicalStructuralPathSegment::Case(case)] =
            binding.path.as_slice()
        && let Some(payload) =
            bindings.established_case_payload(source, *case, binding.field, scalar_type)?
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
                &binding.carrier_path,
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
            if !binding.carrier_path.is_empty() || binding.bounded_leaf {
                return unsupported(
                    "record loan's runtime element read needs a scalar leaf at the view root",
                );
            }
            LoweredDirectExpression::ElementViewRead {
                source,
                index: Box::new(index),
                path: binding.element_path.clone(),
                scalar_type,
            }
        } else {
            LoweredDirectExpression::IndexedPrimitiveRead {
                source,
                path: binding.carrier_path.clone(),
                index: Box::new(index),
                inner_indexes: Vec::new(),
                element_path: binding
                    .element_path
                    .iter()
                    .cloned()
                    .map(LoweredElementPathSegment::Static)
                    .collect(),
                scalar_type,
            }
        });
    }
    if let Some(overlay) = &binding.byte_overlay {
        let Some((terminal_psi::StructuralPathSegment::FixedIndex(index), prefix)) =
            binding.carrier_path.split_last()
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
    let leaf_type = binding.leaf_scalar_type;
    let leaf = match leaf_type {
        _ if crosses_reference => LoweredDirectExpression::PrimitiveRead {
            source: PrimitiveReadSource::Place(source),
            path: binding.primitive_path.clone(),
            scalar_type: leaf_type,
        },
        ScalarType::Boolean => LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::StructuralField {
                source: source.into(),
                path: binding.path.clone(),
                field: binding.field,
            }),
        },
        ScalarType::Integer(_) => LoweredDirectExpression::StructuralField {
            source: source.into(),
            path: binding.path.clone(),
            field: binding.field,
            scalar_type: leaf_type,
        },
        ScalarType::IeeeFloat(_) => LoweredDirectExpression::PrimitiveRead {
            source: PrimitiveReadSource::Place(source),
            path: binding.primitive_path.clone(),
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
