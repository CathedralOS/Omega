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
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return false;
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
            let state = self
                .program
                .machines()
                .iter()
                .find(|machine| machine.symbol == self.machine)
                .and_then(|machine| {
                    self.program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == self.state)
                })?;
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
                u32::try_from(self.statement_index).ok()?,
                call_ordinal,
                call.target_symbol,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
            );
            CheckedStructuralValueKind::Call { source_call }
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
            && literal.case_symbol.is_none()
        {
            self.record_value(expression, expected, values, pure)?
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
            let machine = self
                .program
                .machines()
                .iter()
                .find(|machine| machine.symbol == self.machine)?;
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
        let owner = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.machine)?;
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
        {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            self.program.type_reference_table.type_reference(reference)
        else {
            return None;
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
        if authored.len() != relevant.len() + erased.len() {
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
        Some(CheckedStructuralValueKind::Record {
            data_symbol: literal.type_symbol,
            fields: values.record_fields.insert_many(fields),
        })
    }
}
