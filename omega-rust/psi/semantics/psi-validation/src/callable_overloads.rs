use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::type_identity::NormalizedNamedCallableIdentity;

/// Declaration-side half of named result-domain overload resolution.
///
/// Predicate-only result refinements collapse in the shared identity, so they
/// diagnose as duplicates. Distinct dispatch-bearing sets remain distinct and
/// are selected later from the call site's expected result type. Fixed
/// operators deliberately use their separate operand-directed validator.
pub(crate) fn validate_named_callable_overload_declarations(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_machine_overloads(program, diagnostics);
    validate_trait_requirement_overloads(program, diagnostics);
}

fn validate_machine_overloads(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen: Vec<(NormalizedNamedCallableIdentity, psi_symbols::SymbolHandle)> = Vec::new();
    for machine in program.machines() {
        let Some(identity) = program.normalized_machine_overload_identity(machine) else {
            continue;
        };
        if seen.iter().any(|(previous_identity, previous_symbol)| {
            previous_identity == &identity
                && !program
                    .symbols
                    .source_scopes_separate(*previous_symbol, machine.symbol)
        }) {
            diagnostics.push(duplicate_diagnostic("machine", &identity));
        } else {
            seen.push((identity, machine.symbol));
        }
    }
}

fn validate_trait_requirement_overloads(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for trait_definition in program.traits() {
        let mut seen: Vec<(NormalizedNamedCallableIdentity, psi_symbols::SymbolHandle)> =
            Vec::new();
        for requirement in program.trait_machine_signatures(trait_definition) {
            let identity = program
                .normalized_trait_requirement_overload_identity(trait_definition, requirement);
            if seen.iter().any(|(previous_identity, previous_symbol)| {
                previous_identity == &identity
                    && !program
                        .symbols
                        .source_scopes_separate(*previous_symbol, requirement.symbol)
            }) {
                diagnostics.push(duplicate_diagnostic("requirement", &identity));
            } else {
                seen.push((identity, requirement.symbol));
            }
        }
    }
}

fn duplicate_diagnostic(kind: &str, identity: &NormalizedNamedCallableIdentity) -> Diagnostic {
    let dispatch = if identity.result_dispatch().is_empty() {
        "<empty>".to_owned()
    } else {
        identity.result_dispatch().identity()
    };
    Diagnostic::error(format!(
        "duplicate named {kind} overload `{}` with parameter signature `{}` and result dispatch set `{dispatch}`; predicate-only result refinements do not distinguish overloads",
        identity.path(),
        identity.parameters(),
    ))
}

#[cfg(test)]
mod tests {
    use super::validate_named_callable_overload_declarations;
    use psi_language_semantics::{DomainPredicateBody, DomainSemanticRoles};
    use psi_numerics::arithmetic::ArithmeticDomain;
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::state::State;
    use psi_typed_trees::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};

    fn machine(
        program: &mut TypedTrees,
        machine_symbol: u32,
        state_symbol: u32,
        result: psi_typed_trees::types::TypeReferenceHandle,
    ) -> Machine {
        let state = State {
            symbol: SymbolHandle::from_arena_index(state_symbol),
            name: Identifier::generated("convert"),
            return_type: result,
            ..State::default()
        };
        let mut machine = Machine {
            symbol: SymbolHandle::from_arena_index(machine_symbol),
            name: Identifier::generated("I32::convert"),
            ..Machine::default()
        };
        program.push_machine_state(&mut machine, state);
        machine
    }

    #[test]
    fn predicate_only_result_difference_is_a_duplicate_but_policy_difference_is_not() {
        let mut program = TypedTrees::default();
        let i32_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let positive = program.semantic_domains.intern("Positive");
        let predicate_constraints =
            program
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                    arguments: Vec::new(),
                    name: Identifier::generated("Positive"),
                    subject: Default::default(),
                    symbol: SymbolHandle::invalid(),
                    semantic_id: positive,
                    classification: None,
                    predicate_body: DomainPredicateBody::Present,
                    semantic_roles: DomainSemanticRoles::default(),
                    establishment_routes: Vec::new(),
                    authored_selection: None,
                })]);
        let predicate_result =
            program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: i32_type,
                    constraints: predicate_constraints,
                });
        let saturating_constraints = program.type_reference_table.insert_constraints([
            TypeConstraintNode::ArithmeticDomain(ArithmeticDomain::Saturating),
        ]);
        let saturating_result =
            program
                .type_reference_table
                .insert(TypeReferenceNode::Constrained {
                    base_type: i32_type,
                    constraints: saturating_constraints,
                });
        let unqualified = machine(&mut program, 10, 11, i32_type);
        let predicate = machine(&mut program, 20, 21, predicate_result);
        let saturating = machine(&mut program, 30, 31, saturating_result);
        program.push_machine(unqualified);
        program.push_machine(predicate);
        program.push_machine(saturating);

        let mut diagnostics = Vec::new();
        validate_named_callable_overload_declarations(&program, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("result dispatch set `<empty>`")
        );
        assert!(
            diagnostics[0]
                .message
                .contains("predicate-only result refinements do not distinguish")
        );
    }
}
