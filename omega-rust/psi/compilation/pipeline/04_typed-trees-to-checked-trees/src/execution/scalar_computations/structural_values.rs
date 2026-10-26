//! Fresh structural establishment shares the scalar operand evaluation owner.
//! Reference leaves are classified before referent-oriented normalization:
//! their value is borrowed-storage custody, never a scalar pointer snapshot.
//! Literals, returned values, and whole transfers share the construction-contents rule:
//! exact interval-qualified scalar members use the existing bounded carrier
//! and per-member establishment proof, not a constructor-specific exception.
//! Closed primitive arrays select their scalar-array producer at the value
//! boundary, independent of whether a record, case, or other value consumes it.
//! A match is not that producer: it selects one array, and only the chosen
//! arm may run. Literal constructors still own the row-major leaf roster.
//! Copy permission belongs to the selected leaf, not its root: a readable
//! owned or borrowed root may supply an unrestricted child without moving it.
//! Exact source/path replay and ordinary liveness remain downstream checks.
use super::{
    Builder, CheckedScalarExpressionRole, ExpressionHandle, ExpressionNode, PrimitiveType,
    SymbolHandle, TypeReferenceNode,
};
use crate::values::operator_is_builtin;
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

pub(super) use place_types::{canonical_place_is_borrowable, needs_copied_place_root};

impl Builder<'_, '_> {
    /// Retain runtime children independently of the canonical endpoint's ordinal.
    pub(in crate::execution::scalar_computations) fn projection_selector_evaluations(
        &mut self,
        owner: ExpressionHandle,
        source: ExpressionHandle,
    ) -> Option<
        Vec<(
            ExpressionHandle,
            checked_trees::CheckedScalarComputationHandle,
        )>,
    > {
        let selectors = crate::values::authored_projection_selectors(self.program, source)?;
        self.retain_projection_selector_evaluations(owner, selectors)
    }

    pub(super) fn retain_projection_selector_evaluations(
        &mut self,
        owner: ExpressionHandle,
        selectors: Vec<ExpressionHandle>,
    ) -> Option<
        Vec<(
            ExpressionHandle,
            checked_trees::CheckedScalarComputationHandle,
        )>,
    > {
        if selectors.is_empty() {
            return Some(Vec::new());
        }
        let statement = u32::try_from(self.statement_index).ok()?;
        if let Some(evaluations) =
            self.plans
                .projection_selectors_at(self.machine, self.state, statement, owner)
        {
            let values = if evaluations.is_empty() {
                &[][..]
            } else {
                self.plans.operands.span(evaluations)?
            };
            if selectors.len() != values.len()
                || selectors.iter().zip(values).any(|(expression, root)| {
                    !self.plans.nodes.is_valid(*root)
                        || self.plans.nodes.get(*root).authored_root != *expression
                })
            {
                return None;
            }
            return Some(selectors.into_iter().zip(values.iter().copied()).collect());
        }
        let values = selectors
            .into_iter()
            .enumerate()
            .map(|(ordinal, selector)| {
                // Different consumers of one captured projection share its
                // selector evaluation, not merely an equal-looking index.
                let role = CheckedScalarExpressionRole::StructuralValueSelector {
                    expression: owner,
                    selector_ordinal: u32::try_from(ordinal).ok()?,
                };
                let value = if let Some(root) =
                    self.plans
                        .root_at(self.state, u32::try_from(self.statement_index).ok()?, role)
                {
                    root.root
                } else {
                    self.index_selector(selector)?
                };
                Some((selector, value))
            })
            .collect::<Option<Vec<_>>>()?;
        let evaluations = self
            .plans
            .operands
            .insert_many(values.iter().map(|(_, value)| *value));
        self.plans
            .projection_selectors
            .append(checked_trees::CheckedProjectionSelectorRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: statement,
                expression: owner,
                evaluations,
            });
        Some(values)
    }

    pub(in crate::execution::scalar_computations) fn projection_selector_span(
        &self,
        expression: ExpressionHandle,
        selectors: &[(
            ExpressionHandle,
            checked_trees::CheckedScalarComputationHandle,
        )],
    ) -> Option<arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>> {
        if selectors.is_empty() {
            return Some(Default::default());
        }
        let span = self.plans.projection_selectors_at(
            self.machine,
            self.state,
            u32::try_from(self.statement_index).ok()?,
            expression,
        )?;
        let children = self.plans.operands.span(span)?;
        children
            .iter()
            .copied()
            .eq(selectors.iter().map(|(_, child)| *child))
            .then_some(span)
    }

    pub(in crate::execution::scalar_computations) fn retain_reference_selectors(
        &mut self,
        expression: ExpressionHandle,
        selectors: Vec<(
            ExpressionHandle,
            checked_trees::CheckedScalarComputationHandle,
        )>,
    ) -> Option<arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>> {
        for (ordinal, (_, value)) in selectors.iter().enumerate() {
            self.push_scalar_root(
                CheckedScalarExpressionRole::StructuralValueSelector {
                    expression,
                    selector_ordinal: u32::try_from(ordinal).ok()?,
                },
                *value,
            )?;
        }
        self.projection_selector_span(expression, &selectors)
    }

    /// A nonempty projection names a subtree of the retained value's owner.
    /// Keep that owner and selection path together. Whole-value occurrences
    /// retain their own checking judgments and evaluation identity.
    pub(super) fn retained_argument_owner(
        &mut self,
        value: CheckedStructuralValueHandle,
        reference: TypeReferenceHandle,
    ) -> Option<(
        ExpressionHandle,
        TypeReferenceHandle,
        Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        arena::HandleSpan<checked_trees::CheckedScalarComputationHandle>,
    )> {
        let node = self.structural_values.nodes.get(value);
        let (owner, path, projection_evaluations) = match &node.kind {
            // A noncopy selection retains its containing owner. A selected
            // copy owns an independent completed value, just as a whole-value
            // cast owns its checked memberships; using its source occurrence
            // would lose that new value and its exact root identity.
            CheckedStructuralValueKind::Projection {
                source,
                path,
                projection_evaluations,
                copies_selected_value: false,
                ..
            } if !path.is_empty() => (*source, path.clone(), *projection_evaluations),
            _ => (value, Vec::new(), Default::default()),
        };
        let owner_node = self.structural_values.nodes.get(owner);
        let expression = owner_node.expression;
        // The value node owns the declared carrier of this exact completed
        // occurrence. Its source expression may still spell a projection of
        // the old storage; that syntax is not its new owned home namespace.
        let owner_reference = owner_node.type_reference;
        let statement = u32::try_from(self.statement_index).ok()?;
        if owner_node.machine != self.machine
            || owner_node.state != self.state
            || owner_node.statement_index != statement
        {
            return None;
        }
        if owner == value
            && !validation::declared_carriers_match(self.program, owner_reference, reference)
        {
            return None;
        }
        // Both the containing owner and a selected completed value use the
        // same association owner. It authenticates value handles before any
        // ordinary expression publication, so captured copies keep their own
        // namespace and never acquire a second producer.
        self.retain_structural_value_root(owner, owner_reference)?;
        let retained = self.structural_values.root_for_value(owner)?;
        if self
            .program
            .normalized_type_identity(retained.type_reference)
            != self.program.normalized_type_identity(owner_reference)
        {
            return None;
        }
        Some((expression, owner_reference, path, projection_evaluations))
    }

    /// Authenticate a completed handle before using the common ordinary root
    /// publisher. Captured copies are already associated by value and cannot
    /// be republished through their diagnostic source expression. An occupied
    /// or ambiguous ordinary coordinate cannot accept a different value.
    pub(super) fn retain_structural_value_root(
        &mut self,
        value: CheckedStructuralValueHandle,
        reference: TypeReferenceHandle,
    ) -> Option<()> {
        let node = self.structural_values.nodes.get(value);
        let statement = u32::try_from(self.statement_index).ok()?;
        if node.machine != self.machine
            || node.state != self.state
            || node.statement_index != statement
            || !validation::declared_carriers_match(self.program, node.type_reference, reference)
        {
            return None;
        }
        if let Some(root) = self.structural_values.root_for_value(value) {
            return (root.machine == self.machine
                && root.state == self.state
                && root.statement_ordinal == statement
                && root.expression == node.expression
                && validation::declared_carriers_match(
                    self.program,
                    root.type_reference,
                    node.type_reference,
                ))
            .then_some(());
        }
        if self
            .structural_values
            .roots_for_statement(self.state, statement)
            .iter()
            .any(|root| {
                root.root == value
                    || (!root.captured_copy.is_valid() && root.expression == node.expression)
            })
        {
            return None;
        }
        self.retain_structural_root(statement, node.expression, node.type_reference, value);
        Some(())
    }

    /// A scalar consumer may need the value of readable storage before a later
    /// operand changes it. Use the ordinary copy producer and its declared
    /// capability; a tag observation alone never licenses copying an owner.
    pub(super) fn retain_copied_place_value(
        &mut self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueHandle> {
        let kind = self.copied_place(expression, reference)?;
        let value = self.publish_structural_value(expression, reference, kind)?;
        self.retained_argument_owner(value, reference)?;
        Some(value)
    }

    /// Retain the value established by an argument, independent of how the
    /// consumer accesses it. A shared temporary owns its referent, not a
    /// reference-shaped constructor. Existing places keep their loan/storage
    /// route; retaining a value does not itself authorize lending or cleanup.
    pub(super) fn retain_structural_argument(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueHandle> {
        let (expression, expected) = match self
            .program
            .type_reference_table
            .type_reference(expected)
        {
            _ if self.uses_captured_dispatch_value(expression) => {
                let producer = if validation::reference_shell(self.program, expected).is_some() {
                    validation::expression_result_type_reference(
                        self.program,
                        self.program.machine_by_symbol(self.machine)?,
                        self.program.state_by_symbol(self.state)?,
                        expression,
                    )?
                } else {
                    expected
                };
                (expression, producer)
            }
            TypeReferenceNode::Reference { .. }
                if validation::reference_result_custody::stored_loan_parts(
                    self.program,
                    expected,
                )
                .is_some()
                    && validation::reference_formation_source(self.program, expression)
                        .is_some() =>
            {
                let machine = self.program.machine_by_symbol(self.machine)?;
                let state = self.program.state_by_symbol(self.state)?;
                let actual = crate::values::reference_sources::formed_reference_type(
                    self.program,
                    machine,
                    state,
                    expression,
                )?;
                (expression, actual)
            }
            // A view carries a descriptor value, not an owned slice
            // referent. Preserve its checked reference shell and loan.
            TypeReferenceNode::Reference { .. }
                if validation::slice_view_shell(self.program, expected, &[]).is_some() =>
            {
                // A view is a descriptor value even when it has a named or
                // projected home. Complete that value at this operand's own
                // position; a later argument may replace the old slot. The
                // same value producer forms array-to-view descriptors, and
                // retained loan facts independently preserve backing custody.
                (expression, expected)
            }
            TypeReferenceNode::Reference { .. }
                if let Some(actual) = crate::values::reference_sources::transported_value_type(
                    self.program,
                    self.program.machine_by_symbol(self.machine)?,
                    self.program.state_by_symbol(self.state)?,
                    self.statement_index,
                    expression,
                ) =>
            {
                // Source admission already checked this actual against its
                // formal in the caller's telescope. Retain the produced
                // carrier; replaying correspondence here loses that scope.
                (expression, actual)
            }
            TypeReferenceNode::Reference { .. }
                if validation::reference_formation_source(self.program, expression).is_some() =>
            {
                // An explicit loan is a completed value even when its backing
                // already exists. Its fresh borrowed arrival carries facts at
                // this occurrence without relabeling the backing's old value.
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
                let place = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    expression,
                )?;
                if !matches!(place.root, facts::PlaceRoot::Expression(_)) {
                    return None;
                }
                (expression, expected)
            }
            TypeReferenceNode::Reference { .. } => return None,
            _ => (expression, expected),
        };
        let statement = u32::try_from(self.statement_index).ok()?;
        let retained = self
            .structural_values
            .root_for_expression(self.state, statement, expression);
        let root = if let Some(root) = retained {
            if root.machine != self.machine
                || self.program.normalized_type_identity(root.type_reference)
                    != self.program.normalized_type_identity(expected)
                || !self.structural_values.nodes.is_valid(root.root)
                || self.structural_values.nodes.get(root.root).expression != expression
            {
                return None;
            }
            root.root
        } else {
            self.structural_value(expression, expected)?
        };
        if !argument_needs_establishment(&self.structural_values.nodes.get(root).kind) {
            // A whole-place operand normally uses direct storage transport.
            // Its declaration, however, need not spell the memberships that
            // checking proved at this occurrence (for example an issued call
            // result stored in an unqualified local). A different receiving
            // contract needs this value root so the ordinary completed-value
            // path can retain its actual qualifications. This registers no
            // new storage, membership, claim or permission to copy.
            let actual = validation::expression_result_type_reference(
                self.program,
                self.program.machine_by_symbol(self.machine)?,
                self.program.state_by_symbol(self.state)?,
                expression,
            )?;
            if self.program.normalized_type_identity(actual)
                == self.program.normalized_type_identity(expected)
            {
                return None;
            }
        }
        // A noncopy selection retains its containing producer. A selected
        // copy already establishes its own independent value home; registering
        // its source instead would also duplicate this value's ordinary root.
        // Scalar and statement calls consume the same owner/path relationship.
        if let CheckedStructuralValueKind::Projection {
            source,
            ref path,
            copies_selected_value: false,
            ..
        } = self.structural_values.nodes.get(root).kind
        {
            let source_needs_establishment =
                argument_needs_establishment(&self.structural_values.nodes.get(source).kind);
            if !path.is_empty() {
                // A projection of an existing home transfers through its place.
                // Only a computed owner establishes an argument temporary.
                if !source_needs_establishment {
                    return None;
                }
                self.retained_argument_owner(root, expected)?;
            } else if !source_needs_establishment
                && self.program.normalized_type_identity(
                    self.structural_values.nodes.get(source).type_reference,
                ) == self.program.normalized_type_identity(expected)
            {
                // A whole value has no residual complement to keep with an
                // owner. An existing home transfers through its place unless
                // the value's contract changes. Adding and erasing memberships
                // both require a completed value with the receiving contract.
                return None;
            }
        }
        self.retain_structural_value_root(root, expected)?;
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
    pub(super) fn structural_source(
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
        // Normalization can share an evaluated operand with a later projection.
        // Preserve that occurrence's value identity, not another producer for
        // the same source. Different authored handles never share this root.
        if let Some(root) = self.structural_values.root_for_expression(
            self.state,
            u32::try_from(self.statement_index).ok()?,
            expression,
        ) {
            return (root.machine == self.machine
                && self.program.normalized_type_identity(root.type_reference)
                    == self.program.normalized_type_identity(expected))
            .then_some(root.root);
        }
        if let ExpressionNode::Call(call) = self.program.expression_table.expression(expression)
            && call.quotient_operation.is_some()
        {
            return self.quotient_call_value(expression, expected);
        }
        let kind = if let ExpressionNode::Call(call) =
            self.program.expression_table.expression(expression)
            && let Some(query) = &call.native_code_layout
        {
            let selected_state = self
                .program
                .machine_by_symbol(query.operand.symbol)
                .and_then(|machine| self.program.machine_states(machine).first())
                .map(|state| state.symbol)
                .unwrap_or(query.operand.symbol);
            // Exact carrier type comes from query typing, not a guessed
            // selected-machine return type or an invocation contract.
            let actual = self
                .program
                .expression_table
                .constructed_value_type_reference(expression);
            if self.program.normalized_type_identity(actual)
                != self.program.normalized_type_identity(expected)
            {
                return None;
            }
            CheckedStructuralValueKind::NativeCodeLayout { selected_state }
        } else if self.uses_captured_dispatch_value(expression) {
            // An authenticated completed receiver supplies the value before
            // scalar or reference planning can choose a live-storage read.
            self.copied_dispatch_place(expression, expected)?
        } else if let Some(primitive) = self.program.primitive_type_reference(expected) {
            // Scalar storage is independent of its containing record, array,
            // or argument. Keep the complete receiving type on this node;
            // lowering owes its restriction when it establishes the home.
            let value = self.expression(expression, primitive)?;
            self.plans.nodes.get_mut(value).authored_root = expression;
            self.push_scalar_root(
                CheckedScalarExpressionRole::StructuralScalarValue { expression },
                value,
            )?;
            CheckedStructuralValueKind::Scalar { value }
        } else if let ExpressionNode::Match(dispatch) =
            self.program.expression_table.expression(expression)
            && let Some(selected) = validation::select_anonymous_numeric_match_arm(
                self.program,
                dispatch,
                |expression| operator_is_builtin(self.operators, expression),
            )
        {
            // Exact anonymous selection has no machine-width subject, just as
            // on the scalar route. Its selected value retains the ordinary
            // constructor schedule and custody; a whole-value projection keeps
            // this Match occurrence without inventing another owner or replay.
            let source = self.structural_value(selected, expected)?;
            CheckedStructuralValueKind::Projection {
                source,
                logical_root_type_identity: Default::default(),
                path: Vec::new(),
                projection_evaluations: Default::default(),
                copies_selected_value: false,
                type_identity: validation::stored_structural_type_identity(
                    self.program,
                    expected,
                    &[],
                    &[],
                ),
            }
        } else if let Some(value) = self.shared_carrier_reborrow(expression, expected) {
            value
        } else if let Some(value) = self.same_carrier_structural_cast(expression, expected) {
            value
        } else if let ExpressionNode::Cast(cast) =
            self.program.expression_table.expression(expression)
            && let Some(definition) = self.program.closed_data_definition(cast.target_type)
            && let Some(quotient) = definition.quotient.as_ref()
        {
            if self.program.normalized_type_identity(expected)
                != self.program.normalized_type_identity(cast.target_type)
                || self.program.type_multiplicity(expected)
                    != language_semantics::Multiplicity::Unrestricted
                || self.program.type_multiplicity(quotient.carrier)
                    != language_semantics::Multiplicity::Unrestricted
            {
                return None;
            }
            let actual = validation::expression_type_reference_in_state(
                self.program,
                self.state,
                self.statement_index,
                cast.value,
            )?;
            if self.program.normalized_type_identity(actual)
                != self.program.normalized_type_identity(quotient.carrier)
            {
                return None;
            }
            let source = self.structural_value(cast.value, quotient.carrier)?;
            CheckedStructuralValueKind::QuotientAdaptation {
                source,
                quotient: cast.target_type,
                license: checked_trees::CheckedQuotientAdaptationLicense::Mint,
            }
        } else if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::String(_)
        ) {
            let argument = crate::execution::terminal_unit::calls::byte_sequence_literal_argument(
                self.program,
                expected,
                expression,
            )?;
            match argument.access {
                checked_trees::CheckedStructuralAccess::Owned => {
                    CheckedStructuralValueKind::Place(argument)
                }
                checked_trees::CheckedStructuralAccess::SharedBorrow => {
                    // A shared literal produces an immutable image-backed
                    // descriptor in every value position. Its destination's
                    // domains must hold before that value is published.
                    if !validation::field_domains::string_literal_grants_view_domains(
                        self.program,
                        expected,
                        expression,
                    ) {
                        return None;
                    }
                    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
                        bytes,
                    } = argument.source else {
                        return None;
                    };
                    CheckedStructuralValueKind::ByteLiteralView { bytes }
                }
                _ => return None,
            }
        } else if validation::reference_shell(self.program, expected).is_some()
            // A runtime slice establishes or transports a pointer and captured
            // extent through the view producer below. Ordinary reference
            // capture lends sized storage, including fixed byte arrays; it
            // cannot replace a slice's authored range with a whole-root loan.
            && validation::slice_view_shell(self.program, expected, &[]).is_none()
            && match self.program.expression_table.expression(expression) {
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
                _ => validation::reference_formation_source(self.program, expression).is_some(),
            }
        {
            let state = self
                .program
                .machine_by_symbol(self.machine)
                .and_then(|machine| self.program.state_in_machine(machine, self.state))?;
            let mut selectors = Vec::new();
            let mut source_value = CheckedStructuralValueHandle::invalid();
            if let Some(formation) =
                validation::reference_formation_source(self.program, expression)
            {
                let place = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    formation.source,
                )?;
                // Retain the complete anonymous producer before its selectors.
                // The reference lends its selected path, not a detached copy
                // of that child; the normal value schedule owns its cleanup.
                if let facts::PlaceRoot::Expression(root) = place.root {
                    // A value leaf (literal, arithmetic) keeps no member
                    // position by design; its result type is still the
                    // referent's exact type at this occurrence.
                    let root_reference = validation::canonical_place_type_reference(
                        self.program,
                        self.state,
                        self.statement_index,
                        &validation::CanonicalPlace {
                            root: place.root,
                            segments: Vec::new(),
                        },
                    )
                    .or_else(|| {
                        validation::expression_result_type_reference(
                            self.program,
                            self.program.machine_by_symbol(self.machine)?,
                            self.program.state_by_symbol(self.state)?,
                            root,
                        )
                    })
                    .or_else(|| {
                        // An anonymous literal referent carries no declared
                        // or computed result type of its own; the borrow's
                        // admitted referee is its exact type at this
                        // occurrence. Projected referents keep their member
                        // positions, so only a segment-free root may ask the
                        // shell.
                        place
                            .segments
                            .is_empty()
                            .then(|| validation::reference_shell(self.program, expected))
                            .flatten()
                            .map(|shell| shell.referee)
                    });
                    let root_reference = root_reference?;
                    source_value = self.structural_value(root, root_reference)?;
                    // The anonymous backing is an independently admitted owner,
                    // not a contained descriptor field. Publish the exact root
                    // so its normal disposition can rejoin this retained graph.
                    self.retained_argument_owner(source_value, root_reference)?;
                }
                // Geometry can already know an ordinal from a call's result
                // contract. Its authored evaluation remains a child even then.
                selectors = self.projection_selector_evaluations(expression, formation.source)?;
            }
            let selector = |expression| {
                selectors.iter().find_map(|(selector, value)| {
                    (*selector == expression).then_some(
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                    )
                })
            };
            let (mut source, storage, backing_type) =
                if validation::reference_formation_source(self.program, expression).is_some() {
                    let capture = crate::values::reference_sources::capture(
                        self.program,
                        self.program.machine_by_symbol(self.machine)?,
                        state,
                        self.statement_index,
                        expression,
                        expected,
                        &selector,
                    )?;
                    (capture.source, capture.storage, capture.backing_type)
                } else {
                    (
                        crate::values::reference_sources::plan(
                            self.program,
                            self.program.machine_by_symbol(self.machine)?,
                            state,
                            self.statement_index,
                            expression,
                            expected,
                            &selector,
                        )?,
                        checked_trees::CheckedReferenceStorage::Declared,
                        validation::reference_shell(self.program, expected)?.referee,
                    )
                };
            let projection_evaluations = if selectors.is_empty() {
                Default::default()
            } else {
                self.projection_selector_span(expression, &selectors)?
            };
            source.retain_projection_evaluations(projection_evaluations, &self.plans.operands)?;
            let selectors = self.retain_reference_selectors(expression, selectors)?;
            CheckedStructuralValueKind::Reference {
                source,
                selectors,
                storage,
                backing_type,
                source_value,
            }
        } else if needs_copied_place_root(
            self.program,
            self.state,
            self.statement_index,
            expression,
            expected,
        ) {
            // Reading an already stored carrier copies that value, including
            // a shared view's captured extent. Its backing loan cannot replace
            // the descriptor with a newly formed whole-collection view.
            // Locals and owned parameters share this decision: affine
            // descriptor retirement does not forbid shared-carrier copying.
            // Failure to prepare a copy cannot turn it into an owned move.
            self.copied_place(expression, expected)?
        } else if let Some(view) = self
            .subslice_view(expression, expected)
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
            let call_target = validation::call_target_symbol(self.program, call);
            // This occurrence owns its substituted result vocabulary. The
            // raw declaration still names callee index/region binders and
            // cannot type the caller's completed structural home.
            let returned = validation::expression_result_type_reference(
                self.program,
                self.program.machine_by_symbol(self.machine)?,
                self.program.state_by_symbol(self.state)?,
                expression,
            )?;
            // Result storage does not depend on the source of executable
            // supply. The ordinary call planner retains the exact boundary,
            // body or dynamic route and its independent completion custody.
            if !value_predicates::structural_carriers_match(self.program, returned, expected)
                || crate::execution::terminal_unit::control::checked_structural_result_type(
                    self.program,
                    &mut crate::execution::terminal_unit::types::ShapeCollector::new(self.program),
                    returned,
                    &[],
                )
                .is_none()
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
        } else if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_) | ExpressionNode::SavedSubject(_)
        ) && validation::scalar_case_constructor(self.program, expression).is_none()
            && let Some(argument) = self.owned_structural_place(expression, expected)
        {
            // Reading unrestricted storage snapshots its current contents at
            // this evaluation point. Try the copy routes before whole-place
            // custody: a mutable copyable local also has an owned shape, but
            // its original producer is not the value of a later read.
            CheckedStructuralValueKind::Place(argument)
        } else if let Some(projection) = self.projected_owned_place(expression, expected) {
            projection
        } else if let Some(constructor) = self.case_construction(expression) {
            if !value_predicates::structural_carriers_match(
                self.program,
                constructor.type_reference,
                expected,
            ) {
                return None;
            }
            let field_values = self
                .plans
                .case_fields
                .span(constructor.fields)?
                .iter()
                .map(|field| field.value)
                .collect::<Vec<_>>();
            for (field_ordinal, value) in field_values.iter().enumerate() {
                self.push_scalar_root(
                    CheckedScalarExpressionRole::StructuralValueField {
                        expression,
                        field_ordinal: u32::try_from(field_ordinal).ok()?,
                    },
                    *value,
                )?;
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
        } else if validation::scalar_array_preserves_memberships(self.program, expected)
            && !matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Match(_)
            )
        {
            // Literal and projected constructors flatten to scalar leaves.
            // A match selects one of those arrays; flattening it would require
            // every arm to be a constructor and would evaluate unselected calls.
            self.scalar_array_value(expression, expected)?
        } else if let ExpressionNode::ArrayLiteral(literal) =
            self.program.expression_table.expression(expression)
        {
            self.fixed_array_value(expected, *literal)?
        } else if let Some(symbol) =
            validation::scalar_case_value_source(self.program, expression, expected)
        {
            CheckedStructuralValueKind::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: self.structural_source(symbol),
                path: Vec::new(),
                type_identity: self.program.normalized_type_identity(expected),
                access: checked_trees::CheckedStructuralAccess::Owned,
            })
        } else {
            let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
            else {
                return None;
            };
            let subject = self.dispatch_subject(&dispatch)?;
            let subject_type = match subject {
                checked_trees::CheckedDispatchSubject::Scalar(value) => {
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralValueSubject { expression },
                        value,
                    )?;
                    Some(self.plans.nodes.get(value).primitive_type)
                }
                checked_trees::CheckedDispatchSubject::Structural { .. } => None,
            };
            let authored = self
                .program
                .expression_table
                .match_arms(dispatch.arms)
                .to_vec();
            let mut arms = Vec::new();
            let mut covered = false;
            let mut boolean_coverage = [false; 2];
            for (ordinal, arm) in authored.iter().enumerate() {
                let source_arm = dispatch.arms.handle_at(u32::try_from(ordinal).ok()?);
                let mut equality_use = arena::Handle::invalid();
                let pattern = if let Some(subject_type) = subject_type {
                    match arm.pattern {
                        MatchPattern::Wildcard => {
                            covered = true;
                            checked_trees::CheckedScalarDispatchPattern::Wildcard
                        }
                        MatchPattern::Value(pattern) => {
                            if subject_type == PrimitiveType::Bool
                                && let Some(value) =
                                    self.program.expression_table.boolean_node(pattern)
                            {
                                if boolean_coverage[usize::from(*value)] {
                                    continue;
                                }
                                boolean_coverage[usize::from(*value)] = true;
                                covered = boolean_coverage == [true, true];
                            }
                            if matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64) {
                                equality_use = self.comparison_use(
                                    expression,
                                    checked_trees::CheckedOperatorOccurrence::MatchEquality {
                                        source_arm,
                                    },
                                )?;
                            }
                            let computation = self.expression(pattern, subject_type)?;
                            self.plans.nodes.get_mut(computation).authored_root = pattern;
                            self.push_scalar_root(
                                CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                                computation,
                            )?;
                            checked_trees::CheckedScalarDispatchPattern::Value(computation)
                        }
                        MatchPattern::Destructure(_) => return None,
                    }
                } else {
                    self.structural_dispatch_pattern(expression, &dispatch, &subject, arm.pattern)?
                };
                if let Some(computation) = pattern.computation()
                    && subject_type.is_none()
                {
                    self.push_scalar_root(
                        CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                        computation,
                    )?;
                }
                let value = self.structural_value(arm.value, expected)?;
                arms.push(CheckedStructuralDispatchArm {
                    source_arm,
                    equality_use,
                    pattern,
                    value,
                });
                if matches!(arm.pattern, MatchPattern::Wildcard) || covered {
                    break;
                }
            }
            // Structural coverage was established by the common pattern
            // checker, including case completeness and guarded-arm limits.
            if (!covered && subject_type.is_some()) || arms.is_empty() {
                return None;
            }
            CheckedStructuralValueKind::Dispatch {
                subject,
                arms: self.structural_values.dispatch_arms.insert_many(arms),
            }
        };
        self.publish_structural_value(expression, expected, kind)
    }

    pub(super) fn publish_structural_value(
        &mut self,
        expression: ExpressionHandle,
        type_reference: TypeReferenceHandle,
        kind: CheckedStructuralValueKind,
    ) -> Option<CheckedStructuralValueHandle> {
        Some(self.structural_values.nodes.append(CheckedStructuralValue {
            machine: self.machine,
            state: self.state,
            statement_index: u32::try_from(self.statement_index).ok()?,
            expression,
            source_span: self.program.expression_table.source_span(expression),
            type_reference,
            qualification_decisions: Default::default(),
            content_transformation: Default::default(),
            content_inputs: Default::default(),
            kind,
        }))
    }
}
