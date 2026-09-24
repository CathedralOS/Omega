//! Fresh structural establishment shares the scalar operand evaluation owner.
//! Reference leaves are classified before referent-oriented normalization:
//! their value is borrowed-storage custody, never a scalar pointer snapshot.
use super::{
    Builder, CheckedScalarComputationRoot, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, PrimitiveType, StatementNode,
    SymbolHandle, TypeReferenceNode, TypedTrees,
};
use checked_trees::{
    CheckedStructuralDispatchArm, CheckedStructuralValue, CheckedStructuralValueHandle,
    CheckedStructuralValueKind, CheckedStructuralValuePlans,
};
use typed_trees::expression::MatchPattern;
use typed_trees::types::TypeReferenceHandle;

// Classify the destination before inserting scalar operand roots. A scalar or
// array Match must not leave partial structural plans when a later arm fails.
// A match arm may also select an owned child projected by exact field or
// fixed-index path; its structural node is a Projection over the root place,
// which the owned-selection transfer evidence then checks path-for-path.
/// A case literal carrying non-scalar payload registers its construction root
/// the same way a record literal does: each authored field keeps its own
/// scalar or nested structural value under the selected case.
pub(super) fn is_structural_case_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_)
    ) {
        return false;
    }
    let Some(constructor) = validation::structural_case_constructor(program, expression) else {
        return false;
    };
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    program.normalized_type_identity(constructor.type_reference)
        == program.normalized_type_identity(reference)
}

pub(super) fn is_record_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_) | ExpressionNode::Match(_)
    ) {
        return false;
    }
    let shared_referent = shared_record_reference(program, expected);
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    // A `&T` selection result plans its borrowed arms against the referent
    // record; owned results keep their own carrier.
    let reference = shared_referent.unwrap_or(reference);
    // A primitive referent declares no record to resolve, so there is no field
    // roster to check and no constructor arm it could ever admit: its only
    // admitted arm is the shared borrow of an exact place checked below. The
    // invalid record symbol makes that explicit rather than implied -- no
    // authored `StructLiteral` can carry it.
    let primitive_referent =
        shared_referent.is_some() && program.primitive_type_reference(reference).is_some();
    let mut record_symbol = SymbolHandle::invalid();
    if !primitive_referent {
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => symbol,
            // A lifetime-parameterized record is a `Generic` node whose type
            // arguments are empty — its fields bind lifetimes, not types.
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if program
                .type_reference_table
                .type_reference_handles(*arguments)
                .is_empty() =>
            {
                base_symbol
            }
            _ => return false,
        };
        let Some(record) = program
            .data_definitions()
            .iter()
            .find(|record| record.symbol == *symbol)
        else {
            return false;
        };
        if program
            .data_members(record)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return false;
        }
        record_symbol = *symbol;
    }
    let symbol = &record_symbol;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal)
                if literal.case_symbol.is_none() && literal.type_symbol == *symbol => {}
            ExpressionNode::Name(_)
                if validation::affine_owned_value_source(program, expression, expected)
                    .is_some() => {}
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                if shared_referent.is_none()
                    && projected_leaf_type(program, expression).is_some_and(|reference| {
                        program.normalized_type_identity(reference)
                            == program.normalized_type_identity(expected)
                            && program.type_multiplicity(reference)
                                == language_semantics::Multiplicity::Affine
                    }) => {}
            // A shared-borrow arm does not move its referent's owner: the
            // canonical place is rebuilt with exact state scope during value
            // planning and replayed again at lowering.
            ExpressionNode::Borrow(borrow)
                if shared_referent.is_some_and(|referent| {
                    borrow.access == language_semantics::ReferenceAccess::Shared
                        && crate::flow::canonical_place_from_expression(program, borrow.target)
                            .is_some_and(|place| canonical_place_is_borrowable(&place))
                        && borrowed_place_leaf_type(program, borrow.target).is_some_and(|leaf| {
                            program.normalized_type_identity(leaf)
                                == program.normalized_type_identity(referent)
                        })
                }) => {}
            ExpressionNode::Call(call) => {
                // A call's structural product is a fresh owned arm value: the
                // builder admits it under the same exact return-type and
                // plain-custody rule it checks when constructing the node.
                let Some(returned) =
                    crate::flow::call_target_return_type(program, call.target_symbol)
                else {
                    return false;
                };
                if program.normalized_type_identity(returned)
                    != program.normalized_type_identity(expected)
                    || !(validation::has_linear_owned_contents(program, returned)
                        || validation::reference_result_custody::is_reference_record(
                            program, returned,
                        ))
                {
                    return false;
                }
            }
            ExpressionNode::Match(dispatch) => {
                let arms = program.expression_table.match_arms(dispatch.arms);
                if arms.is_empty() {
                    return false;
                }
                pending.extend(arms.iter().map(|arm| arm.value));
            }
            _ => return false,
        }
    }
    true
}

/// A `let view: &T = &place` establishment admits the same shared-borrow
/// carrier a selection arm does, at statement scope: the referent's owner is
/// never moved, and `borrowed_place` rebuilds the authored target's canonical
/// root and path into a `SharedBorrow` source. This is the driver's
/// pre-classification only -- `borrowed_place` re-checks every condition when
/// constructing the node, and it does no plan mutation before that admission,
/// so a rejected borrow still leaves no partial structural plan.
pub(super) fn is_shared_borrow_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return false;
    };
    borrow.access == language_semantics::ReferenceAccess::Shared
        && shared_record_reference(program, expected).is_some_and(|referent| {
            crate::flow::canonical_place_from_expression(program, borrow.target)
                .is_some_and(|place| canonical_place_is_borrowable(&place))
                && borrowed_place_leaf_type(program, borrow.target).is_some_and(|leaf| {
                    program.normalized_type_identity(leaf)
                        == program.normalized_type_identity(referent)
                })
        })
}

/// A LocalData initializer whose declared type is a borrowed `&[T]` view. The
/// value itself is adjudicated by `borrowed_slice_view_place`, which rejoins
/// the checked loan the view holds; this only decides that the destination is
/// a view at all, so the ordinary owned classifications keep every other local.
pub(super) fn is_borrowed_slice_view_value(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
) -> bool {
    crate::execution::terminal_unit::types::borrowed_slice_view_element(program, expected, &[])
        .is_some()
}

/// A LocalData initializer whose declared type is a fixed array of structural
/// elements spelled as a literal. Primitive arrays keep their scalar-element
/// route; structural elements each register their own value node, the same
/// way record fields do inside a constructor.
pub(super) fn is_fixed_array_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::ArrayLiteral(_)
    ) && validation::unwrapped_type_reference(program, expected).is_some_and(|reference| {
        matches!(
            program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
        )
    }) && !validation::is_closed_primitive_array_type(program, expected)
}

/// The element type of an owned or borrowed contiguous collection place.
/// A fixed array declares its extent and a slice carries a stored one; both
/// lend the same elements, so both can back a view.
fn lent_collection_element(
    program: &TypedTrees,
    mut type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => return Some(*element_type),
            _ => return None,
        }
    }
}

/// The referent of a shared-borrow result type (`&T` where `T` is a named
/// record, or a primitive, that the structural pipeline can carry). `None` for
/// owned results and for borrows the structural pipeline cannot carry. The
/// carrier rule is linear-tolerant: a shared borrow observes the referent
/// without moving it, so a linear declaration's claim never reaches this join
/// and the referent's own owner keeps the whole discharge obligation.
///
/// A primitive referent resolves to no data declaration, so the record rule
/// cannot name its shape. `add_type` registers it as a `PrimitiveScalar`
/// structural carrier exactly as it does for a borrowed primitive elsewhere,
/// and the borrow still denotes the original storage rather than a snapshot.
fn shared_record_reference(
    program: &TypedTrees,
    expected: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(expected)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Shared
        || !validation::has_linear_owned_contents(program, *referee)
    {
        return None;
    }
    if program.primitive_type_reference(*referee).is_some() {
        return Some(*referee);
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    let record = program
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)?;
    (!program
        .data_members(record)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_))))
    .then_some(*referee)
}

/// The declared type of a shared borrow's exact target without state scope: a
/// whole local's declared type, or a projected leaf resolved through its
/// root's declaration. `canonical_place_from_expression` keeps the root and
/// every segment exact.
fn borrowed_place_leaf_type(
    program: &TypedTrees,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(target) {
        ExpressionNode::Name(path)
            if path.symbol.is_valid()
                && path.head_symbol == path.symbol
                && path.members.count() == 1 =>
        {
            symbol_declared_type(program, path.symbol)
        }
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            projected_leaf_type(program, target)
        }
        _ => None,
    }
}

/// The declared type of a field/fixed-index projection leaf, resolved through
/// the authored root's own declaration: a whole local's declared type, or a
/// producing call's declared return type. Only exact field and literal-index
/// segments keep a statically checkable identity.
fn projected_leaf_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    if place.segments.is_empty()
        || !place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        return None;
    }
    let root = match place.root {
        facts::PlaceRoot::Symbol(symbol) => symbol_declared_type(program, symbol)?,
        facts::PlaceRoot::Expression(expression) => {
            match program.expression_table.expression(expression) {
                ExpressionNode::Call(call) => {
                    crate::flow::call_target_return_type(program, call.target_symbol)?
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    crate::flow::project_type_reference_from_segments(program, root, &place.segments)
}

/// The place shape a shared-borrow arm target must canonicalize to: a named
/// root reached through record fields and literal fixed indexes only. That is
/// exactly the carrier `borrowed_place` can rebuild into a `SharedBorrow`
/// source -- a computed root cannot be re-derived at replay, and a dynamic
/// index or range segment has no statically checkable ordinal.
fn canonical_place_is_borrowable(place: &crate::flow::CanonicalPlace) -> bool {
    matches!(place.root, facts::PlaceRoot::Symbol(_))
        && place.segments.iter().all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
}

fn symbol_declared_type(program: &TypedTrees, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            if let Some(parameter) = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
            {
                return Some(parameter.type_reference);
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::LocalData(local) = statement
                    && local.symbol == symbol
                {
                    return Some(local.type_reference);
                }
            }
        }
    }
    None
}

/// A copyable payloadless case-sum leaf: a named `data` whose members are all
/// variants carrying no payload fields, under `Unrestricted` multiplicity.
/// Membership observation can reconstruct such a leaf without reading any
/// storage contents, which is exactly what a `self.bitness`-shape read needs;
/// anything with payload storage or affine custody needs move semantics.
fn scalar_case_place_type(program: &TypedTrees, expected: TypeReferenceHandle) -> bool {
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    if program.type_multiplicity(reference) != language_semantics::Multiplicity::Unrestricted {
        return false;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
    else {
        return false;
    };
    let members = program.data_members(data);
    !members.is_empty()
        && members.iter().all(|member| match member {
            typed_trees::data::DataMember::Variant(variant) => {
                program.data_payload_fields(variant).is_empty()
            }
            _ => false,
        })
}

/// `self.bitness`-shape admission: a member/fixed-index projection rooted at
/// shared-borrowed storage whose leaf is a copyable payloadless case sum. The
/// value keeps the exact canonical path so lowering can observe the held case
/// and re-establish it -- never moving borrowed storage.
pub(super) fn is_scalar_case_place_value(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !scalar_case_place_type(program, expected) {
        return false;
    }
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        expression,
    ) else {
        return false;
    };
    if place.segments.is_empty() || !canonical_place_is_borrowable(&place) {
        return false;
    }
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return false;
    };
    let Some(root) = symbol_declared_type(program, symbol) else {
        return false;
    };
    if !matches!(
        program.type_reference_table.type_reference(root),
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            ..
        }
    ) {
        return false;
    }
    crate::flow::canonical_place_type_reference(program, state, statement_index, &place)
        .is_some_and(|leaf| {
            program.normalized_type_identity(leaf) == program.normalized_type_identity(expected)
        })
}

/// A copyable `Unrestricted` leaf of any structural shape: a named `data`
/// under `Unrestricted` multiplicity. Copying its contents into a fresh
/// owned place reproduces the read a `self.scan_compare_type`-shape member
/// access needs where moving or case-fan-out cannot apply; anything affine
/// still needs move semantics.
fn copied_place_type(program: &TypedTrees, expected: TypeReferenceHandle) -> bool {
    // A stored `&'a [T]` view leaf copies the view value out of the carrier
    // the same way an `Unrestricted` named leaf does — the referent's loan
    // stays exactly where the field's `&` put it. Its bare `Slice` rung
    // unwraps to `Affine`, so the shared-borrow view's own multiplicity is
    // what decides admission.
    if crate::execution::terminal_unit::types::borrowed_slice_view(program, expected)
        && program.type_multiplicity(expected) == language_semantics::Multiplicity::Unrestricted
    {
        return true;
    }
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    if program.type_multiplicity(reference) != language_semantics::Multiplicity::Unrestricted {
        return false;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    program
        .data_definitions()
        .iter()
        .any(|data| data.symbol == *symbol)
}

/// `self.scan_compare_type`-shape admission: a member/fixed-index projection
/// rooted at shared-borrowed storage whose leaf is a copyable `Unrestricted`
/// named type. Same root/path contract as `is_scalar_case_place_value`, but
/// for leaves whose contents cannot be reconstructed by case fan-out. A bare
/// whole-root name is the degenerate path: an owned root's `Unrestricted`
/// contents copy out whole, no loan involved.
pub(super) fn is_copied_place_value(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !copied_place_type(program, expected)
        || is_scalar_case_place_value(program, state, statement_index, expression, expected)
    {
        return false;
    }
    let Some(place) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        expression,
    ) else {
        return false;
    };
    if !canonical_place_is_borrowable(&place) {
        return false;
    }
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return false;
    };
    let Some(root) = symbol_declared_type(program, symbol) else {
        return false;
    };
    if place.segments.is_empty() {
        // A whole owned root reads its declared contents directly: any
        // non-view reference carrier stays on the borrowed-leaf lane below,
        // while an owned `[copy]` root copies into a fresh result with an
        // empty path. A bare `&`-view name is the degenerate copied leaf —
        // the view itself is the `Unrestricted` leaf the copy transfers.
        let root_is_view =
            matches!(
                program.type_reference_table.type_reference(root),
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    ..
                }
            ) && (crate::execution::terminal_unit::types::borrowed_slice_view(program, root)
                || crate::execution::terminal_unit::types::borrowed_named_view(program, root));
        if matches!(
            program.type_reference_table.type_reference(root),
            TypeReferenceNode::Reference { .. }
        ) && !root_is_view
        {
            return false;
        }
    } else if !matches!(
        program.type_reference_table.type_reference(root),
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            ..
        }
    ) {
        return false;
    }
    crate::flow::canonical_place_type_reference(program, state, statement_index, &place)
        .is_some_and(|leaf| {
            program.normalized_type_identity(leaf) == program.normalized_type_identity(expected)
        })
}

/// An omitted runtime field's zero value is admitted only for a
/// `[scalar; N]` leaf: a literal-length fixed array whose element resolves
/// to one plain integer primitive. Anything else — borrowed carriers,
/// bounded integers, nested composites — has no authored zero spelling to
/// synthesize, so the field must stay authored.
fn zeroed_scalar_array_parts(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<(PrimitiveType, u64)> {
    let reference = validation::unwrapped_type_reference(program, type_reference)?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(*element_type)?;
    if !matches!(
        primitive_type,
        PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    Some((primitive_type, u64::try_from(*length).ok()?))
}

/// The checked literal-zero computation one synthesized zeroed field
/// element contributes: a landed `0` in the declared element carrier.
fn zeroed_integer_literal(
    primitive_type: PrimitiveType,
) -> Option<checked_trees::CheckedScalarExpression> {
    let landed_type = match primitive_type {
        PrimitiveType::I8 => numerics::literals::LandedIntegerType::I8,
        PrimitiveType::I16 => numerics::literals::LandedIntegerType::I16,
        PrimitiveType::I32 => numerics::literals::LandedIntegerType::I32,
        PrimitiveType::I64 => numerics::literals::LandedIntegerType::I64,
        PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        PrimitiveType::U16 => numerics::literals::LandedIntegerType::U16,
        PrimitiveType::U32 => numerics::literals::LandedIntegerType::U32,
        PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    Some(checked_trees::CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            "0",
        )
        .ok()?
        .with_landing(numerics::literals::IntegerLanding {
            landed_type,
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
        }),
    })
}

/// The terminal expression carrying a projection's storage: the whole local
/// name for an owned place source, or the producing call for a structural
/// product source.
fn projection_root_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let mut cursor = expression;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            ExpressionNode::Name(_) | ExpressionNode::Call(_) => return Some(cursor),
            _ => return None,
        }
    }
}

impl Builder<'_, '_> {
    pub(super) fn structural_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueHandle> {
        let kind = if validation::reference_result_custody::parts(self.program, expected).is_some()
        {
            let state = crate::lookup::machine_by_symbol(self.program, self.machine).and_then(
                |machine| {
                    self.program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == self.state)
                },
            )?;
            CheckedStructuralValueKind::Reference {
                source: validation::reference_result_custody::initializer_source(
                    self.program,
                    state,
                    expression,
                    expected,
                )?,
            }
        } else if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) && validation::scalar_case_constructor(self.program, expression).is_none()
            && let Some(argument) = self.owned_record_place(expression, expected)
        {
            CheckedStructuralValueKind::Place(argument)
        } else if let Some(view) = self.borrowed_slice_view_place(expression, expected) {
            // A view's authored producer is spelled as a call but establishes
            // no call result: it lends storage that already exists. Classify
            // it before the ordinary call-result branch, which would otherwise
            // demand a returning target for it.
            view
        } else if let ExpressionNode::Call(call) =
            self.program.expression_table.expression(expression)
        {
            let returned = crate::flow::call_target_return_type(self.program, call.target_symbol)?;
            if self.program.normalized_type_identity(returned)
                != self.program.normalized_type_identity(expected)
                || !(validation::has_linear_owned_contents(self.program, returned)
                    || validation::reference_result_custody::is_reference_record(
                        self.program,
                        returned,
                    ))
            {
                return None;
            }
            let (source_call, call_ordinal) = self.call_ordinal(expression, call.target_symbol)?;
            self.record_call_arguments(
                pure,
                values,
                u32::try_from(self.statement_index).ok()?,
                call_ordinal,
                call.target_symbol,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
            );
            CheckedStructuralValueKind::Call { source_call }
        } else if let Some(copy) = self.scalar_case_place(expression, expected) {
            copy
        } else if let Some(copied) = self.copied_place(expression, expected) {
            copied
        } else if let Some(projection) =
            self.projected_selection_place(expression, expected, values, pure)
        {
            projection
        } else if let Some(constructor) = self.case_construction(expression) {
            if self
                .program
                .normalized_type_identity(constructor.type_reference)
                != self.program.normalized_type_identity(expected)
            {
                return None;
            }
            for (field_ordinal, field) in self
                .plans
                .case_fields
                .span(constructor.fields)?
                .iter()
                .enumerate()
            {
                self.plans.roots.append(CheckedScalarComputationRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: u32::try_from(self.statement_index).ok()?,
                    role: CheckedScalarExpressionRole::StructuralValueField {
                        expression,
                        field_ordinal: u32::try_from(field_ordinal).ok()?,
                    },
                    root: field.value,
                });
            }
            CheckedStructuralValueKind::Case(constructor)
        } else if let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
            && literal.case_symbol.is_some()
        {
            self.structural_case_value(expression, expected, values, pure)?
        } else if let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
            && literal.case_symbol.is_none()
        {
            self.record_value(expression, expected, values, pure)?
        } else if let ExpressionNode::ArrayLiteral(literal) =
            self.program.expression_table.expression(expression)
        {
            self.fixed_array_value(expected, *literal, values, pure)?
        } else if let Some(symbol) =
            validation::scalar_case_value_source(self.program, expression, expected)
        {
            CheckedStructuralValueKind::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                },
                path: Vec::new(),
                type_identity: self
                    .program
                    .normalized_type_identity(expected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::Owned,
            })
        } else if let Some(borrowed) = self.borrowed_place(expression, expected) {
            borrowed
        } else {
            let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
            else {
                return None;
            };
            let machine = crate::lookup::machine_by_symbol(self.program, self.machine)?;
            let state = self
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == self.state)?;
            let subject_type = validation::expression_result_type_reference(
                self.program,
                machine,
                state,
                dispatch.subject,
            )
            .and_then(|reference| validation::unwrapped_type_reference(self.program, reference))
            .and_then(|reference| self.program.primitive_type_reference(reference))
            .or_else(|| validation::match_subject_primitive_type(self.program, &dispatch))?;
            let subject = self.expression(dispatch.subject, subject_type)?;
            self.plans.nodes.get_mut(subject).authored_root = dispatch.subject;
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: u32::try_from(self.statement_index).ok()?,
                role: CheckedScalarExpressionRole::StructuralValueSubject { expression },
                root: subject,
            });
            let authored = self
                .program
                .expression_table
                .match_arms(dispatch.arms)
                .to_vec();
            let mut arms = Vec::new();
            let mut covered = false;
            let mut boolean_coverage = [false; 2];
            for (ordinal, arm) in authored.iter().enumerate() {
                // First-match semantics make a repeated literal Boolean arm
                // unreachable. It cannot transfer an owner or evaluate fields.
                if subject_type == PrimitiveType::Bool
                    && let MatchPattern::Value(pattern) = arm.pattern
                    && let ExpressionNode::Boolean(value) =
                        self.program.expression_table.expression(pattern)
                    && boolean_coverage[usize::from(*value)]
                {
                    continue;
                }
                let source_arm = arena::Handle::from_parts(
                    dispatch
                        .arms
                        .start()
                        .arena_index()
                        .checked_add(u32::try_from(ordinal).ok()?)?,
                    dispatch.arms.start().generation(),
                );
                let equality_use = if matches!(arm.pattern, MatchPattern::Value(_))
                    && matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
                {
                    self.comparison_use(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm },
                    )?
                } else {
                    arena::Handle::invalid()
                };
                let pattern = match arm.pattern {
                    MatchPattern::Wildcard => {
                        covered = true;
                        checked_trees::CheckedScalarDispatchPattern::Wildcard
                    }
                    MatchPattern::Value(pattern) => {
                        if subject_type == PrimitiveType::Bool
                            && let ExpressionNode::Boolean(value) =
                                self.program.expression_table.expression(pattern)
                        {
                            boolean_coverage[usize::from(*value)] = true;
                            covered = boolean_coverage.iter().all(|value| *value);
                        }
                        let computation = self.expression(pattern, subject_type)?;
                        self.plans.nodes.get_mut(computation).authored_root = pattern;
                        self.plans.roots.append(CheckedScalarComputationRoot {
                            machine: self.machine,
                            state: self.state,
                            statement_ordinal: u32::try_from(self.statement_index).ok()?,
                            role: CheckedScalarExpressionRole::StructuralValuePattern {
                                source_arm,
                            },
                            root: computation,
                        });
                        checked_trees::CheckedScalarDispatchPattern::Value(computation)
                    }
                };
                let value = self.structural_value(arm.value, expected, values, pure)?;
                arms.push(CheckedStructuralDispatchArm {
                    source_arm,
                    equality_use,
                    pattern,
                    value,
                });
                if covered {
                    break;
                }
            }
            if !covered {
                return None;
            }
            CheckedStructuralValueKind::Dispatch {
                subject,
                arms: values.dispatch_arms.insert_many(arms),
            }
        };
        Some(
            values
                .nodes
                .append(CheckedStructuralValue { expression, kind }),
        )
    }
    /// One copyable payloadless case leaf projected out of shared-borrowed
    /// storage: `self.bitness` reads which case the borrowed record currently
    /// holds. The root and path become a `SharedBorrow` `Place` source exactly
    /// as `borrowed_place` builds for `&place`, so lowering observes the held
    /// case and establishes the same constructor fresh rather than moving a
    /// child out of the loan. Owned roots still move their whole leaf through
    /// the owned arms.
    fn scalar_case_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !is_scalar_case_place_value(
            self.program,
            self.state,
            self.statement_index,
            expression,
            expected,
        ) {
            return None;
        }
        Some(CheckedStructuralValueKind::ScalarCasePlace {
            source: self.borrowed_leaf_argument(expression, expected)?,
        })
    }

    /// One `Unrestricted` leaf copied out of shared-borrowed storage:
    /// `self.scan_compare_type` reads the borrowed record's current contents
    /// and copies the leaf into a fresh owned place, never moving a child
    /// out of the loan.
    fn copied_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !is_copied_place_value(
            self.program,
            self.state,
            self.statement_index,
            expression,
            expected,
        ) {
            return None;
        }
        Some(CheckedStructuralValueKind::CopiedStructuralPlace {
            source: self.borrowed_leaf_argument(expression, expected)?,
        })
    }

    /// The `SharedBorrow` argument plan a borrowed leaf read produces: the
    /// named root as a parameter/local/`self` source plus the canonical
    /// member path, with `type_identity` naming the projected leaf.
    fn borrowed_leaf_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        let source = if let Some((index, parameter)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol)
        {
            if parameter.is_self {
                // The authored `self` name retains the durable machine symbol;
                // binding resolves that normalized root to the is_self slot.
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: self.machine,
                }
            } else {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(index).ok()?,
                }
            }
        } else {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        };
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        // The structural-type registry names a `&[T]` leaf by its borrowed
        // view carrier — the same identity `add_type` mints for the result —
        // so a view leaf records its unwrapped referee's identity; an owned
        // leaf keeps its own.
        let type_identity =
            if crate::execution::terminal_unit::types::borrowed_slice_view(self.program, projected)
            {
                validation::unwrapped_type_reference(self.program, projected).map(|unwrapped| {
                    self.program
                        .normalized_type_identity(unwrapped)
                        .into_string()
                })
            } else {
                None
            }
            .unwrap_or_else(|| {
                self.program
                    .normalized_type_identity(projected)
                    .into_string()
            });
        Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path,
            type_identity,
            access: checked_trees::CheckedStructuralAccess::SharedBorrow,
        })
    }

    /// One selected projected child of an existing owner. The owned-selection
    /// transfer is the authoritative admission: its recorded canonical path
    /// must equal the authored place, and the projected type must be the
    /// enclosing result type. The root source is built as a whole `Place` (or
    /// `Call` product) node carrying the exact root identity.
    fn projected_selection_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        if !matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        // Owned-selection receipts are recorded only after value plans are
        // built, so this projection derives the same canonical place
        // independently. The multiplicity checker's recorded transfer path is
        // compared against this path downstream, not trusted here.
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        if place.segments.is_empty()
            || !place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            })
        {
            return None;
        }
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let root_expression = projection_root_expression(self.program, expression)?;
        let root_reference = crate::flow::canonical_place_type_reference(
            self.program,
            self.state,
            self.statement_index,
            &crate::flow::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        let source = self.structural_value(root_expression, root_reference, values, pure)?;
        Some(CheckedStructuralValueKind::Projection {
            source,
            path,
            type_identity: self
                .program
                .normalized_type_identity(projected)
                .into_string(),
        })
    }

    /// One shared borrow of an exact place selected as a `&T` result. The
    /// referent stays owned by its established root; the authored target's
    /// canonical root and path become a `SharedBorrow` source so lowering can
    /// rejoin borrowed custody at the selection's block parameter instead of
    /// moving a child. `shared_record_reference` pins the referent to a
    /// carrier the pipeline can build -- a record, or a primitive whose
    /// structural shape is its own scalar -- and the target must be the exact
    /// place semantics admitted: a named root under record fields and literal
    /// fixed indexes, whose projected leaf type has to equal the declared
    /// referent.
    fn borrowed_place(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let ExpressionNode::Borrow(borrow) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if borrow.access != language_semantics::ReferenceAccess::Shared {
            return None;
        }
        let referent = shared_record_reference(self.program, expected)?;
        let place = crate::flow::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            borrow.target,
        )?;
        if !canonical_place_is_borrowable(&place) {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // The root's dense structural position is its signature ordinal when
        // the name resolves to a carried parameter, exactly as
        // `owned_record_place` classifies owned arms; anything else is an
        // operation-sequence local keyed by symbol.
        let source = if let Some((index, _)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && !parameter.relevance.is_erased()
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol)
        {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).ok()?,
            }
        } else {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        };
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        if self.program.normalized_type_identity(projected)
            != self.program.normalized_type_identity(referent)
        {
            return None;
        }
        Some(CheckedStructuralValueKind::Reference {
            source: checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity: self
                    .program
                    .normalized_type_identity(projected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        })
    }

    /// One whole shared `&[T]` view of the collection place this statement
    /// lends. Fixed arrays and vectors own contiguous storage and a slice
    /// borrows it, so a view local denotes the lent place under a shared loan
    /// rather than a fresh value with copied elements, and its stored length
    /// is that collection's own extent. Checked borrow admission already
    /// recorded the loan -- its owner is this statement's local, its root and
    /// projection name the lent place, and its kind is the shared read -- so
    /// the value rejoins that exact row instead of rediscovering a producer
    /// from the authored view spelling. A local holding more than one loan, an
    /// exclusive loan, a loan formed at another statement, or a lent place
    /// whose elements are not the view's elements has no admitted view here.
    fn borrowed_slice_view_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        let element = crate::execution::terminal_unit::types::borrowed_slice_view_element(
            self.program,
            expected,
            &[],
        )?;
        let state = crate::semantic::calls::find_state(self.program, self.state)?;
        let StatementNode::LocalData(local) = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(self.statement_index)?
        else {
            return None;
        };
        if local.is_mutable
            || !local.symbol.is_valid()
            || local.initial_value != expression
            || local.type_reference != expected
        {
            return None;
        }
        let borrow_state = self
            .borrow
            .states
            .iter()
            .map(|(_, borrow_state)| borrow_state)
            .find(|borrow_state| {
                borrow_state.machine_symbol == self.machine
                    && borrow_state.state_symbol == self.state
            })?;
        let mut loans = self
            .borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .iter()
            .filter(|loan| loan.owner_symbol == local.symbol);
        let loan = loans.next()?;
        if loans.next().is_some()
            || loan.statement_index != self.statement_index
            || loan.kind != checked_trees::BorrowAccessKind::Read
        {
            return None;
        }
        let mut place = crate::flow::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(loan.root_symbol),
            segments: self
                .borrow
                .access_segments
                .span_or_empty(loan.segments)
                .to_vec(),
        };
        crate::flow::normalize_attached_place_root(
            self.program,
            self.machine,
            self.state,
            &mut place,
        );
        if !canonical_place_is_borrowable(&place) {
            return None;
        }
        let facts::PlaceRoot::Symbol(symbol) = place.root else {
            return None;
        };
        // The root's dense structural position is its signature ordinal when
        // the name resolves to a carried parameter, exactly as the shared
        // `&T` borrow classifies its own root; anything else is an
        // operation-sequence local keyed by symbol.
        let source = if let Some((index, _)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && !parameter.relevance.is_erased()
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == symbol)
        {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(index).ok()?,
            }
        } else {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        };
        let (projected, path) = crate::execution::terminal_unit::calls::projected_argument_path(
            self.program,
            self.state,
            self.statement_index,
            &place,
        )?;
        // The lent place must hold the very elements this view carries; an
        // equal-sized or same-named collection supplies no view custody.
        if self
            .program
            .normalized_type_identity(lent_collection_element(self.program, projected)?)
            != self.program.normalized_type_identity(element)
        {
            return None;
        }
        Some(CheckedStructuralValueKind::BorrowedSliceView {
            source: checked_trees::CheckedUnitStructuralArgumentPlan {
                source,
                path,
                type_identity: self
                    .program
                    .normalized_type_identity(projected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
            },
        })
    }

    fn owned_record_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let ExpressionNode::Name(name) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !name.symbol.is_valid()
            || name.head_symbol != name.symbol
            || name.members.count() != 1
            // The same finite owned walls as plain storage, but a `[linear]`
            // member rides as a claim the selection transfer names.
            || !validation::has_linear_owned_contents(self.program, expected)
        {
            return None;
        }
        let TypeReferenceNode::Named { symbol, .. } =
            self.program.type_reference_table.type_reference(expected)
        else {
            return None;
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        if self
            .program
            .data_members(data)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return None;
        }
        let owner = crate::lookup::machine_by_symbol(self.program, self.machine)?;
        let state = self
            .program
            .machine_states(owner)
            .iter()
            .find(|state| state.symbol == self.state)?;
        let (source, reference) = if let Some((index, parameter)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                // An `[erased]` binding owns no structural position either.
                !parameter.is_const
                    && !parameter.relevance.is_erased()
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == name.symbol)
        {
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(index).ok()?,
                },
                parameter.type_reference,
            )
        } else {
            let local = self
                .program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(self.statement_index)
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local)
                        if local.symbol == name.symbol && local.initial_value.is_valid() =>
                    {
                        Some(local)
                    }
                    _ => None,
                })?;
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                },
                local.type_reference,
            )
        };
        if self.program.normalized_type_identity(reference)
            != self.program.normalized_type_identity(expected)
            || !validation::has_linear_owned_contents(self.program, reference)
            || !matches!(
                self.program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Named { .. }
            )
        {
            return None;
        }
        Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path: Vec::new(),
            type_identity: self
                .program
                .normalized_type_identity(reference)
                .into_string(),
            access: checked_trees::CheckedStructuralAccess::Owned,
        })
    }

    /// Element-wise array literal establishment: each element registers as
    /// its own structural value under the declared element type, exactly as
    /// record fields do inside a constructor.
    fn fixed_array_value(
        &mut self,
        expected: TypeReferenceHandle,
        literal: arena::HandleSpan<ExpressionHandle>,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        if validation::is_closed_primitive_array_type(self.program, expected) {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let TypeReferenceNode::FixedArray { element_type, .. } =
            self.program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let mut elements = Vec::new();
        for element in self.program.expression_table.expression_handles(literal) {
            elements.push(self.structural_value(*element, *element_type, values, pure)?);
        }
        Some(CheckedStructuralValueKind::FixedArray { elements })
    }

    fn record_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression).clone()
        else {
            return None;
        };
        if !validation::has_plain_owned_contents_with_numeric_constraints(self.program, expected)
            && !validation::has_cleanup_owned_contents(self.program, expected)
            && !validation::reference_result_custody::is_reference_record(self.program, expected)
            && !crate::execution::terminal_unit::types::record_with_owned_or_shared_view_fields(
                self.program,
                expected,
            )
        {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let symbol = match self.program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => symbol,
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if self
                .program
                .type_reference_table
                .type_reference_handles(*arguments)
                .is_empty() =>
            {
                base_symbol
            }
            _ => return None,
        };
        if literal.case_name.is_some() || literal.type_symbol != *symbol {
            return None;
        }
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        let declared = self.program.data_members(data);
        if declared
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return None;
        }
        let authored = self
            .program
            .expression_table
            .struct_fields(literal.fields)
            .to_vec();
        let relevant = declared
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let erased = declared
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.relevance.is_erased() => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        // An ordinary runtime field may be omitted only when its zero value
        // satisfies the declared carrier — today that means a `[scalar; N]`
        // leaf whose element is a plain integer primitive. Every authored
        // initializer must still name a declared member exactly once.
        let omitted = relevant
            .iter()
            .filter(|field| {
                authored
                    .iter()
                    .all(|initializer| initializer.field_symbol != field.symbol)
            })
            .copied()
            .collect::<Vec<_>>();
        if authored.len() != relevant.len() + erased.len()
            && (authored.len() > relevant.len() + erased.len()
                || authored.iter().any(|initializer| {
                    declared.iter().all(|member| match member {
                        typed_trees::data::DataMember::Field(field) => {
                            field.symbol != initializer.field_symbol
                        }
                        _ => true,
                    })
                })
                || omitted.iter().any(|field| {
                    zeroed_scalar_array_parts(self.program, field.type_reference).is_none()
                }))
        {
            return None;
        }
        let mut fields = Vec::new();
        for (ordinal, initializer) in authored.into_iter().enumerate() {
            let erased_field = erased
                .iter()
                .find(|field| field.symbol == initializer.field_symbol);
            let field = if let Some(field) = erased_field {
                *field
            } else {
                *relevant
                    .iter()
                    .find(|field| field.symbol == initializer.field_symbol)?
            };
            if erased_field.is_some() {
                // The erased member stays in the checked record's field list —
                // it carries semantic content but no runtime storage, so the
                // checked value keeps its exact initializer for interpretation
                // while emission skips it below the lowered representations.
                let value = checked_trees::CheckedStructuralRecordFieldValue::Structural(
                    self.structural_value(initializer.value, field.type_reference, values, pure)?,
                );
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.symbol,
                    expression: initializer.value,
                    type_reference: field.type_reference,
                    value,
                });
                continue;
            }
            if fields
                .iter()
                .any(|prior: &checked_trees::CheckedStructuralRecordField| {
                    prior.field == field.symbol
                })
            {
                return None;
            }
            let reference =
                validation::unwrapped_type_reference(self.program, field.type_reference)?;
            let value =
                if validation::reference_result_custody::parts(self.program, field.type_reference)
                    .is_some()
                {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(
                            initializer.value,
                            field.type_reference,
                            values,
                            pure,
                        )?,
                    )
                } else if let Some(primitive) = self.program.primitive_type_reference(reference) {
                    let root = self.expression(initializer.value, primitive)?;
                    self.plans.nodes.get_mut(root).authored_root = initializer.value;
                    self.plans.roots.append(CheckedScalarComputationRoot {
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal: u32::try_from(self.statement_index).ok()?,
                        role: CheckedScalarExpressionRole::RecordField {
                            expression,
                            field_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        root,
                    });
                    checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
                } else {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(
                            initializer.value,
                            field.type_reference,
                            values,
                            pure,
                        )?,
                    )
                };
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: field.symbol,
                expression: initializer.value,
                type_reference: field.type_reference,
                value,
            });
        }
        for field in omitted {
            let (primitive_type, element_count) =
                zeroed_scalar_array_parts(self.program, field.type_reference)?;
            let zero = zeroed_integer_literal(primitive_type)?;
            let element = self
                .plans
                .nodes
                .append(checked_trees::CheckedScalarComputation {
                    authored_root: ExpressionHandle::invalid(),
                    value_source: ExpressionHandle::invalid(),
                    primitive_type,
                    kind: checked_trees::CheckedScalarComputationKind::Value(zero),
                });
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: u32::try_from(self.statement_index).ok()?,
                role: CheckedScalarExpressionRole::RecordField {
                    expression,
                    field_ordinal: u32::try_from(fields.len()).ok()?,
                },
                root: element,
            });
            let node = values.nodes.append(CheckedStructuralValue {
                expression,
                kind: CheckedStructuralValueKind::ZeroedScalarArray {
                    element,
                    element_count,
                },
            });
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: field.symbol,
                expression,
                type_reference: field.type_reference,
                value: checked_trees::CheckedStructuralRecordFieldValue::Structural(node),
            });
        }
        Some(CheckedStructuralValueKind::Record {
            data_symbol: literal.type_symbol,
            fields: values.record_fields.insert_many(fields),
        })
    }

    /// A case literal composes its payload fields exactly like a record's:
    /// authored initializers pair against the selected case's declarations in
    /// authored order, and each operand is a scalar computation or a nested
    /// structural value. The case tag stays with the construction, so the
    /// value keeps its discriminated identity a record cannot spell.
    fn structural_case_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        let constructor = validation::structural_case_constructor(self.program, expression)?;
        if self
            .program
            .normalized_type_identity(constructor.type_reference)
            != self.program.normalized_type_identity(expected)
        {
            return None;
        }
        let mut fields = Vec::with_capacity(constructor.fields.len());
        for (ordinal, (symbol, value_expression, declared)) in
            constructor.fields.into_iter().enumerate()
        {
            let reference = validation::unwrapped_type_reference(self.program, declared)?;
            let value =
                if validation::reference_result_custody::parts(self.program, declared).is_some() {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(value_expression, declared, values, pure)?,
                    )
                } else if let Some(primitive) = self.program.primitive_type_reference(reference) {
                    let root = self.expression(value_expression, primitive)?;
                    self.plans.nodes.get_mut(root).authored_root = value_expression;
                    self.plans.roots.append(CheckedScalarComputationRoot {
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal: u32::try_from(self.statement_index).ok()?,
                        role: CheckedScalarExpressionRole::StructuralValueField {
                            expression,
                            field_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        root,
                    });
                    checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
                } else {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(value_expression, declared, values, pure)?,
                    )
                };
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: symbol,
                expression: value_expression,
                type_reference: declared,
                value,
            });
        }
        let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
        else {
            return None;
        };
        Some(CheckedStructuralValueKind::StructuralCase {
            data_symbol: literal.type_symbol,
            case: constructor.case,
            fields: values.record_fields.insert_many(fields),
        })
    }
}
