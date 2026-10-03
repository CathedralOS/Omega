//! Record projections bind a declared field once, then resolve the current local place
//! at each evaluation site. They materialize an ordinary scalar binding before
//! later effects; neither a synthetic parameter nor a delayed return load may
//! stand in for that observation. Source replay separately reconstructs custody.
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
pub(crate) use crate::expression_preparation::bindings::recast_records::LoanedSource;
use crate::expression_preparation::bindings::recast_records::{
    ByteOverlayField, ByteOverlaySource, ByteRecordCapture, ByteRecordLoan, byte_overlay_read,
};
use checked_trees::CheckedUnitStructuralArgumentPlan;
use checked_trees::data::DataMember;
use checked_trees::types::TypeReferenceNode;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

#[derive(Clone)]
pub(crate) struct Binding {
    symbol: symbols::SymbolHandle,
    subject: CheckedUnitStructuralArgumentPlan,
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
    /// Exact declared path for the ordinary float primitive-read operation.
    primitive_path: Vec<terminal_psi::StructuralPathSegment>,
    /// The checked coordinate of the loan's one non-literal index segment,
    /// when the captured path selects an element by runtime value.
    runtime_index: Option<checked_trees::CheckedRuntimeIndex>,
    /// The index selector's integer primitive, retained so `Parameter`/`Local`
    /// coordinate reads reconstruct with their declared type.
    selector_primitive: Option<typed_trees::types::PrimitiveType>,
    /// `true` when the runtime index steps through an element view (live
    /// extent); `false` for a fixed array (literal extent on the type).
    view_container: bool,
    /// The structural type the runtime index selects — the element the
    /// bounded-leaf hoist materializes. `None` for static loans.
    element: Option<StructuralTypeId>,
    /// Canonical segments below the runtime index, inside the selected
    /// element, to the leaf's declaring record. Empty for static loans.
    carrier: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    /// The element path to the scalar leaf: every segment below the runtime
    /// index plus the leaf's own field projection.
    element_path: Vec<terminal_psi::StructuralPathSegment>,
    /// The leaf declares a bounded integer, so the runtime read must hoist
    /// the element instead of carrying the bound.
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

pub(crate) fn prepare_byte_record_loans(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<std::sync::Arc<[ByteRecordLoan]>, LoweringError> {
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
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
        let identity = checked.normalized_type_identity(*referee);
        if !types.iter().any(|declaration| {
            declaration.identity == identity.as_str()
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        }) {
            continue;
        }
        let capture = (|| {
            let Some(loan) = record_loan_subject(checked, machine, state, local.symbol, types)?
            else {
                return Ok(None);
            };
            let Some(overlay) = loan.byte_overlay else {
                return Ok(None);
            };
            // A byte-region record view is not an element-view descriptor. The
            // captured coordinate belongs to its fixed backing array.
            if loan.view_container || !loan.element_suffix.is_empty() {
                return Ok(None);
            }
            let Some(layout) = validation::recast_scalar_layout(&checked.typed, overlay.referee)
            else {
                return unsupported("record byte observation has no normalized scalar layout");
            };
            Ok(Some(ByteRecordCapture {
                coordinate: None,
                source: loan.source,
                carrier_path: loan.carrier_path,
                runtime_index: loan.runtime_index,
                selector_primitive: loan.selector_primitive,
                overlay,
                layout,
            }))
        })();
        let capture = match capture {
            Ok(Some(capture)) => Ok(capture),
            Ok(None) => continue,
            Err(error) => Err(error),
        };
        loans.push(ByteRecordLoan {
            statement_index: u32::try_from(statement_index).map_err(|_| {
                LoweringError::Unsupported("record loan statement ordinal overflow")
            })?,
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

/// The established place a record-loan local borrows from: the captured
/// root join, the canonical prefix to the referent, the referee's
/// structural declaration at that prefix, and — when the prefix ends on a
/// `u8` element (`&self.buf[0] as &T`) — the referee record the byte region
/// overlays.
struct RecordLoanSubject {
    access: language_core::ReferenceAccess,
    source: LoanedSource,
    path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    terminal: StructuralTypeId,
    byte_overlay: Option<ByteOverlaySource>,
    typed_overlay: Option<TypedRecordOverlay>,
    /// The checked coordinate of the loan's non-literal index, when the
    /// captured path selects an element by runtime value.
    runtime_index: Option<checked_trees::CheckedRuntimeIndex>,
    /// The selector's integer primitive for `Parameter`/`Local` coordinate
    /// reads, `None` for a `StructuralField` selector or a static loan.
    selector_primitive: Option<typed_trees::types::PrimitiveType>,
    /// `true` when the runtime index steps through an element view.
    view_container: bool,
    /// The structural type the runtime index selects, for the bounded-leaf
    /// hoist. `None` for static loans.
    element: Option<StructuralTypeId>,
    /// Canonical segments below the runtime index, inside the element.
    carrier: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    /// Terminal segments below the runtime index, inside the element.
    element_suffix: Vec<terminal_psi::StructuralPathSegment>,
}

/// A record-loan local is a silent loan marker: it mints no established
/// place of its own, so a member read through it must resolve the structural
/// parameter the loan captured plus the canonical path from that root to the
/// referee. `None` means the local is not such a loan; `Some` carries the
/// captured source join, the canonical prefix, and the declared type the
/// prefix ends on so the ordinary subject walk can continue there.
fn record_loan_subject(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    symbol: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<Option<RecordLoanSubject>, LoweringError> {
    let (source_machine, source_state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
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
    ) || !matches!(
        checked.type_reference_table.type_reference(*referee),
        TypeReferenceNode::Named { .. }
    ) {
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
    // A `self`-rooted capture names the machine's own declared symbol rather
    // than a parameter spelling; it resolves to the `is_self` parameter, the
    // same join the shared borrow rosters make. A local-rooted capture keeps
    // that local's established place — the owned record is established storage
    // even though the `&` loan binding itself stays a silent marker.
    let authored_index = parameters
        .iter()
        .position(|parameter| parameter.symbol == root)
        .or_else(|| {
            (root == source_machine.symbol)
                .then(|| parameters.iter().position(|parameter| parameter.is_self))?
        });
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
    let mut selector_primitive = None;
    let mut view_container = false;
    let mut selected_element = None;
    let mut runtime_split = None;
    // The last literal index step and the array extent it selected inside —
    // set only when that index ends the prefix (`a[i]` cannot extend into a
    // `u8` element's members, so a byte-carrier prefix always ends there).
    let mut tail = None;
    for segment in segments {
        match segment {
            LoanedPathSegment::Field(identity) => {
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return unsupported("record loan's captured path left its record namespace");
                };
                let mut matching = fields.iter().filter(|field| field.identity == identity);
                let selected = matching.next().ok_or(LoweringError::Unsupported(
                    "record loan's captured field is absent",
                ))?;
                let StructuralFieldType::Structural(nested) = selected.field_type else {
                    return unsupported("record loan's captured field is not structural");
                };
                if matching.next().is_some() || selected.relevance.is_erased() {
                    return unsupported("record loan's captured field is ambiguous or erased");
                }
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
                selector_primitive = checked.typed.primitive_type_reference(
                    validation::expression_result_type_reference(
                        &checked.typed,
                        source_machine,
                        source_state,
                        expression,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "record loan's index selector lost its leaf type",
                    ))?,
                );
                runtime_split = Some(path.len());
                selected_element = Some(element);
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
    Ok(Some(RecordLoanSubject {
        access: *access,
        source,
        path,
        carrier_path,
        terminal,
        byte_overlay,
        typed_overlay,
        runtime_index,
        selector_primitive,
        view_container,
        element: selected_element,
        carrier,
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
    let Some(loan) = record_loan_subject(checked, machine, state, symbol, types)? else {
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
    let Some(loan) = record_loan_subject(checked, machine, state, *symbol, types)? else {
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
    loan: &RecordLoanSubject,
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
                source_expression,
                subject,
                field,
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
            let field_symbol = checked.symbols.get(*field);
            if !field.is_valid() || field_symbol.kind != symbols::SymbolKind::Field {
                return unsupported("record read has no resolved field identity");
            }
            let mut owners = checked
                .data_definitions()
                .iter()
                .filter(|owner| owner.symbol == field_symbol.parent);
            let owner = owners.next().ok_or(LoweringError::Unsupported(
                "record read has no declared field owner",
            ))?;
            if owners.next().is_some() {
                return unsupported("record read has ambiguous field owners");
            }
            let authored = checked
                .data_members(owner)
                .iter()
                .find_map(|member| match member {
                    DataMember::Field(candidate) if candidate.symbol == *field => Some(candidate),
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported("record read field is absent"))?;
            let identity = authored.path_identity();
            let source = validation::local_scalar_record_field(
                &checked.typed,
                root.machine,
                root.state,
                root.statement_ordinal,
                *source_expression,
            )
            .ok_or(LoweringError::Unsupported(
                "record read lost its source declaration path",
            ))?;
            let loan = match subject.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                } => record_loan_subject(checked, root.machine, root.state, symbol, types)?,
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
                selector_primitive,
                view_container,
                element,
                carrier,
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
                    subject.selector_primitive,
                    subject.view_container,
                    subject.element,
                    subject.carrier,
                    subject.element_suffix,
                ),
                None => {
                    let root_identity = checked.normalized_type_identity(source.type_reference);
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
                        None,
                        false,
                        None,
                        Vec::new(),
                        Vec::new(),
                    )
                }
            };
            let (walk, identity, expected_type, leaf_primitive) = match &typed_overlay {
                Some(overlay) => {
                    let (walk, identity, leaf_primitive) =
                        record_view_source_walk(checked, overlay, subject, *field)?;
                    (walk, identity, None, leaf_primitive)
                }
                None => (
                    subject.path.clone(),
                    identity,
                    Some(subject.type_identity.as_str()),
                    node.primitive_type,
                ),
            };
            let mut primitive_path =
                if matches!(leaf_primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                    carrier_path.clone()
                } else {
                    Vec::new()
                };
            // Terminal's scalar field read names record fields followed by at
            // most one literal element selection; a deeper element path
            // (`rows[1].cells[2].value`) has no read form yet.
            if walk.iter().enumerate().any(|(position, segment)| {
                matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                ) && position + 1 != walk.len()
            }) {
                return unsupported("record read selects an element below a trailing element");
            }
            for segment in &walk {
                let identity = match segment {
                    checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => identity,
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
                        if matches!(leaf_primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                            primitive_path
                                .push(terminal_psi::StructuralPathSegment::FixedIndex(*index));
                        }
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
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return unsupported("record read carrier path left its record namespace");
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
                if matches!(leaf_primitive, PrimitiveType::F32 | PrimitiveType::F64) {
                    primitive_path.push(terminal_psi::StructuralPathSegment::Field(
                        selected.identity.clone(),
                    ));
                }
                declaration = types
                    .iter()
                    .find(|declaration| declaration.id == nested)
                    .ok_or(LoweringError::Unsupported(
                        "record read carrier type is absent",
                    ))?;
            }
            if expected_type.is_some_and(|expected| declaration.identity != expected) {
                return unsupported("record read carrier path changed its declared type");
            }
            let StructuralTypeShape::Record { fields } = &declaration.shape else {
                return unsupported("record read requires a record namespace");
            };
            let mut fields = fields.iter().filter(|field| field.identity == identity);
            let retained = fields.next().ok_or(LoweringError::Unsupported(
                "record read has no declared field identity",
            ))?;
            let scalar_type = terminal_scalar_type(node.primitive_type)?;
            let leaf_scalar_type = terminal_scalar_type(leaf_primitive)?;
            if fields.next().is_some()
                || retained.relevance.is_erased()
                || !matches!(
                    retained.field_type,
                    StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_)
                )
                || retained.field_type.scalar_type() != Some(leaf_scalar_type)
            {
                return unsupported("record read changed its scalar field carrier");
            }
            let bounded_leaf =
                matches!(retained.field_type, StructuralFieldType::BoundedInteger(_));
            if matches!(leaf_scalar_type, ScalarType::IeeeFloat(_)) {
                primitive_path.push(terminal_psi::StructuralPathSegment::Field(
                    retained.identity.clone(),
                ));
            }
            let byte_overlay = byte_overlay
                .map(|overlay| overlay.field(checked, &source.path, source.field, scalar_type))
                .transpose()?;
            let mut element_path = element_suffix;
            if runtime_index.is_some() {
                element_path.push(terminal_psi::StructuralPathSegment::Field(
                    retained.identity.clone(),
                ));
            }
            bindings.push(Binding {
                symbol: *field,
                subject: subject.clone(),
                loan_source,
                byte_overlay,
                carrier_path,
                path,
                runtime_index,
                selector_primitive,
                view_container,
                element,
                carrier,
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

/// The source-side walk for a read through a typed record view: the canonical
/// path segments from the source record to the leaf's declaring record, the
/// leaf's own field identity, and the leaf's declared carrier. Only a direct
/// scalar field of the view is admitted. The leaf keeps its source carrier;
/// the ordinary representation-view replay independently checks and emits
/// the carrier relation after the source observation has been lowered.
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
            let CheckedScalarComputationKind::StructuralField {
                source_expression, ..
            } = &plans.nodes.get(handle).kind
            else {
                continue;
            };
            let Some(source) = validation::local_scalar_record_field(
                &checked.typed,
                machine,
                root.state,
                root.statement_ordinal,
                *source_expression,
            ) else {
                continue;
            };
            let identity = checked
                .normalized_type_identity(source.type_reference)
                .into_string();
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
    // The ordinary shared-argument join already resolves an exact live local
    // without transferring it. Field replay admits only StructuralLocal sources;
    // use that same current storage mapping instead of a parallel read map. A
    // shared `&`-record loan local never enters that roster — it mints no place
    // — so the read resolves the captured parameter's place directly and lets
    // the emitted canonical path name the referee below it.
    let source = match binding.loan_source {
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
        None => {
            bindings
                .shared_structural_argument(subject, machine, parameters)?
                .place
        }
    };
    if binding.leaf_scalar_type != scalar_type
        && (binding.runtime_index.is_some() || binding.byte_overlay.is_some())
    {
        return unsupported("record view carrier change needs a static source leaf");
    }
    if let Some(selector) = binding.runtime_index.clone() {
        if let Some(overlay) = &binding.byte_overlay {
            if binding.view_container {
                return unsupported("record byte overlay requires fixed-array backing");
            }
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } =
                binding.subject.source
            else {
                return unsupported("record byte overlay lost its captured loan identity");
            };
            return byte_overlay_read(
                source,
                &binding.carrier_path,
                bindings.byte_record_coordinate(symbol)?,
                overlay.offset,
                overlay.stored_type,
                binding.scalar_type,
            );
        }
        // The canonical read path has no runtime segment: the loan's element
        // read takes the same runtime vocabulary a direct `cells[i].leaf`
        // observation spells, with the selector lowered through the shared
        // scalar rosters.
        let index = match selector {
            checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                bindings.expression(&checked_trees::CheckedScalarExpression::Parameter {
                    position: position as usize,
                    primitive_type: binding.selector_primitive.ok_or(
                        LoweringError::Unsupported(
                            "record loan's index selector lost its primitive",
                        ),
                    )?,
                })?
            }
            checked_trees::CheckedRuntimeIndex::Local { position } => {
                bindings.expression(&checked_trees::CheckedScalarExpression::Local {
                    position: position as usize,
                    primitive_type: binding.selector_primitive.ok_or(
                        LoweringError::Unsupported(
                            "record loan's index selector lost its primitive",
                        ),
                    )?,
                })?
            }
            checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                bindings.runtime_index_field(&field)?
            }
            checked_trees::CheckedRuntimeIndex::AssignmentIndex { .. } => {
                return unsupported("record loan's index is not an assignment selector");
            }
        };
        let index = crate::expression_preparation::prepare_expression::exact_u64_index(index)?;
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
        } else if binding.bounded_leaf {
            LoweredDirectExpression::IndexedElementFieldRead {
                source,
                path: binding.carrier_path.clone(),
                index: Box::new(index),
                element: binding.element.ok_or(LoweringError::Unsupported(
                    "record loan's runtime element lost its type",
                ))?,
                carrier: binding.carrier.clone(),
                field: binding.field,
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

#[cfg(test)]
mod tests {
    #[test]
    fn readonly_array_overlay_needs_no_field_read_or_store() {
        let checked = crate::front_end::checked_program(
            "data Matrix { cells: [[u16; 2]; 2]; }
             data Main { bytes: [u8; 8]; observed: u16; }
             machine Main::main(&mut self) {
                 let view: &Matrix = &self.bytes[0] as &Matrix;
                 transition view.cells[1][0] == 0 {
                     true -> good()
                     _ -> bad()
                 }
                 state good(&mut self) { self.observed = 1; }
                 state bad(&mut self) { self.observed = 2; }
             }",
        );
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("declared referee geometry supports a read-only array loan");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("array reads retain byte bounds and backing permission");
    }

    #[test]
    fn readonly_array_overlay_assignment_rejects_substituted_leaf() {
        let mut checked = crate::front_end::checked_program(
            "data Matrix { cells: [[u16; 2]; 2]; }
             data Main { bytes: [u8; 8]; observed: u16; }
             machine Main::main(&mut self) {
                 let view: &Matrix = &self.bytes[0] as &Matrix;
                 self.observed = view.cells[1][0];
             }",
        );
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("the same array observation composes as a store operand");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("independent read verification");
        let element_path = checked
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter_mut()
            .find_map(|plan| {
                if let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                    element_path,
                    ..
                } = &mut plan.expression
                {
                    Some(element_path)
                } else {
                    None
                }
            })
            .expect("retained indexed read");
        element_path[0] = checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(1);
        assert!(
            crate::lower_machine(
                &checked,
                crate::TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "another valid layout leaf cannot replace the authored observation"
        );
    }

    #[test]
    fn unused_record_loan_does_not_require_observation_support() {
        let checked = crate::front_end::checked_program(
            "data Cell { value: u16; }
             data Main { cells: [[Cell; 2]; 2]; }
             machine Main::main(&self, row: u64, column: u64)
                 requires row < 2 && column < 2
             {
                 let unused: &Cell = &self.cells[row][column];
             }",
        );
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("an unobserved loan does not demand runtime projection lowering");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("unused capture adds neither storage nor unchecked observations");
    }

    const NESTED_BYTE_STORE: &str = "
        data Cell { value: u16; }
        data Pair { first: Cell; second: Cell; }
        data Main { bytes: [u8; 8]; }
        machine Main::main(&mut self) {
            let view: &mut Pair = &mut self.bytes[2] as &mut Pair;
            view.second.value = 513;
        }
    ";

    #[test]
    fn nested_array_overlay_store_retains_backing_and_rejects_substitution() {
        let source = "data Matrix { cells: [[u16; 2]; 2]; }
             data Main { bytes: [u8; 8]; }
             machine Main::main(&mut self) {
                 let view: &mut Matrix = &mut self.bytes[0] as &mut Matrix;
                 view.cells[1][0] = 513;
             }";
        let checked = crate::front_end::checked_program(source);
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("an array endpoint writes the recast loan's original bytes");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("byte stores independently retain their bounds and permission");
        let paths = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::WriteOnlyPrimitiveStore { path, .. } => {
                    Some(path.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                vec![
                    terminal_psi::StructuralPathSegment::Field("bytes".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(4)
                ],
                vec![
                    terminal_psi::StructuralPathSegment::Field("bytes".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(5)
                ],
            ]
        );
        let mut changed = checked;
        let path = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|plan| &mut plan.operations)
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    path,
                    ..
                } => Some(path),
                _ => None,
            })
            .expect("one authored element store");
        path[1] = checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0);
        assert!(
            crate::lower_machine(
                &changed,
                crate::TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "a valid sibling element cannot replace the authored destination"
        );
        let shared = crate::front_end::checked_program(
            &source
                .replace("&mut Matrix", "&Matrix")
                .replace("&mut self.bytes", "&self.bytes"),
        );
        assert!(
            crate::lower_machine(&shared, crate::TerminalMachineSelection::Name("Main::main"))
                .is_err(),
            "projection geometry cannot promote a shared loan to write authority"
        );
    }

    #[test]
    fn nested_byte_overlay_store_without_read_keeps_backing_and_offset() {
        let checked = crate::front_end::checked_program(NESTED_BYTE_STORE);
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("a mutable loan stores into its captured bytes without a later read");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("the stores retain exclusive backing custody");
        let paths = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::WriteOnlyPrimitiveStore { path, .. } => {
                    Some(path.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                vec![
                    terminal_psi::StructuralPathSegment::Field("bytes".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(4)
                ],
                vec![
                    terminal_psi::StructuralPathSegment::Field("bytes".into()),
                    terminal_psi::StructuralPathSegment::FixedIndex(5)
                ],
            ]
        );
    }

    #[test]
    fn byte_overlay_store_rejects_substituted_checked_projection_and_value() {
        for replace_value in [false, true] {
            let mut checked = crate::front_end::checked_program(NESTED_BYTE_STORE);
            let store = checked
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .flat_map(|plan| &mut plan.operations)
                .find_map(|operation| match operation {
                    checked_trees::CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
                        store,
                    ) => Some(store),
                    _ => None,
                })
                .expect("the authored assignment has a checked store");
            if replace_value {
                let checked_trees::CheckedStoreValue::Pure(
                    checked_trees::CheckedScalarExpression::IntegerLiteral { literal },
                ) = &mut store.value
                else {
                    panic!("the store retains its authored scalar literal");
                };
                *literal = numerics::literals::IntegerLiteral::from_parts(
                    false,
                    numerics::literals::IntegerRadix::Decimal,
                    "514",
                )
                .unwrap();
            } else {
                store.carrier_path[0] =
                    checked_trees::CheckedUnitStructuralPathSegment::Field("first".into());
            }
            assert!(
                crate::lower_machine(
                    &checked,
                    crate::TerminalMachineSelection::Name("Main::main")
                )
                .is_err(),
                "a forged store must not substitute another occurrence or RHS value"
            );
        }
    }

    #[test]
    fn runtime_byte_overlay_store_uses_the_captured_coordinate() {
        let checked = crate::front_end::checked_program(
            "data Cell { value: u16; }
             data Pair { first: Cell; second: Cell; }
             data Main { bytes: [u8; 8]; }
             machine Main::main(&mut self) {
                 transition { _ -> write(4) }
                 state write(&mut self, offset: u32) requires offset <= 4 {
                     let view: &mut Pair = &mut self.bytes[offset] as &mut Pair;
                     view.second.value = 513;
                 }
             }",
        );
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("the captured coordinate and normalized field offset compose");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("every byte owns a checked bound at the end of the backing");
    }

    #[test]
    fn runtime_nested_record_overlay_store_retains_array_bounds_and_source() {
        let source = "data Pair { low: u16; high: u16; }
             data Packet { tag: u16; entries: [Pair; 2]; }
             data Main { bytes: [u8; 14]; i: u32 [0..=1]; }
             machine Main::main(main: &mut Main) {
                 main.i = 1;
                 let view: &mut Packet = &mut main.bytes[0] as &mut Packet;
                 view.tag = 513;
                 view.entries[main.i].high = 1541;
             }";
        let checked = crate::front_end::checked_program(source);
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("runtime record-array coordinates retain their declared bounds");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("dimension and byte bounds replay independently");
        let mut changed = checked;
        let path = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(
                    store,
                ) if store.carrier_path.iter().any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                    )
                }) =>
                {
                    Some(&mut store.carrier_path)
                }
                _ => None,
            })
            .expect("retained runtime array coordinate");
        let selector = path
            .iter_mut()
            .find(|segment| {
                matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                )
            })
            .unwrap();
        *selector = checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0);
        assert!(
            crate::lower_machine(
                &changed,
                crate::TerminalMachineSelection::Name("Main::main"),
            )
            .is_err(),
            "a valid sibling coordinate cannot replace the authored selector"
        );
    }

    #[test]
    fn byte_overlay_store_does_not_promote_a_shared_loan() {
        let source = NESTED_BYTE_STORE
            .replace("&mut Pair", "&Pair")
            .replace("&mut self.bytes", "&self.bytes");
        let checked = crate::front_end::checked_program(&source);
        assert!(
            crate::lower_machine(
                &checked,
                crate::TerminalMachineSelection::Name("Main::main")
            )
            .is_err(),
            "read geometry cannot grant write authority at Terminal production"
        );
    }

    #[test]
    fn nested_byte_overlay_keeps_repeated_field_occurrences_distinct() {
        let checked = crate::front_end::checked_program(
            "data Cell { value: u16; }
             data Pair { first: Cell; second: Cell; }
             data Main { bytes: [u8; 4]; first: u16; second: u16; }
             machine Main::main(&mut self) {
                 let view: &Pair = &self.bytes[0] as &Pair;
                 self.first = view.first.value;
                 self.second = view.second.value;
             }",
        );
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("nested byte-backed fields keep the selected occurrence's offset");
        let mut module = terminal_codec::decode_module(
            &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        )
        .unwrap();
        terminal_verifier::verify_module(
            &module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("all four byte observations replay independently");
        assert_eq!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::PrimitiveScalarRead { .. }
                ))
                .count(),
            4,
        );
        let captured_offset = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                terminal_psi::OperationKind::IntegerConstant { value }
                    if *value == semantic_vocabulary::IntegerValue::Unsigned(2) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .expect("the second occurrence has a two-byte offset");
        *captured_offset = semantic_vocabulary::IntegerValue::Unsigned(4);
        assert!(
            terminal_verifier::verify_module(
                &module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "substituting an out-of-bounds offset cannot reuse the byte proofs"
        );
    }

    const RECORD_BYTE_LOAN: &str = r#"
        data Desc { tag: u8; small: u16; count: u32; }
        data Main { buf: [u8; 24]; result: u32; offset: u32 [0..=16]; }
        machine Main::main(&mut self) {
            transition { _ -> read(16) }
            state read(&mut self, offset: u32) requires offset <= 16 {
                let viewed: &Desc = &self.buf[offset] as &Desc;
                transition viewed.tag == 7 && viewed.small == 65534 && viewed.count == 67305985 {
                    true -> good()
                    _ -> bad()
                }
            }
            state good(&mut self) { self.result = 70; }
            state bad(&mut self) { self.result = 71; }
        }
    "#;

    #[test]
    fn runtime_byte_overlay_fields_read_original_bytes_and_verify() {
        let checked = crate::front_end::checked_program(RECORD_BYTE_LOAN);
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("runtime record fields read their exact byte regions");
        let mut module = terminal_codec::decode_module(
            &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        )
        .unwrap();
        terminal_verifier::verify_module(
            &module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("each byte replays its own bound");
        let byte_type = super::terminal_scalar_type(super::PrimitiveType::U8).unwrap();
        let mut reads = 0;
        for operation in module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
        {
            if let terminal_psi::OperationKind::PrimitiveScalarRead { path, .. } = &operation.kind {
                reads += 1;
                assert_eq!(
                    operation.result.scalar_ref().unwrap().scalar_type,
                    byte_type
                );
                assert!(matches!(
                    path.as_slice(),
                    [
                        terminal_psi::StructuralPathSegment::Field(_),
                        terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                    ]
                ));
            }
            assert!(
                !matches!(
                    operation.kind,
                    terminal_psi::OperationKind::StructuralLeafCopy { .. }
                ),
                "a copied record must not replace this borrowed byte storage"
            );
        }
        assert_eq!(reads, 7, "one byte, one halfword, and one word");

        let path = module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                terminal_psi::OperationKind::PrimitiveScalarRead { path, .. } => Some(path),
                _ => None,
            })
            .unwrap();
        path.push(terminal_psi::StructuralPathSegment::Field("count".into()));
        assert!(
            terminal_verifier::verify_module(
                &module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "a byte element cannot acquire the record field projection"
        );
    }

    #[test]
    fn runtime_byte_overlay_signed_fields_replay_the_modular_image() {
        let source = RECORD_BYTE_LOAN
            .replace("small: u16", "small: i16")
            .replace("viewed.small == 65534", "viewed.small == -2");
        let checked = crate::front_end::checked_program(&source);
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("a signed field observes the same byte region");
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("the region's modular image retains each byte's bound");
    }

    #[test]
    fn runtime_byte_overlay_rejects_insufficient_footprint() {
        let source = RECORD_BYTE_LOAN
            .replace("offset <= 16", "offset <= 17")
            .replace("read(16)", "read(17)");
        let Ok(checked) = crate::front_end::checked_program_result(&source) else {
            return;
        };
        let Ok(lowered) = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        ) else {
            return;
        };
        assert!(
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "the final field byte must stay within its original array"
        );
    }

    #[test]
    fn runtime_byte_overlay_never_reloads_a_changed_selector() {
        let source = r#"
            data Desc { tag: u8; small: u16; count: u32; }
            data Main { buf: [u8; 24]; result: u32; offset: u32 [0..=16]; }
            machine Main::main(main: &mut Main) {
                let viewed: &Desc = &main.buf[main.offset] as &Desc;
                main.offset = 16;
                transition viewed.tag == 7 && viewed.small == 65534
                    && viewed.count == 67305985 {
                    true -> good(main)
                    _ -> bad(main)
                }
                state good(main: &mut Main) { main.result = 70; }
                state bad(main: &mut Main) { main.result = 71; }
            }
        "#;
        let checked = crate::front_end::checked_program(source);
        let lowered = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect("changing the selector field preserves the captured byte coordinate");
        let mut module = terminal_codec::decode_module(
            &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        )
        .unwrap();
        let proofs = terminal_codec::decode_proof_bundle(
            &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
        )
        .unwrap();
        terminal_verifier::verify_module(
            &module,
            &proofs,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("decoded observations replay their declaration-time coordinate");

        let captures = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishPrimitiveLocal { .. }
                )
                .then(|| (operation.id, operation.result.structural().unwrap().place))
            })
            .collect::<Vec<_>>();
        let [(producer, captured)] = captures.as_slice() else {
            panic!("one private scalar home freezes this loan's byte coordinate")
        };
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    &operation.kind,
                    terminal_psi::OperationKind::PrimitiveScalarRead { source, path, .. }
                        if source == captured && path.is_empty()
                )),
            "record observations use the snapshot home"
        );
        for block in module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
        {
            block
                .operations
                .retain(|operation| operation.id != *producer);
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &proofs,
                &proof_admission::AdmissionProfile::default(),
            )
            .is_err(),
            "removing the original coordinate capture cannot fall back to a fresh field read"
        );
    }
}
