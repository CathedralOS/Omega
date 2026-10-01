//! Record reads bind a declared field once, then resolve the current local place
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
}

#[derive(Clone)]
struct ByteOverlayField {
    offset: u64,
    stored_type: ScalarType,
}

struct ByteOverlaySource {
    referee: typed_trees::types::TypeReferenceHandle,
    remaining_bytes: Option<u64>,
}

/// One authored segment of a shared loan's captured place: a declared record
/// member projection, a literal index into a fixed array, or a runtime
/// selector the loan's own obligations bound.
enum LoanedPathSegment {
    Field(String),
    FixedIndex(u64),
    RuntimeIndex(ExpressionHandle),
}

/// The established place a shared record loan resolves to at observation: a
/// structural parameter (by dense roster index) or an owned structural local
/// (by symbol, resolved through the ordinary argument join).
#[derive(Clone, Copy)]
enum LoanedSource {
    Parameter(u32),
    Local(symbols::SymbolHandle),
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
    source: LoanedSource,
    path: Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    terminal: StructuralTypeId,
    byte_overlay: Option<ByteOverlaySource>,
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
    let target = loop {
        match checked.expression_table.expression(initializer) {
            ExpressionNode::Cast(cast) if cast.form.is_recast() => initializer = cast.value,
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
    // Segments below the runtime index walk inside the selected element;
    // they leave both path vocabularies so the emitted read can splice the
    // selector in where the canonical spelling has no runtime segment.
    let (carrier, element_suffix) = match runtime_split {
        Some(split) => (path.split_off(split), carrier_path.split_off(split)),
        None => (Vec::new(), Vec::new()),
    };
    Ok(Some(RecordLoanSubject {
        source,
        path,
        carrier_path,
        terminal,
        byte_overlay,
        runtime_index,
        selector_primitive,
        view_container,
        element: selected_element,
        carrier,
        element_suffix,
    }))
}

pub(crate) fn prepare(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<Binding>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut bindings: Vec<Binding> = Vec::new();
    for (_, root) in plans
        .roots
        .iter()
        .filter(|(_, root)| root.machine == machine)
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
                machine,
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
                } => record_loan_subject(checked, machine, root.state, symbol, types)?,
                _ => None,
            };
            let (
                loan_source,
                byte_overlay,
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
            let mut primitive_path =
                if matches!(node.primitive_type, PrimitiveType::F32 | PrimitiveType::F64) {
                    carrier_path.clone()
                } else {
                    Vec::new()
                };
            // Terminal's scalar field read names record fields followed by at
            // most one literal element selection; a deeper element path
            // (`rows[1].cells[2].value`) has no read form yet.
            if subject.path.iter().enumerate().any(|(position, segment)| {
                matches!(
                    segment,
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                ) && position + 1 != subject.path.len()
            }) {
                return unsupported("record read selects an element below a trailing element");
            }
            for segment in &subject.path {
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
                        if matches!(node.primitive_type, PrimitiveType::F32 | PrimitiveType::F64) {
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
                if matches!(node.primitive_type, PrimitiveType::F32 | PrimitiveType::F64) {
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
            if declaration.identity != subject.type_identity {
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
            if fields.next().is_some()
                || retained.relevance.is_erased()
                || !matches!(
                    retained.field_type,
                    StructuralFieldType::Scalar(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::IeeeFloat(_)
                )
                || retained.field_type.scalar_type() != Some(scalar_type)
            {
                return unsupported("record read changed its scalar field carrier");
            }
            let bounded_leaf =
                matches!(retained.field_type, StructuralFieldType::BoundedInteger(_));
            if matches!(scalar_type, ScalarType::IeeeFloat(_)) {
                primitive_path.push(terminal_psi::StructuralPathSegment::Field(
                    retained.identity.clone(),
                ));
            }
            let byte_overlay = byte_overlay
                .map(|overlay| {
                    let projection = validation::recast_scalar_projection(
                        &checked.typed,
                        overlay.referee,
                        &source.path,
                        source.field,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "record byte overlay lost its checked projection geometry",
                    ))?;
                    if terminal_scalar_type(projection.primitive)? != scalar_type
                        || overlay.remaining_bytes.is_some_and(|remaining| {
                            u64::try_from(projection.footprint)
                                .map_or(true, |footprint| footprint > remaining)
                        })
                    {
                        return unsupported("record byte overlay changed its carrier or footprint");
                    }
                    Ok::<_, LoweringError>(ByteOverlayField {
                        offset: u64::try_from(projection.offset).map_err(|_| {
                            LoweringError::Unsupported("record byte overlay offset exceeds u64")
                        })?,
                        stored_type: projection
                            .stored_integer
                            .map(ScalarType::Integer)
                            .unwrap_or(scalar_type),
                    })
                })
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
            });
        }
    }
    Ok(bindings)
}

/// A member read's referee record can be owned by no signature, parameter,
/// result, or operation: a shared-loan local peels to its captured record
/// before the member projection, so the referee reaches the catalog only
/// through the read itself.
pub(crate) fn type_roots(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<String>, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    let mut types = Vec::new();
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
    if let Some(selector) = binding.runtime_index.clone() {
        // A loan captures its selector at formation. Immutable scalar
        // coordinates retain that value, while a field coordinate here would
        // reload possibly changed storage at the later member observation.
        // Supporting that source needs an explicit captured scalar binding.
        if binding.byte_overlay.is_some()
            && !matches!(
                selector,
                checked_trees::CheckedRuntimeIndex::Parameter { .. }
                    | checked_trees::CheckedRuntimeIndex::Local { .. }
            )
        {
            return unsupported(
                "runtime byte overlay loan requires an immutable captured selector",
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
                scalar_type,
            );
        }
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
    Ok(match scalar_type {
        ScalarType::Boolean => LoweredDirectExpression::Boolean {
            expression: Box::new(LoweredBooleanReturnExpression::StructuralField {
                source,
                path: binding.path.clone(),
                field: binding.field,
            }),
        },
        ScalarType::Integer(_) => LoweredDirectExpression::StructuralField {
            source,
            path: binding.path.clone(),
            field: binding.field,
            scalar_type,
        },
        ScalarType::IeeeFloat(_) => LoweredDirectExpression::PrimitiveRead {
            source: PrimitiveReadSource::Place(source),
            path: binding.primitive_path.clone(),
            scalar_type,
        },
    })
}

/// Observe just the selected field's bytes at its exact layout offset. The
/// ordinary region re-view owns the byte assembly and each runtime-index proof;
/// the source recast's footprint judgment cannot discharge those obligations.
fn byte_overlay_read(
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
        }
    };
    let byte_type = terminal_scalar_type(PrimitiveType::U8)?;
    let first_byte = LoweredDirectExpression::IndexedPrimitiveRead {
        source,
        path: path.to_vec(),
        index: Box::new(index),
        inner_indexes: Vec::new(),
        element_path: Vec::new(),
        scalar_type: byte_type,
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

#[cfg(test)]
mod tests {
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
        let source = RECORD_BYTE_LOAN.replace(
            "let viewed: &Desc = &self.buf[offset] as &Desc;",
            "let viewed: &Desc = &self.buf[self.offset] as &Desc; self.offset = 16;",
        );
        let checked = crate::front_end::checked_program(&source);
        let error = crate::lower_machine(
            &checked,
            crate::TerminalMachineSelection::Name("Main::main"),
        )
        .expect_err("a field selector needs an explicit formation-time capture");
        assert!(
            format!("{error:?}").contains("immutable captured selector"),
            "{error:?}"
        );
    }
}
