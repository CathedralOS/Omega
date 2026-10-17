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
pub(super) use value_predicates::{
    is_fixed_array_value, is_record_value, is_structural_case_value,
};

impl Builder<'_, '_> {
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
    )> {
        let node = self.structural_values.nodes.get(value);
        let (owner, path) = match &node.kind {
            // A selected subtree retains its containing owner. A whole-value
            // cast is itself the occurrence that owns checked memberships;
            // scheduling only its operand would discard those judgments.
            CheckedStructuralValueKind::Projection { source, path, .. } if !path.is_empty() => {
                (*source, path.clone())
            }
            _ => (value, Vec::new()),
        };
        let expression = self.structural_values.nodes.get(owner).expression;
        let owner_reference = if owner == value {
            reference
        } else {
            let place = validation::canonical_place_from_expression_in_state(
                self.program,
                self.state,
                self.statement_index,
                expression,
            )?;
            if !place.segments.is_empty() {
                return None;
            }
            validation::canonical_place_type_reference(
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
            TypeReferenceNode::Reference { .. }
                if validation::reference_result_custody::stored_loan_parts(
                    self.program,
                    expected,
                )
                .is_some()
                    && validation::reference_formation_source(self.program, expression)
                        .is_some_and(|formation| formation.view_type.is_valid()) =>
            {
                let machine = self.program.machine_by_symbol(self.machine)?;
                let state = self.program.state_by_symbol(self.state)?;
                let actual = crate::values::reference_sources::formed_view_type(
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
                // An established view keeps its descriptor and loan route.
                // An array has backing storage but no view descriptor:
                // lending it as a slice establishes that descriptor at the
                // argument occurrence, through the same value planner as
                // a local initializer. Calls and state arrivals consume
                // that checked formation rather than inventing a carrier
                // from representation compatibility alone.
                if crate::semantic::calls::view_argument_names_existing_place(
                    self.program,
                    expression,
                ) && !validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state,
                    self.statement_index,
                    expression,
                )
                .and_then(|place| {
                    validation::canonical_place_type_reference(
                        self.program,
                        self.state,
                        self.statement_index,
                        &place,
                    )
                })
                .and_then(|reference| validation::unwrapped_type_reference(self.program, reference))
                .is_some_and(|reference| {
                    matches!(
                        self.program.type_reference_table.type_reference(reference),
                        TypeReferenceNode::FixedArray { .. }
                    )
                }) {
                    return None;
                }
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
                validation::checked_argument_matches_type_reference(
                    self.program,
                    expression,
                    expected,
                )
                .then_some(())?;
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
        let already_retained = retained.is_some();
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
        // Retain the complete producer as well as the selected value. A loan
        // of a field or element must establish its owner's home, never move
        // that child into a detached argument temporary. Scalar calls and
        // statement calls consume the same owner/path relationship.
        if let CheckedStructuralValueKind::Projection {
            source, ref path, ..
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
        if !already_retained {
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
        }
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
        let kind = if let Some(primitive) = self.program.primitive_type_reference(expected) {
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
                    let root_reference = validation::canonical_place_type_reference(
                        self.program,
                        self.state,
                        self.statement_index,
                        &validation::CanonicalPlace {
                            root: place.root,
                            segments: Vec::new(),
                        },
                    )?;
                    source_value = self.structural_value(root, root_reference)?;
                }
                for segment in place.segments {
                    if let facts::PlaceSegment::Index { expression } = segment {
                        selectors.push((expression, self.index_selector(expression)?));
                    }
                }
            }
            let selector = |expression| {
                selectors.iter().find_map(|(selector, value)| {
                    (*selector == expression).then_some(
                        checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                    )
                })
            };
            let (source, storage, backing_type) =
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
            // The typed path can fold fixed selectors. Retain only the
            // remaining executable operands, in their formation order.
            for (ordinal, value) in source.computation_operands().enumerate() {
                self.push_scalar_root(
                    CheckedScalarExpressionRole::StructuralValueSelector {
                        expression,
                        selector_ordinal: u32::try_from(ordinal).ok()?,
                    },
                    value,
                )?;
            }
            CheckedStructuralValueKind::Reference {
                source,
                storage,
                backing_type,
                source_value,
            }
        } else if let Some(copied) = self.copied_place(expression, expected) {
            // Reading an already stored carrier copies that value, including
            // a shared view's captured extent. Its backing loan cannot replace
            // the descriptor with a newly formed whole-collection view.
            // Locals and owned parameters share this decision: affine
            // descriptor retirement does not forbid shared-carrier copying.
            copied
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
            let call_target = validation::call_target_symbol(self.program, call);
            let returned = validation::call_target_return_type(self.program, call_target)?;
            // Result storage does not depend on the source of executable
            // supply. The ordinary call planner retains the exact boundary,
            // body or dynamic route and its independent completion custody.
            if !value_predicates::structural_call_carrier_matches(self.program, returned, expected)
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
            ExpressionNode::Name(_)
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
            if self
                .program
                .normalized_type_identity(constructor.type_reference)
                != self.program.normalized_type_identity(expected)
            {
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
            let machine = self.program.machine_by_symbol(self.machine)?;
            let state = self.program.state_in_machine(machine, self.state)?;
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
            self.push_scalar_root(
                CheckedScalarExpressionRole::StructuralValueSubject { expression },
                subject,
            )?;
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
                        self.push_scalar_root(
                            CheckedScalarExpressionRole::StructuralValuePattern { source_arm },
                            computation,
                        )?;
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
            kind,
        }))
    }
}
