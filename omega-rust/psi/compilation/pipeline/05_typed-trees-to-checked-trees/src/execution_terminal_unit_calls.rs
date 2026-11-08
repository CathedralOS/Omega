/*
    Structural call closure, argument custody, and claim transfer.

    `call_operations.rs` builds the call operation, `structural_arguments.rs` builds the arguments and their claim transfers,
    `argument_paths.rs` the projected paths, `parameter_argument_paths.rs` which
    path a parameter-rooted argument lends, `argument_presentation.rs` owns
    shared carrier compatibility, `boundary_admission.rs` decides
    what a boundary call admits, `signatures.rs` the signatures it is checked
    against and `affine_locals.rs` the unit's opening affine locals; the
    remaining modules cover view subslices (one admission for every site that
    narrows a byte or element view), computation arguments and reference
    forwarding. `dynamic_call.rs` plans a selector call against its
    instantiated slot through the same argument planners.
*/

mod affine_locals {
    /*
        The affine array construction prefix.
    */
    use crate::execution::permission_events;
    use crate::execution::terminal_unit::types::{
        ShapeCollector, parameter_qualifications, type_graph_requires_nominal_drop,
    };
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedAffineConstructionElementPlan, CheckedTrivialAffineStructuralLocalPlan,
        CheckedUnitStructuralTypeShape, Multiplicity, PermissionAccess, PermissionClaimIdentity,
        PermissionEventKind, PermissionEventSource, StatementNode, SymbolHandle, TypeReferenceNode,
    };
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use std::collections::BTreeSet;

    pub(crate) fn build_affine_array_construction_prefix(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        binders: &[(SymbolHandle, String)],
        statements: &[StatementNode],
    ) -> Option<(
        Vec<(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)>,
        usize,
    )> {
        let [StatementNode::LocalData(local), assignments @ ..] = statements else {
            return None;
        };
        let root_length = assignments
            .len()
            .checked_add(1)
            .filter(|length| (3..=26).contains(length))?;
        if !local.is_mutable
            || local.initial_value.is_valid()
            || crate::checks::type_multiplicity(program, local.type_reference)
                != Multiplicity::Affine
            || !parameter_qualifications(program, shapes, local.type_reference, binders)?.is_empty()
            || type_graph_requires_nominal_drop(
                program,
                local.type_reference,
                &[],
                &mut BTreeSet::new(),
            )
        {
            return None;
        }
        let TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(actual_length),
        } = program
            .type_reference_table
            .type_reference(local.type_reference)
        else {
            return None;
        };
        if *actual_length != root_length {
            return None;
        }
        if crate::checks::type_multiplicity(program, *element_type) != Multiplicity::Affine
            || type_graph_requires_nominal_drop(program, *element_type, &[], &mut BTreeSet::new())
        {
            return None;
        }
        let TypeReferenceNode::Named {
            symbol: element_symbol,
            ..
        } = program.type_reference_table.type_reference(*element_type)
        else {
            return None;
        };
        let root_type_identity =
            shapes.add_affine_array_construction_type(local.type_reference, binders)?;
        let element_type_identity = shapes.add_type(*element_type, binders, &[])?;
        let root_shape = shapes.types.get(&root_type_identity)?;
        if !matches!(
            &root_shape.shape,
            CheckedUnitStructuralTypeShape::FixedArray {
                element_type_identity: element,
                length,
            } if element == &element_type_identity
                && length
                    .literal()
                    .is_some_and(|length| usize::try_from(length) == Ok(root_length))
        ) || !matches!(
            shapes.types.get(&element_type_identity).map(|shape| &shape.shape),
            Some(CheckedUnitStructuralTypeShape::Record { fields, .. }) if fields.is_empty()
        ) {
            return None;
        }

        let mut rows = Vec::with_capacity(assignments.len());
        for (expected_index, statement) in assignments.iter().enumerate() {
            let assignment = statement.as_assignment()?;
            let target = place_at(program, state.symbol, expected_index + 1, assignment.target)?;
            if target.root != facts::PlaceRoot::Symbol(local.symbol)
                || target.segments.as_slice()
                    != [facts::PlaceSegment::FixedIndex {
                        index: expected_index,
                    }]
            {
                return None;
            }
            let literal = expressions(program).struct_literal_node(assignment.value)?;
            if literal.case_name.is_some()
                || !expressions(program)
                    .struct_fields(literal.fields)
                    .is_empty()
            {
                return None;
            }
            if literal.type_symbol != *element_symbol {
                return None;
            }
            rows.push((
                CheckedTrivialAffineStructuralLocalPlan {
                    declaration_ordinal: u32::try_from(expected_index).ok()?,
                    type_identity: element_type_identity,
                    construction: Some(CheckedAffineConstructionElementPlan {
                        root_type_identity,
                        index: u64::try_from(expected_index).ok()?,
                    }),
                },
                local.symbol,
            ));
        }
        let declaration_provenance = language_semantics::PermissionProvenance::Established {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            source: PermissionEventSource::Statement { statement_index: 0 },
        };
        let exit_events = permission_events(facts)
            .filter(|(_, event)| {
                event.machine_symbol == machine.symbol
                    && event.state_symbol == state.symbol
                    && event.source == PermissionEventSource::StateExit
                    && event.kind == PermissionEventKind::AffineDrop
                    && event.multiplicity == Multiplicity::Affine
                    && event.access == PermissionAccess::Owned
                    && event.claim_identity == PermissionClaimIdentity::Unknown
                    && event.provenance == declaration_provenance
                    && event.root == facts::PlaceRoot::Symbol(local.symbol)
                    && !event.obligation_live
            })
            .map(|(_, event)| event)
            .collect::<Vec<_>>();
        if !matches!(exit_events.as_slice(), [event]
            if facts.flow.ownership.segments.span_or_empty(event.segments).is_empty())
        {
            return None;
        }
        Some((rows, 1))
    }
}
mod argument_paths {

    /*
        Checked scalar arguments and projected argument paths: field and index
        paths and exactly retained argument carriers.
    */
    use super::super::types::parameter_is_root;
    use super::boundary_admission::fixed_byte_array_view_is_admitted;
    use crate::execution::terminal_unit::types::byte_sequence_carrier;
    use crate::execution::terminal_unit::types::{
        base_type_identity, byte_sequence_type_identity, dense_scalar_parameter_index,
        structural_access_for_type_reference, terminal_field_identity,
    };
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedScalarExpressionRole as ExpressionRole,
        CheckedStructuralAccess as StructuralAccess, CheckedStructuralScalarParameterPlan,
        CheckedUnitCallCoordinate as CallCoordinate,
        CheckedUnitStructuralArgumentPlan as ArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment as PathSegment, Multiplicity, StatementNode, SymbolHandle,
        TypeReferenceHandle, TypeReferenceNode,
    };
    use crate::flow::{expressions, state_statements};
    use crate::values::lower_unit_scalar_argument;
    use crate::values::scalar_expression_type;
    use checked_trees::CheckedByteSequenceCarrier;
    use checked_trees::CheckedCallScalarArgument;
    use checked_trees::CheckedErasedProofParameterPlan;
    use checked_trees::CheckedMemberIdentity;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedProofTerm;
    use checked_trees::CheckedProofTermRole;
    use checked_trees::CheckedRuntimeIndex;
    use checked_trees::CheckedScalarExpression;
    use checked_trees::CheckedStructuralParameterField;
    use checked_trees::CheckedStructuralPredicatePathSegment;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::ReferenceAccess;
    use typed_trees::data::DataMember;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::state_parameter_position;
    use typed_trees::state::State;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::FixedArrayLength;
    use typed_trees::types::PrimitiveType;
    use validation::CanonicalPlace;
    use validation::canonical_place_from_expression;
    use validation::canonical_place_type_reference;
    use validation::expression_type_reference_in_state;
    use validation::field_domains::domain_constraint_symbols;
    use validation::field_domains::string_literal_expression_grants_domain;
    use validation::reference_shell;
    use validation::stored_structural_type_identity;
    use validation::stores_reference_shape;
    use validation::structural_storage_type;
    use validation::unwrapped_type_reference;

    pub(crate) fn checked_call_scalar_arguments(
        facts: &CheckFacts,
        caller_state: SymbolHandle,
        coordinate: CallCoordinate,
        parameters: &[CheckedStructuralScalarParameterPlan],
        boundary: bool,
    ) -> Option<Vec<CheckedCallScalarArgument>> {
        parameters
            .iter()
            .enumerate()
            .map(|(argument_ordinal, parameter)| {
                let role = if boundary {
                    ExpressionRole::BoundaryCallArgument {
                        call_ordinal: coordinate.call_ordinal,
                        argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                    }
                } else {
                    ExpressionRole::UnitCallArgument {
                        call_ordinal: coordinate.call_ordinal,
                        argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                    }
                };
                let computations = &facts.values.scalar_computations;
                let roots = computations
                    .roots
                    .iter()
                    .filter(|(_, root)| {
                        root.state == caller_state
                            && root.statement_ordinal == coordinate.statement_index
                            && root.role == role
                    })
                    .map(|(_, root)| root)
                    .collect::<Vec<_>>();
                if let [root] = roots.as_slice() {
                    if !computations.nodes.is_valid(root.root)
                        || computations.nodes.get(root.root).primitive_type
                            != parameter.primitive_type
                        || facts
                            .values
                            .scalar_expressions
                            .expressions
                            .iter()
                            .any(|expression| {
                                expression.state == caller_state
                                    && expression.statement_ordinal == coordinate.statement_index
                                    && expression.role == role
                            })
                    {
                        return None;
                    }
                    return Some(CheckedCallScalarArgument::Computation(root.root));
                }
                if !roots.is_empty() {
                    return None;
                }
                let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                    caller_state,
                    coordinate.statement_index,
                    role,
                )?;
                (scalar_expression_type(expression)? == parameter.primitive_type)
                    .then(|| CheckedCallScalarArgument::Pure(expression.clone()))
            })
            .collect()
    }

    /*
        Proof-only erased actuals of an in-module Unit call: the retained proof
        terms recorded under `ErasedUnitCallArgument`, in the callee's dense
        erased proof-formal order. Each term's constructed (or forwarded) data
        identity must match the formal's canonical `Nat`-style identity.
    */
    pub(crate) fn checked_call_erased_proof_arguments(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_state: SymbolHandle,
        coordinate: CallCoordinate,
        parameters: &[CheckedErasedProofParameterPlan],
    ) -> Option<Vec<CheckedProofTerm>> {
        parameters
            .iter()
            .enumerate()
            .map(|(erased_ordinal, parameter)| {
                let role = CheckedProofTermRole::ErasedUnitCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    erased_ordinal: u32::try_from(erased_ordinal).ok()?,
                };
                let term = facts
                    .values
                    .proof_terms
                    .term_at(caller_state, coordinate.statement_index, role)
                    .or_else(|| {
                        facts.values.proof_terms.term_at(
                            caller_state,
                            coordinate.statement_index,
                            CheckedProofTermRole::TransitionArgument {
                                argument_ordinal: parameter.source_position,
                            },
                        )
                    })?;
                (proof_term_type_matches(program, caller_state, term, parameter.type_identity)
                    || instantiated_formal_matches(
                        program,
                        caller_state,
                        coordinate,
                        parameter,
                        term,
                    ))
                .then(|| term.clone())
            })
            .collect()
    }

    /*
        The construction or forwarded formal carries exactly the formal's declared
        proof-only data identity.
    */
    fn formal_proof_identity(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        if validation::mathematical_applications::is_source_evidence_carrier(
            program,
            type_reference,
        ) {
            Some(program.normalized_type_identity(type_reference))
        } else {
            base_type_identity(program, type_reference, &[], &[])
        }
    }

    /*
        The callee formal still names its cloned value parameters. The call's
        static arguments are those parameters, so the carriers match only after
        that instantiation. A different argument stays a different type.
    */
    fn instantiated_formal_matches(
        program: &dyn ::validation::CheckingInput,
        caller_state: SymbolHandle,
        coordinate: CallCoordinate,
        callee_plan: &CheckedErasedProofParameterPlan,
        term: &CheckedProofTerm,
    ) -> bool {
        let CheckedProofTerm::Formal { parameter_symbol } = term else {
            return false;
        };
        let Some(caller) = program.machine_holding_state(caller_state) else {
            return false;
        };
        let Some(caller_parameter) = program.state_by_symbol(caller_state).and_then(|state| {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == *parameter_symbol)
        }) else {
            return false;
        };
        let Ok(statement_index) = usize::try_from(coordinate.statement_index) else {
            return false;
        };
        let Ok(call_ordinal) = usize::try_from(coordinate.call_ordinal) else {
            return false;
        };
        let Some(site) = validation::find_call_site(
            program,
            caller.symbol,
            caller_state,
            statement_index,
            call_ordinal,
        ) else {
            return false;
        };
        let direct = match site {
            validation::CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
            validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
            validation::CallSite::TransitionNamed { .. } => &[][..],
        };
        let authored = site.authored_application();
        let authored_arguments = authored.is_valid().then(|| {
            program
                .expression_table
                .static_machine_application_arguments(authored)
        });
        let arguments = if direct.is_empty() {
            authored_arguments.unwrap_or(direct)
        } else {
            direct
        };
        let target = site.target_symbol(program);
        let Some(callee) = program
            .machine_by_symbol(target)
            .or_else(|| program.machine_holding_state(target))
        else {
            return false;
        };
        let Some(callee_parameter) = program.machine_states(callee).iter().find_map(|state| {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == callee_plan.parameter_symbol)
        }) else {
            return false;
        };
        let caller_base = generic_base_symbol(program, caller_parameter.type_reference);
        let callee_base = generic_base_symbol(program, callee_parameter.type_reference);
        if caller_base.is_none() || caller_base != callee_base {
            return false;
        }
        let caller_arguments =
            generic_argument_references(program, caller_parameter.type_reference);
        let callee_arguments =
            generic_argument_references(program, callee_parameter.type_reference);
        let index_symbols = generic_index_symbols(program, callee_parameter.type_reference);
        if caller_arguments.is_empty()
            || caller_arguments.len() != callee_arguments.len()
            || index_symbols.len() != arguments.len()
            || caller_arguments.len() != arguments.len()
        {
            return false;
        }
        let substitutions = index_symbols
            .iter()
            .zip(arguments)
            .map(|(symbol, argument)| {
                (
                    *symbol,
                    typed_trees::type_identity::IndexIdentityArgument::StaticArgument(argument),
                )
            })
            .collect::<Vec<_>>();
        let trees = program.typed_program();
        caller_arguments
            .iter()
            .zip(callee_arguments)
            .all(|(caller_argument, callee_argument)| {
                let actual = typed_trees::type_identity::normalized_index_identity(
                    trees,
                    typed_trees::type_identity::IndexIdentityArgument::TypeReference(
                        *caller_argument,
                    ),
                    &[],
                );
                let expected = typed_trees::type_identity::normalized_index_identity(
                    trees,
                    typed_trees::type_identity::IndexIdentityArgument::TypeReference(
                        callee_argument,
                    ),
                    &substitutions,
                );
                actual.is_some() && actual == expected
            })
    }

    fn generic_base_symbol(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Generic { base_symbol, .. } if base_symbol.is_valid() => {
                Some(*base_symbol)
            }
            _ => None,
        }
    }

    fn generic_argument_references(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<TypeReferenceHandle> {
        let TypeReferenceNode::Generic { arguments, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return Vec::new();
        };
        program
            .type_reference_table
            .type_reference_handles(*arguments)
            .to_vec()
    }

    fn generic_index_symbols(
        program: &dyn ::validation::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<SymbolHandle> {
        let TypeReferenceNode::Generic { arguments, .. } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return Vec::new();
        };
        program
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .filter_map(
                |argument| match program.type_reference_table.type_reference(*argument) {
                    TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => Some(*symbol),
                    TypeReferenceNode::ConstExpression(expression) => {
                        match program.expression_table.expression(*expression) {
                            typed_trees::expression::ExpressionNode::Name(path)
                                if path.symbol.is_valid() =>
                            {
                                Some(path.symbol)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                },
            )
            .collect()
    }

    fn proof_term_type_matches(
        program: &dyn ::validation::CheckingInput,
        caller_state: SymbolHandle,
        term: &CheckedProofTerm,
        type_identity: TypeIdentityId,
    ) -> bool {
        match term {
            CheckedProofTerm::Mathematical {
                type_identity: actual,
                ..
            }
            | CheckedProofTerm::MathematicalApplication {
                type_identity: actual,
                ..
            }
            | CheckedProofTerm::Construction {
                type_identity: actual,
                ..
            }
            | CheckedProofTerm::Array {
                type_identity: actual,
                ..
            } => *actual == type_identity,
            CheckedProofTerm::Projection {
                type_identity: actual,
                ..
            } => *actual == type_identity,
            CheckedProofTerm::Formal { parameter_symbol } => {
                program.state_by_symbol(caller_state).is_some_and(|state| {
                    program.state_parameters(state).iter().any(|parameter| {
                        parameter.symbol == *parameter_symbol
                            && formal_proof_identity(program, parameter.type_reference)
                                == Some(type_identity)
                    })
                })
            }
            CheckedProofTerm::Scalar(_) => false,
        }
    }

    /*
        Proof-only erased actuals of an in-module Unit call: the retained pure
        expressions recorded under `ErasedUnitCallArgument`, in the callee's dense
        erased-formal order.
    */
    pub(crate) fn checked_call_erased_scalar_arguments(
        facts: &CheckFacts,
        caller_state: SymbolHandle,
        coordinate: CallCoordinate,
        parameters: &[CheckedStructuralScalarParameterPlan],
    ) -> Option<Vec<CheckedCallScalarArgument>> {
        parameters
            .iter()
            .enumerate()
            .map(|(erased_ordinal, parameter)| {
                let role = ExpressionRole::ErasedUnitCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    erased_ordinal: u32::try_from(erased_ordinal).ok()?,
                };
                let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                    caller_state,
                    coordinate.statement_index,
                    role,
                )?;
                (scalar_expression_type(expression)? == parameter.primitive_type)
                    .then(|| CheckedCallScalarArgument::Pure(expression.clone()))
            })
            .collect()
    }

    /*
        A borrowed value keeps its declared promises when its pointer is presented
        as runtime storage. Keep that complete type beside the physical endpoint
        and path; removing an ingress reference must not erase its qualifications.
    */
    pub(crate) struct ProjectedArgumentPath {
        pub(crate) declared_type: TypeReferenceHandle,
        pub(crate) storage_type: TypeReferenceHandle,
        pub(crate) path: Vec<PathSegment>,
    }

    /*
        `projected_argument_path` with runtime-index admission for the borrowed
        lanes — a `&`-rooted call receiver or argument, or a `self.nodes[i]`-shape
        copied leaf. A runtime `Index` segment retains `RuntimeIndex` naming the
        checked selector only while the selector resolves to an immutable integer
        operand of the calling state: its dense scalar parameter position, or an
        integer field read below a structural parameter. Terminal re-proves
        `0 <= index < extent` for the segment's obligation; the segment itself
        carries no interval. Every other caller keeps rejecting runtime indexes
        through `projected_argument_path`.
    */
    pub(crate) fn projected_borrowed_path(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
    ) -> Option<ProjectedArgumentPath> {
        projected_argument_path(
            program,
            state_symbol,
            statement_index,
            place,
            Some(machine),
            None,
        )
    }

    /*
        Select the lent endpoint, retaining each stored-pointer crossing beside its
        type. Call operands use this after ordinary geometry resolution regardless
        of whether their home came from a parameter, local or completed producer.
        View descriptors already name their carriers and never gain a pointer hop.
    */
    pub(crate) fn borrowed_argument_endpoint(
        program: &dyn ::validation::CheckingInput,
        mut selected: TypeReferenceHandle,
        mut path: Vec<PathSegment>,
        formal: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
        let Some(requested) = reference_shell(program, formal) else {
            return Some((selected, path));
        };
        let requested_storage =
            stored_structural_type_identity(program, requested.referee, &[], &[]);
        if !requested_storage.is_valid() {
            return None;
        }
        for _ in 0..=program.type_reference_table.type_reference_count() {
            let selected_storage = stored_structural_type_identity(program, selected, &[], &[]);
            if !selected_storage.is_valid() {
                return None;
            }
            if selected_storage == requested_storage {
                return Some((selected, path));
            }
            let Some(shell) = reference_shell(program, selected)
                .filter(|_| stores_reference_shape(program, selected, &[]))
            else {
                return Some((selected, path));
            };
            if !shell.access.is_readable()
                || (shell.access != requested.access && shell.access != ReferenceAccess::Mutable)
            {
                return None;
            }
            path.push(PathSegment::Referent);
            selected = shell.referee;
        }
        None
    }

    /*
        `projected_borrowed_path` for an observation of a runtime-selected
        element, whose selector may also be an immutable scalar local:
        `local_selector` resolves one selector expression to its `Local` position
        in the observing computation's own scalar namespace. Only the observation
        owner holds that namespace. The path ends at the place itself, so a
        reference-typed place names its descriptor: a byte operand observes the
        view (base and extent), not the bytes behind it.
    */
    pub(crate) fn projected_selected_path(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        local_selector: &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<CheckedRuntimeIndex>,
    ) -> Option<ProjectedArgumentPath> {
        projected_argument_path(
            program,
            state_symbol,
            statement_index,
            place,
            Some(machine),
            Some(&|expression| local_selector(expression).map(PathSegment::RuntimeIndex)),
        )
    }

    /// Physical crossings before a projection enters a stored root's contents.
    /// A parameter's referent presentation is a different coordinate and does not
    /// call this operation. The caller retains its own access and endpoint debt.
    pub(crate) fn stored_reference_prefix(
        program: &dyn ::validation::CheckingInput,
        mut reference: TypeReferenceHandle,
        mut can_cross: impl FnMut(ReferenceAccess) -> bool,
    ) -> Option<Vec<PathSegment>> {
        let mut prefix = Vec::new();
        let mut remaining = program.type_reference_table.type_reference_count();
        while let Some(shell) = reference_shell(program, reference) {
            if !can_cross(shell.access) {
                return None;
            }
            if !stores_reference_shape(program, reference, &[]) {
                break;
            }
            remaining = remaining.checked_sub(1)?;
            prefix.push(PathSegment::Referent);
            reference = shell.referee;
        }
        Some(prefix)
    }

    /// `projected_selected_path` for a case observation, which reads the case
    /// value's storage rather than a descriptor. A retained local descriptor is
    /// crossed here; a borrowed parameter already denotes its referee's storage.
    /// A view is itself the indexed carrier, not a Reference to that carrier.
    pub(crate) fn projected_observed_path(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        local_selector: &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<CheckedRuntimeIndex>,
    ) -> Option<ProjectedArgumentPath> {
        let ProjectedArgumentPath {
            mut declared_type,
            storage_type: mut projected,
            mut path,
        } = projected_selected_path(
            program,
            machine,
            state_symbol,
            statement_index,
            place,
            local_selector,
        )?;
        // Locals and completed values own stored descriptors. Parameters already
        // denote their supplied backing and keep the existing root presentation.
        // This is geometry only; the consumer still requires the completed home
        // and its exact read permission before executing the observation.
        let stored_root = match place.root {
            PlaceRoot::Symbol(root) => program
                .state_by_symbol(state_symbol)
                .and_then(|state| program.state_local_declaration(state, root))
                .filter(|(ordinal, _)| *ordinal < statement_index)
                .map(|(_, local)| local.type_reference),
            PlaceRoot::Expression(_) => Some(canonical_place_type_reference(
                program,
                state_symbol,
                statement_index,
                &CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            )?),
            _ => None,
        };
        if let Some(reference) = stored_root {
            let mut crossings =
                stored_reference_prefix(program, reference, |access| access.is_readable())?;
            crossings.append(&mut path);
            path = crossings;
        }
        while let Some(shell) = reference_shell(program, projected) {
            if shell.access == ReferenceAccess::WriteOnly {
                return None;
            }
            if place
                .segments
                .iter()
                .any(|segment| !matches!(segment, PlaceSegment::Case { .. }))
            {
                path.push(PathSegment::Referent);
            }
            declared_type = projected;
            projected = shell.referee;
        }
        Some(ProjectedArgumentPath {
            declared_type,
            storage_type: projected,
            path,
        })
    }

    /*
        A local loan's leading referent crossings are shells of that local.
        The type past those shells stays on the source. Lowering reads it from
        the plan, including when the local has no established result row.
    */
    pub(crate) fn retain_local_reference_source(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        source: CheckedUnitStructuralArgumentSourcePlan,
        path: &[PathSegment],
    ) -> CheckedUnitStructuralArgumentSourcePlan {
        let CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } = source else {
            return source;
        };
        let Some(state) = program.state_by_symbol(state_symbol) else {
            return CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol };
        };
        let Some((_, local)) = program.state_local_declaration(state, symbol) else {
            return CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol };
        };
        if !validation::dispatch_subject_of_erased_binding(program, state, local) {
            return CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol };
        }
        let mut reference = local.type_reference;
        let mut hops = 0u32;
        for segment in path {
            if !matches!(segment, PathSegment::Referent) {
                break;
            }
            let Some(shell) = reference_shell(program, reference) else {
                break;
            };
            if shell.access == ReferenceAccess::WriteOnly
                || !stores_reference_shape(program, reference, &[])
            {
                break;
            }
            reference = shell.referee;
            hops += 1;
        }
        let root_type_identity = stored_structural_type_identity(program, reference, &[], &[]);
        if hops == 0 || !root_type_identity.is_valid() {
            return CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol };
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
            symbol,
            root_type_identity,
            root_reference_hops: hops,
        }
    }

    /*
        A computation owner supplies executable selectors at the same projected
        storage join. Other callers pass no evaluator and cannot invent a value.
    */
    pub(crate) fn projected_evaluated_path(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        place: &CanonicalPlace,
        selector: Option<&dyn Fn(typed_trees::expression::ExpressionHandle) -> Option<PathSegment>>,
    ) -> Option<ProjectedArgumentPath> {
        projected_argument_path(program, state, statement, place, Some(machine), selector)
    }

    /*
        The segment prefix ending before `position` re-rooted as its own place and
        resolved to its container type — the descend every member, index, window or
        case segment takes to learn the extent it must stay inside.
    */
    fn prefix_container_type_reference(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        position: usize,
    ) -> Option<TypeReferenceHandle> {
        canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &CanonicalPlace {
                root: place.root,
                segments: place.segments[..position].to_vec(),
            },
        )
    }

    pub(crate) fn projected_argument_path(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        runtime_index_machine: Option<SymbolHandle>,
        local_selector: Option<
            &dyn Fn(typed_trees::expression::ExpressionHandle) -> Option<PathSegment>,
        >,
    ) -> Option<ProjectedArgumentPath> {
        let mut path = Vec::with_capacity(place.segments.len());
        let inherited_crossings = {
            let mut reference = canonical_place_type_reference(
                program,
                state_symbol,
                statement_index,
                &CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            )?;
            let mut count = 0usize;
            while let Some(shell) = reference_shell(program, reference)
                && stores_reference_shape(program, reference, &[])
            {
                count += 1;
                reference = shell.referee;
            }
            count
        };
        let prefix_container = |position| {
            prefix_container_type_reference(program, state_symbol, statement_index, place, position)
        };
        let mut consumed_carrier_crossings = inherited_crossings;
        let mut payload_field = None;
        for (position, segment) in place.segments.iter().enumerate() {
            if position > 0
                && matches!(
                    segment,
                    PlaceSegment::Field { .. }
                        | PlaceSegment::Case { .. }
                        | PlaceSegment::FixedIndex { .. }
                        | PlaceSegment::Index { .. }
                )
            {
                let mut reference = prefix_container(position)?;
                let mut crossings = 0usize;
                while let Some(shell) = reference_shell(program, reference)
                    && stores_reference_shape(program, reference, &[])
                {
                    crossings += 1;
                    reference = shell.referee;
                }
                for _ in 0..crossings.saturating_sub(consumed_carrier_crossings) {
                    path.push(PathSegment::Referent);
                }
                consumed_carrier_crossings = crossings;
            }
            if !matches!(segment, PlaceSegment::Case { .. }) {
                consumed_carrier_crossings = 0;
            }
            match segment {
                PlaceSegment::Field { symbol } => {
                    let identity = match payload_field.take() {
                        Some((expected, identity)) if expected == *symbol => identity,
                        Some(_) => return None,
                        None => terminal_field_identity(program, *symbol)?,
                    };
                    path.push(PathSegment::Field(identity));
                }
                PlaceSegment::FixedIndex { index } => {
                    let container = prefix_container(position)?;
                    fixed_index_within_extent(program, container, *index)?;
                    path.push(PathSegment::FixedIndex(u64::try_from(*index).ok()?));
                }
                PlaceSegment::Index { expression } => {
                    if let Some(index) = constant_local_index(program, state_symbol, *expression) {
                        let container = prefix_container(position)?;
                        fixed_index_within_extent(program, container, index)?;
                        path.push(PathSegment::FixedIndex(u64::try_from(index).ok()?));
                        continue;
                    }
                    runtime_index_machine?;
                    let runtime_index = local_selector
                        .and_then(|resolve| resolve(*expression))
                        .or_else(|| {
                            runtime_index_parameter_position(program, state_symbol, *expression)
                                .map(|position| CheckedRuntimeIndex::Parameter { position })
                                .or_else(|| {
                                    runtime_index_field_selector(
                                        program,
                                        state_symbol,
                                        statement_index,
                                        *expression,
                                    )
                                })
                                .map(PathSegment::RuntimeIndex)
                        })?;
                    path.push(runtime_index);
                }
                PlaceSegment::OpaqueRange => return None,
                PlaceSegment::FixedRange { start, end } => {
                    let container = prefix_container(position)?;
                    let length = fixed_array_literal_length(program, container)?;
                    if start > end || *end > length {
                        return None;
                    }
                    path.push(PathSegment::FixedRange {
                        start: u64::try_from(*start).ok()?,
                        end: u64::try_from(*end).ok()?,
                    });
                }
                PlaceSegment::Case { variant } => {
                    let reference = prefix_container(position)?;
                    let reference = unwrapped_type_reference(program, reference)?;
                    let owner = program.closed_data_definition(reference)?;
                    let mut cases =
                        program
                            .data_members(owner)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Variant(case)
                                    if variant.is_valid() && case.symbol == *variant =>
                                {
                                    Some(case)
                                }
                                _ => None,
                            });
                    let case = cases.next()?;
                    let Some(PlaceSegment::Field { symbol }) = place.segments.get(position + 1)
                    else {
                        return None;
                    };
                    let mut fields = program.data_payload_fields(case).iter().filter(|field| {
                        symbol.is_valid() && field.symbol == *symbol && !field.relevance.is_erased()
                    });
                    let field = fields.next()?;
                    if cases.next().is_some() || fields.next().is_some() {
                        return None;
                    }
                    path.push(PathSegment::Case(CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    )));
                    payload_field = Some((
                        *symbol,
                        CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                    ));
                }
            }
        }
        let is_range = matches!(place.segments.last(), Some(PlaceSegment::FixedRange { .. }));
        let declared_type = if is_range {
            prefix_container(place.segments.len().checked_sub(1)?)?
        } else {
            canonical_place_type_reference(program, state_symbol, statement_index, place)?
        };
        let mut projected = if is_range {
            unwrapped_type_reference(program, declared_type)?
        } else {
            declared_type
        };
        if place.segments.is_empty()
            && stores_reference_shape(program, projected, &[])
            && let Some(symbol) = place.root.as_symbol()
            && let Some(state) = program.state_by_symbol(state_symbol)
            && program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter_is_root(parameter, symbol))
        {
            projected = structural_storage_type(program, projected, &[]);
            if !projected.is_valid() {
                return None;
            }
        }
        Some(ProjectedArgumentPath {
            declared_type,
            storage_type: projected,
            path,
        })
    }

    /*
        The checked segment a borrow-carried `Index` selector resolves to. An
        erased loan alias injects its captured `Index` segment into a consumer's
        walk, but that selector was never an authored target coordinate — no
        `AssignmentIndex` exists for it — so it resolves here exactly the way
        the same selector resolves in an argument path: a constant local folds
        to `FixedIndex`, a bare parameter or field selector mints the runtime
        coordinate Terminal re-proves against the container's extent.
    */
    pub(crate) fn borrowed_index_segment(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<PathSegment> {
        if let Some(index) = constant_local_index(program, state, expression) {
            return Some(PathSegment::FixedIndex(u64::try_from(index).ok()?));
        }
        let runtime_index = runtime_index_parameter_position(program, state, expression)
            .map(|position| CheckedRuntimeIndex::Parameter { position })
            .or_else(|| {
                runtime_index_field_selector(program, state, statement_index, expression)
            })?;
        Some(PathSegment::RuntimeIndex(runtime_index))
    }

    /*
        A value projection consumes the same checked scalar operands as other
        values, not an assignment destination's pre-RHS capture slot. Dense
        parameter/local positions and declared field observations retain their
        scalar namespace. A storage source names the established home to read at
        this program point; it never substitutes the declaration's initializer.
    */
    pub(crate) fn value_index_segment(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        state: &State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<PathSegment> {
        let reference =
            expression_type_reference_in_state(program, state.symbol, statement_index, expression)?;
        let primitive = program.primitive_type_reference(reference)?;
        if !primitive.is_fixed_integer() {
            return None;
        }
        let operand = lower_unit_scalar_argument(
            program,
            operators,
            state,
            statement_index,
            expression,
            primitive,
        )?;
        let runtime = match operand {
            CheckedScalarExpression::StorageRead { symbol, .. } => {
                CheckedRuntimeIndex::StorageRead { symbol }
            }
            CheckedScalarExpression::Parameter { position, .. } => CheckedRuntimeIndex::Parameter {
                position: u32::try_from(position).ok()?,
            },
            CheckedScalarExpression::Local { position, .. } => CheckedRuntimeIndex::Local {
                position: u32::try_from(position).ok()?,
            },
            CheckedScalarExpression::StructuralParameterField {
                parameter_position,
                path,
                ..
            } => CheckedRuntimeIndex::StructuralField(CheckedStructuralParameterField {
                parameter_position,
                path,
            }),
            _ => return None,
        };
        Some(PathSegment::RuntimeIndex(runtime))
    }

    /*
        The literal index an immutable local scalar names: `let i: u64 = 2`
        selects exactly element 2 for the whole state because nothing can write
        the binding. Only a bare `Name` selector spelled against a state-local
        immutable binding with a constant initializer folds here; every other
        shape stays on the runtime-selector routes.
    */
    fn constant_local_index(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<usize> {
        let name = program.expression_table.name_path_node(expression)?;
        let source_state = program.state_by_symbol(state_symbol)?;
        let local = state_statements(program, source_state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local)
                    if local.symbol == name.symbol && !local.is_mutable =>
                {
                    Some(local)
                }
                _ => None,
            })?;
        usize::try_from(
            expressions(program).constant_integer_value(local.ordinary_value_initializer()?)?,
        )
        .ok()
    }

    /*
        The dense scalar position a runtime-index selector names. The selector
        expression must be one bare direct scalar parameter of the calling state
        -- the coordinate `CheckedScalarExpression::Parameter` and the Terminal
        caller's `parameters` vector share. The segment carries no bound: the
        terminal verifier re-proves `0 <= index < extent` from the facts that
        hold before the operation (the state's published requires, a dominating
        guard), so admission here only resolves the selector.
    */
    fn runtime_index_parameter_position(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<u32> {
        let state = program.state_by_symbol(state_symbol)?;
        let name = program.expression_table.name_path_node(expression)?;
        let parameters = program.state_parameters(state);
        let authored_position = crate::values::parameter_position(program, name, parameters)?;
        let parameter = &parameters[authored_position];
        if parameter.is_mutable
            || !matches!(
                program.primitive_type_reference(parameter.type_reference),
                Some(
                    PrimitiveType::U8
                        | PrimitiveType::U16
                        | PrimitiveType::U32
                        | PrimitiveType::U64
                        | PrimitiveType::I8
                        | PrimitiveType::I16
                        | PrimitiveType::I32
                        | PrimitiveType::I64
                )
            )
        {
            return None;
        }
        dense_scalar_parameter_index(program, parameters, authored_position)
    }

    /*
        The checked coordinate for a runtime-index selector spelled as a field
        read below one structural entry parameter of the calling state —
        `scan.count` in `scan.results[scan.count]`. A field selector's value is
        storage, not an operand: the place must be a pure record-field chain
        rooted at an entry parameter, and its leaf must be an integer primitive —
        the same integer rule the parameter selector enforces. The owning machine
        is no longer consulted: with static machines a selector roots at the
        parameter it names, so the parameter roster answers the question alone.
        Terminal emits the read and re-proves `0 <= value < extent` from the facts
        holding before the call; admission here only resolves the selector's
        subject.
    */
    fn runtime_index_field_selector(
        program: &dyn ::validation::CheckingInput,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<CheckedRuntimeIndex> {
        let place = canonical_place_from_expression(program, expression)?;
        let root = place.root.as_symbol()?;
        let parameters = program.state_parameters(program.state_by_symbol(state_symbol)?);
        let parameter_position = state_parameter_position(parameters, root)?;
        if place.segments.is_empty() {
            return None;
        }
        let mut path = Vec::with_capacity(place.segments.len());
        for segment in &place.segments {
            let PlaceSegment::Field { symbol } = segment else {
                return None;
            };
            path.push(CheckedStructuralPredicatePathSegment::Field(
                terminal_field_identity(program, *symbol)?,
            ));
        }
        let leaf = canonical_place_type_reference(program, state_symbol, statement_index, &place)?;
        if !matches!(
            program.primitive_type_reference(leaf),
            Some(
                PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
            )
        ) {
            return None;
        }
        Some(CheckedRuntimeIndex::StructuralField(
            CheckedStructuralParameterField {
                parameter_position: u32::try_from(parameter_position).ok()?,
                path,
            },
        ))
    }

    /*
        The shared owner under the ordinary rule: the projected type must carry the
        target parameter's exact normalized identity.
        The literal element count of a fixed array reached through any number of
        domain constraints and borrows. A length that is not an authored literal
        bounds no fixed index here.
        `index` selects inside the container: a literal fixed-array length
        bounds it directly here. A slice carries no declared extent — its
        length is a live observation — so the hop stays unproven exactly the
        way an emitted segment's Terminal obligation re-proves it against the
        view's own witness; every other container declines.
    */
    fn fixed_index_within_extent(
        program: &dyn ::validation::CheckingInput,
        container: typed_trees::types::TypeReferenceHandle,
        index: usize,
    ) -> Option<()> {
        match fixed_array_literal_length(program, container) {
            Some(length) => (index < length).then_some(()),
            None => slice_type_reference(program, container).then_some(()),
        }
    }

    /*
        The container is a slice through any number of domain constraints and
        borrows.
    */
    fn slice_type_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        unwrapped_type_reference(program, type_reference).is_some_and(|type_reference| {
            matches!(
                program.type_reference_table.type_reference(type_reference),
                TypeReferenceNode::Slice { .. }
            )
        })
    }

    fn fixed_array_literal_length(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<usize> {
        let type_reference = unwrapped_type_reference(program, type_reference)?;
        let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        Some(*length)
    }

    /*
        A range loan stays a projection of its original fixed-array backing. The
        call owns its lifetime, so no independently live mutable descriptor is
        manufactured. Runtime endpoints need separate extent evidence; this route
        retains only canonical fixed bounds and rechecks the actual array extent.
    */
    pub(crate) fn fixed_byte_array_range_path(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        target: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
        let (PlaceSegment::FixedRange { start, end }, fields) = place.segments.split_last()? else {
            return None;
        };
        byte_array_range_path(
            program,
            state,
            statement_index,
            place,
            fields,
            *start,
            Some(*end),
            target,
        )
    }

    /*
        The open-ended spelling `x[..]`/`x[a..]` reaches the argument walk as a
        runtime `Index` segment: its range end is not an authored constant, so
        place canonicalization cannot pin it. Against a fixed byte array the
        omitted end is the collection's literal extent, which this route resolves
        from the backing type; a start that is present must still be an authored
        constant. An end that survived canonicalization names a dynamic bound and
        is not a fixed range.
    */
    pub(crate) fn open_byte_range_path(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        target: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
        let (PlaceSegment::Index { expression }, fields) = place.segments.split_last()? else {
            return None;
        };
        let range = program.expression_table.range_node(*expression)?;
        let start = if range.start.is_valid() {
            usize::try_from(expressions(program).constant_integer_value(range.start)?).ok()?
        } else {
            0
        };
        if range.end.is_valid() {
            return None;
        }
        byte_array_range_path(
            program,
            state,
            statement_index,
            place,
            fields,
            start,
            None,
            target,
        )
    }

    fn byte_array_range_path(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        place: &CanonicalPlace,
        fields: &[PlaceSegment],
        start: usize,
        end: Option<usize>,
        target: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
        if !fields
            .iter()
            .all(|segment| matches!(segment, PlaceSegment::Field { .. }))
        {
            return None;
        }
        let backing = CanonicalPlace {
            root: place.root,
            segments: fields.to_vec(),
        };
        let ProjectedArgumentPath {
            storage_type: backing_type,
            mut path,
            ..
        } = projected_argument_path(program, state, statement_index, &backing, None, None)?;
        let extent = fixed_array_literal_length(program, backing_type)?;
        let end = end.unwrap_or(extent);
        if extent == 0
            || start > end
            || end > extent
            || !fixed_byte_array_view_is_admitted(program, backing_type, target)
        {
            return None;
        }
        path.push(PathSegment::FixedRange {
            start: u64::try_from(start).ok()?,
            end: u64::try_from(end).ok()?,
        });
        Some((backing_type, path))
    }

    pub(crate) fn byte_sequence_literal_argument(
        program: &dyn ::validation::CheckingInput,
        parameter_type: TypeReferenceHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<ArgumentPlan> {
        let bytes = program.expression_table.string_node(expression)?;
        let access = structural_access_for_type_reference(program, parameter_type)?;
        match (byte_sequence_carrier(program, parameter_type, &[])?, access) {
            (CheckedByteSequenceCarrier::BorrowedView { .. }, StructuralAccess::SharedBorrow) => {}
            (CheckedByteSequenceCarrier::BoundedOwned { capacity }, StructuralAccess::Owned)
                if program.type_multiplicity(parameter_type) == Multiplicity::Unrestricted
                    && u64::try_from(bytes.len()).ok()? <= capacity =>
            {
                for symbol in domain_constraint_symbols(program, parameter_type) {
                    let domain = program.domain_definition(symbol)?;
                    if !domain.establishment_routes.is_empty()
                        || domain.semantic_roles != Default::default()
                        || !string_literal_expression_grants_domain(program, expression, symbol)
                    {
                        return None;
                    }
                }
            }
            _ => return None,
        }
        Some(ArgumentPlan {
            projection_evaluations: Default::default(),
            source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
                bytes: bytes.to_vec(),
            },
            path: Vec::new(),
            type_identity: byte_sequence_type_identity(program, parameter_type, &[], &[])?,
            access,
        })
    }
}
pub(crate) use argument_paths::{projected_evaluated_path, value_index_segment};
pub(super) mod argument_presentation {
    /*
        Carrier presentation shared by ordinary calls, boundary calls and state edges.

        This owner decides representation compatibility only. Call admission keeps
        source access, multiplicity, predicate obligations and live-loan custody.
    */
    use super::boundary_admission::fixed_array_slice_view_is_admitted;
    use crate::execution::terminal_unit::SymbolHandle;
    use crate::execution::terminal_unit::types::{
        base_type_identity, byte_sequence_carrier, lent_storage_identity, substituted_formal_type,
    };

    /*
        A representation relation, not authority to lend the selected storage.
        Array views additionally need initialized, unqualified backing and an
        admitted projection at the consuming operation.
    */
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ArgumentPresentation {
        SameIdentity,
        SharedScalarObservation,
        ByteSequence,
        FixedArraySlice,
    }

    /*
        Classify caller storage's presentation at an ordinary or boundary
        parameter. The identity rule is the transitive lane's: the
        projected type carries the parameter's exact normalized identity.

        A bounded byte owner may also supply a borrowed byte-view parameter. The
        call keeps its exact storage place and projection: lowering derives a
        descriptor over that storage's live extent. This is the same representation
        relation for ordinary helpers and boundary requirements, independent of
        whether the call returns a scalar or Unit. Equal element types alone do not
        establish it. Raw fixed arrays instead retain their exact element type and
        literal extent while lending a slice descriptor. Source
        access and predicate custody must independently admit the call, including
        preservation of a mutable owner's invariants. Whole parameters and
        projections share this relation.
    */
    pub(crate) fn classify(
        program: &dyn ::validation::CheckingInput,
        projected_type: typed_trees::types::TypeReferenceHandle,
        parameter_type: typed_trees::types::TypeReferenceHandle,
        target_identity: typed_trees::type_identity::TypeIdentityId,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    ) -> Option<ArgumentPresentation> {
        if validation::stored_structural_type_identity(program, projected_type, &[], &[])
            == target_identity
            || base_type_identity(program, projected_type, &[], &[])
                .is_some_and(|identity| identity == target_identity)
        {
            return Some(ArgumentPresentation::SameIdentity);
        }
        let formal = substituted_formal_type(program, parameter_type, substitutions);
        if let Some(reference) = validation::reference_shell(program, formal)
            && reference.access == language_core::ReferenceAccess::Shared
            && validation::is_plain_scalar_storage(program, reference.referee)
            && let Some(primitive) = program.primitive_type_reference(projected_type)
            && program.primitive_type_reference(reference.referee) == Some(primitive)
            && base_type_identity(program, formal, &[], substitutions) == Some(target_identity)
        {
            return Some(ArgumentPresentation::SharedScalarObservation);
        }
        if fixed_array_slice_view_is_admitted(program, projected_type, formal)
            && base_type_identity(program, formal, &[], substitutions)
                .is_some_and(|identity| identity == target_identity)
        {
            return Some(ArgumentPresentation::FixedArraySlice);
        }
        let admitted = match (
            byte_sequence_carrier(program, projected_type, &[]),
            byte_sequence_carrier(program, formal, substitutions),
        ) {
            (
                None | Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }),
                Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }),
            ) => {
                base_type_identity(program, formal, &[], substitutions)
                    .is_some_and(|identity| identity == target_identity)
                    && lent_storage_identity(program, projected_type, &[]).is_some_and(|identity| {
                        lent_storage_identity(program, formal, substitutions)
                            .is_some_and(|expected| identity == expected)
                    })
            }
            (
                Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }),
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
            ) => true,
            (
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
                Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }),
            ) => {
                shared_view_reference(program, projected_type)
                    && crate::execution::terminal_unit::types::structural_access_for_type_reference(
                        program, formal,
                    ) == Some(checked_trees::CheckedStructuralAccess::SharedBorrow)
            }
            _ => false,
        };
        admitted.then_some(ArgumentPresentation::ByteSequence)
    }

    /*
        Whether a declared field type is a shared `&[u8]` view, through any
        domain constraint shells.
    */
    fn shared_view_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        crate::checks::reference_access(program, type_reference)
            .is_some_and(|access| access == language_core::ReferenceAccess::Shared)
    }
}
mod boundary_admission {
    /*
        Which boundary scalar targets, argument presentations and
        mutable byte-array views a structural call may admit.
    */
    use crate::checks::type_multiplicity;
    use crate::execution::terminal_unit::{
        Multiplicity, PrimitiveType, SymbolHandle, TypeReferenceNode,
    };

    /*
        A boundary requirement call answers to its static namespace: the exact
        declaring trait. Selecting a provider and establishing an entry context
        belong to realization; no parameter, field or value supplies a provider,
        and this query invents no implicit `self` authority.
    */
    pub(crate) fn boundary_trait_receiver_is_namespace(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
        call_site: &validation::CallSite<'_>,
        requirement: SymbolHandle,
    ) -> bool {
        call.has_receiver
            && namespace_receiver_matches(program, state, call, call_site, requirement)
    }

    /*
        A static namespace is the exact declaring trait. Runtime selectors resolve
        their applied ancestor environment through ordinary receiver custody.
    */
    fn namespace_receiver_matches(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
        call_site: &validation::CallSite<'_>,
        requirement: SymbolHandle,
    ) -> bool {
        let Some(definition) = program.trait_definition_by_symbol(call.receiver_symbol) else {
            return false;
        };
        let scope = match call_site {
            validation::CallSite::Expression { call, .. }
                if call.requirement_application.is_valid() =>
            {
                let Some(machine) = program.machine_holding_state(state.symbol) else {
                    return false;
                };
                let Some(scope) =
                    program.applied_requirement_scope(machine.symbol, call.requirement_application)
                else {
                    return false;
                };
                scope
            }
            _ => typed_trees::trait_definition::AppliedTraitScope::abstract_scope(definition),
        };
        definition.is_boundary
            && definition.symbol == requirement
            && scope.declaration.symbol == requirement
            && program
                .closed_requirement_application(call.target_symbol)
                .is_none_or(|application| {
                    program
                        .requirement_application_matches_scope(application.signature.symbol, &scope)
                })
    }

    /*
        Failure-only explanation for an unclaimed boundary occurrence. This uses
        the same admission relation as planning; it never supplies missing custody.
    */
    pub(crate) fn boundary_authority_refusal(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        state: &typed_trees::state::State,
        call: &checked_trees::FlowCallFact,
    ) -> Option<&'static str> {
        let owner = program.traits().iter().find(|owner| {
            owner.is_boundary
                && program
                    .executable_trait_machine_instances(owner)
                    .any(|signature| signature.symbol == call.target_symbol)
        })?;
        let site = validation::find_call_site(
            program,
            machine,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        (!namespace_receiver_matches(program, state, call, &site, owner.symbol))
            .then_some("boundary authority: receiver is not the requirement's declaring namespace")
    }

    /*
        A fixed array lends its initialized extent as the borrowed element view a
        checked-body callee declares. This is the element-typed sibling of the byte
        presentation above: the source keeps its literal backing, the target keeps
        its declared `&[T]`/`&mut [T]` view, and the projected path names the
        storage rather than materializing a view local.
        Lending does not copy the array or its elements. Their multiplicity stays
        with the backing owner; callers independently establish initialized storage,
        qualifications and exact loan access before presenting this carrier.
    */
    pub(crate) fn fixed_array_slice_view_is_admitted(
        program: &dyn ::validation::CheckingInput,
        source: typed_trees::types::TypeReferenceHandle,
        target: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let Some((_, element_type, target_element)) =
            fixed_array_slice_view_parts(program, source, target)
        else {
            return false;
        };
        program.normalized_type_identity(*element_type)
            == program.normalized_type_identity(*target_element)
    }

    /* A fixed byte array lends initialized elements without becoming a bounded owner. */
    pub(crate) fn fixed_byte_array_view_is_admitted(
        program: &dyn ::validation::CheckingInput,
        source: typed_trees::types::TypeReferenceHandle,
        target: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let Some((source, element_type, target_element)) =
            fixed_array_slice_view_parts(program, source, target)
        else {
            return false;
        };
        [*element_type, *target_element].into_iter().all(|element| {
            matches!(
                program.type_reference_table.type_reference(element),
                TypeReferenceNode::Named { .. }
            ) && program.primitive_type_reference(element) == Some(PrimitiveType::U8)
        }) && type_multiplicity(program, source) == Multiplicity::Unrestricted
    }

    /*
        Peels one reference layer off `source` and destructures it as a literal
        fixed array while `target` destructures as a shared-or-mutable reference to
        a slice. Returns the peeled source and both element types when the source/
        target shape is the fixed-array-to-slice view both admissions test.
    */
    fn fixed_array_slice_view_parts(
        program: &dyn ::validation::CheckingInput,
        mut source: typed_trees::types::TypeReferenceHandle,
        target: typed_trees::types::TypeReferenceHandle,
    ) -> Option<(
        typed_trees::types::TypeReferenceHandle,
        &typed_trees::types::TypeReferenceHandle,
        &typed_trees::types::TypeReferenceHandle,
    )> {
        source = validation::peel_declared_constraints(program, source);
        if let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(source)
        {
            source = validation::peel_declared_constraints(program, *referee);
        }
        let TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(_),
        } = program.type_reference_table.type_reference(source)
        else {
            return None;
        };
        let TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::Shared,
            referee,
            ..
        } = program
            .type_reference_table
            .type_reference(validation::peel_declared_constraints(program, target))
        else {
            return None;
        };
        let TypeReferenceNode::Slice {
            element_type: target_element,
        } = program
            .type_reference_table
            .type_reference(validation::peel_declared_constraints(program, *referee))
        else {
            return None;
        };
        Some((source, element_type, target_element))
    }
}
mod call_operations {
    /*
        Building one checked call operation and the value result it is expected
        to produce.
    */
    use crate::execution::terminal_unit::ScalarCalleeClosure;
    use crate::execution::terminal_unit::is_reference;
    use crate::execution::terminal_unit::reference_results::has_borrowed_parts;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::execution::terminal_unit::types::base_type_identity;
    use crate::execution::terminal_unit::types::borrowed_named_view;
    use crate::execution::terminal_unit::types::borrowed_slice_result;
    use crate::execution::terminal_unit::types::erased_proof_parameter_plans;
    use crate::execution::terminal_unit::types::erased_scalar_parameter_plans;
    use crate::execution::terminal_unit::types::substituted_formal_type;
    use crate::values::mutable_scalar_parameter_type;

    use crate::execution::terminal_unit::calls::argument_paths::{
        checked_call_erased_proof_arguments, checked_call_erased_scalar_arguments,
        checked_call_scalar_arguments,
    };
    use crate::execution::terminal_unit::calls::structural_arguments::{
        call_claim_transfers, structural_call_arguments,
    };
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::types::{is_unit, machine_binders};
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedStructuralScalarParameterPlan, CheckedTrivialAffineStructuralLocalPlan,
        CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan as OperationPlan,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralResultBindingPlan, Multiplicity, PrimitiveType, SymbolHandle,
        strips_erased_parameter,
    };
    use checked_trees::{
        CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
        CheckedUnitCallTarget, CheckedUnitCallTransfers as CallTransfers,
    };
    use validation::exact_external_leaf_boundary_requirement;

    pub(in crate::execution) enum ExpectedCallValueResult<'result> {
        Scalar(PrimitiveType),
        Structural(&'result CheckedUnitStructuralResultBindingPlan),
    }

    /*
        Match a completed result carrier independently of whether its selected
        implementation is an ordinary body or a boundary supply. This classifies
        storage and access only: source checking owns the captured input-to-result
        loan relation, and realization must still install an admitted provider.
        A shared slice descriptor is unrestricted; a reference permission carrier
        is affine. Neither result extends the lifetime of its backing storage.
    */
    pub(crate) fn call_value_result_matches(
        program: &dyn ::validation::CheckingInput,
        return_type: typed_trees::types::TypeReferenceHandle,
        expected: &ExpectedCallValueResult<'_>,
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    ) -> bool {
        let return_type = substituted_formal_type(program, return_type, substitutions);
        match expected {
            ExpectedCallValueResult::Scalar(expected) => {
                program.primitive_type_reference(return_type) == Some(*expected)
            }
            ExpectedCallValueResult::Structural(expected)
                if borrowed_slice_result(program, return_type)
                    || borrowed_named_view(program, return_type) =>
            {
                let multiplicity = if borrowed_slice_result(program, return_type) {
                    validation::reference_result_custody::binding_multiplicity(program, return_type)
                } else {
                    Multiplicity::Affine
                };
                let peeled =
                    validation::structural_type_identity(program, return_type, &[], substitutions);
                let shelled =
                    program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                        substitutions,
                        ..typed_trees::type_identity::TypeIdentityRequest::ordinary(return_type)
                    });
                expected.multiplicity == multiplicity
                    && (peeled == expected.type_identity || shelled == expected.type_identity)
            }
            ExpectedCallValueResult::Structural(expected)
                if has_borrowed_parts(program, return_type) =>
            {
                expected.multiplicity == Multiplicity::Affine
                    && program.type_identity(typed_trees::type_identity::TypeIdentityRequest {
                        substitutions,
                        ..typed_trees::type_identity::TypeIdentityRequest::ordinary(return_type)
                    }) == expected.type_identity
            }
            ExpectedCallValueResult::Structural(expected) => {
                program.primitive_type_reference(return_type).is_none()
                    && !is_unit(program, return_type)
                    && !is_reference(program, return_type)
                    && validation::reference_result_custody::result_multiplicity(
                        program,
                        return_type,
                    ) == expected.multiplicity
                    && base_type_identity(program, return_type, &[], substitutions)
                        == Some(expected.type_identity)
            }
        }
    }

    /*
        A resolved call through an ordinary or bodyless-boundary target: the
        target, its contract and every argument lane, ready for the plan the
        call's expected result selects.
    */
    pub(super) struct PlannedCall<'p> {
        pub(super) coordinate: CheckedUnitCallCoordinate,
        pub(super) source_site: Option<checked_trees::NominalMachineUseSite>,
        pub(super) call: &'p checked_trees::FlowCallFact,
        pub(super) target_machine: &'p typed_trees::machine::Machine,
        pub(super) target_state: &'p typed_trees::state::State,
        pub(super) target_contract: &'p checked_trees::MachineContractPlan,
        pub(super) scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
        pub(super) erased_scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
        pub(super) erased_proof_arguments: Vec<checked_trees::CheckedProofTerm>,
        pub(super) structural_arguments: Vec<CheckedUnitStructuralArgumentPlan>,
        pub(super) transfers: Vec<checked_trees::CheckedUnitClaimTransferPlan>,
    }

    /*
        Build one call with the scalar-callee evidence available to this pass.
        `None` supplies no registered scalar targets; it never consults published catalogs.
    */
    pub(in crate::execution) fn build_call_operation(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        facts: &CheckFacts,
        scalar_callees: Option<ScalarCalleeClosure>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        caller_parameters: &[CheckedUnitStructuralParameterPlan],
        caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        call: &checked_trees::FlowCallFact,
        expected_call_result: Option<ExpectedCallValueResult<'_>>,
        caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
        caller_operations: &[OperationPlan],
        trace: &LocalConstructionTrace,
    ) -> Option<OperationPlan> {
        let coordinate = CheckedUnitCallCoordinate {
            statement_index: u32::try_from(call.statement_index).ok()?,
            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
        };
        let phase = |name: &'static str| {
            trace.phase(name);
            trace.statement(Some(coordinate.statement_index));
        };
        phase("call operation: call site");
        let call_site = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let source_site = match &call_site {
            validation::CallSite::Statement(_) => {
                let offset = u32::try_from(call.statement_index).ok()?;
                Some(checked_trees::NominalMachineUseSite::Statement(
                    arena::Handle::from_parts(
                        state
                            .statement_nodes
                            .start()
                            .arena_index()
                            .checked_add(offset)?,
                        state.statement_nodes.start().generation(),
                    ),
                ))
            }
            validation::CallSite::Expression { expression, .. } => Some(
                checked_trees::NominalMachineUseSite::Expression(*expression),
            ),
            validation::CallSite::TransitionNamed { .. } => None,
        };

        if let Some(operation) = program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            .and_then(|builtin| {
                language_core::inline_assembly::AsmGlobalDescriptorTableOperation::from_intrinsic_name(
                    builtin.name(),
                )
            })
        {
            phase("descriptor instruction: borrowed bytes");
            if expected_call_result.is_some() {
                return None;
            }
            let arguments = validation::call_site_argument_expressions(program, &call_site);
            let [operand] = arguments else {
                return None;
            };
            let reference = validation::compiler_operations::global_descriptor_table_operand(
                program,
                machine,
                Some(state),
                *operand,
                operation,
            )
            .ok()?;
            let parameter = super::structural_arguments::StructuralArgumentType {
                type_reference: reference,
                erased: false,
            };
            let mut arguments = super::structural_arguments::structural_arguments_for_types(
                program,
                shapes,
                facts,
                call,
                machine,
                state,
                caller_parameters,
                caller_trivial_affine_locals,
                std::slice::from_ref(&parameter),
                language_semantics::MachineSupplyMode::CheckedBody,
                false,
                None,
                &call_site,
                call.statement_index,
                caller_structural_results,
                caller_operations,
                trace,
            )?;
            if arguments.len() != 1 {
                return None;
            }
            let storage = arguments.pop()?;
            phase("descriptor instruction: exact MachineControl service");
            let rows = &facts.service_reaches.rows;
            let [service] = rows.services(call.service_reach.direct) else {
                return None;
            };
            if !rows
                .services(call.service_reach.transitive)
                .contains(service)
            {
                return None;
            }
            return Some(OperationPlan::GlobalDescriptorTable {
                coordinate,
                operation,
                storage,
                service: *service,
                service_reach: call.service_reach,
            });
        }

        if let Some(operation) = program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)
            .and_then(|builtin| {
                language_core::inline_assembly::AsmRegisterOperation::from_intrinsic_name(
                    builtin.name(),
                )
            })
        {
            phase("register access: result and operands");
            if !operation.is_admitted()
                || match expected_call_result {
                    Some(ExpectedCallValueResult::Scalar(primitive_type)) => {
                        operation.result_carrier().is_none_or(|carrier| {
                            PrimitiveType::from_name(carrier.name()) != Some(primitive_type)
                        })
                    }
                    None => operation.produces_value(),
                    _ => true,
                }
            {
                return None;
            }
            let parameter_types = operation
                .input_carriers()
                .iter()
                .enumerate()
                .map(|(position, name)| {
                    Some(CheckedStructuralScalarParameterPlan {
                        source_position: u32::try_from(position).ok()?,
                        primitive_type: PrimitiveType::from_name(name.name())?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            if validation::call_site_argument_expressions(program, &call_site).len()
                != parameter_types.len()
            {
                return None;
            }
            return Some(OperationPlan::RegisterAccess {
                coordinate,
                operation,
                result: None,
                arguments: checked_call_scalar_arguments(
                    facts,
                    state.symbol,
                    coordinate,
                    &parameter_types,
                    false,
                )?,
                service_reach: call.service_reach,
            });
        }

        let selected_realization =
            exact_external_leaf_boundary_requirement(program, call.target_symbol);
        phase("call operation: target state");
        let destination = super::resolve_call_target(program, call.target_symbol)?;
        let (target_machine, target_state) = match destination {
            super::ResolvedCallTarget::Requirement(requirement) => {
                return super::trait_boundary_call::build(
                    program,
                    shapes,
                    facts,
                    machine,
                    state,
                    caller_parameters,
                    call,
                    expected_call_result.as_ref(),
                    caller_structural_results,
                    caller_operations,
                    trace,
                    coordinate,
                    &call_site,
                    source_site,
                    requirement,
                    selected_realization,
                );
            }
            super::ResolvedCallTarget::DynamicSlot(slot) => {
                return super::dynamic_call::build(
                    program,
                    shapes,
                    facts,
                    machine,
                    state,
                    caller_parameters,
                    caller_trivial_affine_locals,
                    call,
                    expected_call_result.as_ref(),
                    caller_structural_results,
                    caller_operations,
                    trace,
                    coordinate,
                    &call_site,
                    source_site,
                    slot,
                );
            }
            super::ResolvedCallTarget::Declared { machine, state } => (machine, state),
        };
        let (target_machine_identity, target_state_identity) = destination.identity();
        phase("call operation: target contract");
        let target_contract = facts.contract_plans.for_machine(target_machine.symbol)?;
        let call_target = CheckedUnitCallTarget {
            machine: target_machine_identity,
            state: target_state_identity,
            contract_report_fingerprint: target_contract.report_fingerprint,
            service_reach: call.service_reach,
        };
        let boundary = target_machine.has_bodyless_signature();
        let no_normal = !boundary
            && validation::normal_completion_possible(program, target_state) == Some(false);
        phase("call operation: result shape");
        if !no_normal
            && match &expected_call_result {
                None => !is_unit(program, target_state.return_type),
                Some(expected) => {
                    !call_value_result_matches(program, target_state.return_type, expected, &[])
                }
            }
        {
            return None;
        }
        phase("call operation: target supply mode");
        if !boundary && !target_machine.supplies_checked_body() {
            return None;
        }
        phase("call operation: structural arguments");
        let structural_arguments = structural_call_arguments(
            program,
            shapes,
            facts,
            call,
            machine,
            state,
            caller_parameters,
            caller_trivial_affine_locals,
            super::CallTarget::declared(program, target_machine, target_state),
            &call_site,
            call.statement_index,
            caller_structural_results,
            caller_operations,
            trace,
        )?;
        phase("call operation: scalar parameters");
        let mut scalar_parameters = Vec::new();
        for (position, parameter) in program.state_parameters(target_state).iter().enumerate() {
            if strips_erased_parameter(parameter)? {
                continue;
            }
            let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference)
            else {
                continue;
            };
            if (parameter.is_const)
                || (parameter.is_mutable
                    && mutable_scalar_parameter_type(program, parameter).is_none())
            {
                return None;
            }
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position: u32::try_from(position).ok()?,
                primitive_type,
            });
        }
        phase("call operation: scalar arguments");
        let scalar_arguments = if boundary {
            checked_call_scalar_arguments(
                facts,
                state.symbol,
                coordinate,
                &scalar_parameters,
                true,
            )?
        } else {
            checked_call_scalar_arguments(
                facts,
                state.symbol,
                coordinate,
                &scalar_parameters,
                false,
            )?
        };
        phase("call operation: erased scalar arguments");
        let erased_scalar_parameters = erased_scalar_parameter_plans(program, target_state)?;
        let erased_scalar_arguments = if boundary {
            if !erased_scalar_parameters.is_empty() {
                return None;
            }
            Vec::new()
        } else {
            checked_call_erased_scalar_arguments(
                facts,
                state.symbol,
                coordinate,
                &erased_scalar_parameters,
            )?
        };
        phase("call operation: erased proof arguments");
        let erased_proof_parameters = erased_proof_parameter_plans(program, target_state)?;
        let erased_proof_arguments = if boundary {
            if !erased_proof_parameters.is_empty() {
                return None;
            }
            Vec::new()
        } else {
            checked_call_erased_proof_arguments(
                program,
                facts,
                state.symbol,
                coordinate,
                &erased_proof_parameters,
            )?
        };
        if let Some(contract) =
            validation::exact_toolchain_activation_storage_contract(program, target_machine)
        {
            phase("activation storage: closed operation contract");
            let bound = if matches!(
                contract.operation,
                validation::ActivationStorageOperation::Establish
                    | validation::ActivationStorageOperation::Split
            ) {
                Some(
                    validation::activation_storage_bound(program, target_machine)
                        .inspect_err(|reason| phase(reason))
                        .ok()?,
                )
            } else {
                None
            };
            let route = CheckedUnitCallRoute::ActivationStorage {
                operation: contract.operation,
                bound,
            };
            if let Some(ExpectedCallValueResult::Structural(result)) = expected_call_result {
                return super::structural_result_call::with_structural_custody(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    trace,
                    CheckedUnitCallParts {
                        coordinate,
                        source_site,
                        target: call_target,
                        route,
                        completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                        scalar_arguments,
                        structural_arguments,
                        result: CheckedUnitCallResult::Structural {
                            binding: result.clone(),
                            discard_result_on_return: false,
                        },
                        transfers: CallTransfers::StructuralCustody(Default::default()),
                    },
                );
            }
            if expected_call_result.is_some()
                || contract.operation != validation::ActivationStorageOperation::Release
            {
                phase("activation storage: operation/result mismatch");
                return None;
            }
            let transfers = call_claim_transfers(
                program,
                facts,
                machine.symbol,
                state.symbol,
                call,
                caller_parameters,
                caller_structural_results,
                &structural_arguments,
            )
            .inspect_err(|reason| phase(reason))
            .ok()?;
            return CheckedUnitCallPlan::new(CheckedUnitCallParts {
                coordinate,
                source_site,
                target: call_target,
                route,
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                scalar_arguments,
                structural_arguments,
                result: CheckedUnitCallResult::Unit,
                transfers: CallTransfers::Claims(transfers),
            })
            .map(OperationPlan::Call);
        }
        if !boundary
            && !no_normal
            && let Some(ExpectedCallValueResult::Structural(result)) = &expected_call_result
            && !validation::linear_claim_frontier(program, target_state.return_type).is_empty()
        {
            phase("call operation: claim-bearing structural result");
            if !machine_binders(program, target_machine).is_empty()
                || !program
                    .machine_states(target_machine)
                    .first()
                    .is_some_and(|entry| entry.symbol == target_state.symbol)
            {
                return None;
            }
            return super::structural_result_call::with_structural_custody(
                program,
                facts,
                machine.symbol,
                state.symbol,
                trace,
                super::structural_result_call::structural_custody_parts(
                    coordinate,
                    source_site,
                    call_target,
                    target_contract.commitment,
                    erased_scalar_arguments,
                    erased_proof_arguments,
                    scalar_arguments,
                    structural_arguments,
                    (*result).clone(),
                    result.multiplicity == Multiplicity::Affine,
                ),
            );
        }
        phase("call operation: claim transfers");
        let completion = if boundary {
            validation::boundary_requirement_completion(program, call_target.machine)
        } else {
            semantic_vocabulary::BoundaryCompletion::NormalReturn
        };
        let transfers = if matches!(
            completion,
            semantic_vocabulary::BoundaryCompletion::External(_)
        ) {
            Vec::new()
        } else {
            call_claim_transfers(
                program,
                facts,
                machine.symbol,
                state.symbol,
                call,
                caller_parameters,
                caller_structural_results,
                &structural_arguments,
            )
            .inspect_err(|reason| phase(reason))
            .ok()?
        };

        if let Some((operation, expected_service)) =
            validation::compiler_operations::descriptor_table_operation(program, target_machine)
                .inspect_err(|reason| phase(reason))
                .ok()?
        {
            let expected = match operation {
                language_core::descriptor_tables::DescriptorTableOperation::ActivateTask {
                    ..
                }
                | language_core::descriptor_tables::DescriptorTableOperation::ReloadSegments {
                    ..
                } => PrimitiveType::U8,
                language_core::descriptor_tables::DescriptorTableOperation::ReadTask
                | language_core::descriptor_tables::DescriptorTableOperation::ReadSegment {
                    ..
                } => PrimitiveType::U16,
            };
            if !matches!(expected_call_result, Some(ExpectedCallValueResult::Scalar(actual)) if actual == expected)
            {
                phase("descriptor operation lost its scalar result");
                return None;
            }
            let rows = &facts.service_reaches.rows;
            let [service] = rows.services(call_target.service_reach.direct) else {
                phase("descriptor operation requires exactly its MachineControl service");
                return None;
            };
            if *service != expected_service
                || !rows
                    .services(call_target.service_reach.transitive)
                    .contains(service)
            {
                phase("descriptor call changed its checked MachineControl authority");
                return None;
            }
            return CheckedUnitCallPlan::new(CheckedUnitCallParts {
                coordinate,
                source_site,
                target: call_target,
                route: CheckedUnitCallRoute::DescriptorTable {
                    operation,
                    service: *service,
                    result_type: expected,
                },
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                scalar_arguments,
                structural_arguments,
                result: CheckedUnitCallResult::Unit,
                transfers: CallTransfers::Claims(transfers),
            })
            .map(OperationPlan::Call);
        }

        if no_normal {
            CheckedUnitCallPlan::new(CheckedUnitCallParts {
                coordinate,
                source_site,
                target: call_target,
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                route: CheckedUnitCallRoute::Module {
                    contract_commitment: Some(target_contract.commitment),
                    erased_scalar_arguments,
                    erased_proof_arguments,
                },
                scalar_arguments,
                structural_arguments,
                result: CheckedUnitCallResult::NoNormal,
                transfers: CallTransfers::Claims(transfers),
            })
            .map(OperationPlan::Call)
        } else if boundary {
            CheckedUnitCallPlan::new(CheckedUnitCallParts {
                coordinate,
                source_site,
                target: call_target,
                route: CheckedUnitCallRoute::Boundary,
                completion,
                scalar_arguments,
                structural_arguments,
                result: CheckedUnitCallResult::Unit,
                transfers: match completion {
                    semantic_vocabulary::BoundaryCompletion::NormalReturn => {
                        CallTransfers::CompletionReceipts(transfers)
                    }
                    semantic_vocabulary::BoundaryCompletion::External(_) => {
                        CallTransfers::Abandonment
                    }
                },
            })
            .map(OperationPlan::Call)
        } else if let Some(ExpectedCallValueResult::Structural(result)) = expected_call_result {
            super::structural_result_call::build(
                program,
                facts,
                machine,
                state,
                trace,
                PlannedCall {
                    coordinate,
                    source_site,
                    call,
                    target_machine,
                    target_state,
                    target_contract,
                    scalar_arguments,
                    erased_scalar_arguments,
                    erased_proof_arguments,
                    structural_arguments,
                    transfers,
                },
                result,
            )
        } else if expected_call_result.is_some()
            && (!structural_arguments.is_empty() || !transfers.is_empty())
            && scalar_callees.is_none()
        {
            phase("call operation: scalar result producer");
            None
        } else {
            CheckedUnitCallPlan::new(CheckedUnitCallParts {
                coordinate,
                source_site,
                target: call_target,
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                route: CheckedUnitCallRoute::Module {
                    contract_commitment: None,
                    erased_scalar_arguments,
                    erased_proof_arguments,
                },
                scalar_arguments,
                structural_arguments,
                result: CheckedUnitCallResult::Unit,
                transfers: CallTransfers::Claims(transfers),
            })
            .map(OperationPlan::Call)
        }
    }
}
mod call_targets {
    /*
        One resolved callable destination shared by construction and correspondence.
        Source calls may name an exact requirement, a dynamic slot, or a declared
        machine/state. These are semantic identities, not interchangeable raw handles
        or selected provider identities. Resolve once through the producer's order.
    */
    use super::trait_boundary_call::BoundaryRequirement;
    use symbols::SymbolHandle;

    #[derive(Clone, Copy)]
    pub(in crate::execution::terminal_unit) enum ResolvedCallTarget<'program> {
        Requirement(BoundaryRequirement<'program>),
        DynamicSlot(&'program typed_trees::dynamic_traits::DynamicSlotSignature),
        Declared {
            machine: &'program typed_trees::machine::Machine,
            state: &'program typed_trees::state::State,
        },
    }

    impl ResolvedCallTarget<'_> {
        pub(in crate::execution::terminal_unit) fn identity(self) -> (SymbolHandle, SymbolHandle) {
            match self {
                Self::Requirement(requirement) => {
                    let target = requirement.target_symbol();
                    (target, target)
                }
                Self::DynamicSlot(slot) => (slot.symbol, slot.symbol),
                Self::Declared { machine, state } => (machine.symbol, state.symbol),
            }
        }
    }

    pub(in crate::execution::terminal_unit) fn resolve_call_target(
        program: &dyn validation::CheckingInput,
        target: SymbolHandle,
    ) -> Option<ResolvedCallTarget<'_>> {
        match boundary_requirement(program, target) {
            BoundaryTarget::Requirement(requirement) => {
                return Some(ResolvedCallTarget::Requirement(requirement));
            }
            BoundaryTarget::Ambiguous => return None,
            BoundaryTarget::Body => {}
        }
        if let Some(slot) = program.dynamic_slot_by_symbol(target) {
            return Some(ResolvedCallTarget::DynamicSlot(slot));
        }
        let (machine, state) = validation::find_machine_head(program, target)
            .or_else(|| validation::find_state(program, target))?;
        Some(ResolvedCallTarget::Declared { machine, state })
    }

    /* What a call's authored target completes through. */
    enum BoundaryTarget<'program> {
        /* A boundary requirement's call route. */
        Requirement(super::trait_boundary_call::BoundaryRequirement<'program>),
        /* More than one boundary requirement declares the target. */
        Ambiguous,
        /* No boundary requirement: a machine body or a dynamic slot. */
        Body,
    }

    /*
        Only an authored requirement or operator invokes a selected provider.
        A concrete external leaf keeps its own declared callable coordinate even
        when a target-scoped caller already resolves its exact family member.
        Its satisfaction edge supplies a contract, not another call destination.
        Nominal binders still require specialization before executable planning.
    */
    fn boundary_requirement(
        program: &dyn ::validation::CheckingInput,
        target: SymbolHandle,
    ) -> BoundaryTarget<'_> {
        let static_boundaries = program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .flat_map(|definition| {
                program
                    .executable_trait_machine_instances(definition)
                    .filter(move |signature| signature.symbol == target)
                    .map(move |signature| (definition, signature))
            })
            .collect::<Vec<_>>();
        match static_boundaries.as_slice() {
            [(definition, signature)] => BoundaryTarget::Requirement(
                super::trait_boundary_call::BoundaryRequirement::Trait {
                    definition,
                    signature,
                },
            ),
            [] => program
                .operators()
                .iter()
                .find(|operator| operator.is_boundary && operator.symbol == target)
                .map_or(BoundaryTarget::Body, |operator| {
                    BoundaryTarget::Requirement(
                        super::trait_boundary_call::BoundaryRequirement::Operator(operator),
                    )
                }),
            _ => BoundaryTarget::Ambiguous,
        }
    }
}
pub(super) use call_targets::{ResolvedCallTarget, resolve_call_target};
mod computation_arguments {

    /*
        Structural homes, projected borrowed operands, and primitive referents
        in scalar computations. Whole owned operands use the same storage contract
        for parameters and locals. Their call occurrence retains identity until
        checked ownership supplies copy or transfer authority; a nominal hook changes
        the receiving owner's cleanup, not the argument's storage representation.
    */
    use super::super::structural_scalar_store::primitive_local_before;
    use super::super::types::byte_sequence_carrier;
    use super::super::types::parameter_is_root;
    use super::argument_paths::borrowed_argument_endpoint;
    use super::argument_presentation::classify;
    use super::view_subslice::admit_resolved;
    use crate::execution::terminal_unit::CheckedStructuralAccess;
    use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
    use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
    use crate::execution::terminal_unit::ExpressionNode;
    use crate::execution::terminal_unit::PrimitiveType;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::types::{
        base_type_identity, borrowed_slice_view_element, dense_scalar_parameter_index,
        dense_structural_parameter_index, parameter_qualifications,
        structural_access_for_type_reference,
    };
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use checked_trees::BorrowFacts;
    use checked_trees::CheckedOperatorFacts;
    use checked_trees::CheckedScalarComputationPlans;
    use checked_trees::CheckedScalarExpressionPlans;
    use checked_trees::CheckedSubsliceSite;
    use checked_trees::CheckedUnitStructuralPathSegment;
    use checked_trees::FlowCallFact;
    use checked_trees::checked_borrowed_access_pair_pattern;
    use facts::PlaceRoot;
    use language_semantics::ReferenceAccess;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::state_parameter_position;
    use typed_trees::state::State;
    use validation::CallSite;
    use validation::CanonicalPlace;
    use validation::ContentsRequirement;
    use validation::call_target_entry_symbol;
    use validation::call_target_parameters;
    use validation::call_target_symbol;
    use validation::canonical_place_type_reference;
    use validation::has_contents;
    use validation::peel_declared_constraints;
    use validation::reference_shell;
    use validation::subslice_expression;

    use crate::execution::terminal_unit::StateParameter;
    use crate::execution::terminal_unit::SymbolHandle;
    use crate::execution::terminal_unit::TypeReferenceHandle;
    use crate::execution::terminal_unit::TypeReferenceNode;

    use crate::execution::terminal_unit::calls::argument_paths::projected_evaluated_path;
    use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_borrow_access;
    use crate::execution::terminal_unit::calls::view_subslice::{ViewKind, view_identity};

    /*
        Argument plans borrow one immutable call occurrence. The complete ordered
        access roster is authenticated only when an argument needs that route,
        then shared by its other arguments and fallbacks. Graph/shape publication
        cannot change these source facts. Retained-only and literal routes keep
        their existing admission; this preparation grants no argument authority.
    */
    pub(crate) struct ComputationCallArguments<'input> {
        program: &'input dyn ::validation::CheckingInput,
        facts: crate::flow::CapturedSubjectFacts<'input>,
        machine: SymbolHandle,
        state: &'input State,
        call: &'input FlowCallFact,
        preparation: CallAccessPreparation<'input>,
    }

    enum CallAccessPreparation<'input> {
        Pending,
        Unavailable,
        Ready(CallSite<'input>),
    }

    impl<'input> ComputationCallArguments<'input> {
        pub(crate) fn new(
            program: &'input dyn ::validation::CheckingInput,
            facts: crate::flow::CapturedSubjectFacts<'input>,
            machine: SymbolHandle,
            state: &'input State,
            call: &'input FlowCallFact,
        ) -> Self {
            Self {
                program,
                facts,
                machine,
                state,
                call,
                preparation: CallAccessPreparation::Pending,
            }
        }

        fn rejoined_call(&mut self) -> Option<&CallSite<'input>> {
            if matches!(self.preparation, CallAccessPreparation::Pending) {
                self.preparation = match crate::borrow::accesses::rejoin_expression_call_accesses(
                    self.program,
                    self.facts,
                    self.machine,
                    self.state.symbol,
                    self.call,
                ) {
                    Some(site @ CallSite::Expression { .. }) => CallAccessPreparation::Ready(site),
                    _ => CallAccessPreparation::Unavailable,
                };
            }
            match &self.preparation {
                CallAccessPreparation::Ready(site) => Some(site),
                CallAccessPreparation::Pending | CallAccessPreparation::Unavailable => None,
            }
        }
    }

    impl ComputationCallArguments<'_> {
        pub(crate) fn structural_argument(
            &mut self,
            shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
            expression: ExpressionHandle,
            target: &StateParameter,
            selectors: Option<
                &dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>,
            >,
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let program = self.program;
            let facts = self.facts;
            let machine = self.machine;
            let state = self.state;
            let call = self.call;
            let borrow = facts.borrow;
            if target.is_const {
                return None;
            }
            let target_access =
                structural_access_for_type_reference(program, target.type_reference)?;
            let named = match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(argument) => {
                    let shell = reference_shell(program, target.type_reference)?;
                    if argument.access != shell.access {
                        return None;
                    }
                    argument.target
                }
                ExpressionNode::Name(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Indexed(_) => expression,
                _ => return None,
            };
            let place = place_at(program, state.symbol, call.statement_index, expression)?;

            self.rejoined_call()?;
            exact_actual(program, call, expression, target)?;
            if let Some(symbol) = place.root.as_symbol()
                && primitive_local_before(program, state, call.statement_index, symbol).is_some()
            {
                return primitive_local_argument(
                    program,
                    borrow,
                    machine,
                    state,
                    call,
                    &place,
                    &place,
                    target.type_reference,
                );
            }
            if matches!(
                target_access,
                CheckedStructuralAccess::SharedBorrow
                    | CheckedStructuralAccess::MutableBorrow
                    | CheckedStructuralAccess::WriteOnlyBorrow
            ) && let Some(argument) = borrowed_storage_argument(
                program,
                shapes,
                borrow,
                machine,
                state,
                call,
                &place,
                target.type_reference,
                selectors,
            ) {
                return Some(argument);
            }
            if let Some(argument) = borrowed_view_argument(
                program,
                borrow,
                machine,
                state,
                call,
                &place,
                target.type_reference,
                selectors,
            ) {
                return Some(argument);
            }
            if target_access == CheckedStructuralAccess::Owned {
                if named != expression {
                    return None;
                }
                // Noncopy saved subjects observe an existing owner. Its exact
                // captured backing, rather than the alias used for value facts,
                // supplies the subtree transferred by this argument.
                let place = validation::ownership_place_from_expression_in_state(
                    program,
                    state.symbol,
                    call.statement_index,
                    expression,
                )?;
                return storage_argument(
                    program,
                    shapes,
                    machine,
                    state,
                    call.statement_index,
                    &place,
                    target,
                    CheckedStructuralAccess::Owned,
                    selectors,
                );
            }
            let name = program.expression_table.name_path_node(named)?;
            if !program.expression_table.name_path_is_single_member(name)
                || place.root != PlaceRoot::Symbol(name.symbol)
                || !place.segments.is_empty()
            {
                return None;
            }
            let target_type = plain_primitive_referent(program, target.type_reference)?;
            let parameters = program.state_parameters(state);
            let source_position = parameters
                .iter()
                .position(|source| source.symbol == name.symbol)?;
            let source = &parameters[source_position];
            if (source.is_const)
                || plain_primitive_referent(program, source.type_reference)? != target_type
            {
                return None;
            }
            let source_access =
                structural_access_for_type_reference(program, source.type_reference)?;
            if !matches!(
                (source_access, target_access),
                (
                    CheckedStructuralAccess::SharedBorrow,
                    CheckedStructuralAccess::SharedBorrow
                ) | (
                    CheckedStructuralAccess::MutableBorrow,
                    CheckedStructuralAccess::SharedBorrow
                        | CheckedStructuralAccess::MutableBorrow
                        | CheckedStructuralAccess::WriteOnlyBorrow
                ) | (
                    CheckedStructuralAccess::WriteOnlyBorrow,
                    CheckedStructuralAccess::WriteOnlyBorrow
                )
            ) {
                return None;
            }
            let access = exact_structural_borrow_access(
                program,
                borrow,
                machine,
                state.symbol,
                call,
                &place,
                target_access,
                &[],
            )?;
            if access != target_access {
                return None;
            }
            let parameter_index =
                dense_structural_parameter_index(program, parameters, source_position)?;
            Some(CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
                path: Vec::new(),
                type_identity: base_type_identity(program, target.type_reference, &[], &[])?,
                access,
            })
        }

        /*
            The range is still an actual of this exact checked call, not merely an
            equal-typed view expression. Its bounds use dense explicit structural
            ordinals, which exclude the implicit receiver.
        */
        pub(crate) fn subslice_argument(
            &mut self,
            shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
            operators: &CheckedOperatorFacts,
            expressions: &CheckedScalarExpressionPlans,
            computations: &CheckedScalarComputationPlans,
            expression: ExpressionHandle,
            target: &StateParameter,
            argument_ordinal: u32,
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let program = self.program;
            let facts = self.facts;
            let machine = self.machine;
            let state = self.state;
            let call = self.call;
            let borrow = facts.borrow;
            if target.is_const {
                return None;
            }
            exact_actual(program, call, expression, target)?;
            self.rejoined_call()?;
            let owner = program.machine(machine)?;
            let place = place_at(program, state.symbol, call.statement_index, expression)?;

            subslice_expression(program, expression)?;
            let parameters = if !program
                .state_parameters(state)
                .iter()
                .any(|parameter| place.root == PlaceRoot::Symbol(parameter.symbol))
            {
                Vec::new()
            } else {
                super::free_structural_scalar_signature(program, shapes, state, &[])?.0
            };
            let subslice = admit_resolved(
                program,
                operators,
                expressions,
                computations,
                borrow,
                owner,
                state,
                &parameters,
                target.type_reference,
                expression,
                call.statement_index,
                CheckedSubsliceSite::CallArgument {
                    call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                    argument_ordinal,
                },
            )?;
            let argument = subslice.argument();
            (exact_structural_borrow_access(
                program,
                borrow,
                machine,
                state.symbol,
                call,
                &place,
                argument.access,
                &[],
            )? == argument.access)
                .then_some(argument)
        }
    }

    fn exact_actual(
        program: &dyn ::validation::CheckingInput,
        call: &FlowCallFact,
        expression: ExpressionHandle,
        target: &StateParameter,
    ) -> Option<()> {
        let target_position = call_target_parameters(program, call.target_symbol)?
            .iter()
            .position(|parameter| parameter == target)?;
        let ExpressionNode::Call(authored) =
            expressions(program).expression(call.authored_expression)
        else {
            return None;
        };
        let exact_actual = expressions(program)
            .expression_handles(authored.arguments)
            .get(target_position)
            == Some(&expression);
        if call_target_entry_symbol(program, call_target_symbol(program, authored))
            != call.target_symbol
            || !exact_actual
        {
            return None;
        }
        Some(())
    }

    /*
        A borrowed argument names its existing parameter or local result
        home. Initializer spelling does not select custody; publication independently
        rejoins this symbol to its dominating structural establishment.
    */
    pub(crate) fn borrowed_storage_argument(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
        borrow: &BorrowFacts,
        machine: SymbolHandle,
        state: &State,
        call: &FlowCallFact,
        place: &CanonicalPlace,
        target: TypeReferenceHandle,
        selectors: Option<&dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>>,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let target_shell = reference_shell(program, target)?;
        let target_access = CheckedStructuralAccess::from(target_shell.access);
        let parameters = program.state_parameters(state);
        let (reference, mut source) = if let Some(expression) = place.root.as_expression() {
            let reference = canonical_place_type_reference(
                program,
                state.symbol,
                call.statement_index,
                &CanonicalPlace {
                    root: place.root,
                    segments: Vec::new(),
                },
            )?;
            (
                reference,
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                    value_dependency: None,
                    expression,
                    // ProjectedArgumentPath omits only the inherited root shells.
                    // Keep that checked logical root beside the completed owner;
                    // nested descriptor hops remain explicit in the suffix.
                    logical_root_type_identity: if place.segments.is_empty() {
                        None
                    } else {
                        Some(shapes.add_logical_type(reference, &[], &[])?)
                    },
                },
            )
        } else {
            let symbol = place.root.as_symbol()?;
            if let Some(position) = state_parameter_position(parameters, symbol) {
                let parameter = &parameters[position];
                if parameter.is_const {
                    return None;
                }
                let reference = match program
                    .type_reference_table
                    .type_reference(peel_declared_constraints(program, parameter.type_reference))
                {
                    TypeReferenceNode::Reference {
                        access, referee, ..
                    } if borrowed_access_supplies(*access, target_shell.access) => *referee,
                    TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
                    | TypeReferenceNode::FixedArray { .. } => parameter.type_reference,
                    _ => return None,
                };
                let source = if program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                {
                    if !place.segments.is_empty()
                        || (target_access != CheckedStructuralAccess::SharedBorrow
                            && !parameter.is_mutable)
                    {
                        return None;
                    }
                    let parameter_index =
                        dense_scalar_parameter_index(program, parameters, position)?;
                    CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { parameter_index }
                } else {
                    let parameter_index =
                        dense_structural_parameter_index(program, parameters, position)?;
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }
                };
                (reference, source)
            } else {
                let (_, local) = program
                    .state_local_declaration(state, symbol)
                    .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
                let reference = match program
                    .type_reference_table
                    .type_reference(peel_declared_constraints(program, local.type_reference))
                {
                    TypeReferenceNode::Reference {
                        access, referee, ..
                    } if (target_access == CheckedStructuralAccess::SharedBorrow
                        || !local.is_mutable)
                        && borrowed_access_supplies(*access, target_shell.access) =>
                    {
                        *referee
                    }
                    TypeReferenceNode::Named { .. }
                    | TypeReferenceNode::Generic { .. }
                    | TypeReferenceNode::FixedArray { .. } => local.type_reference,
                    _ => return None,
                };
                (
                    reference,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                )
            }
        };
        let whole_storage = place.segments.is_empty()
            || matches!(
                source,
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            );
        let whole_storage_type = reference;
        let whole_storage_identity = if whole_storage {
            shapes.add_stored_type(reference, &[], &[])?
        } else {
            Default::default()
        };
        if let CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } = source {
            source = CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                symbol,
                root_type_identity: if place.segments.is_empty() {
                    whole_storage_identity
                } else {
                    shapes.add_logical_type(reference, &[], &[])?
                },
                root_reference_hops: 0,
            };
        }
        let ProjectedArgumentPath {
            storage_type: reference,
            path,
            ..
        } = if place.segments.is_empty() {
            ProjectedArgumentPath {
                declared_type: reference,
                storage_type: reference,
                path: Vec::new(),
            }
        } else {
            projected_evaluated_path(
                program,
                machine,
                state.symbol,
                call.statement_index,
                place,
                selectors,
            )?
        };
        let (reference, path) = borrowed_argument_endpoint(program, reference, path, target)?;
        let identity = if reference == whole_storage_type && whole_storage_identity.is_valid() {
            whole_storage_identity
        } else {
            shapes.add_stored_type(reference, &[], &[])?
        };
        if place.segments.is_empty()
            && let CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow {
                root_type_identity,
                root_reference_hops,
                ..
            } = &mut source
        {
            if !path
                .iter()
                .all(|step| matches!(step, CheckedUnitStructuralPathSegment::Referent))
            {
                return None;
            }
            *root_type_identity = identity;
            *root_reference_hops = u32::try_from(path.len()).ok()?;
        }
        let borrowed_referent = shapes.types.contains_key(&identity)
            && has_contents(
                program,
                reference,
                &[],
                ContentsRequirement::BorrowedStorage,
            );
        let presentation_identity = shapes.add_type(target, &[], &[])?;
        classify(program, reference, target, presentation_identity, &[])?;
        parameter_qualifications(program, shapes, reference, &[])?;
        if !borrowed_referent {
            return None;
        }
        let observed_access = exact_structural_borrow_access(
            program,
            borrow,
            machine,
            state.symbol,
            call,
            place,
            target_access,
            &[],
        );
        if observed_access? != target_access {
            return None;
        }
        Some(CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source,
            path,
            type_identity: presentation_identity,
            access: target_access,
        })
    }

    /*
        A storage projection attenuates the existing loan; it never acquires
        readable authority from a write-only parent. Exact call-occurrence evidence
        below remains responsible for ownership, suspension and restoration.
    */
    fn borrowed_access_supplies(supplied: ReferenceAccess, requested: ReferenceAccess) -> bool {
        supplied == requested || supplied == ReferenceAccess::Mutable
    }

    /*
        A whole shared or mutable borrowed-view actual loans its established
        view carrier: a parameter, local or projected descriptor in existing storage.
        The reference shell is the loan itself — the same presentation the
        call-statement lane gives a borrowed view — while this lane names the
        view's established home. Source and access rules are identical for both
        view families; only the carrier test and the type identity differ.
    */
    pub(in crate::execution) fn borrowed_view_argument(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        machine: SymbolHandle,
        state: &State,
        call: &FlowCallFact,
        place: &CanonicalPlace,
        target: TypeReferenceHandle,
        selectors: Option<&dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>>,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let symbol = place.root.as_symbol()?;
        let kind = if byte_sequence_carrier(program, target, &[])
            .is_some_and(|carrier| carrier.is_borrowed_view())
        {
            ViewKind::Bytes
        } else {
            ViewKind::Elements
        };
        let target_access = structural_access_for_type_reference(program, target)?;
        if !matches!(target_access, checked_borrowed_access_pair_pattern!()) {
            return None;
        }
        let is_view = |type_reference| match kind {
            ViewKind::Elements => {
                borrowed_slice_view_element(program, type_reference, &[]).is_some()
            }
            ViewKind::Bytes => byte_sequence_carrier(program, type_reference, &[])
                .is_some_and(|carrier| carrier.is_borrowed_view()),
        };
        if !is_view(target) {
            return None;
        }
        let identity = view_identity(program, kind, target)?;
        let parameters = program.state_parameters(state);
        let (source, source_type) = if let Some(position) = parameters
            .iter()
            .position(|parameter| parameter_is_root(parameter, symbol))
        {
            let parameter = &parameters[position];
            if parameter.is_const {
                return None;
            }
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: dense_structural_parameter_index(
                        program, parameters, position,
                    )?,
                },
                parameter.type_reference,
            )
        } else {
            let (_, local) = program
                .state_local_declaration(state, symbol)
                .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
            if !local.initial_value.is_valid() {
                return None;
            }
            (
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                local.type_reference,
            )
        };
        let ProjectedArgumentPath {
            storage_type: source_type,
            path,
            ..
        } = if place.segments.is_empty() {
            ProjectedArgumentPath {
                declared_type: source_type,
                storage_type: source_type,
                path: Vec::new(),
            }
        } else {
            projected_evaluated_path(
                program,
                machine,
                state.symbol,
                call.statement_index,
                place,
                selectors,
            )?
        };
        let source_access = structural_access_for_type_reference(program, source_type)?;
        if !is_view(source_type)
            || !matches!(source_access, checked_borrowed_access_pair_pattern!())
            || (target_access == CheckedStructuralAccess::MutableBorrow
                && source_access != target_access)
            || view_identity(program, kind, source_type)? != identity
        {
            return None;
        }
        if exact_structural_borrow_access(
            program,
            borrow,
            machine,
            state.symbol,
            call,
            place,
            target_access,
            &[],
        )? != target_access
        {
            return None;
        }
        Some(CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source,
            path,
            type_identity: identity,
            access: target_access,
        })
    }

    /*
        Preserve one established home for an owned transfer or shared observation.
        The selected access determines whether reference crossings are permitted;
        neither representation nor initialization grants copying or disposal authority.
    */
    pub(crate) fn storage_argument(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
        machine: SymbolHandle,
        state: &State,
        statement_index: usize,
        place: &CanonicalPlace,
        target: &StateParameter,
        access: CheckedStructuralAccess,
        selectors: Option<&dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>>,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        if !matches!(
            access,
            CheckedStructuralAccess::Owned | CheckedStructuralAccess::SharedBorrow
        ) {
            return None;
        }
        let parameters = program.state_parameters(state);
        let source_symbol = place.root.as_symbol()?;
        let (source, source_type) = if let Some(position) = parameters
            .iter()
            .position(|source| source.symbol == source_symbol)
        {
            let parameter = &parameters[position];
            if parameter.is_const {
                return None;
            }
            (
                CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: dense_structural_parameter_index(
                        program, parameters, position,
                    )?,
                },
                parameter.type_reference,
            )
        } else {
            let (_, local) = program
                .state_local_declaration(state, source_symbol)
                .filter(|&(ordinal, _)| ordinal < statement_index)?;
            (
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: source_symbol,
                },
                local.type_reference,
            )
        };
        let source_access = structural_access_for_type_reference(program, source_type)?;
        if source_access != CheckedStructuralAccess::Owned
            && !(access == CheckedStructuralAccess::SharedBorrow
                && matches!(
                    source_access,
                    CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
                ))
        {
            return None;
        }
        for length in 0..=place.segments.len() {
            let mut reference = validation::project_type_reference_from_segments(
                program,
                source_type,
                &place.segments[..length],
            )?;
            while let Some(shell) = reference_shell(program, reference) {
                if access == CheckedStructuralAccess::Owned || !shell.access.is_readable() {
                    return None;
                }
                reference = shell.referee;
            }
        }
        let ProjectedArgumentPath {
            storage_type: mut selected_type,
            mut path,
            ..
        } = projected_evaluated_path(
            program,
            machine,
            state.symbol,
            statement_index,
            place,
            selectors,
        )?;
        while let Some(shell) = reference_shell(program, selected_type) {
            if access == CheckedStructuralAccess::Owned || !shell.access.is_readable() {
                return None;
            }
            path.push(CheckedUnitStructuralPathSegment::Referent);
            selected_type = shell.referee;
        }
        if access == CheckedStructuralAccess::Owned
            && path
                .iter()
                .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Referent))
        {
            return None;
        }
        for reference in [selected_type, target.type_reference] {
            parameter_qualifications(program, shapes, reference, &[])?;
            shapes.add_stored_type(reference, &[], &[])?;
            let storage = peel_declared_constraints(program, reference);
            if program.primitive_type_reference(storage).is_some()
                || structural_access_for_type_reference(program, reference)?
                    != CheckedStructuralAccess::Owned
            {
                return None;
            }
        }
        let type_identity = base_type_identity(program, selected_type, &[], &[])?;
        if type_identity != base_type_identity(program, target.type_reference, &[], &[])?
            || crate::checks::type_multiplicity(program, selected_type)
                != crate::checks::type_multiplicity(program, target.type_reference)
        {
            return None;
        }
        Some(CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source,
            path,
            type_identity,
            access,
        })
    }

    fn plain_primitive_referent(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if !matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        ) {
            return None;
        }
        program.primitive_type_reference(*referee)
    }

    /*
        Bind the resolved storage home while retaining the authored access occurrence.
        An erased reference alias names the same home, but its declaration is a
        descriptor rather than the primitive owner. Using that descriptor for the
        storage/type lookup rejects valid reborrows; using the owner for access would
        instead lose the exact loan that authorizes the call.
    */
    pub(super) fn primitive_local_argument(
        program: &dyn ::validation::CheckingInput,
        borrow: &BorrowFacts,
        machine: SymbolHandle,
        state: &State,
        call: &FlowCallFact,
        storage: &CanonicalPlace,
        access_place: &CanonicalPlace,
        target_type: TypeReferenceHandle,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let symbol = storage.root.as_symbol()?;
        let local = primitive_local_before(program, state, call.statement_index, symbol)?;
        let target_access = structural_access_for_type_reference(program, target_type)?;
        let access = exact_structural_borrow_access(
            program,
            borrow,
            machine,
            state.symbol,
            call,
            access_place,
            target_access,
            &[],
        )?;
        let type_identity = base_type_identity(program, target_type, &[], &[])?;
        if !storage.segments.is_empty()
            || target_access == CheckedStructuralAccess::Owned
            || access != target_access
            || base_type_identity(program, local.type_reference, &[], &[])? != type_identity
        {
            return None;
        }
        Some(CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source: if local.is_mutable {
                CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol }
            } else {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
            },
            path: Vec::new(),
            type_identity,
            access,
        })
    }
}
mod dynamic_call {
    /*
        A selector call `Trait::member(selector, data...)` from a Unit body.

        Checking retargets the call to its instantiated slot
        (`monomorphization::dynamic_slots`), whose roster is the selector followed
        by the requirement's instantiated formals. The selector is planned exactly
        like any scalar operand and then moved into the route, so the plan carries
        the dispatch operand apart from the data arguments every implementation
        receives. Data arguments take the same checked-body custody an ordinary
        call's do, because every selectable implementation is a checked body; no
        implementation's body is inspected to plan them.

        Owned structural results use the ordinary result-custody judgment against
        the public slot signature. Returned loans still require a public origin
        contract; erased or mutable owned scalar formals retain their existing
        proof-lane coverage boundaries. Suspension retains the ordinary call frontier.
        A requirement contract needs no route evidence: checking discharged its
        `requires` against these operands, and every selectable implementation
        refines it where its conformance is checked.
    */
    use super::CallTarget;
    use super::ResolvedCallTarget;
    use super::call_operations::call_value_result_matches;
    use super::structural_result_call::with_structural_custody;
    use crate::execution::terminal_unit::calls::argument_paths::checked_call_scalar_arguments;
    use crate::execution::terminal_unit::calls::structural_arguments::{
        call_claim_transfers, structural_call_arguments,
    };
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::types::is_unit;
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedStructuralScalarParameterPlan, CheckedTrivialAffineStructuralLocalPlan,
        CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, PrimitiveType,
        SymbolHandle, strips_erased_parameter,
    };
    use checked_trees::{
        CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
        CheckedUnitCallTarget, CheckedUnitCallTransfers,
    };

    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        caller_parameters: &[CheckedUnitStructuralParameterPlan],
        caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        call: &checked_trees::FlowCallFact,
        expected_call_result: Option<&super::ExpectedCallValueResult<'_>>,
        caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
        caller_operations: &[CheckedUnitEffectOperationPlan],
        trace: &LocalConstructionTrace,
        coordinate: CheckedUnitCallCoordinate,
        call_site: &validation::CallSite<'_>,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        slot: &typed_trees::dynamic_traits::DynamicSlotSignature,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let phase = |name: &'static str| {
            trace.phase(name);
            trace.statement(Some(coordinate.statement_index));
        };
        phase("call operation: dynamic result shape");
        let result_matches = match expected_call_result {
            None => is_unit(program, slot.return_type),
            Some(expected) => call_value_result_matches(program, slot.return_type, expected, &[]),
        };
        if !result_matches {
            return None;
        }
        let requirement = program.dynamic_slot_requirement(slot)?;
        phase("call operation: dynamic structural arguments");
        let structural_arguments = structural_call_arguments(
            program,
            shapes,
            facts,
            call,
            machine,
            state,
            caller_parameters,
            caller_trivial_affine_locals,
            CallTarget::dynamic_slot(slot),
            call_site,
            call.statement_index,
            caller_structural_results,
            caller_operations,
            trace,
        )?;
        phase("call operation: dynamic scalar parameters");
        let mut scalar_parameters = Vec::new();
        for (position, parameter) in slot.parameters.iter().enumerate() {
            if strips_erased_parameter(parameter)? {
                return None;
            }
            let Some(primitive_type) = program.primitive_type_reference(parameter.type_reference)
            else {
                continue;
            };
            if parameter.is_const || parameter.is_mutable {
                return None;
            }
            scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                source_position: u32::try_from(position).ok()?,
                primitive_type,
            });
        }
        if scalar_parameters
            .first()
            .map(|parameter| (parameter.source_position, parameter.primitive_type))
            != Some((0, PrimitiveType::Selector))
        {
            return None;
        }
        phase("call operation: dynamic scalar arguments");
        let mut scalar_arguments = checked_call_scalar_arguments(
            facts,
            state.symbol,
            coordinate,
            &scalar_parameters,
            false,
        )?;
        if scalar_arguments.is_empty() {
            return None;
        }
        let selector = scalar_arguments.remove(0);
        phase("call operation: dynamic family operands");
        let family_operands = dynamic_family_operands(
            program,
            facts,
            state.symbol,
            coordinate,
            call_site,
            requirement,
        )?;
        let structural_result = match expected_call_result {
            Some(super::ExpectedCallValueResult::Structural(result)) => Some(*result),
            _ => None,
        };
        phase("call operation: dynamic claim transfers");
        let transfers = if structural_result.is_none() {
            call_claim_transfers(
                program,
                facts,
                machine.symbol,
                state.symbol,
                call,
                caller_parameters,
                caller_structural_results,
                &structural_arguments,
            )
            .inspect_err(|reason| phase(reason))
            .ok()?
        } else {
            Vec::new()
        };
        let (target_machine, target_state) = ResolvedCallTarget::DynamicSlot(slot).identity();
        let parts = CheckedUnitCallParts {
            coordinate,
            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
            source_site,
            target: CheckedUnitCallTarget {
                machine: target_machine,
                state: target_state,
                contract_report_fingerprint: 0,
                service_reach: call.service_reach,
            },
            route: CheckedUnitCallRoute::Dynamic {
                selector,
                interface_identity: slot.interface_identity,
                requirement: slot.requirement,
                family_operands,
            },
            scalar_arguments,
            structural_arguments,
            result: structural_result.map_or(CheckedUnitCallResult::Unit, |binding| {
                CheckedUnitCallResult::Structural {
                    binding: binding.clone(),
                    discard_result_on_return: binding.multiplicity
                        == language_semantics::Multiplicity::Affine,
                }
            }),
            transfers: if structural_result.is_some() {
                CheckedUnitCallTransfers::StructuralCustody(Default::default())
            } else {
                CheckedUnitCallTransfers::Claims(transfers)
            },
        };
        if structural_result.is_some() {
            phase("call operation: dynamic structural custody");
            with_structural_custody(program, facts, machine.symbol, state.symbol, trace, parts)
        } else {
            CheckedUnitCallPlan::new(parts).map(CheckedUnitEffectOperationPlan::Call)
        }
    }

    /*
        The evaluated row-selection actuals of a finite-family call: every
        `DynamicFamilyArgument` expression the scalar plan recorded for the call,
        in roster-binder order and checked against each binder's declared
        primitive type. A call carrying no family actuals lowers with an empty
        roster only when its requirement declares no finite family; any other
        generic-argument shape keeps refusing here.
    */
    fn dynamic_family_operands(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_state: SymbolHandle,
        coordinate: CheckedUnitCallCoordinate,
        call_site: &validation::CallSite<'_>,
        requirement: &typed_trees::signature::StateSignature,
    ) -> Option<Vec<checked_trees::CheckedCallScalarArgument>> {
        let machine_arguments: &[typed_trees::expression::StaticMachineArgument] = match call_site {
            validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
            validation::CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
            validation::CallSite::TransitionNamed { .. } => &[],
        };
        let typed_trees::finite_family::FamilyProbe::Finite { arity, .. } =
            program.finite_signature_family(requirement)
        else {
            return machine_arguments.is_empty().then(Vec::new);
        };
        if machine_arguments.len() != arity {
            return None;
        }
        let binders = program
            .state_signature_type_parameters(requirement)
            .iter()
            .filter(|parameter| parameter.kind.is_const_or_value())
            .collect::<Vec<_>>();
        if binders.len() != arity {
            return None;
        }
        machine_arguments
            .iter()
            .enumerate()
            .map(|(family_ordinal, _)| {
                let role = checked_trees::CheckedScalarExpressionRole::DynamicFamilyArgument {
                    call_ordinal: coordinate.call_ordinal,
                    family_ordinal: u32::try_from(family_ordinal).ok()?,
                };
                let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                    caller_state,
                    coordinate.statement_index,
                    role,
                )?;
                let binder = binders[family_ordinal];
                let expected = program.primitive_type_reference(match binder.kind {
                    typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                        type_reference
                    }
                    _ => return None,
                })?;
                (crate::values::scalar_expression_type(expression)? == expected)
                    .then(|| checked_trees::CheckedCallScalarArgument::Pure(expression.clone()))
            })
            .collect()
    }
}
mod parameter_argument_paths {
    /*
        Parameter-rooted argument paths: which caller parameter an argument place
        names, and which checked path its segments lend to the callee formal. Each
        call lane keeps one admission table whose first admitting row decides. The
        formal's access stays an `Option` because a nested reference has none: it
        rejects the argument at the first row that reads it, never earlier, so a
        row deciding without it keeps its outcome and its caller's trace phase.
    */
    use super::argument_paths::borrowed_argument_endpoint;
    use super::argument_presentation::classify;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::calls::argument_paths::{
        fixed_byte_array_range_path, open_byte_range_path, projected_argument_path,
        projected_borrowed_path, projected_evaluated_path,
    };
    use crate::execution::terminal_unit::calls::argument_presentation::ArgumentPresentation;
    use crate::execution::terminal_unit::calls::boundary_admission::fixed_byte_array_view_is_admitted;
    use crate::execution::terminal_unit::types::{
        base_type_identity, byte_sequence_carrier, parameter_is_root,
    };
    use crate::execution::terminal_unit::{
        CheckedStructuralAccess, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralPathSegment, MachineSupplyMode, Multiplicity, StateParameter,
        SymbolHandle, TypeReferenceHandle,
    };
    use CheckedStructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    use checked_trees::CheckedByteSequenceCarrier;
    use facts::PlaceSegment;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::state_parameter_position;
    use typed_trees::type_identity::TypeIdentityId;
    use validation::CanonicalPlace;

    type ArgumentPath = Vec<CheckedUnitStructuralPathSegment>;

    /*
        The caller parameter an argument place is rooted at, with the dense index
        of its structural plan.
    */
    #[derive(Clone, Copy)]
    pub(crate) struct SourceParameter<'parameters> {
        pub(crate) declaration: &'parameters StateParameter,
        pub(crate) plan: &'parameters CheckedUnitStructuralParameterPlan,
        pub(crate) index: usize,
    }

    impl<'parameters> SourceParameter<'parameters> {
        pub(crate) fn resolve(
            declarations: &'parameters [StateParameter],
            plans: &'parameters [CheckedUnitStructuralParameterPlan],
            root: SymbolHandle,
        ) -> Option<Self> {
            let declaration = declarations
                .iter()
                .find(|parameter| parameter_is_root(parameter, root))?;
            let position = state_parameter_position(declarations, declaration.symbol)?;
            let position = u32::try_from(position).unwrap_or(u32::MAX);
            let (index, plan) = plans
                .iter()
                .enumerate()
                .find(|(_, plan)| plan.position == position)?;
            Some(Self {
                declaration,
                plan,
                index,
            })
        }

        /* What the parameter lends to a formal of `target_access`. */
        pub(crate) fn custody(
            self,
            target_access: Option<CheckedStructuralAccess>,
        ) -> ArgumentCustody {
            ArgumentCustody {
                access: self.plan.access,
                multiplicity: self.plan.multiplicity,
                unqualified: self.plan.qualifications.is_empty(),
                target_access,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ArgumentCustody {
        pub(crate) access: CheckedStructuralAccess,
        pub(crate) multiplicity: Multiplicity,
        pub(crate) unqualified: bool,
        pub(crate) target_access: Option<CheckedStructuralAccess>,
    }

    impl ArgumentCustody {
        fn lends_borrow(self) -> bool {
            matches!(self.access, MutableBorrow | SharedBorrow)
        }

        /* The custody that lends a fixed byte-array window or view. */
        pub(crate) fn lends_unrestricted_borrow(self) -> bool {
            self.lends_borrow()
                && self.multiplicity == Multiplicity::Unrestricted
                && self.unqualified
        }

        /*
            The custody that may lend a fixed byte-array window to a call: an
            unrestricted unqualified borrow, or unrestricted owned storage whose
            window the callee reads in place through the call-scoped path. The
            fixed-array-as-view admission inside the row still decides the formal
            shape, so this predicate names only which caller custody can lend.
        */
        pub(crate) fn lends_fixed_byte_window(self) -> bool {
            self.lends_unrestricted_borrow()
                || (self.access == Owned
                    && self.multiplicity == Multiplicity::Unrestricted
                    && self.unqualified)
        }
    }

    /* An argument place's segments below its root, classified once. */
    enum ArgumentPathShape {
        Whole,
        /* A fixed window closing any prefix, `[.., FixedRange]`. */
        FixedRange,
        /* Statically selected storage, including exact case payloads. */
        Static {
            fields: usize,
            fixed_indexes: usize,
            case_qualified: bool,
        },
        /*
            An unnormalized `Index` (a runtime element or an open-ended range)
            among field and fixed-index selectors.
        */
        RuntimeIndexed,
        /* An interior range, which no argument path spells. */
        Unsupported,
    }

    impl ArgumentPathShape {
        fn of(segments: &[PlaceSegment]) -> Self {
            use facts::PlaceSegment::{Case, Field, FixedIndex, FixedRange, Index, OpaqueRange};
            match segments.last() {
                None => return Self::Whole,
                Some(FixedRange { .. }) => return Self::FixedRange,
                Some(_) => {}
            }
            let (mut fields, mut fixed_indexes, mut runtime_indexed) = (0, 0, false);
            let mut case_qualified = false;
            for segment in segments {
                match segment {
                    Field { .. } => fields += 1,
                    FixedIndex { .. } => fixed_indexes += 1,
                    Index { .. } => runtime_indexed = true,
                    Case { .. } => case_qualified = true,
                    FixedRange { .. } | OpaqueRange => return Self::Unsupported,
                }
            }
            if runtime_indexed && case_qualified {
                Self::Unsupported
            } else if runtime_indexed {
                Self::RuntimeIndexed
            } else {
                Self::Static {
                    fields,
                    fixed_indexes,
                    case_qualified,
                }
            }
        }
    }

    /*
        One parameter-rooted argument: the canonical place the caller names and
        the callee formal it must present as.
    */
    #[derive(Clone, Copy)]
    pub(crate) struct ParameterArgument<'argument> {
        pub(crate) program: &'argument dyn ::validation::CheckingInput,
        pub(crate) machine: SymbolHandle,
        pub(crate) state: SymbolHandle,
        pub(crate) statement_index: usize,
        pub(crate) place: &'argument CanonicalPlace,
        pub(crate) target: TypeReferenceHandle,
        pub(crate) target_identity: TypeIdentityId,
        pub(crate) selectors:
            Option<&'argument dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>>,
    }

    /* The path an ordinary call lends from a caller parameter. */
    pub(crate) fn ordinary_parameter_path(
        argument: ParameterArgument<'_>,
        custody: ArgumentCustody,
        supply_mode: MachineSupplyMode,
    ) -> Option<ArgumentPath> {
        let checked_body = supply_mode == MachineSupplyMode::CheckedBody;
        let unrestricted = custody.multiplicity == Multiplicity::Unrestricted;
        let access = custody.access;
        match ArgumentPathShape::of(&argument.place.segments) {
            ArgumentPathShape::Whole => Some(Vec::new()),
            _ if !custody.unqualified => None,
            ArgumentPathShape::FixedRange
                if (checked_body || supply_mode.is_boundary_declaration())
                    && custody.lends_fixed_byte_window() =>
            {
                argument.fixed_range_path()
            }
            ArgumentPathShape::Static { fixed_indexes, .. }
                if checked_body
                    && custody.lends_borrow()
                    && matches!(custody.target_access?, MutableBorrow | SharedBorrow)
                    && argument.target_is_byte_view() =>
            {
                argument.presented_projection_path(&[], unrestricted && fixed_indexes == 0)
            }
            ArgumentPathShape::Static { .. }
                if checked_body
                    && unrestricted
                    && matches!(
                        (access, custody.target_access?),
                        (
                            MutableBorrow,
                            SharedBorrow | MutableBorrow | WriteOnlyBorrow
                        ) | (SharedBorrow, SharedBorrow)
                            | (WriteOnlyBorrow, WriteOnlyBorrow)
                    ) =>
            {
                argument.slice_view_projection_path()
            }
            ArgumentPathShape::Static { .. } | ArgumentPathShape::RuntimeIndexed
                if checked_body
                    && matches!(access, Owned | MutableBorrow | SharedBorrow)
                    && custody.target_access? == Owned
                    && validation::reference_result_custody::source_value_is_copied(
                        argument.program,
                        argument.target,
                    ) =>
            {
                argument.exact_projection_path()
            }
            ArgumentPathShape::Static { .. } | ArgumentPathShape::RuntimeIndexed
                if checked_body
                    && custody.multiplicity == Multiplicity::Affine
                    && access == Owned
                    && custody.target_access? == Owned =>
            {
                argument.exact_projection_path()
            }
            ArgumentPathShape::Static {
                case_qualified: true,
                ..
            } => None,
            ArgumentPathShape::Static {
                fields: 0,
                fixed_indexes: 1..=3,
                ..
            } => argument.exact_projection_path(),
            ArgumentPathShape::Static { fields: 0, .. }
                if checked_body
                    && access == WriteOnlyBorrow
                    && custody.target_access? == WriteOnlyBorrow =>
            {
                argument.exact_projection_path()
            }
            ArgumentPathShape::Static {
                fixed_indexes: 0, ..
            } => argument.exact_projection_path(),
            ArgumentPathShape::RuntimeIndexed
                if checked_body
                    && match custody.target_access? {
                        SharedBorrow => {
                            custody.lends_borrow()
                                || (access == Owned && custody.multiplicity == Multiplicity::Affine)
                        }
                        MutableBorrow => access == MutableBorrow,
                        _ => false,
                    } =>
            {
                argument.runtime_indexed_path(custody)
            }
            _ => None,
        }
    }

    /*
        The path a boundary-requirement call lends from a caller parameter: a
        fixed byte window from an unrestricted borrow, and any other projection
        through the formal's ABI presentation. The caller's access replay is the
        remaining custody check; a runtime selector passes only when it folds to a
        constant.
    */
    pub(crate) fn boundary_parameter_path(
        argument: ParameterArgument<'_>,
        custody: ArgumentCustody,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<ArgumentPath> {
        match ArgumentPathShape::of(&argument.place.segments) {
            ArgumentPathShape::Whole => Some(Vec::new()),
            _ if !custody.unqualified => None,
            ArgumentPathShape::FixedRange if custody.lends_fixed_byte_window() => {
                argument.fixed_range_path()
            }
            ArgumentPathShape::Static {
                case_qualified: false,
                ..
            }
            | ArgumentPathShape::RuntimeIndexed => {
                argument.presented_projection_path(substitutions, true)
            }
            _ => None,
        }
    }

    impl ParameterArgument<'_> {
        /*
            Whole storage and projected storage use the same carrier presentation.
            This proves representation, not permission: the call's access and
            predicate checks still govern the loan. The separate fixed-array rule
            retains its unrestricted, unqualified custody requirement.
        */
        pub(crate) fn whole_parameter_is_admitted(
            self,
            source: SourceParameter<'_>,
            byte_view: bool,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        ) -> bool {
            source.plan.type_identity == self.target_identity
                || classify(
                    self.program,
                    source.declaration.type_reference,
                    self.target,
                    self.target_identity,
                    substitutions,
                )
                .is_some_and(|presentation| match presentation {
                    ArgumentPresentation::FixedArraySlice => {
                        byte_view
                            && fixed_byte_array_view_is_admitted(
                                self.program,
                                source.declaration.type_reference,
                                self.target,
                            )
                    }
                    _ => true,
                })
        }

        fn target_is_byte_view(self) -> bool {
            byte_sequence_carrier(self.program, self.target, &[]).is_some_and(|carrier| {
                carrier.is_borrowed()
                    || matches!(carrier, CheckedByteSequenceCarrier::BoundedOwned { .. })
            })
        }

        fn fixed_range_path(self) -> Option<ArgumentPath> {
            let (program, place, target) = (self.program, self.place, self.target);
            fixed_byte_array_range_path(program, self.state, self.statement_index, place, target)
                .map(|(_, path)| path)
        }

        fn exact_projection_path(self) -> Option<ArgumentPath> {
            let ProjectedArgumentPath {
                storage_type: selected,
                path,
                ..
            } = projected_evaluated_path(
                self.program,
                self.machine,
                self.state,
                self.statement_index,
                self.place,
                self.selectors,
            )?;
            let (selected, path) =
                borrowed_argument_endpoint(self.program, selected, path, self.target)?;
            (base_type_identity(self.program, selected, &[], &[])? == self.target_identity)
                .then_some(path)
        }

        /*
            A projection at the formal's identity, or a fixed array lent as the
            formal's borrowed slice view.
        */
        fn slice_view_projection_path(self) -> Option<ArgumentPath> {
            let (program, target) = (self.program, self.target);
            let ProjectedArgumentPath {
                storage_type: projected,
                path,
                ..
            } = projected_borrowed_path(
                program,
                self.machine,
                self.state,
                self.statement_index,
                self.place,
            )?;
            let (projected, path) = borrowed_argument_endpoint(program, projected, path, target)?;
            let presentation = classify(program, projected, target, self.target_identity, &[])?;
            (presentation != ArgumentPresentation::ByteSequence).then_some(path)
        }

        /*
            A projection the formal presents through the carrier presentation
            rule or, when `byte_view` allows, as a fixed byte array's byte view.
        */
        fn presented_projection_path(
            self,
            substitutions: &[(SymbolHandle, TypeReferenceHandle)],
            byte_view: bool,
        ) -> Option<ArgumentPath> {
            let (program, target) = (self.program, self.target);
            let ProjectedArgumentPath {
                storage_type: projected,
                path,
                ..
            } = projected_argument_path(
                program,
                self.state,
                self.statement_index,
                self.place,
                None,
                None,
            )?;
            let (projected, path) = borrowed_argument_endpoint(program, projected, path, target)?;
            classify(
                program,
                projected,
                target,
                self.target_identity,
                substitutions,
            )
            .is_some_and(|presentation| match presentation {
                ArgumentPresentation::FixedArraySlice => {
                    byte_view && fixed_byte_array_view_is_admitted(program, projected, target)
                }
                _ => true,
            })
            .then_some(path)
        }

        /*
            The open-ended spelling `x[..]`/`x[a..]` survives canonicalization as
            an `Index` carrying its range, since its end is not an authored
            constant; against a fixed byte array it is the fixed-range argument
            ending at the backing's literal extent. Any other `Index` keeps the
            runtime-selector route: `projected_borrowed_path` retains the selector
            only when the caller's published integer range proves it inside the
            enclosing fixed array's extent, and terminal verification replays it.
        */
        fn runtime_indexed_path(self, custody: ArgumentCustody) -> Option<ArgumentPath> {
            let (program, state, statement_index, place) =
                (self.program, self.state, self.statement_index, self.place);
            if custody.lends_unrestricted_borrow()
                && let Some((_, path)) =
                    open_byte_range_path(program, state, statement_index, place, self.target)
            {
                return Some(path);
            }
            let ProjectedArgumentPath {
                storage_type: projected,
                path,
                ..
            } = projected_evaluated_path(
                program,
                self.machine,
                state,
                statement_index,
                place,
                self.selectors,
            )?;
            let (projected, path) =
                borrowed_argument_endpoint(program, projected, path, self.target)?;
            let presentation =
                classify(program, projected, self.target, self.target_identity, &[])?;
            (presentation != ArgumentPresentation::FixedArraySlice
                || custody.lends_unrestricted_borrow())
            .then_some(path)
        }
    }
}
mod reference_forwarding {
    /*
        A bare reference parameter reads its carrier without weakening its referent.
    */
    use crate::execution::terminal_unit::CheckedStructuralAccess;
    use crate::execution::terminal_unit::ExpressionNode;
    use crate::execution::terminal_unit::SymbolHandle;
    use crate::execution::terminal_unit::types::structural_access_for_type_reference;
    use crate::flow::expressions;
    use typed_trees::statement::StatementNode;

    use crate::execution::terminal_unit::types::byte_sequence_carrier;

    pub(super) fn preserves_mutable_referent(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        borrow_state: &checked_trees::StateBorrowFact,
        borrow_call: &checked_trees::BorrowCallFact,
        call: &checked_trees::FlowCallFact,
        source_symbol: SymbolHandle,
        returned_loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
    ) -> bool {
        exact_mutable_referent(
            program,
            borrow,
            borrow_state,
            borrow_call,
            call,
            source_symbol,
            returned_loans,
        )
        .is_some()
    }

    fn exact_mutable_referent(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        borrow_state: &checked_trees::StateBorrowFact,
        borrow_call: &checked_trees::BorrowCallFact,
        call: &checked_trees::FlowCallFact,
        source_symbol: SymbolHandle,
        returned_loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
    ) -> Option<()> {
        let state = validation::find_state_in_machine(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
        )?;
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let mut sources = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.symbol == source_symbol);
        let source = sources.next()?;
        if (sources.next().is_some() || !source_symbol.is_valid())
            || source.is_const
            || structural_access_for_type_reference(program, source.type_reference)?
                != CheckedStructuralAccess::MutableBorrow
        {
            return None;
        }
        prefix_preserves_parameter(program, borrow, borrow_state, call, source_symbol)?;
        let site = validation::find_call_site(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let target_symbol = match &site {
            validation::CallSite::Statement(authored) => authored.target_symbol,
            validation::CallSite::Expression {
                expression,
                call: authored,
            } if *expression == call.authored_expression => {
                validation::call_target_symbol(program, authored)
            }
            _ => return None,
        };
        if !target_symbol.is_valid()
            || validation::call_target_entry_symbol(program, target_symbol) != call.target_symbol
        {
            return None;
        }
        let parameters = validation::call_target_parameters(program, target_symbol)?;
        let arguments = validation::call_site_argument_expressions(program, &site);
        let formals: Vec<&typed_trees::signature::StateParameter> = parameters.iter().collect();
        if formals.len() != arguments.len() || formals.iter().any(|parameter| parameter.is_const) {
            return None;
        }
        let mut matched = false;
        for (parameter, expression) in formals.iter().zip(arguments) {
            let Some(path) = program.expression_table.name_path_node(*expression) else {
                continue;
            };
            if path.symbol != source_symbol {
                continue;
            }
            let members = program.expression_table.name_path_members(path.members);
            if matched
                || path.head_symbol != source_symbol
                || members.len() != 1
                || members[0].as_str() != source.name.as_str()
                || !parameter.symbol.is_valid()
                || structural_access_for_type_reference(program, parameter.type_reference)?
                    != CheckedStructuralAccess::MutableBorrow
            {
                return None;
            }
            matched = true;
        }
        if !matched {
            return None;
        }
        let mut accesses = borrow
            .argument_accesses
            .span_or_empty(borrow_call.accesses)
            .iter()
            .filter(|access| access.root == facts::PlaceRoot::Symbol(source_symbol));
        let access = accesses.next()?;
        if accesses.next().is_some()
            || !borrow.access_segments(access).is_empty()
            || access.kind != checked_trees::BorrowAccessKind::Read
        {
            return None;
        }
        let result_owner = program
            .statement_table
            .statement_at(state, call.statement_index)
            .and_then(|statement| match statement {
                StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            });
        if borrow.state_loans(borrow_state).any(|(handle, loan)| {
            !returned_loans.contains(&handle)
                && order.at_or_before(loan.statement_index, call.statement_index)
                && order.at_or_before(call.statement_index, loan.last_use_statement_index)
                && (loan.root == facts::PlaceRoot::Symbol(source_symbol)
                    || loan.owner_symbol == source_symbol
                    || loan.source_owner_symbol == source_symbol)
                && !(loan.statement_index == call.statement_index
                    && Some(loan.owner_symbol) == result_owner)
        }) {
            return None;
        }
        Some(())
    }

    /*
        Whether the carrier still names the referent it arrived with at this call.

        A `record: &mut Record` parameter is a reference value; forwarding it bare
        reborrows whatever referent that value names now. Contents changing is not
        the hazard: a projected store (`record.field = ..`) or a completed call
        that reborrowed the bare carrier writes through the referent and leaves the
        carrier naming the same storage. Only a write to the whole parameter place
        can replace the carrier, so only such a write refuses. The static-machine
        contract spells every helper call this way (`Record::step(record)` then
        `Record::step(record)` again), so refusing referent writes would refuse
        the second call of every such sequence.
    */
    fn prefix_preserves_parameter(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        borrow_state: &checked_trees::StateBorrowFact,
        call: &checked_trees::FlowCallFact,
        source_symbol: SymbolHandle,
    ) -> Option<()> {
        let state = validation::find_state_in_machine(
            program,
            borrow_state.machine_symbol,
            borrow_state.state_symbol,
        )?;
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let call_frames = validation::CallFrameResolver::new(program);
        for (statement_index, statement) in program
            .statement_table
            .admitted_execution_statements(state.statement_nodes)
            .take(order.position(call.statement_index)?)
        {
            let writes = crate::flow::statement_storage_writes(
                program,
                borrow_state.machine_symbol,
                borrow_state.state_symbol,
                statement_index,
                statement,
                call_frames.as_ref(),
            )?;
            if writes
                .iter()
                .any(|place| replaces_carrier(place, source_symbol))
            {
                return None;
            }
        }
        let summaries = crate::flow::StateMutationSummaryCache::default();
        for preceding in borrow.calls.span_or_empty(borrow_state.calls) {
            if order.precedes(call.statement_index, preceding.statement_index)
                || (preceding.statement_index == call.statement_index
                    && preceding.call_ordinal == call.call_ordinal)
            {
                continue;
            }
            let writes = crate::flow::call_mutated_places(
                program,
                borrow_state.machine_symbol,
                borrow_state.state_symbol,
                borrow,
                preceding,
                &summaries,
                call_frames.as_ref(),
            )?;
            let writes = crate::flow::close_storage_places_over_aliases(
                program,
                borrow_state.machine_symbol,
                borrow_state.state_symbol,
                preceding.statement_index,
                writes,
                call_frames.as_ref(),
            )?;
            if writes
                .iter()
                .any(|place| replaces_carrier(place, source_symbol))
                && !(order.precedes(preceding.statement_index, call.statement_index)
                    && preceding_reborrow_preserves_carrier(
                        program,
                        borrow,
                        borrow_state,
                        preceding,
                        source_symbol,
                    ))
            {
                return None;
            }
        }
        Some(())
    }

    /*
        A write rooted at the parameter replaces its carrier only when it names
        the whole parameter place; a projected write lands inside the referent.
    */
    fn replaces_carrier(place: &validation::CanonicalPlace, source_symbol: SymbolHandle) -> bool {
        place.root == facts::PlaceRoot::Symbol(source_symbol) && place.segments.is_empty()
    }

    /*
        A completed call that received the bare carrier through a borrowed formal
        held only a reborrow of the referent: it can change the referent, whole or
        in part, but has no route to the caller's carrier binding. Byte views are
        the same case through their view formals. This does not admit assignments
        or restore live loans.
    */
    fn preceding_reborrow_preserves_carrier(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        state: &checked_trees::StateBorrowFact,
        call: &checked_trees::BorrowCallFact,
        source_symbol: SymbolHandle,
    ) -> bool {
        let Some(source_state) =
            validation::find_state_in_machine(program, state.machine_symbol, state.state_symbol)
        else {
            return false;
        };
        let Some(source) = program.state_parameter_by_symbol(source_state, source_symbol) else {
            return false;
        };
        let Some(validation::CallSite::Statement(authored)) = validation::find_call_site(
            program,
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        ) else {
            return false;
        };
        if validation::call_target_entry_symbol(program, authored.target_symbol)
            != call.target_symbol
        {
            return false;
        }
        let Some(parameters) = validation::call_target_parameters(program, call.target_symbol)
        else {
            return false;
        };
        let arguments = program
            .statement_table
            .expression_handles(authored.arguments);
        let parameters = parameters.iter();
        if parameters.clone().count() != arguments.len() {
            return false;
        }
        let mut matching = parameters.zip(arguments).filter(|(_, expression)| {
            matches!(program.expression_table.expression(**expression), ExpressionNode::Name(name) if name.symbol == source_symbol)
        });
        let Some((parameter, expression)) = matching.next() else {
            return false;
        };
        if (matching.next().is_some())
            || parameter.is_const
            || !(crate::execution::terminal_unit::calls::boundary_admission::fixed_byte_array_view_is_admitted(
                program,
                source.type_reference,
                parameter.type_reference,
            ) || [source.type_reference, parameter.type_reference]
                .into_iter()
                .all(|reference| {
                    structural_access_for_type_reference(program, reference)
                        == Some(CheckedStructuralAccess::MutableBorrow)
                        && byte_sequence_carrier(program, reference, &[])
                            .is_some_and(checked_trees::CheckedByteSequenceCarrier::is_borrowed)
                })
                || (structural_access_for_type_reference(program, source.type_reference)
                    == Some(CheckedStructuralAccess::MutableBorrow)
                    && matches!(
                        structural_access_for_type_reference(program, parameter.type_reference),
                        Some(
                            CheckedStructuralAccess::MutableBorrow
                                | CheckedStructuralAccess::SharedBorrow
                                | CheckedStructuralAccess::WriteOnlyBorrow
                        )
                    )))
        {
            return false;
        }
        let Some(name) = program.expression_table.name_path_node(*expression) else {
            return false;
        };
        if name.head_symbol != source_symbol
            || expressions(program).name_path_members(name.members).len() != 1
        {
            return false;
        }
        let mut accesses = borrow
            .argument_accesses
            .span_or_empty(call.accesses)
            .iter()
            .filter(|access| access.root == facts::PlaceRoot::Symbol(source_symbol));
        let Some(access) = accesses.next() else {
            return false;
        };
        accesses.next().is_none()
            && borrow.access_segments(access).is_empty()
            && matches!(
                access.kind,
                checked_trees::BorrowAccessKind::Read | checked_trees::BorrowAccessKind::Mutable
            )
    }
}
mod result_arguments {
    /*
        Result operands retain exact source access, ownership, and projected storage.
        Projection admission belongs to that source/formal correspondence and its
        custody, not whether the consuming call is a statement or a value operand.
        A completed producer has already established its declared contents. This
        planner preserves type, qualification, access and storage correspondence;
        it does not admit the value again through a contents whitelist. Source flow
        checks copyability and loans, and call_claim_transfers carries owned claims.
    */
    use crate::execution::terminal_unit::CheckFacts;
    use crate::execution::terminal_unit::CheckedStructuralAccess;
    use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
    use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
    use crate::execution::terminal_unit::CheckedUnitStructuralResultBindingPlan;
    use crate::execution::terminal_unit::ExpressionNode;
    use crate::execution::terminal_unit::Multiplicity;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::execution::terminal_unit::types::borrowed_named_view;
    use crate::execution::terminal_unit::types::slice_view_carrier;
    use crate::execution::terminal_unit::types::stored_type_presents_borrowed;
    use crate::execution::terminal_unit::types::{
        base_type_identity, structural_access_for_type_reference,
    };
    use crate::flow::expressions;
    use crate::flow::state_statements;
    use crate::published::places::place_at;
    use crate::published::semantic::calls::shared_argument_referent;
    use checked_trees::CheckedUnitEffectOperationPlan;
    use checked_trees::CheckedUnitStructuralPathSegment;
    use checked_trees::FlowCallFact;
    use checked_trees::checked_borrowed_access_pair_pattern;
    use facts::PlaceRoot;
    use facts::PlaceSegment;
    use language_semantics::ReferenceAccess;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::state::State;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;
    use validation::CanonicalPlace;
    use validation::assignment_value_type;
    use validation::canonical_place_type_reference;
    use validation::declared_place_type_raw;
    use validation::find_state;
    use validation::has_builtin_subslice_meaning;
    use validation::reference_result_custody::result_multiplicity;
    use validation::stored_structural_type_identity;
    use validation::structural_result_contract;
    use validation::structural_result_qualifications;
    use validation::structural_type_identity;

    use crate::execution::terminal_unit::StatementNode;
    use crate::execution::terminal_unit::SymbolHandle;

    use crate::execution::terminal_unit::calls::argument_paths::projected_evaluated_path;
    use crate::execution::terminal_unit::calls::argument_presentation::ArgumentPresentation;
    use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_argument_access;

    mod anonymous_shared {
        /*
            A shared call retains its complete temporary owner through normal completion.
            Admission rejoins the published loan and each exact claim occurrence;
            source checking owns eligibility and contextual hook requirements.
            The completed producer supplies its retained storage type. Re-inferring
            that type from a literal's leaves loses contextual construction identity;
            the receiving reference formal must not supply a replacement owner type.
            Callee supply mode does not change the caller's temporary lifetime: a
            boundary call retains the same owner and closes the same loan on return.
            Provider admission is separate from this exact occurrence/claim join.
        */
        use super::{Multiplicity, SymbolHandle};
        use crate::execution::terminal_unit::{
            PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
        };
        use crate::published::places::place_at;

        pub(crate) fn validate(
            program: &dyn ::validation::CheckingInput,
            flow: &checked_trees::FlowFacts,
            machine: SymbolHandle,
            state: SymbolHandle,
            consumer: &checked_trees::FlowCallFact,
            expression: typed_trees::expression::ExpressionHandle,
            owner_type: typed_trees::types::TypeReferenceHandle,
        ) -> Option<()> {
            let site = validation::find_call_site(
                program,
                machine,
                state,
                consumer.statement_index,
                consumer.call_ordinal,
            )?;
            let arguments = validation::call_site_argument_expressions(program, &site);
            let parameters = validation::call_target_parameters(program, consumer.target_symbol)?;
            if parameters.len() != arguments.len() {
                return None;
            }
            let root = facts::PlaceRoot::Expression(expression);
            let mut operands =
                arguments
                    .iter()
                    .zip(parameters.iter())
                    .filter_map(|(argument, parameter)| {
                        if parameter.is_const
                            || parameter.is_mutable
                            || parameter.relevance.is_erased()
                        {
                            return None;
                        }
                        let (borrowed, _) =
                            crate::published::semantic::calls::shared_argument_referent(
                                program,
                                *argument,
                                parameter.type_reference,
                            )?;
                        let place = place_at(program, state, consumer.statement_index, borrowed)?;
                        (place.root == root).then_some(place)
                    });
            let borrowed = operands.next()?;
            if operands.next().is_some() {
                return None;
            }
            let multiplicity = crate::checks::type_multiplicity(program, owner_type);
            let (source_machine, source_state) = validation::find_state(program, state)?;
            if source_machine.symbol != machine {
                return None;
            }
            let evaluation = crate::flow::call_permission_evaluation(
                program,
                source_machine,
                source_state,
                consumer.statement_index,
                consumer.call_ordinal,
            );
            if evaluation == checked_trees::FlowPermissionEvaluation::Unknown {
                return None;
            }
            let producer_source = if program.expression_table.call_node(expression).is_some() {
                let state_flow = flow.control.state(machine, state)?;
                let mut producers =
                    flow.control
                        .calls
                        .span(state_flow.calls)?
                        .iter()
                        .filter(|call| {
                            call.statement_index == consumer.statement_index
                                && call.authored_expression == expression
                        });
                let producer = producers.next()?;
                if producers.next().is_some()
                    || crate::flow::call_permission_evaluation(
                        program,
                        source_machine,
                        source_state,
                        producer.statement_index,
                        producer.call_ordinal,
                    ) != evaluation
                {
                    return None;
                }
                PermissionEventSource::Call {
                    statement_index: producer.statement_index,
                    call_ordinal: producer.call_ordinal,
                    target_symbol: producer.target_symbol,
                }
            } else {
                PermissionEventSource::Statement {
                    statement_index: consumer.statement_index,
                }
            };
            if multiplicity == Multiplicity::Unrestricted {
                return Some(());
            }
            if !crate::flow::whole_cleanup_is_eligible(program, owner_type) {
                return None;
            }
            let consumer_source = PermissionEventSource::Call {
                statement_index: consumer.statement_index,
                call_ordinal: consumer.call_ordinal,
                target_symbol: consumer.target_symbol,
            };
            let events = flow
                .ownership
                .permissions
                .iter()
                .map(|(_, event)| event)
                .filter(|event| {
                    event.machine_symbol == machine
                        && event.state_symbol == state
                        && event.root == root
                })
                .collect::<Vec<_>>();
            if events.iter().any(|event| event.evaluation != evaluation) {
                return None;
            }
            let mut loans = events
                .iter()
                .copied()
                .filter(|event| event.access == PermissionAccess::Shared);
            let loan = loans.next()?;
            if loans.next().is_some()
                || loan.kind != PermissionEventKind::Establish
                || loan.source != consumer_source
                || loan.multiplicity != Multiplicity::Unrestricted
                || loan.claim_identity != PermissionClaimIdentity::Unknown
                || loan.obligation_live
                || flow.ownership.segments.span(loan.segments)? != borrowed.segments.as_slice()
            {
                return None;
            }
            let frontier = validation::linear_claim_frontier(program, owner_type);
            let paths = if frontier.is_empty() {
                vec![(Vec::new(), Multiplicity::Affine, false)]
            } else {
                frontier
                    .into_iter()
                    .map(|claim| (claim.path, Multiplicity::Linear, claim.conditional))
                    .collect()
            };
            let mut covered = 1;
            for (path, claim_multiplicity, conditional) in paths {
                let mut establishments = events.iter().copied().filter(|event| {
                    event.access == PermissionAccess::Owned
                        && event.kind == PermissionEventKind::Establish
                        && flow.ownership.segments.span_or_empty(event.segments) == path
                });
                let establishment = establishments.next()?;
                if establishments.next().is_some()
                    || establishment.source != producer_source
                    || establishment.multiplicity != claim_multiplicity
                    || (claim_multiplicity == Multiplicity::Linear
                        && !establishment.obligation_live
                        && !conditional)
                    || (establishment.obligation_live
                        && establishment.claim_identity == PermissionClaimIdentity::Unknown)
                {
                    return None;
                }
                covered += 1;
                if claim_multiplicity == Multiplicity::Linear && !establishment.obligation_live {
                    continue;
                }
                let mut deaths = events.iter().copied().filter(|event| {
                    event.access == PermissionAccess::Owned
                        && event.kind
                            == if claim_multiplicity == Multiplicity::Linear {
                                PermissionEventKind::Consume
                            } else {
                                PermissionEventKind::AffineDrop
                            }
                        && flow.ownership.segments.span_or_empty(event.segments) == path
                });
                let death = deaths.next()?;
                if deaths.next().is_some()
                    || death.source != consumer_source
                    || death.multiplicity != claim_multiplicity
                    || death.obligation_live
                    || death.claim_identity != establishment.claim_identity
                    || death.provenance != establishment.provenance
                {
                    return None;
                }
                covered += 1;
            }
            (covered == events.len()).then_some(())
        }
    }
    mod bounded_owner {
        /*
            A whole bounded byte owner local lends its own storage to a call.

            An explicit variable-fill carrier retains its storage and live length.
            Lending it leaves the owner in place, so no custody moves and no claim
            transfers. Exact fixed arrays use ordinary fixed-array storage and loans;
            qualification never chooses a capacity or hidden live-length field.

            A shared byte-view formal may also observe the owner's live extent, using
            the ordinary whole-local argument or an explicit shared borrow. Other
            accesses require the same bounded carrier and explicit borrow spelling.
            Every formal domain must already be declared on the local, and each route
            retains the exact captured access. Terminal checks the current predicates
            and presentation independently; lending never changes the owner's invariants.
        */
        use super::{
            CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
            CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, SymbolHandle,
            exact_structural_argument_access, names_whole_local,
        };
        use crate::execution::terminal_unit::types::{
            byte_sequence_carrier, byte_sequence_type_identity,
            structural_access_for_type_reference,
        };
        use crate::flow::expressions;
        use checked_trees::CheckedByteSequenceCarrier;
        use typed_trees::types::TypeReferenceHandle;

        #[allow(clippy::too_many_arguments)]
        pub(super) fn loan(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: SymbolHandle,
            source_state: &typed_trees::state::State,
            call: &checked_trees::FlowCallFact,
            expression: typed_trees::expression::ExpressionHandle,
            place: &validation::CanonicalPlace,
            result: &CheckedUnitStructuralResultBindingPlan,
            parameter: TypeReferenceHandle,
            target_identity: typed_trees::type_identity::TypeIdentityId,
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let symbol = place.root.as_symbol()?;
            if !place.segments.is_empty() {
                return None;
            }
            let local = crate::flow::state_statements(program, source_state.statement_nodes)
                .get(usize::try_from(result.statement_index).ok()?)?
                .as_local_data()?;
            let CheckedByteSequenceCarrier::BoundedOwned { capacity } =
                byte_sequence_carrier(program, local.type_reference, &[])?
            else {
                return None;
            };
            let access = structural_access_for_type_reference(program, parameter)?;
            let formal_carrier = byte_sequence_carrier(program, parameter, &[])?;
            let shared_view = access == CheckedStructuralAccess::SharedBorrow
                && matches!(
                    formal_carrier,
                    CheckedByteSequenceCarrier::BorrowedView { .. }
                )
                && structural_access_for_type_reference(program, local.type_reference)
                    == Some(CheckedStructuralAccess::Owned)
                && super::super::argument_presentation::classify(
                    program,
                    local.type_reference,
                    parameter,
                    target_identity,
                    &[],
                )
                .is_some();
            if !shared_view
                && formal_carrier != (CheckedByteSequenceCarrier::BoundedOwned { capacity })
            {
                return None;
            }
            let expected_borrow = match access {
                CheckedStructuralAccess::SharedBorrow => {
                    language_semantics::ReferenceAccess::Shared
                }
                CheckedStructuralAccess::MutableBorrow => {
                    language_semantics::ReferenceAccess::Mutable
                }
                CheckedStructuralAccess::WriteOnlyBorrow => {
                    language_semantics::ReferenceAccess::WriteOnly
                }
                CheckedStructuralAccess::Owned => return None,
            };
            let value_expression = match program.expression_table.expression(expression) {
                ExpressionNode::Borrow(borrow) if borrow.access == expected_borrow => borrow.target,
                ExpressionNode::Name(_) if shared_view => expression,
                _ => return None,
            };
            if !expressions(program).expression_is_valid(value_expression)
                || local.symbol != symbol
                || !names_whole_local(program, call, value_expression, symbol)
                || result.multiplicity != program.type_multiplicity(local.type_reference)
                || !matches!(
                    result.multiplicity,
                    Multiplicity::Unrestricted | Multiplicity::Affine
                )
                || usize::try_from(result.statement_index).ok()? >= call.statement_index
                || (!shared_view && result.type_identity != target_identity)
                || byte_sequence_type_identity(program, parameter, &[], &[])? != target_identity
                || byte_sequence_type_identity(program, local.type_reference, &[], &[])?
                    != result.type_identity
            {
                return None;
            }
            let declared =
                validation::field_domains::domain_constraint_symbols(program, local.type_reference);
            if !validation::field_domains::domain_constraint_symbols(program, parameter)
                .iter()
                .all(|domain| declared.contains(domain))
            {
                return None;
            }
            if exact_structural_argument_access(
                program,
                facts,
                machine,
                source_state.symbol,
                call,
                place,
                access,
            )? != access
            {
                return None;
            }
            Some(CheckedUnitStructuralArgumentPlan::structural_result(
                result.binding_ordinal,
                target_identity.to_owned(),
                access,
            ))
        }
    }
    mod reference {
        /*
            Calls borrow through the stored descriptor published by an ordinary result
            binding. Source checking already owns formation and lifetime admission;
            this operation retains the descriptor crossing and selected referent path.
        */
        use super::super::argument_paths::borrowed_argument_endpoint;
        use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
        use crate::execution::terminal_unit::calls::argument_paths::projected_evaluated_path;
        use crate::execution::terminal_unit::control::LocalConstructionTrace;
        use crate::execution::terminal_unit::types::structural_access_for_type_reference;
        use checked_trees::{
            CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
            CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
            CheckedUnitStructuralResultBindingPlan,
        };
        use facts::PlaceRoot;
        use language_semantics::Multiplicity;
        use symbols::SymbolHandle;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::state::State;
        use typed_trees::type_identity::TypeIdentityId;
        use typed_trees::types::TypeReferenceHandle;
        use validation::{
            CanonicalPlace, reference_result_custody, stored_structural_type_identity,
        };

        /* Slice views have their own descriptor argument route. */
        pub(crate) fn is_carrier(
            program: &dyn ::validation::CheckingInput,
            reference: TypeReferenceHandle,
        ) -> bool {
            reference_result_custody::stored_loan_parts(program, reference).is_some()
        }

        pub(crate) fn argument(
            program: &dyn ::validation::CheckingInput,
            machine: SymbolHandle,
            state: &State,
            statement_index: usize,
            place: &CanonicalPlace,
            owner: PlaceRoot,
            reference: TypeReferenceHandle,
            result: &CheckedUnitStructuralResultBindingPlan,
            parameter: TypeReferenceHandle,
            target_identity: TypeIdentityId,
            trace: &LocalConstructionTrace,
            selectors: Option<
                &dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>,
            >,
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let phase = |description| {
                trace.phase(description);
                trace.statement(u32::try_from(statement_index).ok());
            };
            phase("stored reference argument: carrier");
            let (referent, carrier_access) =
                reference_result_custody::stored_loan_parts(program, reference)?;
            phase("result reference argument: formal access");
            let access = structural_access_for_type_reference(program, parameter)?;
            phase("stored reference argument: access");
            if access == CheckedStructuralAccess::Owned {
                return None;
            }
            phase("stored reference argument: producer custody");
            if place.root != owner || result.multiplicity != Multiplicity::Affine {
                return None;
            }
            phase("stored reference argument: producer identity");
            if result.type_identity != stored_structural_type_identity(program, reference, &[], &[])
            {
                return None;
            }
            phase("stored reference argument: projected referent");
            let lends_descriptor = place.segments.is_empty()
                && stored_structural_type_identity(program, reference, &[], &[]) == target_identity;
            if !lends_descriptor
                && carrier_access != CheckedStructuralAccess::MutableBorrow
                && carrier_access != access
            {
                return None;
            }
            let (selected, path) = if lends_descriptor {
                (reference, Vec::new())
            } else if place.segments.is_empty() {
                borrowed_argument_endpoint(
                    program,
                    referent,
                    vec![CheckedUnitStructuralPathSegment::Referent],
                    parameter,
                )?
            } else {
                phase("result reference argument: projected referent path");
                let (_, mut path) = borrowed_argument_endpoint(
                    program,
                    referent,
                    vec![CheckedUnitStructuralPathSegment::Referent],
                    parameter,
                )?;
                let ProjectedArgumentPath {
                    storage_type: selected,
                    path: suffix,
                    ..
                } = projected_evaluated_path(
                    program,
                    machine,
                    state.symbol,
                    statement_index,
                    place,
                    selectors,
                )?;
                path.extend(suffix);
                borrowed_argument_endpoint(program, selected, path, parameter)?
            };
            phase("stored reference argument: lent endpoint");
            phase("stored reference argument: receiving identity");
            if stored_structural_type_identity(program, selected, &[], &[]) != target_identity {
                return None;
            }
            Some(CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                path,
                type_identity: target_identity.to_owned(),
                access,
            })
        }
    }

    pub(crate) use anonymous_shared::validate as validate_shared_temporary;
    pub(crate) use reference::{
        argument as retained_reference_argument, is_carrier as is_stored_reference,
    };

    #[allow(clippy::too_many_arguments)]
    pub(super) fn argument(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &FlowCallFact,
        expression: ExpressionHandle,
        place: &CanonicalPlace,
        result: &CheckedUnitStructuralResultBindingPlan,
        parameter: TypeReferenceHandle,
        target_identity: TypeIdentityId,
        operations: &[CheckedUnitEffectOperationPlan],
        trace: &LocalConstructionTrace,
        selectors: Option<&dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>>,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let mut argument = argument_contract(
            program,
            shapes,
            facts,
            machine,
            state,
            call,
            expression,
            place,
            result,
            parameter,
            target_identity,
            operations,
            trace,
            selectors,
        )?;
        if matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
        ) && let Some(symbol) = place.root.as_symbol()
        {
            trace.phase("result argument: local storage source state");
            trace.statement(u32::try_from(call.statement_index).ok());
            let source_state = program.state_by_symbol(state)?;
            trace.phase("call result argument: current local storage");
            trace.statement(u32::try_from(call.statement_index).ok());
            argument.source =
                super::super::local_storage::source(program, source_state, operations, symbol)?;
        }
        Some(argument)
    }

    #[allow(clippy::too_many_arguments)]
    fn argument_contract(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &FlowCallFact,
        expression: ExpressionHandle,
        place: &CanonicalPlace,
        result: &CheckedUnitStructuralResultBindingPlan,
        parameter: TypeReferenceHandle,
        target_identity: TypeIdentityId,
        operations: &[CheckedUnitEffectOperationPlan],
        trace: &LocalConstructionTrace,
        selectors: Option<&dyn Fn(ExpressionHandle) -> Option<CheckedUnitStructuralPathSegment>>,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let phase = |name| {
            trace.phase(name);
            trace.statement(u32::try_from(call.statement_index).ok());
        };
        phase("call result argument: producer contract");
        let (source_machine, source_state) = find_state(program, state)?;
        let produced_type = place.root.as_expression().and_then(|source| {
            let contextual = facts
                .values
                .structural_values
                .root_for_expression(state, result.statement_index, source)
                .map(|root| root.type_reference);
            let declared =
                declared_place_type_raw(program, source_machine, Some(source_state), source);
            [contextual, declared]
                .into_iter()
                .flatten()
                .flat_map(|source_type| [source_type, assignment_value_type(program, source_type)])
                .find(|source_type| {
                    result_multiplicity(program, *source_type) == result.multiplicity
                        && (stored_structural_type_identity(program, *source_type, &[], &[])
                            == result.type_identity
                            || structural_type_identity(program, *source_type, &[], &[])
                                == result.type_identity)
                })
        });
        if let Some(argument) = super::super::reference_results::record_argument(
            program,
            facts,
            machine,
            source_state,
            u32::try_from(call.statement_index).ok()?,
            expression,
            result,
            parameter,
        ) {
            return Some(argument);
        }
        if let Some(StatementNode::LocalData(local)) =
            state_statements(program, source_state.statement_nodes)
                .get(result.statement_index as usize)
            && reference::is_carrier(program, local.type_reference)
            && place.root == PlaceRoot::Symbol(local.symbol)
        {
            phase("result argument: named stored descriptor");
            return reference::argument(
                program,
                machine,
                source_state,
                call.statement_index,
                place,
                PlaceRoot::Symbol(local.symbol),
                local.type_reference,
                result,
                parameter,
                target_identity,
                trace,
                selectors,
            );
        }
        if let Some(owner) = place.root.as_expression() {
            phase("result argument: anonymous source type");
            let reference = facts
                .values
                .structural_values
                .root_for_expression(state, result.statement_index, owner)
                .map(|root| root.type_reference)
                .or_else(|| {
                    declared_place_type_raw(program, source_machine, Some(source_state), owner)
                })?;
            if reference::is_carrier(program, reference)
                && result.type_identity
                    == stored_structural_type_identity(program, reference, &[], &[])
            {
                phase("result argument: anonymous producer epoch");
                if result.statement_index as usize != call.statement_index {
                    return None;
                }
                phase("result argument: anonymous unique completed producer");
                if operations
                    .iter()
                    .filter_map(|operation| operation.structural_result_binding())
                    .filter(|binding| *binding == result)
                    .count()
                    != 1
                {
                    return None;
                }
                return reference::argument(
                    program,
                    machine,
                    source_state,
                    call.statement_index,
                    place,
                    PlaceRoot::Expression(owner),
                    reference,
                    result,
                    parameter,
                    target_identity,
                    trace,
                    selectors,
                );
            }
        }
        phase("result argument: bounded owner loan");
        if let Some(argument) = bounded_owner::loan(
            program,
            facts,
            machine,
            source_state,
            call,
            expression,
            place,
            result,
            parameter,
            target_identity,
        ) {
            return Some(argument);
        }
        phase("call result argument: storage presentation");
        let access = structural_access_for_type_reference(program, parameter)?;
        let formal_type = parameter;
        let projected = !place.segments.is_empty();
        let unrestricted = result.multiplicity == Multiplicity::Unrestricted;
        let linear = result.multiplicity == Multiplicity::Linear;
        let projected_borrow = projected && access != CheckedStructuralAccess::Owned;
        let byte_window = projected_borrow
            .then(|| {
                match program.expression_table.borrow_expression(expression) {
                    Some(borrow) => {
                        if !has_builtin_subslice_meaning(
                            program,
                            source_machine,
                            Some(source_state),
                            borrow.target,
                        ) {
                            return None;
                        }
                    }
                    None => {
                        if !place.segments.last().is_some_and(|segment| {
                            matches!(segment, PlaceSegment::FixedRange { .. })
                        }) || !has_builtin_subslice_meaning(
                            program,
                            source_machine,
                            Some(source_state),
                            expression,
                        ) {
                            return None;
                        }
                    }
                }
                super::argument_paths::fixed_byte_array_range_path(
                    program,
                    state,
                    call.statement_index,
                    place,
                    parameter,
                )
                .or_else(|| {
                    super::argument_paths::open_byte_range_path(
                        program,
                        state,
                        call.statement_index,
                        place,
                        parameter,
                    )
                })
            })
            .flatten();
        let borrowed_storage = if projected_borrow {
            byte_window
                .or_else(|| {
                    projected_evaluated_path(
                        program,
                        machine,
                        state,
                        call.statement_index,
                        place,
                        selectors,
                    )
                    .map(|projection| (projection.storage_type, projection.path))
                })
                .and_then(|(selected, path)| {
                    super::argument_paths::borrowed_argument_endpoint(
                        program, selected, path, parameter,
                    )
                })
        } else {
            None
        };
        let storage_type = if projected {
            borrowed_storage.as_ref().map(|(storage, _)| *storage)
        } else {
            whole_local_storage_type(program, source_state, place, result)
        };
        let result_presents = |identity| {
            result.type_identity == identity
                || [storage_type, produced_type]
                    .into_iter()
                    .flatten()
                    .any(|source| {
                        stored_type_presents_borrowed(
                            program,
                            source,
                            result.type_identity,
                            identity,
                        )
                    })
        };
        let result_presents_formal = result_presents(target_identity);
        let presentation = storage_type.and_then(|storage| {
            super::argument_presentation::classify(
                program,
                storage,
                parameter,
                target_identity,
                &[],
            )
        });
        let array_view = access != CheckedStructuralAccess::Owned
            && presentation == Some(ArgumentPresentation::FixedArraySlice);
        let projected_path = if projected {
            if projected_borrow {
                let (_, path) = borrowed_storage?;
                let presentation = presentation?;
                if presentation == ArgumentPresentation::FixedArraySlice
                    && !path.iter().all(|segment| {
                        matches!(
                            segment,
                            CheckedUnitStructuralPathSegment::Field(_)
                                | CheckedUnitStructuralPathSegment::FixedRange { .. }
                        )
                    })
                {
                    return None;
                }
                Some(path)
            } else {
                phase("call result argument: owned projected storage");
                projected_evaluated_path(
                    program,
                    machine,
                    state,
                    call.statement_index,
                    place,
                    selectors,
                )
                .and_then(
                    |ProjectedArgumentPath {
                         storage_type: selected,
                         path,
                         ..
                     }| {
                        (base_type_identity(program, selected, &[], &[])? == target_identity)
                            .then_some(path)
                    },
                )
            }
        } else {
            None
        };
        if !projected
            && ((matches!(access, checked_borrowed_access_pair_pattern!())
                && slice_view_carrier(program, parameter)
                && result_presents_formal)
                || (access == CheckedStructuralAccess::SharedBorrow
                    && borrowed_named_view(program, parameter)
                    && result.type_identity
                        == stored_structural_type_identity(program, parameter, &[], &[])))
            && let Some(view_symbol) = place.root.as_symbol()
        {
            let Some(StatementNode::LocalData(local)) =
                state_statements(program, source_state.statement_nodes)
                    .get(result.statement_index as usize)
            else {
                return None;
            };
            if local.symbol != view_symbol
                || result.statement_index as usize >= call.statement_index
                || !names_whole_local(program, call, expression, view_symbol)
            {
                return None;
            }
            if slice_view_carrier(program, parameter) {
                if exact_structural_argument_access(
                    program, facts, machine, state, call, place, access,
                )? != access
                {
                    return None;
                }
                return Some(CheckedUnitStructuralArgumentPlan::structural_result(
                    result.binding_ordinal,
                    target_identity.to_owned(),
                    access,
                ));
            }
            let argument = super::computation_arguments::borrowed_storage_argument(
                program,
                shapes,
                &facts.borrow,
                machine,
                source_state,
                call,
                place,
                parameter,
                None,
            )?;
            if !matches!(argument.source,
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { symbol, .. }
                    if symbol == view_symbol)
                || argument.type_identity != target_identity
            {
                return None;
            }
            return Some(argument);
        }
        if !projected
            && ((matches!(access, checked_borrowed_access_pair_pattern!())
                && slice_view_carrier(program, parameter))
                || (access == CheckedStructuralAccess::SharedBorrow
                    && borrowed_named_view(program, parameter)))
            && result_presents_formal
            && let Some(source) = place.root.as_expression()
            && source == expression
        {
            if usize::try_from(result.statement_index).ok()? != call.statement_index {
                return None;
            }
            return Some(CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                },
                path: Vec::new(),
                type_identity: target_identity.to_owned(),
                access,
            });
        }
        if linear && projected && access == CheckedStructuralAccess::Owned {
            return None;
        }
        let path = if projected {
            projected_path?
        } else {
            Vec::new()
        };
        phase("call result argument: captured loan access");
        let (value_expression, referent) = match access {
            CheckedStructuralAccess::Owned => (expression, formal_type),
            CheckedStructuralAccess::SharedBorrow => {
                trace.phase("call operation: result argument: shared referent");
                let (referent_expression, referee) =
                    shared_argument_referent(program, expression, parameter)?;
                let referee = if array_view { storage_type? } else { referee };
                match place.root {
                    PlaceRoot::Symbol(_) => {
                        trace.phase("call operation: result argument: retained local loan");
                        if exact_structural_argument_access(
                            program, facts, machine, state, call, place, access,
                        )? != access
                        {
                            return None;
                        }
                    }
                    PlaceRoot::Expression(source) => {
                        let referent_place =
                            place_at(program, state, call.statement_index, referent_expression);
                        let (owner, owner_type) = if referent_place.as_ref() == Some(place) {
                            (source, produced_type?)
                        } else if place.segments.is_empty()
                            && source == referent_expression
                            && let Some(producer) = referent_place
                                .as_ref()
                                .and_then(|place| place.root.as_expression())
                        {
                            (
                                producer,
                                canonical_place_type_reference(
                                    program,
                                    state,
                                    call.statement_index,
                                    &CanonicalPlace {
                                        root: PlaceRoot::Expression(producer),
                                        segments: Vec::new(),
                                    },
                                )?,
                            )
                        } else {
                            return None;
                        };
                        validate_shared_temporary(
                            program,
                            &facts.flow,
                            machine,
                            state,
                            call,
                            owner,
                            owner_type,
                        )?;
                    }
                    _ => return None,
                }
                (referent_expression, referee)
            }
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow => {
                if projected
                    && expressions(program).borrow_expression(expression).is_none()
                    && matches!(place.root, PlaceRoot::Expression(_))
                {
                    return Some(CheckedUnitStructuralArgumentPlan {
                        projection_evaluations: Default::default(),
                        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        },
                        path,
                        type_identity: target_identity.to_owned(),
                        access,
                    });
                }
                let expected_access = match access {
                    CheckedStructuralAccess::MutableBorrow => ReferenceAccess::Mutable,
                    CheckedStructuralAccess::WriteOnlyBorrow => ReferenceAccess::WriteOnly,
                    _ => return None,
                };
                let TypeReferenceNode::Reference {
                    access: reference_access,
                    referee,
                    ..
                } = program.type_reference_table.type_reference(parameter)
                else {
                    return None;
                };
                let borrow = program.expression_table.borrow_expression(expression)?;
                if *reference_access != expected_access
                    || borrow.access != expected_access
                    || !program.expression_table.expression_is_valid(borrow.target)
                {
                    return None;
                }
                let _ = place.root.as_symbol()?;
                if exact_structural_argument_access(
                    program, facts, machine, state, call, place, access,
                )? != access
                {
                    return None;
                }
                (
                    borrow.target,
                    if array_view { storage_type? } else { *referee },
                )
            }
        };
        trace.phase("call operation: result argument: referent storage identity");
        let storage_identity_matches = projected
            || array_view
            || if access == CheckedStructuralAccess::Owned {
                result.type_identity == target_identity
            } else {
                result_presents(stored_structural_type_identity(program, referent, &[], &[]))
            };
        let transferred_multiplicity = if projected && access == CheckedStructuralAccess::Owned {
            result_multiplicity(
                program,
                canonical_place_type_reference(program, state, call.statement_index, place)?,
            )
        } else {
            result.multiplicity
        };
        if !storage_identity_matches && ::validation::trace_flags::struct_trace() {
            eprintln!(
                "RESULT_ARGUMENT_IDENTITY machine={} state={} statement={} root={:?} access={:?} referent={} storage={:?} established={:?} formal={:?}",
                program.symbols.display_path(machine, "::"),
                program.symbols.display_path(state, "::"),
                call.statement_index,
                place,
                access,
                program.normalized_type_identity_text(referent),
                base_type_identity(program, referent, &[], &[])
                    .and_then(|id| program.type_identity_text(id)),
                program.type_identity_text(result.type_identity),
                program.type_identity_text(target_identity)
            );
        }
        if !storage_identity_matches
            || (access == CheckedStructuralAccess::Owned
                && result_multiplicity(program, referent) != transferred_multiplicity)
            || usize::try_from(result.statement_index).ok()? > call.statement_index
        {
            return None;
        }
        match place.root {
            PlaceRoot::Symbol(symbol) => {
                trace.phase("call operation: result argument: earlier whole local correspondence");
                if usize::try_from(result.statement_index).ok()? == call.statement_index
                    || !symbol.is_valid()
                    || (!projected && !names_whole_local(program, call, value_expression, symbol))
                {
                    return None;
                }
                trace.phase("call operation: result argument: local binding custody");
                let source_state = program.state_by_symbol(state)?;
                let local = state_statements(program, source_state.statement_nodes)
                    .get(usize::try_from(result.statement_index).ok()?)?
                    .as_local_data()?;
                let view_binding = projected_borrow
                    && local.symbol == symbol
                    && matches!(
                        program
                            .type_reference_table
                            .type_reference(local.type_reference),
                        TypeReferenceNode::Reference { referee, .. }
                            if matches!(
                                program.type_reference_table.type_reference(*referee),
                                TypeReferenceNode::Slice { .. }
                            )
                    );
                if !view_binding
                    && (local.symbol != symbol
                        || result_multiplicity(program, local.type_reference)
                            != result.multiplicity
                        || base_type_identity(program, local.type_reference, &[], &[])?
                            != result.type_identity)
                {
                    return None;
                }
                if access == CheckedStructuralAccess::Owned
                    && (linear || unrestricted)
                    && structural_result_qualifications(program, local.type_reference).ok()?
                        != structural_result_qualifications(program, referent).ok()?
                {
                    return None;
                }
            }
            PlaceRoot::Expression(source) if source == value_expression || projected => {
                phase("call result argument: anonymous producer correspondence");
                if usize::try_from(result.statement_index).ok()? != call.statement_index
                    || operations
                        .iter()
                        .filter_map(|operation| operation.structural_result_binding())
                        .filter(|binding| *binding == result)
                        .count()
                        != 1
                    || (projected
                        && place_at(program, state, call.statement_index, expression).as_ref()
                            != Some(place))
                {
                    return None;
                }
                phase("call result argument: anonymous produced value type");
                let source_type = produced_type?;
                let source_type = if access == CheckedStructuralAccess::Owned {
                    assignment_value_type(program, source_type)
                } else {
                    source_type
                };
                let contract = (access == CheckedStructuralAccess::Owned)
                    .then(|| structural_result_contract(program, source_type).ok())
                    .flatten();
                phase("call result argument: anonymous receiving contract");
                if result_multiplicity(program, source_type) != result.multiplicity
                    || structural_type_identity(program, source_type, &[], &[])
                        != result.type_identity
                    || (!projected
                        && access == CheckedStructuralAccess::Owned
                        && contract?.qualifications
                            != structural_result_contract(program, referent)
                                .ok()?
                                .qualifications)
                {
                    return None;
                }
            }
            _ => return None,
        }
        Some(CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            path,
            type_identity: target_identity.to_owned(),
            access,
        })
    }

    /*
        Whether the argument spelling names exactly the whole local `symbol`: an
        authored `Name` expression, or a method-spelled receiver that the call
        fact names by symbol because no argument expression exists for it.
    */
    fn names_whole_local(
        program: &dyn ::validation::CheckingInput,
        call: &FlowCallFact,
        mut value_expression: ExpressionHandle,
        symbol: SymbolHandle,
    ) -> bool {
        if !value_expression.is_valid() {
            return call.has_receiver && call.receiver_symbol == symbol;
        }
        while let ExpressionNode::Cast(cast) = program.expression_table.expression(value_expression)
        {
            if cast.form.is_recast()
                || cast.domain != numerics::arithmetic::ArithmeticDomain::Exact
                || program.primitive_type_reference(cast.target_type).is_some()
            {
                break;
            }
            value_expression = cast.value;
        }
        matches!(program.expression_table.expression(value_expression),
            ExpressionNode::Name(name) if name.symbol == symbol
                && name.head_symbol == symbol
                && program.expression_table.name_path_members(name.members).len() == 1)
    }

    /*
        The declared type of the local that binds `result` when the receiver
        place is exactly that whole local.
    */
    fn whole_local_storage_type(
        program: &dyn ::validation::CheckingInput,
        source_state: &State,
        place: &CanonicalPlace,
        result: &CheckedUnitStructuralResultBindingPlan,
    ) -> Option<TypeReferenceHandle> {
        let symbol = place.root.as_symbol()?;
        let local = state_statements(program, source_state.statement_nodes)
            .get(usize::try_from(result.statement_index).ok()?)?
            .as_local_data()?;
        (place.segments.is_empty() && local.symbol == symbol).then_some(local.type_reference)
    }
}
mod result_owners {
    /*
        Retained structural bindings name their actual production owner.
        Consumers share this query instead of rebuilding a partial producer list.
        Named bindings belong to their declaration; anonymous operands belong to
        the exact expression that produced them, never their enclosing statement.
    */
    use checked_trees::{
        CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitLocalBindingPlan, FlowCallFact,
    };
    use facts::PlaceRoot;
    use symbols::SymbolHandle;

    pub(in crate::execution::terminal_unit) fn structural_result_owner(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: SymbolHandle,
        calls: &[FlowCallFact],
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Option<PlaceRoot> {
        let result = operation.structural_result_binding()?;
        if result.local.is_named() {
            let state = program.state_by_symbol(state)?;
            let local = crate::flow::state_statements(program, state.statement_nodes)
                .get(usize::try_from(result.statement_index).ok()?)?
                .as_local_data()?;
            return (local.symbol == result.local.symbol)
                .then_some(PlaceRoot::Symbol(result.local.symbol));
        }
        let expression = if let Some(call) = operation.call() {
            let mut producers = calls.iter().filter(|source| {
                source.statement_index == call.coordinate.statement_index as usize
                    && source.call_ordinal == call.coordinate.call_ordinal as usize
            });
            let source = producers.next()?;
            if producers.next().is_some() {
                return None;
            }
            source.authored_expression
        } else {
            match operation {
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
                ) if facts.values.structural_values.nodes.is_valid(*value) => {
                    facts.values.structural_values.nodes.get(*value).expression
                }
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    CheckedUnitLocalBindingPlan::ScalarArray {
                        source:
                            checked_trees::CheckedArrayConstructionSource::CallArgument {
                                call_ordinal,
                                parameter_position,
                            },
                        ..
                    },
                ) => {
                    let machine = program.symbols.get(state).parent;
                    let site = validation::find_call_site(
                        program,
                        machine,
                        state,
                        result.statement_index as usize,
                        *call_ordinal as usize,
                    )?;
                    *validation::call_site_argument_expressions(program, &site)
                        .get(*parameter_position as usize)?
                }
                CheckedUnitEffectOperationPlan::EstablishReference {
                    formation_expression,
                    ..
                } => *formation_expression,
                CheckedUnitEffectOperationPlan::MoveStructuralField { expression, .. } => {
                    *expression
                }
                _ => return None,
            }
        };
        expression
            .is_valid()
            .then_some(PlaceRoot::Expression(expression))
    }
}
pub(super) use result_owners::structural_result_owner;
mod signatures {
    /*
        The structural and scalar signatures a call is checked against, and the
        claims a unit holds on entry.
    */
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::types::{
        ShapeCollector, fold_parameter_entry_domains, parameter_qualifications,
        projected_parameter_qualifications, structural_access_for_type_reference,
        substituted_formal_type,
    };
    use crate::execution::terminal_unit::{
        CarryPolicy, CheckFacts, CheckedStructuralAccess as StructuralAccess,
        CheckedStructuralScalarParameterPlan as ParameterPlan, CheckedUnitEntryClaimPlan,
        CheckedUnitStructuralParameterPlan, Multiplicity, PermissionAccess,
        PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, StateParameter,
        SymbolHandle, strips_erased_parameter,
    };

    fn scalar_parameter_signature(
        program: &dyn ::validation::CheckingInput,
        position: usize,
        parameter: &StateParameter,
    ) -> Option<ParameterPlan> {
        if parameter.is_const {
            return None;
        }
        Some(ParameterPlan {
            source_position: u32::try_from(position).ok()?,
            primitive_type: program.primitive_type_reference(parameter.type_reference)?,
        })
    }

    pub(crate) fn free_structural_scalar_signature(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        state: &typed_trees::state::State,
        binders: &[(SymbolHandle, String)],
    ) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
        free_structural_scalar_signature_traced(
            program,
            shapes,
            state,
            binders,
            &LocalConstructionTrace::default(),
        )
    }

    /* `free_structural_scalar_signature` with per-parameter trace marks. */
    pub(crate) fn free_structural_scalar_signature_traced(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        state: &typed_trees::state::State,
        binders: &[(SymbolHandle, String)],
        trace: &LocalConstructionTrace,
    ) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
        scalar_and_structural_parameters(program, shapes, state, binders, trace)
    }

    /*
        The entry roster shared by composed bodies and their public family heads.
        Ordinary invocations retain Requires in their separate contract channel;
        composed graphs additionally expose authored entry memberships as invariants.
    */
    pub(crate) fn free_structural_scalar_entry_signature_traced(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        state: &typed_trees::state::State,
        binders: &[(SymbolHandle, String)],
        trace: &LocalConstructionTrace,
    ) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
        let (mut structural, scalar) =
            free_structural_scalar_signature_traced(program, shapes, state, binders, trace)?;
        fold_parameter_entry_domains(program, shapes, state.symbol, binders, &mut structural)?;
        Some((structural, scalar))
    }

    fn scalar_and_structural_parameters(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        state: &typed_trees::state::State,
        binders: &[(SymbolHandle, String)],
        trace: &LocalConstructionTrace,
    ) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
        let (mut structural, scalar) = parameter_signatures(
            program,
            shapes,
            program.state_parameters(state),
            binders,
            &[],
            trace,
        )?;
        fold_parameter_entry_domains(program, shapes, state.symbol, binders, &mut structural)?;
        Some((structural, scalar))
    }

    /*
        Classify a callee's formals into its scalar and structural lanes. A
        typed state, a selector call's instantiated slot, a bodyless attached
        boundary declaration and a boundary-trait requirement all present their
        formals to callers identically, so one classifier serves every plan: a
        requirement's plan and its adapters' plans must describe one signature.
        `substitutions` close a prepared requirement's generic formals to the
        specialization every checked call retained; an ordinary state passes none.
    */
    pub(crate) fn parameter_signatures(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        parameters: &[StateParameter],
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
        trace: &LocalConstructionTrace,
    ) -> Option<(Vec<CheckedUnitStructuralParameterPlan>, Vec<ParameterPlan>)> {
        let mut structural_parameters = Vec::new();
        let mut scalar_parameters = Vec::new();
        for (position, parameter) in parameters.iter().enumerate() {
            trace.phase("state graph: state signature: parameter signature: erased parameter");
            if strips_erased_parameter(parameter)? {
                continue;
            }
            let source_position = u32::try_from(position).ok()?;
            let parameter_type =
                substituted_formal_type(program, parameter.type_reference, substitutions);
            if program.primitive_type_reference(parameter_type).is_some() {
                trace.phase("state graph: state signature: parameter signature: scalar parameter");
                scalar_parameters.push(scalar_parameter_signature(program, position, parameter)?);
                continue;
            }
            trace.phase("state graph: state signature: parameter signature: const parameter");
            if parameter.is_const {
                return None;
            }
            trace.phase(
                "state graph: state signature: parameter signature: structural parameter type",
            );
            let type_identity = shapes.add_type(parameter_type, binders, substitutions)?;
            if validation::slice_view_shell(program, parameter_type, substitutions).is_some() {
                shapes.add_stored_type(parameter_type, binders, substitutions)?;
            }
            trace.phase(
                "state graph: state signature: parameter signature: parameter qualifications",
            );
            let qualifications =
                parameter_qualifications(program, shapes, parameter_type, binders)?;
            trace.phase("state graph: state signature: parameter signature: parameter access");
            let access = structural_access_for_type_reference(program, parameter_type)?;
            let multiplicity = if access == StructuralAccess::Owned {
                validation::reference_result_custody::result_multiplicity(program, parameter_type)
            } else {
                crate::checks::type_multiplicity(program, parameter_type)
            };
            trace.phase(
                "state graph: state signature: parameter signature: projected qualifications",
            );
            let projected_qualifications =
                projected_parameter_qualifications(program, shapes, parameter_type, binders)?;
            let mut additional_input_qualifications = Vec::new();
            if let Some(shell) = validation::reference_shell(program, parameter_type) {
                let carrier = program
                    .type_reference_table
                    .unconstrained_type_reference(shell.referee);
                let storage =
                    validation::structural_qualification_contract(program, carrier).ok()?;
                additional_input_qualifications.extend(
                    qualifications
                        .iter()
                        .filter(|domain| !storage.qualifications.contains(domain))
                        .map(|domain| checked_trees::CheckedStructuralPathQualification {
                            path: Vec::new(),
                            domain: *domain,
                        }),
                );
                additional_input_qualifications.extend(
                    projected_qualifications
                        .iter()
                        .filter(|row| !storage.projected_qualifications.contains(row))
                        .cloned(),
                );
            }
            additional_input_qualifications.retain(|row| {
                let mut matching = shapes
                    .domains
                    .iter()
                    .filter(|domain| domain.domain == row.domain);
                let Some(first) = matching.next() else {
                    return false;
                };
                std::iter::once(first).chain(matching).all(|domain| {
                    domain.complete_predicate_theory
                        && domain.establishment_routes.is_empty()
                        && domain
                            .required_memberships
                            .as_ref()
                            .is_some_and(|required| required.is_empty())
                })
            });
            structural_parameters.push(CheckedUnitStructuralParameterPlan {
                position: source_position,
                primitive_referent: shapes.primitive_scalar(type_identity),
                type_identity,
                multiplicity,
                access,
                qualifications,
                projected_qualifications,
                additional_input_qualifications,
            });
        }
        Some((structural_parameters, scalar_parameters))
    }

    pub(crate) fn entry_claims(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        structural_parameters: &[CheckedUnitStructuralParameterPlan],
        source_parameters: &[StateParameter],
    ) -> Option<Vec<CheckedUnitEntryClaimPlan>> {
        let ownership = &facts.flow.ownership;
        let events = ownership.with_state_permissions(state, |handles| {
            handles
                .iter()
                .map(|handle| ownership.permissions.get(*handle))
                .filter(|event| {
                    event.machine_symbol == machine
                        && event.state_symbol == state
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.multiplicity == Multiplicity::Linear
                        && event.obligation_live
                })
                .collect::<Vec<_>>()
        });
        let mut output = Vec::new();
        for (parameter_index, parameter) in structural_parameters.iter().enumerate() {
            if parameter.multiplicity == Multiplicity::Unrestricted {
                continue;
            }
            let source = source_parameters.get(parameter.position as usize)?;
            let expected_root = facts::PlaceRoot::Symbol((source).symbol);
            let matching = events
                .iter()
                .filter(|event| event.root == expected_root)
                .collect::<Vec<_>>();
            if matching.is_empty() {
                if parameter.multiplicity == Multiplicity::Affine {
                    continue;
                }
                return None;
            }
            let source_type = validation::unwrapped_type_reference(program, source.type_reference)?;
            for event in matching {
                if event.claim_identity == PermissionClaimIdentity::Unknown {
                    return None;
                }
                let policies = facts
                    .carry
                    .claim_policies
                    .iter()
                    .filter(|policy| policy.claim_identity == event.claim_identity)
                    .collect::<Vec<_>>();
                let carry = match policies.as_slice() {
                    [] => CarryPolicy::STRICT,
                    [policy] => policy.effective,
                    _ => return None,
                };
                let path = validation::structural_claim_path(
                    program,
                    source_type,
                    facts.flow.ownership.segments.span_or_empty(event.segments),
                )
                .ok()?;
                output.push(CheckedUnitEntryClaimPlan {
                    claim_identity: event.claim_identity,
                    parameter_index: u32::try_from(parameter_index).ok()?,
                    path,
                    carry,
                });
            }
        }
        output.sort_by(|left, right| {
            (left.parameter_index, &left.path).cmp(&(right.parameter_index, &right.path))
        });
        (output.len() == events.len()).then_some(output)
    }
}
pub(crate) mod structural_arguments {

    /*
        Structural call arguments: their custody, restored reborrow aliases,
        exact borrow access and the claim transfers a call performs.

        The borrow roster retains the full operand evaluation history for conflict
        checking. Invocation contracts consume only accesses still live at the call:
        a completed nested operand can mutate a place before a later shared argument
        without supplying mutable access to the enclosing invocation.
    */
    use crate::execution::terminal_unit::ExpressionNode::Indexed;
    use crate::execution::terminal_unit::ExpressionNode::Range;
    use crate::execution::terminal_unit::calls::argument_paths::byte_sequence_literal_argument;
    use crate::execution::terminal_unit::calls::computation_arguments;
    use crate::execution::terminal_unit::calls::parameter_argument_paths::{
        ParameterArgument, SourceParameter, boundary_parameter_path, ordinary_parameter_path,
    };
    use crate::execution::terminal_unit::calls::reference_forwarding;
    use crate::execution::terminal_unit::calls::result_arguments;
    use crate::execution::terminal_unit::calls::view_subslice;
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::receiver_aliases::ReceiverAlias;
    use crate::execution::terminal_unit::reference_results::is_reference_record;
    use crate::execution::terminal_unit::reference_results::local_record_loans;
    use crate::execution::terminal_unit::reference_results::result_loans;
    use crate::execution::terminal_unit::structural_scalar_store::primitive_local_before;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::execution::terminal_unit::types::{
        base_type_identity, is_unit, structural_access_for_type_reference,
    };
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedStructuralAccess as StructuralAccess,
        CheckedTrivialAffineStructuralLocalPlan, CheckedUnitClaimTransferPlan as TransferPlan,
        CheckedUnitStructuralArgumentPlan as ArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralResultBindingPlan as BindingPlan, MachineSupplyMode, Multiplicity,
        PermissionAccess, PermissionClaimIdentity as ClaimIdentity, PermissionEventKind,
        PermissionEventSource, SymbolHandle, strips_erased_parameter,
    };
    use crate::flow::state_statements;
    use crate::published::places::place_at;
    use crate::published::semantic::calls::shared_argument_referent;
    use crate::values::authored_projection_selectors;

    pub(crate) fn structural_call_arguments(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        facts: &CheckFacts,
        call: &checked_trees::FlowCallFact,
        caller_machine: &typed_trees::machine::Machine,
        caller_state: &typed_trees::state::State,
        caller_parameters: &[CheckedUnitStructuralParameterPlan],
        caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        callee: super::CallTarget<'_>,
        call_site: &validation::CallSite<'_>,
        statement_index: usize,
        caller_structural_results: &[(BindingPlan, facts::PlaceRoot)],
        caller_operations: &[checked_trees::CheckedUnitEffectOperationPlan],
        trace: &LocalConstructionTrace,
    ) -> Option<Vec<ArgumentPlan>> {
        let parameters = callee
            .parameters
            .iter()
            .map(|parameter| {
                Some(StructuralArgumentType {
                    type_reference: parameter.type_reference,
                    erased: strips_erased_parameter(parameter)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        structural_arguments_for_types(
            program,
            shapes,
            facts,
            call,
            caller_machine,
            caller_state,
            caller_parameters,
            caller_trivial_affine_locals,
            &parameters,
            callee.supply_mode,
            callee.has_bodyless_signature(),
            Some(callee),
            call_site,
            statement_index,
            caller_structural_results,
            caller_operations,
            trace,
        )
    }

    /* Plan borrowed operands from their checked types, including instructions without a callable declaration. */
    #[derive(Clone, Copy)]
    pub(crate) struct StructuralArgumentType {
        pub(crate) type_reference: typed_trees::types::TypeReferenceHandle,
        pub(crate) erased: bool,
    }

    pub(crate) fn structural_arguments_for_types(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut ShapeCollector<'_>,
        facts: &CheckFacts,
        call: &checked_trees::FlowCallFact,
        caller_machine: &typed_trees::machine::Machine,
        caller_state: &typed_trees::state::State,
        caller_parameters: &[CheckedUnitStructuralParameterPlan],
        caller_trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
        target_parameters: &[StructuralArgumentType],
        supply_mode: MachineSupplyMode,
        bodyless: bool,
        declaration: Option<super::CallTarget<'_>>,
        call_site: &validation::CallSite<'_>,
        statement_index: usize,
        caller_structural_results: &[(BindingPlan, facts::PlaceRoot)],
        caller_operations: &[checked_trees::CheckedUnitEffectOperationPlan],
        trace: &LocalConstructionTrace,
    ) -> Option<Vec<ArgumentPlan>> {
        let source_parameters = program.state_parameters(caller_state);
        let borrow_aliases = crate::execution::terminal_unit::receiver_aliases::aliases(
            program,
            facts,
            caller_machine,
            caller_state,
        )
        .unwrap_or_default();
        let explicit_arguments = validation::call_site_argument_expressions(program, call_site);
        let mut explicit_index = 0usize;
        let mut output = Vec::new();
        let mut structural_argument_ordinal = 0usize;
        let arm = |name: &'static str| {
            trace.phase(name);
            trace.statement(u32::try_from(statement_index).ok());
        };

        let materialized_argument_owner =
            |expression: typed_trees::expression::ExpressionHandle,
             formal: typed_trees::types::TypeReferenceHandle| {
                let materialized = |candidate| {
                    caller_structural_results
                        .iter()
                        .any(|(_, root)| *root == facts::PlaceRoot::Expression(candidate))
                };
                if materialized(expression) {
                    Some(expression)
                } else {
                    let (referent, _) = shared_argument_referent(program, expression, formal)?;
                    materialized(referent).then_some(referent)
                }
            };

        for target in target_parameters {
            arm("call operation: structural arguments: erased position");
            if target.erased {
                explicit_arguments.get(explicit_index)?;
                explicit_index = explicit_index.checked_add(1)?;
                continue;
            }
            arm("call operation: structural arguments: scalar position");
            if program
                .primitive_type_reference(target.type_reference)
                .is_some()
            {
                explicit_arguments.get(explicit_index)?;
                explicit_index = explicit_index.checked_add(1)?;
                continue;
            }
            let argument_ordinal = structural_argument_ordinal;
            let selector = |expression| {
                let root = facts.values.scalar_computations.root_at(
                    caller_state.symbol,
                    u32::try_from(statement_index).ok()?,
                    checked_trees::CheckedScalarExpressionRole::CallSelector {
                        call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                        argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                        expression,
                    },
                )?;
                Some(checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(root.root))
            };
            structural_argument_ordinal = structural_argument_ordinal.checked_add(1)?;
            arm("call operation: structural arguments: argument place");
            let authored_place = {
                let expression = *explicit_arguments.get(explicit_index)?;
                explicit_index += 1;
                if let Some(owner_expression) =
                    materialized_argument_owner(expression, target.type_reference)
                {
                    validation::CanonicalPlace {
                        root: facts::PlaceRoot::Expression(owner_expression),
                        segments: Vec::new(),
                    }
                } else {
                    if supply_mode == MachineSupplyMode::CheckedBody
                        && let Some(literal) = byte_sequence_literal_argument(
                            program,
                            target.type_reference,
                            expression,
                        )
                    {
                        output.push(literal);
                        continue;
                    }
                    if validation::subslice_expression(program, expression).is_some() {
                        let subslice = view_subslice::admit(
                            program,
                            facts,
                            caller_machine,
                            caller_state,
                            caller_parameters,
                            target.type_reference,
                            expression,
                            statement_index,
                            checked_trees::CheckedSubsliceSite::CallArgument {
                                call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                                argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                            },
                        )?;
                        if let Some(argument) =
                            subslice.call_argument(program, caller_state, statement_index, bodyless)
                        {
                            output.push(argument);
                            continue;
                        }
                    }
                    if matches!(program.expression_table.expression(expression),
                    Indexed(indexed)
                        if matches!(program.expression_table.expression(indexed.index),
                            Range(_)))
                        && !validation::has_builtin_subslice_meaning(
                            program,
                            caller_machine,
                            Some(caller_state),
                            expression,
                        )
                    {
                        return None;
                    }
                    place_at(program, caller_state.symbol, statement_index, expression)?
                }
            };
            arm("call operation: structural arguments: alias and identity");
            let materialized = |root| {
                caller_structural_results
                    .iter()
                    .any(|(_, candidate)| *candidate == root)
            };
            let resolved_alias = crate::execution::terminal_unit::receiver_aliases::resolve(
                &borrow_aliases,
                &authored_place,
                materialized,
            );
            let materialized_root = resolved_alias.as_ref().unwrap_or(&authored_place).root;
            let restored_alias = (!materialized(materialized_root))
                .then(|| {
                    reborrow_restored_call_alias_target(
                        facts,
                        caller_machine.symbol,
                        caller_state.symbol,
                        call,
                        &authored_place,
                    )
                    .or_else(|| {
                        declaration.and_then(|callee| {
                            reborrow_restored_shared_cohort_observation_alias_target(
                                program,
                                facts,
                                caller_machine.symbol,
                                caller_state,
                                callee,
                                call,
                                call_site,
                                &authored_place,
                            )
                        })
                    })
                })
                .flatten();
            let place = restored_alias
                .clone()
                .or_else(|| resolved_alias.clone())
                .unwrap_or_else(|| authored_place.clone());
            let target_identity = base_type_identity(program, target.type_reference, &[], &[])?;
            arm("call operation: structural arguments: caller structural result");
            if let Some((result, _)) = caller_structural_results
                .iter()
                .find(|(_, root)| *root == place.root)
            {
                let expression = *explicit_arguments.get(explicit_index.checked_sub(1)?)?;
                output.push(result_arguments::argument(
                    program,
                    shapes,
                    facts,
                    caller_machine.symbol,
                    caller_state.symbol,
                    call,
                    expression,
                    &place,
                    result,
                    target.type_reference,
                    target_identity,
                    caller_operations,
                    trace,
                    Some(&selector),
                )?);
                continue;
            }
            arm("call operation: structural arguments: retained temporary");
            let Some(source_symbol) = place.root.as_symbol() else {
                let expression = place.root.as_expression()?;
                let descriptor = validation::slice_view_shell(program, target.type_reference, &[]);
                let authored = *explicit_arguments.get(explicit_index.checked_sub(1)?)?;
                let (access, referent) = match program
                    .type_reference_table
                    .type_reference(target.type_reference)
                {
                    typed_trees::types::TypeReferenceNode::Reference { access, .. }
                        if descriptor.is_some() =>
                    {
                        ((*access).into(), target.type_reference)
                    }
                    typed_trees::types::TypeReferenceNode::Reference {
                        access: language_semantics::ReferenceAccess::Shared,
                        ..
                    } => (
                        StructuralAccess::SharedBorrow,
                        shared_argument_referent(program, authored, target.type_reference)?.1,
                    ),
                    _ => return None,
                };
                let statement = u32::try_from(statement_index).ok()?;
                let retained = facts.values.structural_values.root_for_expression(
                    caller_state.symbol,
                    statement,
                    expression,
                )?;
                let node = facts.values.structural_values.nodes.get(retained.root);
                let (owner, path, projection_evaluations) = match &node.kind {
                    checked_trees::CheckedStructuralValueKind::Projection {
                        source,
                        path,
                        projection_evaluations,
                        ..
                    } if !path.is_empty() => (*source, path.clone(), *projection_evaluations),
                    _ => (retained.root, Vec::new(), Default::default()),
                };
                let owner_expression = facts.values.structural_values.nodes.get(owner).expression;
                let owner_type = if owner == retained.root {
                    referent
                } else {
                    let owner_place = place_at(
                        program,
                        caller_state.symbol,
                        statement_index,
                        owner_expression,
                    )?;
                    if !owner_place.segments.is_empty() {
                        return None;
                    }
                    validation::canonical_place_type_reference(
                        program,
                        caller_state.symbol,
                        statement_index,
                        &owner_place,
                    )?
                };
                if descriptor.is_none() {
                    validation::structural_result_contract(program, referent).ok()?;
                    super::validate_shared_temporary(
                        program,
                        &facts.flow,
                        caller_machine.symbol,
                        caller_state.symbol,
                        call,
                        owner_expression,
                        owner_type,
                    )?;
                }
                output.push(ArgumentPlan {
                    projection_evaluations,
                    source: SourcePlan::StructuralExpression {
                        value_dependency: None,
                        expression: owner_expression,
                        logical_root_type_identity: None,
                    },
                    path,
                    type_identity: if descriptor.is_some() {
                        validation::structural_type_identity(program, referent, &[], &[])
                    } else {
                        validation::stored_structural_type_identity(program, referent, &[], &[])
                    },
                    access,
                });
                continue;
            };
            if primitive_local_before(program, caller_state, statement_index, source_symbol)
                .is_some()
            {
                if restored_alias.is_some() {
                    return None;
                }
                output.push(computation_arguments::primitive_local_argument(
                    program,
                    &facts.borrow,
                    caller_machine.symbol,
                    caller_state,
                    call,
                    &place,
                    &authored_place,
                    target.type_reference,
                )?);
                continue;
            }
            arm("call operation: structural arguments: trivial affine local");
            if let Some((local, _)) = caller_trivial_affine_locals
                .iter()
                .find(|(_, symbol)| *symbol == source_symbol)
            {
                let exact_transfer =
                    facts
                        .flow
                        .ownership
                        .with_state_permissions(caller_state.symbol, |handles| {
                            handles.iter().copied().any(|handle| {
                                let event = facts.flow.ownership.permissions.get(handle);
                                event.machine_symbol == caller_machine.symbol
                                    && event.state_symbol == caller_state.symbol
                                    && event.source
                                        == PermissionEventSource::Call {
                                            statement_index: call.statement_index,
                                            call_ordinal: call.call_ordinal,
                                            target_symbol: call.target_symbol,
                                        }
                                    && event.kind == PermissionEventKind::Transfer
                                    && event.multiplicity == Multiplicity::Affine
                                    && event.access == PermissionAccess::Owned
                                    && event.claim_identity == ClaimIdentity::Unknown
                                    && !event.obligation_live
                                    && event.root == facts::PlaceRoot::Symbol(source_symbol)
                                    && facts
                                        .flow
                                        .ownership
                                        .segments
                                        .span_or_empty(event.segments)
                                        .is_empty()
                            })
                        });
                if !place.segments.is_empty()
                    || local.type_identity != target_identity
                    || structural_access_for_type_reference(program, target.type_reference)?
                        != StructuralAccess::Owned
                    || !exact_transfer
                {
                    return None;
                }
                output.push(ArgumentPlan {
                    projection_evaluations: Default::default(),
                    source: SourcePlan::TrivialAffineLocal {
                        declaration_ordinal: local.declaration_ordinal,
                    },
                    path: Vec::new(),
                    type_identity: target_identity,
                    access: StructuralAccess::Owned,
                });
                continue;
            }
            arm("call operation: structural arguments: view local");
            if let Some(plan) = computation_arguments::borrowed_view_argument(
                program,
                &facts.borrow,
                caller_machine.symbol,
                caller_state,
                call,
                &place,
                target.type_reference,
                Some(&selector),
            ) {
                output.push(plan);
                continue;
            }
            arm("call operation: structural arguments: borrowed local");
            if let Some(plan) = computation_arguments::borrowed_storage_argument(
                program,
                shapes,
                &facts.borrow,
                caller_machine.symbol,
                caller_state,
                call,
                &place,
                target.type_reference,
                Some(&selector),
            ) {
                output.push(plan);
                continue;
            }
            arm("call operation: structural arguments: parameter source");
            let source =
                SourceParameter::resolve(source_parameters, caller_parameters, source_symbol)?;
            let target_access =
                structural_access_for_type_reference(program, target.type_reference);
            let custody = source.custody(target_access);
            if validation::is_closed_primitive_array_type(
                program,
                source.declaration.type_reference,
            ) && !(place.segments.is_empty()
                && custody.access == StructuralAccess::Owned
                && custody.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(program, target.type_reference)
                && target_access == Some(StructuralAccess::Owned))
            {
                return None;
            }
            arm("call operation: structural arguments: parameter path");
            let argument = ParameterArgument {
                program,
                machine: caller_machine.symbol,
                state: caller_state.symbol,
                statement_index,
                place: &place,
                target: target.type_reference,
                target_identity,
                selectors: Some(&selector),
            };
            let path = if bodyless {
                boundary_parameter_path(argument, custody, &[])
            } else {
                ordinary_parameter_path(argument, custody, supply_mode)
            }?;
            arm("call operation: structural arguments: parameter identity");
            if path.is_empty()
                && !argument.whole_parameter_is_admitted(
                    source,
                    supply_mode == MachineSupplyMode::CheckedBody
                        && custody.lends_unrestricted_borrow(),
                    &[],
                )
            {
                return None;
            }
            arm("call operation: structural arguments: parameter access");
            let target_access = target_access?;
            output.push(ArgumentPlan {
                projection_evaluations: Default::default(),
                source: SourcePlan::Parameter {
                    parameter_index: u32::try_from(source.index).ok()?,
                },
                path,
                type_identity: target_identity,
                access: if restored_alias.is_some() {
                    target_access
                } else {
                    let authored_access = exact_structural_argument_access(
                        program,
                        facts,
                        caller_machine.symbol,
                        caller_state.symbol,
                        call,
                        &authored_place,
                        target_access,
                    )?;
                    if authored_access == target_access {
                        authored_access
                    } else {
                        alias_forwarded_access(
                            &borrow_aliases,
                            &authored_place,
                            authored_access,
                            target_access,
                            call.statement_index,
                        )?
                    }
                },
            });
        }
        arm("call operation: structural arguments: explicit argument count");
        if explicit_index != explicit_arguments.len() {
            return None;
        }
        arm("call operation: structural arguments: retained structural positions");
        let explicit = target_parameters
            .iter()
            .zip(explicit_arguments)
            .filter_map(|(parameter, expression)| {
                (!parameter.erased
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none())
                .then_some((parameter, *expression))
            })
            .collect::<Vec<_>>();
        arm("call operation: structural arguments: structural argument count");
        if explicit.len() != output.len() {
            return None;
        }
        for (argument, (parameter, expression)) in output.iter_mut().zip(explicit) {
            if argument.path.is_empty() || !argument.projection_evaluations.is_empty() {
                continue;
            }
            if materialized_argument_owner(expression, parameter.type_reference).is_some() {
                continue;
            }
            arm("call operation: structural arguments: authored projection selectors");
            if authored_projection_selectors(program, expression)?.is_empty() {
                continue;
            }
            arm("call operation: structural arguments: retained projection selectors");
            let selectors = facts.values.scalar_computations.projection_selectors_at(
                caller_machine.symbol,
                caller_state.symbol,
                u32::try_from(statement_index).ok()?,
                expression,
            )?;
            arm("call operation: structural arguments: selector evaluation correspondence");
            if argument
                .retain_projection_evaluations(
                    selectors,
                    &facts.values.scalar_computations.operands,
                )
                .is_none()
            {
                if ::validation::trace_flags::check_sites() {
                    let plans = &facts.values.scalar_computations;
                    let children = plans
                        .operands
                        .span_or_empty(selectors)
                        .iter()
                        .map(|child| {
                            (
                                *child,
                                plans
                                    .nodes
                                    .is_valid(*child)
                                    .then(|| plans.nodes.get(*child).authored_root),
                            )
                        })
                        .collect::<Vec<_>>();
                    eprintln!(
                        "CHECKSITE structural-selector-correspondence machine={:?} state={:?} statement={} call={} expression={:?} source={:?} path={:?} children={:?}",
                        caller_machine.symbol,
                        caller_state.symbol,
                        statement_index,
                        call.call_ordinal,
                        expression,
                        argument.source,
                        argument.path,
                        children,
                    );
                }
                return None;
            }
        }
        Some(output)
    }

    /*
        The access a moved borrow-carrier argument actually forwards. The authored
        call records a Read of the carrier's reference value, so the ordinary
        access evidence reports SharedBorrow while the erased loan itself supplies
        the target's exclusive access. Admission requires exactly that shape: a
        bare, segment-free alias root, only a carrier read at the call, the alias's
        terminal use at this statement, and the loan kind matching the target
        access exactly.
    */
    fn alias_forwarded_access(
        aliases: &[ReceiverAlias],
        authored_place: &validation::CanonicalPlace,
        authored_access: StructuralAccess,
        target_access: StructuralAccess,
        statement_index: usize,
    ) -> Option<StructuralAccess> {
        if authored_access != StructuralAccess::SharedBorrow || !authored_place.segments.is_empty()
        {
            return None;
        }
        let owner = authored_place.root.as_symbol()?;
        let alias = aliases.iter().find(|alias| alias.owner == owner)?;
        if alias.last_use != statement_index {
            return None;
        }
        let forwarded = match alias.kind {
            checked_trees::BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
            checked_trees::BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
        };
        (forwarded == target_access).then_some(target_access)
    }

    /*
        Translate the one bare reference carrier admitted by the checked
        post-reactivation certificate back to its exact restored structural place.
        This is intentionally not a general local-alias resolver.
    */
    fn reborrow_restored_call_alias_target(
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        authored_place: &validation::CanonicalPlace,
    ) -> Option<validation::CanonicalPlace> {
        let authored_root = authored_place.root.as_symbol()?;
        if !authored_place.segments.is_empty() {
            return None;
        }
        let candidates = facts
            .borrow
            .reborrow_disposition_events
            .iter()
            .filter_map(|(_, event)| {
                if event.machine_symbol != machine
                    || event.state_symbol != state
                    || !event.restored_call.is_valid()
                {
                    return None;
                }
                let checked_trees::CheckedParentBorrowResource::DirectRoot {
                    resource: parent_resource,
                } = event.parent_resource
                else {
                    return None;
                };
                if !facts.borrow.direct_loan_resources.is_valid(parent_resource) {
                    return None;
                }
                let parent = facts.borrow.direct_loan_resources.get(parent_resource);
                if parent.owner_symbol != authored_root
                    || parent.access != checked_trees::BorrowAccessKind::Mutable
                {
                    return None;
                }
                let certified_call = facts.flow.control.calls.get(event.restored_call);
                (certified_call.statement_index == call.statement_index
                    && certified_call.call_ordinal == call.call_ordinal
                    && certified_call.target_symbol == call.target_symbol
                    && certified_call.receiver_symbol == call.receiver_symbol
                    && certified_call.has_receiver == call.has_receiver
                    && certified_call.accesses == call.accesses)
                    .then_some(validation::CanonicalPlace {
                        root: parent.captured_place.root,
                        segments: parent.captured_place.segments.clone(),
                    })
            })
            .collect::<Vec<_>>();
        let [place] = candidates.as_slice() else {
            return None;
        };
        Some(place.clone())
    }

    /*
        Translate any member of the exact two- or three-child shared-freeze cohort for
        its final observation call. The checked restoration certificate remains
        the authority: every bare child alias must occur exactly once in the call,
        every target parameter must be a shared reference, and the certified
        whole-parent mutation must be the immediately following statement.
    */
    fn reborrow_restored_shared_cohort_observation_alias_target(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        caller_state: &typed_trees::state::State,
        callee: super::CallTarget<'_>,
        call: &checked_trees::FlowCallFact,
        call_site: &validation::CallSite<'_>,
        authored_place: &validation::CanonicalPlace,
    ) -> Option<validation::CanonicalPlace> {
        let authored_root = authored_place.root.as_symbol()?;
        if !authored_place.segments.is_empty() || call.call_ordinal != 0 {
            return None;
        }

        let target_parameters = callee.parameters;
        if callee.supply_mode != MachineSupplyMode::CheckedBody
            || callee.declaration.is_none_or(|(_, state)| {
                !state_statements(program, state.statement_nodes).is_empty()
            })
            || !is_unit(program, callee.return_type)
            || !matches!(target_parameters.len(), 2 | 3)
            || target_parameters.iter().any(|parameter| {
                structural_access_for_type_reference(program, parameter.type_reference)
                    != Some(StructuralAccess::SharedBorrow)
            })
        {
            return None;
        }
        let arguments = validation::call_site_argument_expressions(program, call_site);
        if arguments.len() != target_parameters.len() {
            return None;
        }
        let argument_roots = arguments
            .iter()
            .map(|expression| {
                let place = place_at(
                    program,
                    caller_state.symbol,
                    call.statement_index,
                    *expression,
                )?;
                let root = place.root.as_symbol()?;
                place.segments.is_empty().then_some(root)
            })
            .collect::<Option<Vec<_>>>()?;
        if !argument_roots
            .iter()
            .enumerate()
            .all(|(index, root)| !argument_roots[..index].contains(root))
        {
            return None;
        }

        let candidates = facts
            .borrow
            .reborrow_disposition_events
            .iter()
            .filter_map(|(_, event)| {
                if event.machine_symbol != machine
                    || event.state_symbol != caller_state.symbol
                    || !event.restored_call.is_valid()
                {
                    return None;
                }
                let checked_trees::CheckedParentBorrowResource::DirectRoot {
                    resource: parent_resource,
                } = event.parent_resource
                else {
                    return None;
                };
                if !facts.borrow.direct_loan_resources.is_valid(parent_resource) {
                    return None;
                }
                let parent = facts.borrow.direct_loan_resources.get(parent_resource);
                let certified_call = facts.flow.control.calls.get(event.restored_call);
                if certified_call.target_symbol == call.target_symbol
                    || certified_call.statement_index != call.statement_index.checked_add(1)?
                    || certified_call.call_ordinal != 0
                    || event.shared_cohort.len() != target_parameters.len()
                {
                    return None;
                }
                let member_roots = event
                    .shared_cohort
                    .iter()
                    .map(|member| {
                        if !facts.borrow.reborrow_loan_resources.is_valid(*member) {
                            return None;
                        }
                        let member = facts.borrow.reborrow_loan_resources.get(*member);
                        (member.machine_symbol == machine
                            && member.state_symbol == caller_state.symbol
                            && member.access == checked_trees::BorrowAccessKind::Read)
                            .then_some(member.owner_symbol)
                    })
                    .collect::<Option<Vec<_>>>()?;
                if !member_roots
                    .iter()
                    .enumerate()
                    .all(|(index, root)| !member_roots[..index].contains(root))
                    || argument_roots != member_roots
                    || !member_roots.contains(&authored_root)
                {
                    return None;
                }
                Some(validation::CanonicalPlace {
                    root: parent.captured_place.root,
                    segments: parent.captured_place.segments.clone(),
                })
            })
            .collect::<Vec<_>>();
        let [place] = candidates.as_slice() else {
            return None;
        };
        Some(place.clone())
    }

    /*
        The `(root, segments)` spelling the borrow facts record for one authored
        place. An attached-data field is its own borrow root, so a place rooted at
        the attached machine with a leading field segment is recorded as that
        field with the remaining segments; every other place is already spelled
        the way it was authored.
    */
    fn borrow_access_spelling(
        machine: SymbolHandle,
        place: &validation::CanonicalPlace,
    ) -> Option<(facts::PlaceRoot, &[facts::PlaceSegment])> {
        let root = place.root;
        if !matches!(
            root,
            facts::PlaceRoot::Symbol(_) | facts::PlaceRoot::Expression(_)
        ) {
            return None;
        }
        if root == facts::PlaceRoot::Symbol(machine)
            && let Some((facts::PlaceSegment::Field { symbol }, remaining)) =
                place.segments.split_first()
        {
            return Some((facts::PlaceRoot::Symbol(*symbol), remaining));
        }
        Some((root, place.segments.as_slice()))
    }

    pub(crate) fn exact_structural_argument_access(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        place: &validation::CanonicalPlace,
        target_access: StructuralAccess,
    ) -> Option<StructuralAccess> {
        let returned_loans = program
            .state_by_symbol(state)
            .and_then(|source_state| {
                let local = program
                    .statement_table
                    .statement_at(source_state, call.statement_index)?
                    .as_local_data()?;
                if is_reference_record(program, local.type_reference) {
                    if call.authored_expression != local.initial_value {
                        return None;
                    }
                    return local_record_loans(
                        program,
                        facts,
                        machine,
                        source_state,
                        u32::try_from(call.statement_index).ok()?,
                    )
                    .and_then(|leaves| {
                        let mut captured = Vec::new();
                        for (_, group) in leaves {
                            captured.extend(facts.borrow.loan_group(group)?.map(|(loan, _)| loan));
                        }
                        Some(captured)
                    });
                }
                let result = BindingPlan {
                    statement_index: u32::try_from(call.statement_index).ok()?,
                    binding_ordinal: 0,
                    type_identity: program.normalized_type_identity(local.type_reference),
                    multiplicity: Multiplicity::Affine,
                    local: Default::default(),
                };
                result_loans(program, facts, machine, source_state, call, &result).and_then(
                    |loans| {
                        facts
                            .borrow
                            .loan_group(loans)
                            .map(|group| group.map(|(loan, _)| loan).collect::<Vec<_>>())
                    },
                )
            })
            .unwrap_or_default();
        exact_structural_borrow_access(
            program,
            &facts.borrow,
            machine,
            state,
            call,
            place,
            target_access,
            &returned_loans,
        )
    }

    pub(crate) fn exact_structural_borrow_access(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        place: &validation::CanonicalPlace,
        target_access: StructuralAccess,
        returned_loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
    ) -> Option<StructuralAccess> {
        if target_access == StructuralAccess::Owned {
            return Some(StructuralAccess::Owned);
        }
        let (root, segments) = borrow_access_spelling(machine, place)?;
        let (_, borrow_state) = borrow.first_state_borrow(machine, state)?;
        let matching_calls = borrow
            .calls
            .span_or_empty(borrow_state.calls)
            .iter()
            .filter(|candidate| {
                candidate.statement_index == call.statement_index
                    && candidate.call_ordinal == call.call_ordinal
                    && candidate.target_symbol == call.target_symbol
            })
            .collect::<Vec<_>>();
        let [borrow_call] = matching_calls.as_slice() else {
            return None;
        };
        let kinds = borrow
            .argument_accesses
            .span_or_empty(borrow_call.accesses)
            .iter()
            .filter(|access| {
                access.end_before_access.is_none()
                    && borrow.access_segments(access) == segments
                    && access.root == root
            })
            .map(|access| &access.kind)
            .collect::<Vec<_>>();
        let first = kinds.first()?;
        if kinds.iter().any(|candidate| *candidate != *first) {
            return None;
        }
        if target_access == StructuralAccess::MutableBorrow
            && **first == checked_trees::BorrowAccessKind::Read
            && segments.is_empty()
            && let Some(root_symbol) = root.as_symbol()
            && reference_forwarding::preserves_mutable_referent(
                program,
                borrow,
                borrow_state,
                borrow_call,
                call,
                root_symbol,
                returned_loans,
            )
        {
            return Some(StructuralAccess::MutableBorrow);
        }
        Some(match *first {
            checked_trees::BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
            checked_trees::BorrowAccessKind::Mutable
                if target_access == StructuralAccess::SharedBorrow =>
            {
                StructuralAccess::SharedBorrow
            }
            checked_trees::BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
            checked_trees::BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
        })
    }

    /*
        Join owned arguments to the call's complete linear permission-event set.
        Argument preparation already checks each source, path and loan. A loan
        never transfers its backing's claim, independently of whether it comes
        from a parameter, local, projection or computed result. Event coverage
        below still refuses a purported owned transfer with no owned argument.
    */
    pub(crate) fn call_claim_transfers(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        caller_parameters: &[CheckedUnitStructuralParameterPlan],
        caller_structural_results: &[(BindingPlan, facts::PlaceRoot)],
        arguments: &[ArgumentPlan],
    ) -> Result<Vec<TransferPlan>, &'static str> {
        let events = call_claim_events(&facts.flow, machine, state, call);
        let mut output = Vec::new();
        let caller_state = program
            .state_by_symbol(state)
            .ok_or("claim transfer: caller state is missing")?;
        let source_parameters = program.state_parameters(caller_state);
        for (argument_index, argument) in arguments.iter().enumerate() {
            if argument.access != StructuralAccess::Owned {
                continue;
            }
            if matches!(argument.source, SourcePlan::PrimitiveLocal { .. })
                || view_subslice::is_view_subslice(argument)
            {
                return Err("claim transfer: owned argument has no structural owner");
            }
            if argument.byte_sequence_literal().is_some() {
                if !argument.path.is_empty() {
                    return Err("claim transfer: owned literal has a projected path");
                }
                continue;
            }
            let root = match argument.source {
                SourcePlan::Parameter { parameter_index } => {
                    let parameter = caller_parameters
                        .get(parameter_index as usize)
                        .ok_or("claim transfer: argument parameter is missing")?;
                    let source = source_parameters
                        .get(parameter.position as usize)
                        .ok_or("claim transfer: parameter source is missing")?;
                    facts::PlaceRoot::Symbol(source.symbol)
                }
                SourcePlan::StructuralResult { binding_ordinal } => {
                    caller_structural_results
                        .iter()
                        .find(|(result, _)| result.binding_ordinal == binding_ordinal)
                        .ok_or("claim transfer: evaluated argument result is missing")?
                        .1
                }
                SourcePlan::StructuralLocal { symbol } => facts::PlaceRoot::Symbol(symbol),
                _ if argument.source_local_declaration_ordinal().is_some()
                    && argument.path.is_empty() =>
                {
                    continue;
                }
                _ => return Err("claim transfer: argument source has no owner coordinate"),
            };
            for (claim_identity, path) in owner_claim_crossings(
                program,
                &facts.flow,
                &facts.values.scalar_computations,
                state,
                call,
                argument_index,
                &events,
                root,
                &argument.path,
            )
            .ok_or("claim transfer: owner path has no unique transferable claim")?
            {
                if output.iter().any(|transfer: &TransferPlan| {
                    (claim_identity != ClaimIdentity::Unknown
                        && transfer.claim_identity == claim_identity)
                        || (usize::try_from(transfer.argument_index).ok() == Some(argument_index)
                            && transfer.path == path)
                }) {
                    return Err("claim transfer: two arguments carry the same linear claim");
                }
                output.push(TransferPlan {
                    claim_identity,
                    argument_index: u32::try_from(argument_index).map_err(
                        |_| "claim transfer: argument ordinal exceeds its representation",
                    )?,
                    path,
                });
            }
        }
        if output.len() != events.len() {
            return Err(
                "claim transfer: owned arguments do not cover the call's permission events",
            );
        }
        Ok(output)
    }

    /*
        The linear claims leaving the caller at one call: every live owned
        transfer or consumption the permission pass recorded at its coordinate.
        Whether the destination is a checked body or a boundary does not change
        the explicit argument's ownership judgment; the call route decides whether
        these rows become ingress transfers or normal-completion receipts.
    */
    fn call_claim_events<'facts>(
        flow: &'facts checked_trees::FlowFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
    ) -> Vec<&'facts checked_trees::FlowPermissionEventFact> {
        flow.ownership.with_state_permissions(state, |handles| {
            handles
                .iter()
                .map(|&handle| flow.ownership.permissions.get(handle))
                .filter(|event| {
                    event.machine_symbol == machine
                        && event.state_symbol == state
                        && event.source
                            == PermissionEventSource::Call {
                                statement_index: call.statement_index,
                                call_ordinal: call.call_ordinal,
                                target_symbol: call.target_symbol,
                            }
                        && matches!(
                            event.kind,
                            PermissionEventKind::Transfer | PermissionEventKind::Consume
                        )
                        && event.access == PermissionAccess::Owned
                        && event.multiplicity == Multiplicity::Linear
                        && event.obligation_live
                })
                .collect()
        })
    }

    /*
        The claims one owned operand carries across its call: the events rooted at
        the operand's owner beneath its selected path. A projected operand carries
        only the claims beneath that path; sibling claims stay with their owner or
        cross at another operand of the same call. Named, parameter and anonymous
        owners share this join: the owner's root, not the operand's source
        spelling, identifies the claims it holds.
    */
    fn owner_claim_crossings(
        program: &dyn ::validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        computations: &checked_trees::CheckedScalarComputationPlans,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        argument_index: usize,
        events: &[&checked_trees::FlowPermissionEventFact],
        root: facts::PlaceRoot,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Option<
        Vec<(
            ClaimIdentity,
            Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        )>,
    > {
        let evaluated = validation::CanonicalPlace {
            root,
            segments: Vec::new(),
        };
        let completed_claims = claim_crossings_at_place(
            program,
            flow,
            computations,
            state,
            call,
            argument_index,
            events,
            &evaluated,
            path,
        )?;
        let Some(expression) = root.as_expression() else {
            return Some(completed_claims);
        };
        let Some(owner) = place_at(program, state, call.statement_index, expression) else {
            return Some(completed_claims);
        };
        if owner == evaluated {
            return Some(completed_claims);
        }
        let source_claims = claim_crossings_at_place(
            program,
            flow,
            computations,
            state,
            call,
            argument_index,
            events,
            &owner,
            path,
        )?;
        match (completed_claims.is_empty(), source_claims.is_empty()) {
            (false, false) => None,
            (false, true) => Some(completed_claims),
            (true, _) => Some(source_claims),
        }
    }

    fn claim_crossings_at_place(
        program: &dyn ::validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        computations: &checked_trees::CheckedScalarComputationPlans,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        argument_index: usize,
        events: &[&checked_trees::FlowPermissionEventFact],
        owner: &validation::CanonicalPlace,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Option<
        Vec<(
            ClaimIdentity,
            Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        )>,
    > {
        let mut crossing_events = events
            .iter()
            .filter(|event| {
                event.root == owner.root
                    && flow
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .starts_with(&owner.segments)
            })
            .peekable();
        if crossing_events.peek().is_none() {
            return Some(Vec::new());
        }
        let root_type = validation::canonical_place_type_reference(
            program,
            state,
            call.statement_index,
            &validation::CanonicalPlace {
                root: owner.root,
                segments: Vec::new(),
            },
        )?;
        let (mut selected_path, _) =
            validation::structural_owned_path(program, root_type, &owner.segments).ok()?;
        selected_path.extend_from_slice(path);
        let mut claims: Vec<(
            ClaimIdentity,
            Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        )> = Vec::new();
        for event in crossing_events {
            let (event_path, _) = validation::structural_owned_path(
                program,
                root_type,
                flow.ownership.segments.span_or_empty(event.segments),
            )
            .ok()?;
            let relative = if let Some(relative) = event_path.strip_prefix(selected_path.as_slice())
            {
                relative
            } else if claim_operand_path_matches(
                program,
                computations,
                state,
                call,
                argument_index,
                owner.root,
                event.expression,
                &selected_path,
            ) {
                event_path.get(selected_path.len()..)?
            } else {
                continue;
            };
            if event.kind != PermissionEventKind::Transfer
                || claims.iter().any(|(identity, existing_path)| {
                    (event.claim_identity != ClaimIdentity::Unknown
                        && *identity == event.claim_identity)
                        || existing_path.as_slice() == relative
                })
            {
                return None;
            }
            claims.push((event.claim_identity, relative.to_vec()));
        }
        Some(claims)
    }

    /*
        Authenticate the whole consuming operand against its already checked
        selector roots. Matching a path's length or an unrelated computed index is
        insufficient: every coordinate belongs to this exact invocation operand.
    */
    #[allow(clippy::too_many_arguments)]
    fn claim_operand_path_matches(
        program: &dyn validation::CheckingInput,
        computations: &checked_trees::CheckedScalarComputationPlans,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        argument_index: usize,
        owner: facts::PlaceRoot,
        expression: typed_trees::expression::ExpressionHandle,
        selected_path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> bool {
        let Some(actual) = place_at(program, state, call.statement_index, expression) else {
            return false;
        };
        if actual.root != owner {
            return false;
        }
        let Some((machine, _)) = validation::find_state(program, state) else {
            return false;
        };
        let selector = |expression| {
            let root = computations.root_at(
                state,
                u32::try_from(call.statement_index).ok()?,
                checked_trees::CheckedScalarExpressionRole::CallSelector {
                    call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                    argument_ordinal: u32::try_from(argument_index).ok()?,
                    expression,
                },
            )?;
            if root.machine != machine.symbol {
                return None;
            }
            let segment = checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(root.root);
            selected_path.contains(&segment).then_some(segment)
        };
        super::argument_paths::projected_evaluated_path(
            program,
            machine.symbol,
            state,
            call.statement_index,
            &actual,
            Some(&selector),
        )
        .is_some_and(|projection| projection.path == selected_path)
    }

    /*
        Join a computed call's owned operands to the call's complete linear
        permission-event set, exactly as an ordinary call operation's arguments
        join it. Each operand names its owner through the canonical place its
        authored actual denotes, so a parameter, a named local and an anonymous
        construction or call result transfer the claims their owner holds. A
        claim no owned operand carries, or one the callee consumes in place,
        leaves the computation without a transfer plan.
    */
    pub(crate) fn computed_call_claim_transfers(
        program: &dyn ::validation::CheckingInput,
        flow: &checked_trees::FlowFacts,
        computations: &checked_trees::CheckedScalarComputationPlans,
        machine: SymbolHandle,
        state: SymbolHandle,
        call: &checked_trees::FlowCallFact,
        operands: &[(typed_trees::expression::ExpressionHandle, &ArgumentPlan)],
    ) -> Option<Vec<TransferPlan>> {
        let events = call_claim_events(flow, machine, state, call);
        let mut output = Vec::new();
        for (argument_index, (expression, argument)) in operands.iter().enumerate() {
            if argument.access != StructuralAccess::Owned {
                continue;
            }
            let root = match argument.source {
                SourcePlan::StructuralExpression { expression, .. } => {
                    facts::PlaceRoot::Expression(expression)
                }
                _ => place_at(program, state, call.statement_index, *expression)?.root,
            };
            for (claim_identity, path) in owner_claim_crossings(
                program,
                flow,
                computations,
                state,
                call,
                argument_index,
                &events,
                root,
                &argument.path,
            )? {
                if output.iter().any(|transfer: &TransferPlan| {
                    (claim_identity != ClaimIdentity::Unknown
                        && transfer.claim_identity == claim_identity)
                        || (usize::try_from(transfer.argument_index).ok() == Some(argument_index)
                            && transfer.path == path)
                }) {
                    return None;
                }
                output.push(TransferPlan {
                    claim_identity,
                    argument_index: u32::try_from(argument_index).ok()?,
                    path,
                });
            }
        }
        (output.len() == events.len()).then_some(output)
    }
}
pub(crate) use structural_arguments::structural_call_arguments;
mod structural_result_call {
    /*
        A call whose value lands in a structural result binding. Shared operand
        admission owns current storage, type and access; this operation owns the
        returned contents and custody, not another whitelist of operand producers.
        Returned references retain exact captured loans, and claim-bearing results
        retain their checked frontier. Both use one ordinary call whose custody is
        decided here and recorded in the plan (`with_structural_custody`).
    */
    use crate::execution::terminal_unit::reference_results::has_borrowed_parts;
    use crate::execution::terminal_unit::reference_results::is_reference_record;
    use crate::execution::terminal_unit::reference_results::result_loans;
    use crate::execution::terminal_unit::types::borrowed_named_view;
    use crate::execution::terminal_unit::types::borrowed_slice_result;
    use checked_trees::{
        CheckedUnitCallParts, CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute,
        CheckedUnitCallTarget, CheckedUnitCallTransfers,
    };

    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::types::machine_binders;
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralResultBindingPlan,
        Multiplicity, SymbolHandle,
    };

    pub(super) fn build(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        trace: &LocalConstructionTrace,
        planned: super::call_operations::PlannedCall<'_>,
        result: &CheckedUnitStructuralResultBindingPlan,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let super::call_operations::PlannedCall {
            coordinate,
            source_site,
            call,
            target_machine,
            target_state,
            target_contract,
            scalar_arguments,
            erased_scalar_arguments,
            erased_proof_arguments,
            structural_arguments,
            transfers,
        } = planned;
        let call_target = CheckedUnitCallTarget {
            machine: target_machine.symbol,
            state: target_state.symbol,
            contract_report_fingerprint: target_contract.report_fingerprint,
            service_reach: call.service_reach,
        };
        let phase = |name: &'static str| {
            trace.phase(name);
            trace.statement(Some(coordinate.statement_index));
        };
        phase("call operation: structural result loan");
        let forwarded_reference_sources =
            validation::reference_result_custody::terminal_call_reference_sources(
                program, facts, state, call, result,
            )
            .unwrap_or_default();
        let reference_result = has_borrowed_parts(program, target_state.return_type);
        let reference_loans = if !forwarded_reference_sources.is_empty() {
            arena::HandleSpan::empty()
        } else if reference_result {
            result_loans(program, facts, machine.symbol, state, call, result)?
        } else {
            arena::HandleSpan::empty()
        };
        let represented_copy_result = result.multiplicity == Multiplicity::Unrestricted
            && validation::has_contents(
                program,
                target_state.return_type,
                &[],
                validation::ContentsRequirement::RepresentedOwned,
            );
        if (!transfers.is_empty()
            || represented_copy_result
            || reference_result
            || !forwarded_reference_sources.is_empty()
            || is_reference_record(program, target_state.return_type)
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_contents(
                program,
                target_state.return_type,
                &[],
                validation::ContentsRequirement::StructuralConstruction,
            ))
            || (result.multiplicity == Multiplicity::Unrestricted
                && validation::is_closed_primitive_array_type(program, target_state.return_type))
            || (matches!(
                result.multiplicity,
                Multiplicity::Affine | Multiplicity::Unrestricted
            ) && validation::has_contents(
                program,
                target_state.return_type,
                &[],
                validation::ContentsRequirement::OwnedOrSharedView,
            ))
            || (result.multiplicity
                == validation::reference_result_custody::result_multiplicity(
                    program,
                    target_state.return_type,
                )
                && (borrowed_slice_result(program, target_state.return_type)
                    || borrowed_named_view(program, target_state.return_type))))
            && program
                .machine_states(target_machine)
                .first()
                .is_some_and(|entry| entry.symbol == target_state.symbol)
            && machine_binders(program, target_machine).is_empty()
        {
            let discard_mint = result.multiplicity == Multiplicity::Affine
                && !reference_result
                && reference_loans.is_empty();
            return with_structural_custody(
                program,
                facts,
                machine.symbol,
                state.symbol,
                trace,
                structural_custody_parts(
                    coordinate,
                    source_site,
                    call_target,
                    target_contract.commitment,
                    erased_scalar_arguments,
                    erased_proof_arguments,
                    scalar_arguments,
                    structural_arguments,
                    result.clone(),
                    discard_mint,
                ),
            );
        }
        None
    }

    /* The Module-route structural-custody call parts both admissible sites build. */
    pub(super) fn structural_custody_parts(
        coordinate: checked_trees::CheckedUnitCallCoordinate,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        target: checked_trees::CheckedUnitCallTarget,
        commitment: checked_trees::MachineContractCommitment,
        erased_scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
        erased_proof_arguments: Vec<checked_trees::CheckedProofTerm>,
        scalar_arguments: Vec<checked_trees::CheckedCallScalarArgument>,
        structural_arguments: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
        binding: checked_trees::CheckedUnitStructuralResultBindingPlan,
        discard_result_on_return: bool,
    ) -> checked_trees::CheckedUnitCallParts {
        checked_trees::CheckedUnitCallParts {
            coordinate,
            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
            source_site,
            target,
            route: CheckedUnitCallRoute::Module {
                contract_commitment: Some(commitment),
                erased_scalar_arguments,
                erased_proof_arguments,
            },
            scalar_arguments,
            structural_arguments,
            result: CheckedUnitCallResult::Structural {
                binding,
                discard_result_on_return,
            },
            transfers: CheckedUnitCallTransfers::StructuralCustody(Default::default()),
        }
    }

    /*
        Admit one ordinary structural call and record its custody: the checker's
        single judgment over the exact call and ownership facts. Lowering reads
        the recorded custody; a call the judgment refuses is not planned.
    */
    pub(super) fn with_structural_custody(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller_machine: SymbolHandle,
        caller_state: SymbolHandle,
        trace: &LocalConstructionTrace,
        parts: CheckedUnitCallParts,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let mut operation = CheckedUnitEffectOperationPlan::Call(CheckedUnitCallPlan::new(parts)?);
        let custody = match validation::structural_call_custody(
            program,
            facts,
            caller_machine,
            caller_state,
            &operation,
        ) {
            Ok(custody) => custody,
            Err(reason) => {
                trace.phase(reason);
                return None;
            }
        };
        *operation.call_mut()?.structural_custody_mut()? = custody;
        Some(operation)
    }
}
mod trait_boundary_call {
    /*
        Calls to authored boundary requirements, including legacy operators.

        The requirement's formals stay generic; the call's checked
        specialization supplies each `Type` binder's actual and each `machine`
        binder's admitted provider, and the replay substitutes them before the
        argument and result evidence is compared with the caller's concrete
        shapes. Owned arguments transfer into the boundary, and every live claim
        gets a normal-completion receipt at its argument. Source checking owns
        authored preconditions and normal guarantees; this planner retains the exact
        checked requirement commitment rather than replaying a contract shape gate.
    */
    use super::super::structural_scalar_store::primitive_local_before;
    use super::computation_arguments::borrowed_storage_argument;
    use super::computation_arguments::primitive_local_argument;
    use crate::execution::terminal_unit::calls::view_subslice;
    use crate::published::places::place_at;

    use crate::execution::terminal_unit::calls::argument_paths::{
        byte_sequence_literal_argument, checked_call_scalar_arguments,
    };
    use crate::execution::terminal_unit::calls::boundary_admission::boundary_trait_receiver_is_namespace;
    use crate::execution::terminal_unit::calls::call_operations::call_value_result_matches;
    use crate::execution::terminal_unit::calls::parameter_argument_paths::{
        ParameterArgument, SourceParameter, boundary_parameter_path,
    };
    use crate::execution::terminal_unit::calls::result_arguments;
    use crate::execution::terminal_unit::calls::structural_arguments::{
        call_claim_transfers, exact_structural_argument_access,
    };
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::types::{base_type_identity, substituted_formal_type};
    use crate::execution::terminal_unit::types::{is_unit, structural_access_for_type_reference};
    use crate::execution::terminal_unit::{
        CheckFacts, CheckedStructuralScalarParameterPlan, CheckedUnitCallCoordinate,
        CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralResultBindingPlan, SymbolHandle,
    };

    /*
        Authored requirement declarations share one call planner. This is a
        borrowed view of the actual declaration, never a manufactured machine or
        a choice of provider; declaration identity survives into the call row.
    */
    #[derive(Clone, Copy)]
    pub(in crate::execution::terminal_unit) enum BoundaryRequirement<'program> {
        Trait {
            definition: &'program typed_trees::trait_definition::TraitDefinition,
            signature: &'program typed_trees::signature::StateSignature,
        },
        Operator(&'program typed_trees::operator::OperatorDefinition),
    }

    impl BoundaryRequirement<'_> {
        pub(super) fn target_symbol(self) -> SymbolHandle {
            match self {
                Self::Trait { signature, .. } => signature.symbol,
                Self::Operator(operator) => operator.symbol,
            }
        }
    }

    /*
        Plan a requirement call with its original argument/result custody.
        `None` means some operand, receiver, or contract has no admitted boundary
        presentation; selection never weakens that signature.
    */
    pub(super) fn build(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut crate::execution::terminal_unit::types::ShapeCollector<'_>,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        caller_parameters: &[CheckedUnitStructuralParameterPlan],
        call: &checked_trees::FlowCallFact,
        expected_call_result: Option<&super::ExpectedCallValueResult<'_>>,
        caller_structural_results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
        caller_operations: &[CheckedUnitEffectOperationPlan],
        trace: &LocalConstructionTrace,
        coordinate: CheckedUnitCallCoordinate,
        call_site: &validation::CallSite<'_>,
        source_site: Option<checked_trees::NominalMachineUseSite>,
        requirement: BoundaryRequirement<'_>,
        selected_realization: Option<(SymbolHandle, SymbolHandle)>,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let phase = |name: &'static str| {
            trace.phase(name);
            trace.statement(Some(coordinate.statement_index));
        };
        phase("call operation: boundary signature");
        let target = requirement.target_symbol();
        let (owner, source_parameters, signature_type_parameters, return_type, callback_parameters) =
            match requirement {
                BoundaryRequirement::Trait {
                    definition,
                    signature,
                } => (
                    definition.symbol,
                    program.state_signature_parameters(signature),
                    program.state_signature_type_parameters(signature),
                    signature.return_type,
                    signature.native_callback_parameters.as_slice(),
                ),
                BoundaryRequirement::Operator(operator) => (
                    operator.symbol,
                    program.operator_parameters(operator),
                    program.operator_type_parameters(operator),
                    operator.return_type,
                    &[][..],
                ),
            };
        let operator_arguments = match (requirement, call_site) {
            (BoundaryRequirement::Operator(_), validation::CallSite::Expression { call, .. }) => {
                Some(
                    crate::published::facts::operator_crashes::named_call_operands(
                        program,
                        call,
                        source_parameters,
                    )?,
                )
            }
            _ => None,
        };
        let arguments = operator_arguments
            .as_deref()
            .unwrap_or_else(|| validation::call_site_argument_expressions(program, call_site));
        if source_parameters
            .iter()
            .any(|parameter| parameter.relevance.is_erased())
        {
            return None;
        }
        let abi_parameters = source_parameters.iter().enumerate().collect::<Vec<_>>();
        let caller_source_parameters = program.state_parameters(state);
        let specialization = source_site.and_then(|site| {
            facts
                .requirement_call_specializations
                .for_site(site, call.target_symbol)
        });
        let substitutions = specialization
            .map(|specialization| {
                specialization
                    .type_bindings
                    .iter()
                    .map(|binding| (binding.parameter, binding.actual))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let authored_machine_arguments = match &call_site {
            validation::CallSite::Statement(call) => call.machine_arguments.as_ref(),
            validation::CallSite::Expression { call, .. } => call.machine_arguments.as_ref(),
            validation::CallSite::TransitionNamed { .. } => &[],
        }
        .iter()
        .filter(|argument| {
            matches!(
                program.symbols.get(argument.symbol).kind,
                symbols::SymbolKind::State | symbols::SymbolKind::MachineParameter
            )
        })
        .collect::<Vec<_>>();
        let specialized_telescope = specialization.is_some_and(|specialization| {
            signature_type_parameters
                .iter()
                .all(|parameter| match &parameter.kind {
                    typed_trees::data::TypeParameterKind::Type => specialization
                        .type_bindings
                        .iter()
                        .any(|binding| binding.parameter == parameter.symbol),
                    typed_trees::data::TypeParameterKind::Machine { .. } => {
                        specialization.machine_selections.iter().any(|selection| {
                            selection.parameter == parameter.symbol
                                && usize::try_from(selection.static_machine_ordinal)
                                    .ok()
                                    .and_then(|ordinal| authored_machine_arguments.get(ordinal))
                                    .is_some_and(|argument| argument.symbol == selection.selected)
                        })
                    }
                    _ => false,
                })
        });
        let callback_contract_parameters = match requirement {
            BoundaryRequirement::Trait { signature, .. } => {
                program.requirement_invocation_contract_parameters(signature)
            }
            BoundaryRequirement::Operator(operator) => program.operator_type_parameters(operator),
        };
        let callback_machine_parameters = callback_contract_parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Machine { .. }
                )
            })
            .collect::<Vec<_>>();
        let registration_operation = program
            .closed_requirement_application(target)
            .map_or(target, |application| application.template);
        let nominal_use_at_binder =
            |ordinal: usize, parameter: &typed_trees::data::TypeParameter| {
                let typed_trees::data::TypeParameterKind::Machine {
                    contract:
                        typed_trees::data::MachineParameterContract::Nominal {
                            trait_definition,
                            requirement,
                            ..
                        },
                } = parameter.kind
                else {
                    return None;
                };
                facts.nominal_machine_uses.uses.iter().find(|nominal_use| {
                    Some(nominal_use.site) == source_site
                        && nominal_use.registration_operation == registration_operation
                        && usize::try_from(nominal_use.static_machine_ordinal).ok() == Some(ordinal)
                        && nominal_use.satisfaction_trait == trait_definition
                        && nominal_use.satisfaction_requirement == requirement
                })
            };
        let admitted_callback_telescope = source_site.is_some()
            && callback_parameters.len() <= callback_machine_parameters.len()
            && callback_parameters.iter().all(|callback| {
                callback_machine_parameters
                    .iter()
                    .filter(|parameter| parameter.name == callback.binder)
                    .count()
                    == 1
            })
            && callback_machine_parameters
                .iter()
                .enumerate()
                .all(|(ordinal, parameter)| {
                    let Some(nominal_use) = nominal_use_at_binder(ordinal, parameter) else {
                        return false;
                    };
                    let mut destinations = callback_parameters
                        .iter()
                        .filter(|callback| callback.binder == parameter.name);
                    match (destinations.next(), destinations.next()) {
                        (Some(_), None) => true,
                        (None, None) => nominal_use.callback_entry.is_some(),
                        _ => false,
                    }
                });
        let mut scalar_parameters = Vec::new();
        let mut structural_arguments = Vec::new();
        phase("call operation: boundary arguments");
        for (abi_position, ((_, parameter), argument)) in
            abi_parameters.iter().zip(arguments.iter()).enumerate()
        {
            let source_position = u32::try_from(abi_position).ok()?;
            let formal = substituted_formal_type(
                program,
                parameter.type_reference,
                substitutions.as_slice(),
            );
            if let Some(primitive_type) = program.primitive_type_reference(formal) {
                scalar_parameters.push(CheckedStructuralScalarParameterPlan {
                    source_position,
                    primitive_type,
                });
                continue;
            }
            phase("call operation: boundary argument identity");
            let target_identity =
                base_type_identity(program, formal, &[], substitutions.as_slice())?;
            let produced_place = caller_structural_results
                .iter()
                .find(|(_, root)| *root == facts::PlaceRoot::Expression(*argument))
                .map(|_| validation::CanonicalPlace {
                    root: facts::PlaceRoot::Expression(*argument),
                    segments: Vec::new(),
                });
            if produced_place.is_none() {
                if let Some(literal) = byte_sequence_literal_argument(program, formal, *argument) {
                    structural_arguments.push(literal);
                    continue;
                }
                if validation::subslice_expression(program, *argument).is_some() {
                    let subslice = view_subslice::admit(
                        program,
                        facts,
                        machine,
                        state,
                        caller_parameters,
                        formal,
                        *argument,
                        call.statement_index,
                        checked_trees::CheckedSubsliceSite::CallArgument {
                            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(structural_arguments.len()).ok()?,
                        },
                    )?;
                    if let Some(argument) =
                        subslice.call_argument(program, state, call.statement_index, true)
                    {
                        structural_arguments.push(argument);
                        continue;
                    }
                }
            }

            phase("call operation: boundary argument place");
            let place = produced_place
                .or_else(|| place_at(program, state.symbol, call.statement_index, *argument))?;
            if let Some((result, _)) = caller_structural_results
                .iter()
                .find(|(_, root)| *root == place.root)
            {
                phase("call operation: boundary result argument");
                let selector = |expression| {
                    let root = facts.values.scalar_computations.root_at(
                        state.symbol,
                        u32::try_from(call.statement_index).ok()?,
                        checked_trees::CheckedScalarExpressionRole::CallSelector {
                            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
                            argument_ordinal: u32::try_from(structural_arguments.len()).ok()?,
                            expression,
                        },
                    )?;
                    Some(checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(root.root))
                };
                structural_arguments.push(result_arguments::argument(
                    program,
                    shapes,
                    facts,
                    machine.symbol,
                    state.symbol,
                    call,
                    *argument,
                    &place,
                    result,
                    parameter.type_reference,
                    target_identity,
                    caller_operations,
                    trace,
                    Some(&selector),
                )?);
                continue;
            }
            phase("call operation: boundary argument parameter");
            let source_symbol = place.root.as_symbol()?;
            if primitive_local_before(program, state, call.statement_index, source_symbol).is_some()
            {
                structural_arguments.push(primitive_local_argument(
                    program,
                    &facts.borrow,
                    machine.symbol,
                    state,
                    call,
                    &place,
                    &place,
                    formal,
                )?);
                continue;
            }
            phase("call operation: boundary borrowed storage");
            if let Some(argument) = borrowed_storage_argument(
                program,
                shapes,
                &facts.borrow,
                machine.symbol,
                state,
                call,
                &place,
                formal,
                None,
            ) {
                structural_arguments.push(argument);
                continue;
            }
            phase("call operation: boundary argument parameter");
            let source = SourceParameter::resolve(
                caller_source_parameters,
                caller_parameters,
                source_symbol,
            )?;
            let custody = source.custody(structural_access_for_type_reference(program, formal));
            let argument = ParameterArgument {
                program,
                machine: machine.symbol,
                state: state.symbol,
                statement_index: call.statement_index,
                place: &place,
                target: formal,
                target_identity,
                selectors: None,
            };
            phase("call operation: boundary argument projection");
            let path = boundary_parameter_path(argument, custody, substitutions.as_slice())?;
            phase("call operation: boundary argument presentation");
            if path.is_empty()
                && !argument.whole_parameter_is_admitted(
                    source,
                    custody.unqualified,
                    substitutions.as_slice(),
                )
            {
                return None;
            }
            phase("call operation: boundary argument access");
            structural_arguments.push(CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(source.index).ok()?,
                },
                path,
                type_identity: target_identity,
                access: exact_structural_argument_access(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    call,
                    &place,
                    custody.target_access?,
                )?,
            });
        }
        phase("call operation: unclosed boundary declaring trait");
        if let BoundaryRequirement::Trait { definition, .. } = requirement
            && !program.trait_type_parameters(definition).is_empty()
            && !program
                .closed_requirement_application(target)
                .is_some_and(|application| application.declaring_trait == definition.symbol)
        {
            return None;
        }
        phase("call operation: unadmitted boundary executable telescope");
        if !signature_type_parameters.is_empty()
            && !admitted_callback_telescope
            && !specialized_telescope
        {
            return None;
        }
        phase("call operation: unadmitted boundary native callback telescope");
        if !callback_parameters.is_empty() && !admitted_callback_telescope {
            return None;
        }
        phase("call operation: boundary parameter custody");
        if source_parameters.iter().any(|parameter| {
            parameter.is_const
                || (parameter.is_mutable
                    && program
                        .primitive_type_reference(substituted_formal_type(
                            program,
                            parameter.type_reference,
                            substitutions.as_slice(),
                        ))
                        .is_some()
                    && crate::values::mutable_scalar_parameter_type(program, parameter).is_none())
        }) {
            return None;
        }
        phase("call operation: boundary receiver");
        if arguments.len() != abi_parameters.len() {
            return None;
        }
        if if let Some((_, qualifier)) = selected_realization {
            call.has_receiver && call.receiver_symbol != qualifier
        } else if matches!(requirement, BoundaryRequirement::Trait { .. }) {
            let namespace =
                boundary_trait_receiver_is_namespace(program, state, call, call_site, owner);
            if !namespace
                && let Some(reason) =
                    super::boundary_authority_refusal(program, machine.symbol, state, call)
            {
                phase(reason);
            }
            !namespace
        } else {
            false
        } {
            return None;
        }
        phase("call operation: boundary result");
        let signature_return =
            substituted_formal_type(program, return_type, substitutions.as_slice());
        if match expected_call_result {
            None => !is_unit(program, signature_return),
            Some(expected) => {
                !call_value_result_matches(program, return_type, expected, substitutions.as_slice())
            }
        } {
            return None;
        }
        let capsule = facts.contract_plans.crash_capsule(owner, target)?;
        phase("call operation: boundary claim transfers");
        let completion = validation::boundary_requirement_completion(program, target);
        let transfers = match completion {
            semantic_vocabulary::BoundaryCompletion::External(_) => {
                checked_trees::CheckedUnitCallTransfers::Abandonment
            }
            semantic_vocabulary::BoundaryCompletion::NormalReturn => {
                checked_trees::CheckedUnitCallTransfers::CompletionReceipts(
                    call_claim_transfers(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        call,
                        caller_parameters,
                        caller_structural_results,
                        &structural_arguments,
                    )
                    .inspect_err(|reason| phase(reason))
                    .ok()?,
                )
            }
        };
        phase("call operation: boundary scalar arguments");
        let scalar_arguments = checked_call_scalar_arguments(
            facts,
            state.symbol,
            coordinate,
            &scalar_parameters,
            true,
        )?;
        checked_trees::CheckedUnitCallPlan::new(checked_trees::CheckedUnitCallParts {
            coordinate,
            source_site,
            target: checked_trees::CheckedUnitCallTarget {
                machine: target,
                state: target,
                contract_report_fingerprint: capsule.target_contract_report_fingerprint(),
                service_reach: call.service_reach,
            },
            route: checked_trees::CheckedUnitCallRoute::Boundary,
            completion,
            scalar_arguments,
            structural_arguments,
            result: checked_trees::CheckedUnitCallResult::Unit,
            transfers,
        })
        .map(CheckedUnitEffectOperationPlan::Call)
    }
}
pub(crate) mod view_subslice {
    /*
        Admit one checked collection range with its exact access and endpoints.

        Local bindings, calls and state transfers carry the same range operation.
        Source storage and the requested reference determine authority; a type or
        endpoint equation cannot amplify a shared loan into a mutable one. Local
        array ranges additionally rejoin the exact checked loan, including access,
        formation statement and captured projection. Call-scoped ranges retain
        their ordinary call-argument overlap judgment.

        Arrays first lend a whole descriptor; established views narrow directly.
        This is a carrier distinction, not a local-versus-parameter alternative.
        Present endpoints retain their checked site and evaluation order. Byte and
        element counts remain separate. Checking proves the authored call's domain
        requirements; Terminal independently reconstructs subslice-preserving byte
        predicates from the exact source. The result never asserts a qualification
        merely because its consumer wants one.
    */
    use crate::execution::terminal_unit::CheckFacts;
    use crate::execution::terminal_unit::CheckedScalarExpressionRole;
    use crate::execution::terminal_unit::CheckedStructuralAccess;
    use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
    use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
    use crate::execution::terminal_unit::CheckedUnitStructuralParameterPlan;
    use crate::execution::terminal_unit::Multiplicity;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::published::places::place_at;

    use crate::execution::terminal_unit::PrimitiveType;
    use crate::execution::terminal_unit::StatementNode;
    use crate::execution::terminal_unit::TypeReferenceHandle;
    use crate::execution::terminal_unit::TypeReferenceNode;
    use crate::execution::terminal_unit::types::{
        borrowed_slice_view_element, byte_sequence_carrier, byte_sequence_type_identity,
        structural_access_for_type_reference,
    };
    use checked_trees::{
        CheckedScalarExpressionPlans, CheckedStorageRoot,
        CheckedStructuralControlTransferSourcePlan, CheckedSubsliceSite,
        CheckedUnitStructuralPathSegment,
    };
    use typed_trees::expression::ExpressionHandle;

    /* Which borrowed view family a range narrows. */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(in crate::execution) enum ViewKind {
        Bytes,
        Elements,
    }

    /* A descriptor can be narrowed directly; array storage must first lend one. */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ViewStorage {
        Established,
        FixedArray,
    }

    /*
        The exact shape of one admitted range: its established source and the
        narrowed view's type identity. Endpoints are attached by `admit`.
    */
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(in crate::execution) struct ViewRange {
        pub(in crate::execution) root: CheckedStorageRoot,
        pub(in crate::execution) root_type_identity: typed_trees::type_identity::TypeIdentityId,
        storage: ViewStorage,
        /*
            Exact ordered projection to the collection or established descriptor.
            Storage kind comes from the selected declaration, never path length.
        */
        pub(in crate::execution) collection: Vec<CheckedUnitStructuralPathSegment>,
        /*
            Exact Terminal type identity of the collection's storage carrier,
            enrolled in the checked structural type table.
        */
        pub(in crate::execution) storage_type_identity: typed_trees::type_identity::TypeIdentityId,
        pub(in crate::execution) kind: ViewKind,
        pub(in crate::execution) access: CheckedStructuralAccess,
        pub(in crate::execution) type_identity: typed_trees::type_identity::TypeIdentityId,
    }

    /*
        One admitted range with its present endpoints, copied from the unique
        source-bound `SubsliceStart`/`SubsliceEnd` facts at its site.
    */
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ViewSubslice {
        pub(in crate::execution) range: ViewRange,
        pub(in crate::execution) expression: ExpressionHandle,
        pub(in crate::execution) start: Option<checked_trees::CheckedCallScalarArgument>,
        pub(in crate::execution) end: Option<checked_trees::CheckedCallScalarArgument>,
        pub(in crate::execution) end_inclusive: bool,
        pub(in crate::execution) site: CheckedSubsliceSite,
    }

    impl ViewSubslice {
        /*
            A bodyless signature cannot prove that a result does not retain its
            descriptor argument. Preserve fixed projected windows as call-scoped
            places instead; dynamic windows and whole views keep their evaluated
            descriptor. This presentation depends on the declared retention
            boundary, not whether provider selection reached it through a map.
            `None` means an admitted fixed projection, not a failed range admission.
        */
        pub(crate) fn call_argument(
            self,
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            statement_index: usize,
            bodyless_signature: bool,
        ) -> Option<CheckedUnitStructuralArgumentPlan> {
            let fixed_projection = bodyless_signature
                && !(self.range.kind == ViewKind::Bytes && self.range.collection.is_empty())
                && matches!(
                    place_at(program, state.symbol, statement_index, self.expression)
                        .and_then(|place| place.segments.last().cloned()),
                    Some(facts::PlaceSegment::FixedRange { .. })
                );
            (!fixed_projection).then(|| self.argument())
        }

        /*
            The access-preserving argument plan a call, a return or a view-local
            binding carries for this range.
        */
        pub(crate) fn argument(self) -> CheckedUnitStructuralArgumentPlan {
            let ViewSubslice {
                range,
                expression,
                start,
                end,
                end_inclusive,
                site,
            } = self;
            CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: match range.kind {
                    ViewKind::Bytes => {
                        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                            root: range.root,
                            root_type_identity: range.root_type_identity,
                            expression,
                            start,
                            end,
                            end_inclusive,
                            site,
                            storage_type_identity: range.storage_type_identity,
                        }
                    }
                    ViewKind::Elements => {
                        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                            root: range.root,
                            root_type_identity: range.root_type_identity,
                            expression,
                            start,
                            end,
                            end_inclusive,
                            site,
                            storage_type_identity: range.storage_type_identity,
                        }
                    }
                },
                path: range.collection,
                type_identity: range.type_identity,
                access: range.access,
            }
        }

        /*
            The edge transfer for this range: its argument plan, admitted at the
            edge's `TransitionArgument` site, so the endpoints are the edge's own.
        */
        pub(in crate::execution) fn transfer(self) -> CheckedStructuralControlTransferSourcePlan {
            CheckedStructuralControlTransferSourcePlan::ViewSubslice(self.argument())
        }
    }

    /*
        Admit an exact builtin exclusive range at `site`, vetoing any range whose
        operator use did not resolve to the builtin meaning.
    */
    pub(in crate::execution) fn admit(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        target: TypeReferenceHandle,
        expression: ExpressionHandle,
        statement_index: usize,
        site: CheckedSubsliceSite,
    ) -> Option<ViewSubslice> {
        admit_resolved(
            program,
            &facts.operators,
            &facts.values.scalar_expressions,
            &facts.values.scalar_computations,
            &facts.borrow,
            machine,
            state,
            parameters,
            target,
            expression,
            statement_index,
            site,
        )
    }

    /*
        Whether `argument` produces a range descriptor. Its path selects the
        backing collection; it does not transfer an owned subtree to the callee.
    */
    pub(crate) fn is_view_subslice(argument: &CheckedUnitStructuralArgumentPlan) -> bool {
        matches!(
            argument.source,
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
                | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. }
        )
    }

    /*
        Shared admission also serves computation construction, before CheckFacts
        is assembled. Operator meaning and source-bound endpoint facts are both
        required; a range with omitted endpoints still needs the operator veto.
    */
    pub(crate) fn admit_resolved(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        expressions: &CheckedScalarExpressionPlans,
        computations: &checked_trees::CheckedScalarComputationPlans,
        borrow: &checked_trees::BorrowFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        target: TypeReferenceHandle,
        expression: ExpressionHandle,
        statement_index: usize,
        site: CheckedSubsliceSite,
    ) -> Option<ViewSubslice> {
        builtin_range_operation(program, operators, expression)?;
        let subslice = admit_replayed(
            program,
            expressions,
            computations,
            machine,
            state,
            parameters,
            target,
            expression,
            statement_index,
            site,
        )?;
        let established = match site {
            _ if subslice.range.storage == ViewStorage::Established => true,
            CheckedSubsliceSite::LocalBinding => lends_collection(
                program,
                borrow,
                machine,
                state,
                statement_index,
                expression,
                subslice.range.access,
            ),
            CheckedSubsliceSite::CallArgument { .. } | CheckedSubsliceSite::Expression { .. } => {
                true
            }
            CheckedSubsliceSite::TransitionArgument { .. } => match subslice.range.root {
                CheckedStorageRoot::Parameter { index } => {
                    parameters.get(index as usize).is_some_and(|parameter| {
                        matches!(
                            parameter.access,
                            checked_trees::checked_borrowed_access_pair_pattern!()
                        ) || (parameter.access == CheckedStructuralAccess::Owned
                            && parameter.multiplicity == Multiplicity::Unrestricted
                            && subslice.range.storage == ViewStorage::FixedArray)
                    })
                }
                CheckedStorageRoot::OwnedLocal { .. } => {
                    subslice.range.storage == ViewStorage::FixedArray
                }
                CheckedStorageRoot::ViewLocal { .. } => false,
            },
        };
        established.then_some(subslice)
    }

    /*
        Join a range occurrence to the selected builtin operator, independently of
        whether its collection already has a named storage home.
    */
    pub(crate) fn builtin_range_operation(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let operation = validation::subslice_expression(program, expression)?;
        let spelling = language_core::OperatorSpelling::Range;
        if operators.expression_use(operation).is_some_and(|selected| {
            selected.spelling != spelling
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                )
        }) {
            return None;
        }
        Some(operation)
    }

    /*
        Whether checked borrow admission recorded the `let` at `statement_index`
        as one exact formation, with matching access, of the fixed-array range `expression`
        narrows. The view the range establishes is that loan: the ledger, not
        the authored spelling, is what keeps the array unwritten while it lives.
    */
    #[allow(clippy::too_many_arguments)]
    fn lends_collection(
        program: &dyn ::validation::CheckingInput,
        borrow: &checked_trees::BorrowFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
        access: CheckedStructuralAccess,
    ) -> bool {
        let Some(StatementNode::LocalData(local)) =
            program.statement_table.statement_at(state, statement_index)
        else {
            return false;
        };
        if local.initial_value != expression {
            return false;
        }
        let Some(place) = place_at(program, state.symbol, statement_index, expression) else {
            return false;
        };

        let Some((_, borrow_state)) = borrow.first_state_borrow(machine.symbol, state.symbol)
        else {
            return false;
        };
        let mut loans = borrow
            .loans
            .span_or_empty(borrow_state.loans)
            .iter()
            .filter(|loan| {
                loan.owner_symbol == local.symbol
                    && loan.statement_index == statement_index
                    && loan.formation_expression == expression
            });
        let (Some(loan), None) = (loans.next(), loans.next()) else {
            return false;
        };
        let lent = validation::CanonicalPlace {
            root: loan.root,
            segments: borrow.access_segments.span_or_empty(loan.segments).to_vec(),
        };

        loan.statement_index == statement_index
            && loan.kind
                == match access {
                    CheckedStructuralAccess::SharedBorrow => checked_trees::BorrowAccessKind::Read,
                    CheckedStructuralAccess::MutableBorrow => {
                        checked_trees::BorrowAccessKind::Mutable
                    }
                    _ => return false,
                }
            && lent == place
    }

    /*
        Admission without the operator-resolution veto. Lanes that replay emitted
        endpoint bindings inherit that veto through the bindings' presence: no
        builtin range means no endpoints to rebind.
    */
    pub(in crate::execution) fn admit_replayed(
        program: &dyn ::validation::CheckingInput,
        expressions: &CheckedScalarExpressionPlans,
        computations: &checked_trees::CheckedScalarComputationPlans,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        target: TypeReferenceHandle,
        expression: ExpressionHandle,
        statement_index: usize,
        site: CheckedSubsliceSite,
    ) -> Option<ViewSubslice> {
        let range = shape(
            program,
            machine,
            state,
            parameters,
            target,
            expression,
            statement_index,
        )?;
        let (start, end, end_inclusive) = retained_endpoints(
            program,
            expressions,
            computations,
            machine.symbol,
            state.symbol,
            statement_index,
            expression,
            site,
        )?;
        Some(ViewSubslice {
            range,
            expression,
            start,
            end,
            end_inclusive,
            site,
        })
    }

    /* Exact endpoint bindings shared by named collection ranges and value ranges. */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn retained_endpoints(
        program: &dyn ::validation::CheckingInput,
        expressions: &CheckedScalarExpressionPlans,
        computations: &checked_trees::CheckedScalarComputationPlans,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        site: CheckedSubsliceSite,
    ) -> Option<(
        Option<checked_trees::CheckedCallScalarArgument>,
        Option<checked_trees::CheckedCallScalarArgument>,
        bool,
    )> {
        let operation = validation::subslice_expression(program, expression)?;
        let indexed = program.expression_table.indexed_node(operation)?;
        let authored = program.expression_table.range_node(indexed.index)?;
        let statement_ordinal = u32::try_from(statement_index).ok()?;
        let endpoint = |endpoint: ExpressionHandle, role| {
            if !endpoint.is_valid() {
                return Some(None);
            }
            if let Some(root) = computations.root_at(state, statement_ordinal, role) {
                if !computations.nodes.is_valid(root.root)
                    || expressions
                        .expression_at(state, statement_ordinal, role)
                        .is_some()
                {
                    return None;
                }
                let value = computations.nodes.get(root.root);
                return (root.machine == machine
                    && value.authored_root == endpoint
                    && value.primitive_type.integer_sign_bits().is_some())
                .then_some(Some(checked_trees::CheckedCallScalarArgument::Computation(
                    root.root,
                )));
            }
            let (binding, value) =
                expressions.bound_expression_at(state, statement_ordinal, role)?;
            (binding.expression == endpoint
                && !binding.destination.is_valid()
                && value
                    .primitive_type()
                    .is_some_and(|primitive| primitive.integer_sign_bits().is_some()))
            .then(|| {
                Some(checked_trees::CheckedCallScalarArgument::Pure(
                    value.clone(),
                ))
            })
        };
        let start = endpoint(
            authored.start,
            CheckedScalarExpressionRole::SubsliceStart { site },
        )?;
        let end = endpoint(
            authored.end,
            CheckedScalarExpressionRole::SubsliceEnd { site },
        )?;
        Some((start, end, authored.end_inclusive))
    }

    /*
        Range admission without endpoints: the target is an exact readable
        borrowed view, the authored range is builtin, and the
        collection either holds an established view of the same kind and identity
        or names readable array storage with exactly the view's element type.
    */
    pub(in crate::execution) fn shape(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        target: TypeReferenceHandle,
        expression: ExpressionHandle,
        statement_index: usize,
    ) -> Option<ViewRange> {
        let kind = view_kind(program, target).or_else(|| {
            borrowed_slice_view_element(program, target, &[]).map(|_| ViewKind::Elements)
        })?;
        shape_of_kind(
            program,
            machine,
            state,
            parameters,
            target,
            kind,
            expression,
            statement_index,
        )
    }

    /* `shape` for a target of the view family `kind`. */
    #[allow(clippy::too_many_arguments)]
    fn shape_of_kind(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        target: TypeReferenceHandle,
        kind: ViewKind,
        expression: ExpressionHandle,
        statement_index: usize,
    ) -> Option<ViewRange> {
        let access = borrowed_view_access(program, target, expression)?;
        let expression = validation::subslice_expression(program, expression)?;
        let indexed = program.expression_table.indexed_node(expression)?;
        let type_identity = view_identity(program, kind, target)?;
        let (root, root_type_identity, collection, storage, storage_kind) = collection_storage(
            program,
            machine,
            state,
            parameters,
            target,
            indexed.collection,
            statement_index,
        )?;
        validation::has_builtin_subslice_meaning(program, machine, Some(state), expression)
            .then_some(ViewRange {
                root,
                root_type_identity,
                storage: storage_kind,
                collection,
                storage_type_identity: validation::structural_type_identity(
                    program,
                    storage,
                    &[],
                    &[],
                ),
                kind,
                access,
                type_identity,
            })
    }

    /*
        Resolve the canonical borrowed collection operand once. Its declared
        carrier decides whether storage must lend a descriptor or an existing
        descriptor narrows directly; local/formal origins and member projections
        do not select another range producer. Source access, exact element type,
        local loan admission and endpoint formation remain independent obligations.
    */
    #[allow(clippy::too_many_arguments)]
    fn collection_storage(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        target: TypeReferenceHandle,
        collection: ExpressionHandle,
        statement_index: usize,
    ) -> Option<(
        CheckedStorageRoot,
        typed_trees::type_identity::TypeIdentityId,
        Vec<CheckedUnitStructuralPathSegment>,
        TypeReferenceHandle,
        ViewStorage,
    )> {
        let target_view = validation::slice_view_shell(program, target, &[])?;
        let element = target_view.element;
        let mutable = target_view.access == language_core::ReferenceAccess::Mutable;
        let place = place_at(program, state.symbol, statement_index, collection)?;

        let symbol = place.root.as_symbol()?;
        let (root, root_type_identity) = if let Some(position) =
            typed_trees::signature::state_parameter_position(
                program.state_parameters(state),
                symbol,
            ) {
            let parameter_index = parameters
                .iter()
                .position(|parameter| parameter.position as usize == position)?;
            if !matches!(
                parameters[parameter_index].access,
                CheckedStructuralAccess::SharedBorrow
                    | CheckedStructuralAccess::MutableBorrow
                    | CheckedStructuralAccess::Owned
            ) {
                return None;
            }
            if mutable
                && parameters[parameter_index].access == CheckedStructuralAccess::SharedBorrow
            {
                return None;
            }
            (
                CheckedStorageRoot::Parameter {
                    index: u32::try_from(parameter_index).ok()?,
                },
                validation::structural_type_identity(
                    program,
                    program
                        .state_parameters(state)
                        .get(position)?
                        .type_reference,
                    &[],
                    &[],
                ),
            )
        } else {
            let (_, local) = program
                .state_local_declaration(state, symbol)
                .filter(|&(ordinal, _)| ordinal < statement_index)?;
            let source_access =
                structural_access_for_type_reference(program, local.type_reference)?;
            if !matches!(
                source_access,
                CheckedStructuralAccess::Owned
                    | CheckedStructuralAccess::SharedBorrow
                    | CheckedStructuralAccess::MutableBorrow
            ) || mutable && source_access == CheckedStructuralAccess::SharedBorrow
            {
                return None;
            }
            (
                if source_access == CheckedStructuralAccess::Owned {
                    CheckedStorageRoot::OwnedLocal { symbol }
                } else {
                    CheckedStorageRoot::ViewLocal { symbol }
                },
                validation::structural_type_identity(program, local.type_reference, &[], &[]),
            )
        };
        let ProjectedArgumentPath {
            storage_type: mut projected,
            path,
            ..
        } = super::projected_borrowed_path(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &place,
        )?;
        if let Some(source_view) = validation::slice_view_shell(program, projected, &[]) {
            if source_view.access == language_core::ReferenceAccess::WriteOnly
                || mutable && source_view.access != language_core::ReferenceAccess::Mutable
                || program.normalized_type_identity(source_view.element)
                    != program.normalized_type_identity(element)
            {
                return None;
            }
            return Some((
                root,
                root_type_identity,
                path,
                projected,
                ViewStorage::Established,
            ));
        }
        let storage = projected;
        if path.is_empty()
            && matches!(root, CheckedStorageRoot::Parameter { .. })
            && let Some(reference) = validation::reference_shell(program, projected)
        {
            if !matches!(
                reference.access,
                language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable
            ) {
                return None;
            }
            projected = reference.referee;
        }
        let array_element = loop {
            match program.type_reference_table.type_reference(projected) {
                TypeReferenceNode::Constrained { base_type, .. } => projected = *base_type,
                TypeReferenceNode::FixedArray { element_type, .. } => break *element_type,
                _ => return None,
            }
        };
        (program.normalized_type_identity(array_element)
            == program.normalized_type_identity(element))
        .then_some((
            root,
            root_type_identity,
            path,
            storage,
            ViewStorage::FixedArray,
        ))
    }

    /*
        The whole-extent spelling of the same range: `let v: &T = coll` lends
        `coll`'s entire backing as the `coll[..]` range — no `..` operator is
        spelled, so its endpoints stay omitted and the backing's own extent
        names the end.
    */
    pub(in crate::execution) fn whole_extent(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[CheckedUnitStructuralParameterPlan],
        target: TypeReferenceHandle,
        expression: ExpressionHandle,
        statement_index: usize,
    ) -> Option<(ViewSubslice, TypeReferenceHandle)> {
        let kind = view_kind(program, target).or_else(|| {
            borrowed_slice_view_element(program, target, &[]).map(|_| ViewKind::Elements)
        })?;
        let access = borrowed_view_access(program, target, expression)?;
        let type_identity = view_identity(program, kind, target)?;
        let (root, root_type_identity, collection, storage, storage_kind) = collection_storage(
            program,
            machine,
            state,
            parameters,
            target,
            expression,
            statement_index,
        )?;
        if storage_kind != ViewStorage::FixedArray {
            return None;
        }
        Some((
            ViewSubslice {
                range: ViewRange {
                    root,
                    root_type_identity,
                    storage: ViewStorage::FixedArray,
                    collection,
                    storage_type_identity: validation::structural_type_identity(
                        program,
                        storage,
                        &[],
                        &[],
                    ),
                    kind,
                    access,
                    type_identity,
                },
                expression,
                start: None,
                end: None,
                end_inclusive: false,
                site: CheckedSubsliceSite::LocalBinding,
            },
            storage,
        ))
    }

    /*
        Whether `reference` is an immutable byte view under domain constraints
        only (`&[u8] in Path`): an exact shared `&[u8]` whose reference or slice
        carries one or more constraint shells, each naming only domains.
    */
    fn domain_qualified_byte_view(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        let mut constrained = false;
        let mut peel = |mut reference: TypeReferenceHandle| {
            while let TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } = program.type_reference_table.type_reference(reference)
            {
                if !program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| {
                        matches!(
                            constraint,
                            typed_trees::types::TypeConstraintNode::Domain(_)
                        )
                    })
                {
                    return None;
                }
                constrained = true;
                reference = *base_type;
            }
            Some(reference)
        };
        let Some(TypeReferenceNode::Reference {
            referee,
            access: language_semantics::ReferenceAccess::Shared,
            ..
        }) = peel(reference).map(|outer| program.type_reference_table.type_reference(outer))
        else {
            return false;
        };
        let Some(TypeReferenceNode::Slice { element_type }) =
            peel(*referee).map(|inner| program.type_reference_table.type_reference(inner))
        else {
            return false;
        };
        constrained
            && matches!(
                program.type_reference_table.type_reference(*element_type),
                TypeReferenceNode::Named { .. }
            )
            && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
    }

    /* The borrowed view family of an exact shared or mutable slice type. */
    pub(in crate::execution) fn view_kind(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<ViewKind> {
        if domain_qualified_byte_view(program, reference) {
            return Some(ViewKind::Bytes);
        }
        let TypeReferenceNode::Reference { referee, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let TypeReferenceNode::Slice { element_type } =
            program.type_reference_table.type_reference(*referee)
        else {
            return None;
        };
        if !matches!(
            structural_access_for_type_reference(program, reference),
            Some(checked_trees::checked_borrowed_access_pair_pattern!())
        ) {
            return None;
        }
        if matches!(
            program.type_reference_table.type_reference(*element_type),
            TypeReferenceNode::Named { .. }
        ) && checked_trees::is_borrowed_view(byte_sequence_carrier(program, reference, &[]))
        {
            return Some(ViewKind::Bytes);
        }
        borrowed_slice_view_element(program, reference, &[]).map(|_| ViewKind::Elements)
    }

    /* The type identity of a view of `kind`, as the view's own family spells it. */
    pub(in crate::execution) fn view_identity(
        program: &dyn ::validation::CheckingInput,
        kind: ViewKind,
        reference: TypeReferenceHandle,
    ) -> Option<typed_trees::type_identity::TypeIdentityId> {
        match kind {
            ViewKind::Bytes => byte_sequence_type_identity(program, reference, &[], &[]),
            ViewKind::Elements => Some(validation::structural_type_identity(
                program,
                reference,
                &[],
                &[],
            )),
        }
    }

    /*
        A view root's declared access must itself be a borrow, and when the view
        expression is spelled `&`/`&mut` its authored flavor must match exactly —
        a mismatched spelling selects no subslice.
    */
    fn borrowed_view_access(
        program: &dyn ::validation::CheckingInput,
        target: TypeReferenceHandle,
        expression: ExpressionHandle,
    ) -> Option<CheckedStructuralAccess> {
        let access = structural_access_for_type_reference(program, target)?;
        if !matches!(
            access,
            checked_trees::checked_borrowed_access_pair_pattern!()
        ) {
            return None;
        }
        if let Some(borrow) = program.expression_table.borrow_expression(expression) {
            if borrow.access == language_core::ReferenceAccess::WriteOnly {
                return None;
            }
            if CheckedStructuralAccess::from(borrow.access) != access {
                return None;
            }
        }
        Some(access)
    }
}

pub(crate) use result_arguments::{
    is_stored_reference, retained_reference_argument, validate_shared_temporary,
};

/*
    The callee as argument planning reads it: formals, result and supply.

    An ordinary or boundary target is a typed machine state the planner may
    also inspect. A selector call's target is its instantiated slot: every
    implementation it can reach is a checked body, so its operands take
    checked-body custody, but no single body is known to inspect.
*/
#[derive(Clone, Copy)]
pub(crate) struct CallTarget<'program> {
    pub(crate) parameters: &'program [typed_trees::signature::StateParameter],
    pub(crate) return_type: typed_trees::types::TypeReferenceHandle,
    pub(crate) supply_mode: language_semantics::MachineSupplyMode,
    /* The target's own declaration, when it has one a caller can inspect. */
    pub(crate) declaration: Option<(
        &'program typed_trees::machine::Machine,
        &'program typed_trees::state::State,
    )>,
}

impl<'program> CallTarget<'program> {
    /*
        Signature-only transport is independent of the declaration's trust tier.
        A missing checked body is not permission to pretend it is an opaque one.
    */
    pub(crate) fn has_bodyless_signature(self) -> bool {
        self.declaration
            .is_some_and(|(machine, _)| machine.has_bodyless_signature())
    }

    pub(crate) fn declared(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
    ) -> Self {
        Self {
            parameters: program.state_parameters(state),
            return_type: state.return_type,
            supply_mode: machine.supply_mode,
            declaration: Some((machine, state)),
        }
    }

    pub(crate) fn dynamic_slot(
        slot: &'program typed_trees::dynamic_traits::DynamicSlotSignature,
    ) -> Self {
        Self {
            parameters: &slot.parameters,
            return_type: slot.return_type,
            supply_mode: language_semantics::MachineSupplyMode::CheckedBody,
            declaration: None,
        }
    }
}

pub(crate) use affine_locals::build_affine_array_construction_prefix;
pub(crate) use argument_paths::{
    ProjectedArgumentPath, borrowed_argument_endpoint, borrowed_index_segment,
    byte_sequence_literal_argument, projected_argument_path, projected_borrowed_path,
    projected_observed_path, projected_selected_path, retain_local_reference_source,
    stored_reference_prefix,
};
pub(crate) use boundary_admission::{
    boundary_authority_refusal, boundary_trait_receiver_is_namespace,
};
pub(in super::super) use call_operations::ExpectedCallValueResult;
pub(in super::super) use call_operations::build_call_operation;
pub(crate) use computation_arguments::{ComputationCallArguments, storage_argument};
pub(crate) use signatures::{
    entry_claims, free_structural_scalar_entry_signature_traced, free_structural_scalar_signature,
    free_structural_scalar_signature_traced, parameter_signatures,
};
pub(crate) use structural_arguments::{call_claim_transfers, computed_call_claim_transfers};
