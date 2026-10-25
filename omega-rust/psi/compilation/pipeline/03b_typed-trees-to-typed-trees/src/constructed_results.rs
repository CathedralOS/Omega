//! Retain complete constructed value types before checking freezes them.
//!
//! A projection can know its element or member without knowing its complete
//! value type. Ownership needs the latter: an array's authored extent and a
//! constructor's applied declaration determine each child's claim path. Derive
//! compound values from completed children as calls bind, never from a
//! same-shaped declaration or the consumer's formal type. Child-before-parent
//! preparation gives nested constructors the same ordinary result-type route.
//! No ownership, membership or loan permission is established by this derivation.
//! Reference formation also needs a value type before static call inference.
//! Its target may be storage or another completed value, so borrows belong in
//! the same postorder as arrays and records, not a preceding place-only sweep.
//! A declared place query loses constructed targets and silently leaves their
//! borrowers unprepared. Result types grant no storage or loan permission.
//! Reborrowing lends the existing referent, not the reference carrier itself.
//! Calls apply their declared result to the caller in that same postorder.
//! Retyping only inferred locals leaves nested operands carrying undeclared
//! callee regions, so no parent construction can derive its complete type.
//! Contract expressions belong to their declaring telescope too. Prepare their
//! children alongside body expressions: a constructor in a postcondition needs
//! its applied type before checking can establish its equality meaning. This
//! supplies type vocabulary, not the truth of the contract or its premises.
//! Abstract operators and requirement signatures use their exact parameter
//! roster without manufacturing a machine or demanding runtime specialization.

use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

mod call_results;

/// A value-type query is not source admission. Preserve its refusal for an
/// inferred declaration that requires this result, without masking independent
/// source errors in a body whose explicit declarations already supply types.
pub(crate) struct UnavailableValueType {
    pub(crate) expression: ExpressionHandle,
    pub(crate) diagnostics: Vec<diagnostics::Diagnostic>,
}

/// Executable storage and abstract signature parameters supply different
/// lexical scopes to the same completed-value derivation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueTypeOwner {
    State {
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    },
    Declaration {
        callable: symbols::SymbolHandle,
        parameters: arena::HandleSpan<typed_trees::signature::StateParameter>,
    },
}

impl ValueTypeOwner {
    fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::State { machine, .. } => machine,
            Self::Declaration { callable, .. } => callable,
        }
    }

    /// Retain applications from this actual source's lexical root. Whole
    /// computed values may have no enclosing application; an unresolved
    /// projected prefix withholds inference rather than guessing a region.
    fn source_applications(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<Vec<TypeReferenceHandle>> {
        match self {
            Self::State { state, .. } => {
                let place = validation::canonical_place_from_expression_in_state(
                    program,
                    state,
                    usize::MAX,
                    expression,
                )?;
                match validation::canonical_place_source_type_application(
                    program,
                    state,
                    usize::MAX,
                    &place,
                ) {
                    Some((_, applications)) => Some(applications),
                    None if place.segments.is_empty() => Some(Vec::new()),
                    None => None,
                }
            }
            Self::Declaration {
                callable,
                parameters,
            } => validation::parameter_source_type_application(
                program,
                callable,
                program.state_parameters.span_or_empty(parameters),
                expression,
            )
            .map(|(_, applications)| applications),
        }
    }

    fn result_type(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        match self {
            Self::State { machine, state } => validation::expression_result_type_reference(
                program,
                program.machine_by_symbol(machine)?,
                program.state_by_symbol(state)?,
                expression,
            ),
            Self::Declaration {
                callable,
                parameters,
            } => validation::parameter_expression_result_type_reference(
                program,
                callable,
                program.state_parameters.span_or_empty(parameters),
                expression,
            ),
        }
    }
}

fn occurrences(
    program: &TypedTrees,
    selected: fn(&ExpressionNode) -> bool,
) -> Vec<(ValueTypeOwner, ExpressionHandle)> {
    let mut occurrences = Vec::new();
    for machine in program.machines() {
        for (state_ordinal, state) in program.machine_states(machine).iter().enumerate() {
            let mut expressions = Vec::new();
            // Machine contracts name entry parameters; a state contract names
            // that state's own parameters. Never replay an entry contract in
            // every state or borrow a caller's destination for its literals.
            let contracts = if state_ordinal == 0 {
                program.machine_contracts(machine)
            } else {
                &[]
            };
            for contract in contracts.iter().chain(program.state_contracts(state)) {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    validation::append_fact_expression_roots(program, fact, &mut expressions);
                }
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                validation::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut expressions,
                );
            }
            let mut visited = Vec::new();
            for expression in expressions {
                // A child may be shared by several parents. Actual postorder,
                // not reversed preorder, ensures it is derived first.
                validation::expression_traversal::visit_post_order(
                    program,
                    expression,
                    &mut visited,
                    |expression, node| {
                        if selected(node) {
                            occurrences.push((
                                ValueTypeOwner::State {
                                    machine: machine.symbol,
                                    state: state.symbol,
                                },
                                expression,
                            ));
                        }
                    },
                );
            }
        }
    }
    let mut declarations = Vec::<(ValueTypeOwner, Vec<ExpressionHandle>)>::new();
    for (callable, _, parameters, contract) in validation::callable_contract_scopes(program) {
        // Entry contracts already use their actual executable state above.
        if program.machine_by_symbol(callable).is_some_and(|machine| {
            program
                .machine_states(machine)
                .first()
                .is_some_and(|state| state.parameters == parameters)
        }) {
            continue;
        }
        let owner = ValueTypeOwner::Declaration {
            callable,
            parameters,
        };
        let position = declarations
            .iter()
            .position(|(candidate, _)| *candidate == owner)
            .unwrap_or_else(|| {
                declarations.push((owner, Vec::new()));
                declarations.len() - 1
            });
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            validation::append_fact_expression_roots(program, fact, &mut declarations[position].1);
        }
    }
    for (owner, roots) in declarations {
        let mut visited = Vec::new();
        for root in roots {
            validation::expression_traversal::visit_post_order(
                program,
                root,
                &mut visited,
                |expression, node| {
                    if selected(node) {
                        occurrences.push((owner, expression));
                    }
                },
            );
        }
    }
    occurrences
}

/// Static inference needs the argument value, not the place stripped of its
/// borrow. Derive each formation in its own owner after substitution; this
/// records type vocabulary only. Loan admission still checks storage, access,
/// lifetime correspondence and parent lineage independently.
fn retain_borrow_result_type(
    program: &mut TypedTrees,
    scope: ValueTypeOwner,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let derived = (|| {
        let formation = validation::reference_formation_source(program, expression)?;
        let target = scope.result_type(program, formation.source)?;
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(target);
        let (referee, lifetime) = match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Reference {
                referee, lifetime, ..
            } => (*referee, lifetime.clone()),
            _ => (target, None),
        };
        Some(TypeReferenceNode::Reference {
            referee: if formation.view_type.is_valid() {
                formation.view_type
            } else {
                referee
            },
            access: formation.access,
            lifetime,
        })
    })();
    derived.map(|node| {
        // Preparation repeats as calls specialize. Reuse exact vocabulary
        // rather than adding another shell on every fixed-point round.
        let retained = program
            .expression_table
            .constructed_value_type_reference(expression);
        let existing = if retained.is_valid()
            && program.type_reference_table.type_reference(retained) == &node
        {
            Some(retained)
        } else {
            program
                .type_reference_table
                .type_reference_rows()
                .find_map(|(reference, candidate)| (candidate == &node).then_some(reference))
        };
        existing.unwrap_or_else(|| program.type_reference_table.insert(node))
    })
}

pub(crate) fn retain_constructed_result_types(
    program: &mut TypedTrees,
) -> Vec<UnavailableValueType> {
    let mut unavailable = Vec::new();
    for (scope, expression) in occurrences(program, |node| {
        matches!(
            node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Borrow(_)
                | ExpressionNode::Call(_)
        ) || matches!(node, ExpressionNode::Cast(cast) if cast.form.is_recast())
    }) {
        let reference = if program.expression_table.call_node(expression).is_some() {
            let result = match scope {
                ValueTypeOwner::State { machine, state } => {
                    call_results::retain(program, machine, state, expression)
                }
                // Logical declarations retain only the source-selected result;
                // they never demand an executable specialization or body.
                ValueTypeOwner::Declaration { .. } => {
                    // Rebinding revisits the occurrence. Its previous result
                    // cannot witness the newly selected application itself.
                    program
                        .expression_table
                        .set_constructed_value_type_reference(
                            expression,
                            TypeReferenceHandle::invalid(),
                        );
                    Ok(scope.result_type(program, expression))
                }
            };
            match result {
                Ok(reference) => reference,
                Err(diagnostics) => {
                    unavailable.push(UnavailableValueType {
                        expression,
                        diagnostics,
                    });
                    None
                }
            }
        } else if validation::reference_formation_source(program, expression).is_some() {
            retain_borrow_result_type(program, scope, expression)
        } else if program
            .expression_table
            .struct_literal_node(expression)
            .is_some()
        {
            retain_constructor_result_type(program, scope, expression)
        } else {
            let derived = (|| {
                let elements = program.expression_table.array_literal_node(expression)?;
                let elements = program.expression_table.expression_handles(*elements);
                let references = elements
                    .iter()
                    .map(|element| scope.result_type(program, *element))
                    .collect::<Vec<_>>();
                let element_type = validation::join_result_type_references(
                    program,
                    &references,
                    &vec![false; references.len()],
                )?;
                Some((element_type, elements.len()))
            })();
            derived.map(|(element_type, length)| {
                let node = TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                };
                let retained = program
                    .expression_table
                    .constructed_value_type_reference(expression);
                if retained.is_valid()
                    && program.type_reference_table.type_reference(retained) == &node
                {
                    retained
                } else {
                    program.type_reference_table.insert(node)
                }
            })
        };
        let Some(reference) = reference else {
            // A private execution graph may be prepared again after call
            // rebinding. Unresolved new children cannot retain the old type.
            program
                .expression_table
                .set_constructed_value_type_reference(
                    expression,
                    typed_trees::types::TypeReferenceHandle::invalid(),
                );
            continue;
        };
        program
            .expression_table
            .set_constructed_value_type_reference(expression, reference);
    }
    unavailable
}

/// A constructor's selected declaration supplies its telescope; completed
/// child types supply the arguments. Incomplete or conflicting tuples stay
/// unresolved. This establishes a type, never a resource claim or field proof.
fn retain_constructor_result_type(
    program: &mut TypedTrees,
    scope: ValueTypeOwner,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let literal = program.expression_table.struct_literal_node(expression)?;
    let owner = program.unique_data_definition_by_symbol(literal.type_symbol)?;
    let parameters = program.data_type_parameters(owner);
    if parameters.is_empty() && owner.lifetime_parameters.is_empty() {
        return program
            .type_reference_table
            .find_named_type_reference(owner.symbol);
    }
    let type_parameters = parameters
        .iter()
        .filter(|parameter| parameter.kind == typed_trees::data::TypeParameterKind::Type)
        .map(|parameter| (parameter.symbol, parameter.name.to_string()))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter_map(|parameter| match parameter.kind {
            typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                Some((parameter.symbol, parameter.name.to_string(), type_reference))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let context = crate::monomorphization::BindingContext::new(
        program,
        &type_parameters,
        &const_parameters,
        None,
        0,
    );
    let mut inferred = crate::monomorphization::StaticProposals::default();
    let mut lifetime_bindings = Vec::new();
    let mut lifetime_conflict = false;
    for field in program.expression_table.struct_fields(literal.fields) {
        // Inference reads an open declaration, not concrete construction
        // obligations. Select by the authored member handle inside this owner.
        let required = program
            .data_members(owner)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(declared)
                    if declared.symbol == field.field_symbol =>
                {
                    Some(declared)
                }
                typed_trees::data::DataMember::Variant(case)
                    if Some(case.symbol) == literal.case_symbol =>
                {
                    program
                        .data_payload_fields(case)
                        .iter()
                        .find(|declared| declared.symbol == field.field_symbol)
                }
                _ => None,
            })?;
        let Some(actual) = scope.result_type(program, field.value) else {
            continue;
        };
        crate::monomorphization::infer_static_bindings(
            context,
            required.type_reference,
            actual,
            &mut inferred,
        );
        // Static type/const proposals above do not need region inference.
        // Empty telescopes and primitive scalar fields make no such demand.
        if owner.lifetime_parameters.is_empty()
            || program
                .primitive_type_reference(required.type_reference)
                .is_some()
        {
            continue;
        }
        let applications = scope.source_applications(program, field.value)?;
        crate::conformance::conformance_application_lifetimes::collect_declared_field_lifetime_bindings(
            program,
            owner,
            required,
            scope.symbol(),
            &applications,
            actual,
            &mut lifetime_bindings,
            &mut lifetime_conflict,
        )?;
    }
    if lifetime_conflict {
        return None;
    }
    let lifetime_arguments = owner
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            lifetime_bindings
                .iter()
                .find(|(name, _)| name == parameter.as_str())
                .map(|(_, argument)| argument.as_str().into())
        })
        .collect::<Option<Vec<_>>>()?;
    let mut arguments = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let proposals = match parameter.kind {
            typed_trees::data::TypeParameterKind::Type => {
                let ordinal = type_parameters
                    .iter()
                    .position(|(symbol, _)| *symbol == parameter.symbol)?;
                inferred
                    .types
                    .iter()
                    .filter(|(_, candidate, _)| *candidate == ordinal)
                    .map(|(_, _, reference)| *reference)
                    .collect::<Vec<_>>()
            }
            typed_trees::data::TypeParameterKind::Const { .. }
            | typed_trees::data::TypeParameterKind::Value { .. } => {
                let ordinal = const_parameters
                    .iter()
                    .position(|(symbol, _, _)| *symbol == parameter.symbol)?;
                inferred
                    .constants
                    .iter()
                    .filter(|(_, candidate, _)| *candidate == ordinal)
                    .map(|(_, _, reference)| *reference)
                    .collect::<Vec<_>>()
            }
            _ => return None,
        };
        let first = *proposals.first()?;
        let identity =
            typed_trees::type_identity::applied_type_identity(program, first, scope.symbol(), &[])?;
        if proposals.iter().any(|reference| {
            typed_trees::type_identity::applied_type_identity(
                program,
                *reference,
                scope.symbol(),
                &[],
            )
            .is_none_or(|candidate| candidate != identity)
        }) {
            return None;
        }
        arguments.push(first);
    }
    let base_symbol = owner.symbol;
    let base_name = owner.name.clone();
    // Static inference revisits this same postorder as calls specialize. Keep
    // an unchanged application rather than growing type vocabulary each round.
    let retained = program
        .expression_table
        .constructed_value_type_reference(expression);
    if retained.is_valid()
        && let TypeReferenceNode::Generic {
            base_symbol: previous_base,
            lifetime_arguments: previous_lifetimes,
            arguments: previous_arguments,
            ..
        } = program.type_reference_table.type_reference(retained)
        && *previous_base == base_symbol
        && *previous_lifetimes == lifetime_arguments
        && program
            .type_reference_table
            .type_reference_handles(*previous_arguments)
            == arguments.as_slice()
    {
        return Some(retained);
    }
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles(arguments);
    Some(
        program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            }),
    )
}
