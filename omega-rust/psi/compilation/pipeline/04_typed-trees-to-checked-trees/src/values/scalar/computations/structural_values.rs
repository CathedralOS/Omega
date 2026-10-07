//! Fresh structural establishment shares the scalar operand evaluation owner.
//! Reference leaves are classified before referent-oriented normalization:
//! their value is borrowed-storage custody, never a scalar pointer snapshot.
//! Literals, returned values, and whole transfers share the construction-contents rule:
//! exact interval-qualified scalar members use the existing bounded carrier
//! and per-member establishment proof, not a constructor-specific exception.
//! Closed primitive arrays select their scalar-array producer at the value
//! boundary, independent of whether a record, case, or other value consumes it.
//! The array occurrence owns its row-major leaf roots and independent replay.
//! Copy permission belongs to the selected leaf, not its root: a readable
//! owned or borrowed root may supply an unrestricted child without moving it.
//! Exact source/path replay and ordinary liveness remain downstream checks.
use super::{
    Builder, CheckedScalarComputationRoot, CheckedScalarExpressionRole, ExpressionHandle,
    ExpressionNode, PrimitiveType, SymbolHandle, TypeReferenceNode,
};
use checked_trees::{
    CheckedStructuralDispatchArm, CheckedStructuralValue, CheckedStructuralValueHandle,
    CheckedStructuralValueKind,
};
use typed_trees::expression::MatchPattern;
use typed_trees::types::TypeReferenceHandle;
use value_predicates::argument_needs_establishment;

mod family_values;
mod place_types;
mod place_values;
mod value_predicates;

pub(super) use place_types::{
    canonical_place_is_borrowable, is_scalar_case_place_value, needs_copied_place_root,
};
pub(super) use value_predicates::{
    is_borrowed_slice_view_value, is_fixed_array_value, is_record_value, is_shared_borrow_value,
    is_structural_case_value,
};

impl Builder<'_, '_> {
    /// A projection names a subtree of the retained value's real owner. Keep
    /// that owner as the expression source and the selection as an argument
    /// path, so every evaluation schedule establishes and disposes it once.
    pub(super) fn retained_argument_owner(
        &mut self,
        value: CheckedStructuralValueHandle,
        reference: TypeReferenceHandle,
    ) -> Option<(
        ExpressionHandle,
        TypeReferenceHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    )> {
        let node = self.structural_values.nodes.get(value);
        let (owner, path) = match &node.kind {
            CheckedStructuralValueKind::Projection { source, path, .. } => (*source, path.clone()),
            _ => (value, Vec::new()),
        };
        let expression = self.structural_values.nodes.get(owner).expression;
        let owner_reference = if owner == value {
            reference
        } else {
            let place = crate::flow::canonical_place_from_expression_in_state(
                self.program,
                self.state,
                self.statement_index,
                expression,
            )?;
            if !place.segments.is_empty() {
                return None;
            }
            crate::flow::canonical_place_type_reference(
                self.program,
                self.state,
                self.statement_index,
                &place,
            )?
        };
        if !argument_needs_establishment(&self.structural_values.nodes.get(owner).kind) {
            return None;
        }
        let statement = u32::try_from(self.statement_index).ok()?;
        if let Some(retained) = self
            .structural_values
            .root_for_expression(self.state, statement, expression)
        {
            if retained.machine != self.machine
                || self
                    .program
                    .normalized_type_identity(retained.type_reference)
                    != self.program.normalized_type_identity(owner_reference)
            {
                return None;
            }
        } else {
            self.structural_values
                .roots
                .append(checked_trees::CheckedStructuralValueRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: statement,
                    expression,
                    type_reference: owner_reference,
                    root: owner,
                });
        }
        Some((expression, owner_reference, path))
    }

    /// Retain the value established by an argument, independent of how the
    /// consumer accesses it. A shared temporary owns its referent, not a
    /// reference-shaped constructor. Existing places keep their loan/storage
    /// route; retaining a value does not itself authorize lending or cleanup.

    /// Retain the value established by an argument, independent of how the
    /// consumer accesses it. A shared temporary owns its referent, not a
    /// reference-shaped constructor. Existing places keep their loan/storage
    /// route; retaining a value does not itself authorize lending or cleanup.
    pub(super) fn retain_structural_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueHandle> {
        let (expression, expected) =
            match self.program.type_reference_table.type_reference(expected) {
                // A view carries a descriptor value, not an owned slice
                // referent. Preserve its checked reference shell and loan.
                TypeReferenceNode::Reference { .. }
                    if validation::slice_view_shell(self.program, expected, &[]).is_some() =>
                {
                    // Existing storage, a stored view however qualified or a
                    // whole array lent as a view, keeps its loan route. Only a
                    // computed formation needs an operand result.
                    if crate::semantic::calls::view_argument_names_existing_place(
                        self.program,
                        expression,
                    ) {
                        return None;
                    }
                    (expression, expected)
                }
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    ..
                } => {
                    let (expression, expected) = crate::semantic::calls::shared_argument_referent(
                        self.program,
                        expression,
                        expected,
                    )?;
                    let place = crate::flow::canonical_place_from_expression_in_state(
                        self.program,
                        self.state,
                        self.statement_index,
                        expression,
                    )?;
                    if place.root != facts::PlaceRoot::Expression(expression)
                        || !place.segments.is_empty()
                    {
                        return None;
                    }
                    (expression, expected)
                }
                TypeReferenceNode::Reference { .. } => return None,
                _ => (expression, expected),
            };
        if self.program.primitive_type_reference(expected).is_some() {
            return None;
        }
        let statement = u32::try_from(self.statement_index).ok()?;
        if let Some(root) = self
            .structural_values
            .root_for_expression(self.state, statement, expression)
        {
            return (root.machine == self.machine
                && self.program.normalized_type_identity(root.type_reference)
                    == self.program.normalized_type_identity(expected)
                && self.structural_values.nodes.is_valid(root.root)
                && self.structural_values.nodes.get(root.root).expression == expression
                && argument_needs_establishment(
                    &self.structural_values.nodes.get(root.root).kind,
                ))
            .then_some(root.root);
        }
        let root = self.structural_value(expression, expected)?;
        if !argument_needs_establishment(&self.structural_values.nodes.get(root).kind) {
            return None;
        }
        self.structural_values
            .roots
            .append(checked_trees::CheckedStructuralValueRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: statement,
                expression,
                type_reference: expected,
                root,
            });
        Some(root)
    }

    /// The dense structural position `symbol` occupies in this state's
    /// signature, with the parameter's declared type, or `None` when the name
    /// is not a carried structural formal. Dense positions count only the
    /// parameters that own a structural ingress slot: a `const` or `[erased]`
    /// binding owns none, and a primitive parameter travels the scalar roster
    /// instead. Consumers resolve `Parameter { parameter_index }` against the
    /// retained structural parameter roster, which counts the same way, so the
    /// index is a shared coordinate rather than a local convention. One known
    /// boundary: stage 05's `parameter_declaration` omits the `[erased]`
    /// predicate, so a state that mixes an erased structural binding with a
    /// later structural formal is the one shape where the two sides disagree.

    /// The dense structural position `symbol` occupies in this state's
    /// signature, with the parameter's declared type, or `None` when the name
    /// is not a carried structural formal. Dense positions count only the
    /// parameters that own a structural ingress slot: a `const` or `[erased]`
    /// binding owns none, and a primitive parameter travels the scalar roster
    /// instead. Consumers resolve `Parameter { parameter_index }` against the
    /// retained structural parameter roster, which counts the same way, so the
    /// index is a shared coordinate rather than a local convention. One known
    /// boundary: stage 05's `parameter_declaration` omits the `[erased]`
    /// predicate, so a state that mixes an erased structural binding with a
    /// later structural formal is the one shape where the two sides disagree.
    fn structural_parameter(&self, symbol: SymbolHandle) -> Option<(u32, TypeReferenceHandle)> {
        let (index, parameter) = self
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
            .find(|(_, parameter)| parameter.symbol == symbol)?;
        Some((u32::try_from(index).ok()?, parameter.type_reference))
    }

    /// The source plan naming where `symbol`'s storage lives: its dense
    /// signature position when the name resolves to a carried structural
    /// formal, and the operation sequence's local slot otherwise. Every whole
    /// owned structural source this builder plans classifies through here,
    /// because a parameter root described as a local has no consumption owner:
    /// the returned-value walk looks for a result binding that a formal never
    /// mints, and the affine residual routes key on the dense position.

    /// The source plan naming where `symbol`'s storage lives: its dense
    /// signature position when the name resolves to a carried structural
    /// formal, and the operation sequence's local slot otherwise. Every whole
    /// owned structural source this builder plans classifies through here,
    /// because a parameter root described as a local has no consumption owner:
    /// the returned-value walk looks for a result binding that a formal never
    /// mints, and the affine residual routes key on the dense position.
    fn structural_source(
        &self,
        symbol: SymbolHandle,
    ) -> checked_trees::CheckedUnitStructuralArgumentSourcePlan {
        match self.structural_parameter(symbol) {
            Some((parameter_index, _)) => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                }
            }
            None => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            }
        }
    }

    pub(super) fn structural_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueHandle> {
        let kind = if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::String(_)
        ) {
            let argument = crate::execution::terminal_unit::calls::byte_sequence_literal_argument(
                self.program,
                expected,
                expression,
            )?;
            if argument.access != checked_trees::CheckedStructuralAccess::Owned {
                return None;
            }
            CheckedStructuralValueKind::Place(argument)
        } else if validation::reference_result_custody::stored_loan_parts(self.program, expected)
            .is_some()
            && match self.program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) => {
                    borrow.access != language_semantics::ReferenceAccess::Shared
                }
                // Borrowed formals denote ingress referents; unlike retained
                // locals, they do not own stored descriptor values to move.
                ExpressionNode::Name(name) => {
                    self.structural_parameter(name.symbol)
                        .is_some_and(|(_, declared)| {
                            validation::reference_result_custody::stored_loan_parts(
                                self.program,
                                declared,
                            )
                            .is_some()
                        })
                }
                _ => false,
            }
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
            && let Some(argument) = self.owned_structural_place(expression, expected)
            && matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
            )
        {
            // A whole owned parameter has no producer in this body: reading it
            // transfers the caller's value, which an unrestricted carrier
            // copies. Only a local's later read takes the copy route below,
            // because the local's initializer need not be its current value.
            CheckedStructuralValueKind::Place(argument)
        } else if let Some(view) = self
            .subslice_view_place(expression, expected)
            .or_else(|| self.slice_view_place(expression, expected))
        {
            // A view's authored producer is spelled as a call but establishes
            // no call result: it lends storage that already exists. Classify
            // it before the ordinary call-result branch, which would otherwise
            // demand a returning target for it.
            view
        } else if let ExpressionNode::Call(call) =
            self.program.expression_table.expression(expression)
        {
            let call_target = crate::flow::call_target_symbol(self.program, call);
            let returned = crate::flow::call_target_return_type(self.program, call_target)?;
            if self.program.normalized_type_identity(returned)
                != self.program.normalized_type_identity(expected)
                || !(validation::has_contents(
                    self.program,
                    returned,
                    validation::ContentsRequirement::StructuralConstruction,
                ) || validation::has_contents(
                    self.program,
                    returned,
                    validation::ContentsRequirement::LinearOwned,
                ) || validation::reference_result_custody::is_reference_record(
                    self.program,
                    returned,
                ))
            {
                return None;
            }
            let (source_call, call_ordinal) = self.call_ordinal(expression, call_target)?;
            self.record_call_arguments(
                u32::try_from(self.statement_index).ok()?,
                call_ordinal,
                call_target,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
            );
            CheckedStructuralValueKind::Call { source_call }
        } else if let Some(copy) = self.scalar_case_place(expression, expected) {
            copy
        } else if let Some(copy) = self.view_element_copy(expression, expected) {
            copy
        } else if let Some(copied) = self.copied_place(expression, expected) {
            copied
        } else if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) && validation::scalar_case_constructor(self.program, expression).is_none()
            && let Some(argument) = self.owned_structural_place(expression, expected)
        {
            // Reading unrestricted storage snapshots its current contents at
            // this evaluation point. Try the copy routes before whole-place
            // custody: a mutable copyable local also has an owned shape, but
            // its original producer is not the value of a later read.
            CheckedStructuralValueKind::Place(argument)
        } else if let Some(projection) = self.projected_selection_place(expression, expected) {
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
            self.structural_case_value(expression, expected)?
        } else if let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression)
            && literal.case_symbol.is_none()
        {
            self.record_value(expression, expected)?
        } else if validation::is_closed_primitive_array_type(self.program, expected) {
            self.scalar_array_value(expression, expected)?
        } else if let ExpressionNode::ArrayLiteral(literal) =
            self.program.expression_table.expression(expression)
        {
            self.fixed_array_value(expected, *literal)?
        } else if let Some(symbol) =
            validation::scalar_case_value_source(self.program, expression, expected)
        {
            CheckedStructuralValueKind::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                source: self.structural_source(symbol),
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
                let value = self.structural_value(arm.value, expected)?;
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
                arms: self.structural_values.dispatch_arms.insert_many(arms),
            }
        };
        Some(
            self.structural_values
                .nodes
                .append(CheckedStructuralValue { expression, kind }),
        )
    }
}
