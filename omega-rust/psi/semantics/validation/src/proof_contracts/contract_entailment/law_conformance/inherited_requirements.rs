//! Inherited requirement proposition applications and satisfier
//! parameters.

use crate::proof_contracts::contract_entailment::law_conformance::proposition_laws::synthesize_indexed_law_binder_labels;
use crate::proof_contracts::contract_entailment::{StateSignature, TraitDefinition, TypedTrees};

/// The proposition endpoint one requirement contract application carries once
/// it migrates onto a satisfying machine state through an exact conformance
/// edge: the authored application with any proposition-family parameter
/// replaced by the edge's selected family, plus the binder labels the
/// satisfier's carrier telescope pins down.
pub struct InheritedRequirementApplication {
    pub application: typed_trees::proposition::PropositionApplication,
    pub binder_labels: Vec<String>,
}

/// Instantiate one requirement proposition application through the exact
/// conformance edge (`trait_arguments` positionally bound to
/// `trait_type_parameters(trait_definition)`) and the satisfying state.
/// `None` means the edge is incomplete -- a proposition parameter without a
/// matching trait argument, a non-proposition endpoint, or an indexed family
/// whose binder telescope the representative parameters cannot synthesize --
/// so the migrated row keeps its authored schema identity rather than
/// publishing a partial substitution.
pub fn inherited_requirement_proposition_application(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    satisfier_state: &typed_trees::state::State,
    trait_arguments: &[typed_trees::types::TypeReferenceHandle],
    application: &typed_trees::proposition::PropositionApplication,
) -> Option<InheritedRequirementApplication> {
    let mut instantiated = application.clone();
    if program.symbols.get(application.proposition).kind
        == symbols::SymbolKind::PropositionParameter
    {
        let parameter_index = program
            .trait_type_parameters(trait_definition)
            .iter()
            .position(|parameter| parameter.symbol == application.proposition)?;
        let argument = trait_arguments.get(parameter_index)?;
        let typed_trees::types::TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(*argument)
        else {
            return None;
        };
        instantiated.proposition = *symbol;
        instantiated.name = name.clone();
    }
    let binder_labels = if instantiated.binder_arguments.is_empty() {
        synthesize_indexed_law_binder_labels(
            program,
            application,
            &instantiated,
            requirement,
            satisfier_state,
        )?
    } else {
        instantiated
            .binder_arguments
            .iter()
            .map(|argument| argument.display_name())
            .collect::<Vec<_>>()
    };
    Some(InheritedRequirementApplication {
        application: instantiated,
        binder_labels,
    })
}

/// The exact semantic label one requirement proposition application carries on
/// the satisfying state: the selected proposition family with its binder
/// telescope, plus the requirement's argument expressions renamed onto the
/// satisfier's own parameters positionally (requirement parameter order is the
/// contract, so renames are by position, never by display-name coincidence).
/// `None` fails closed -- the migrated fact keeps its authored schema identity
/// rather than publishing a partial substitution.
pub fn inherited_requirement_proposition_label(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    satisfier_state: &typed_trees::state::State,
    trait_arguments: &[typed_trees::types::TypeReferenceHandle],
    application: &typed_trees::proposition::PropositionApplication,
) -> Option<String> {
    let endpoint = inherited_requirement_proposition_application(
        program,
        trait_definition,
        requirement,
        satisfier_state,
        trait_arguments,
        application,
    )?;
    let satisfier_parameters =
        inherited_satisfier_parameters(program, trait_definition, requirement, satisfier_state)?;
    let substitutions = program
        .state_signature_parameters(requirement)
        .iter()
        .zip(satisfier_parameters)
        .map(|(required, actual)| {
            (
                required.symbol,
                required.name.as_str().to_owned(),
                actual.name.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let argument_labels = program
        .expression_table
        .expression_handles(endpoint.application.arguments)
        .iter()
        .map(|argument| program.render_proof_expression_with_parameters(*argument, &substitutions))
        .collect::<Vec<_>>();
    program
        .normalize_proposition_application_with_labels(
            &endpoint.application,
            &endpoint.binder_labels,
            &argument_labels,
        )
        .map(|formula| formula.identity_label())
}

/// The satisfying state's parameters aligned with the requirement's own
/// parameter telescope. Mirrors the signature check's one exception: a
/// boundary satisfier may carry one extra LEADING trait-typed parameter that
/// the adapter forwards to. `None` on arity drift keeps the migrated fact at
/// its authored schema identity.
pub fn inherited_satisfier_parameters<'a>(
    program: &'a TypedTrees,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    satisfier_state: &'a typed_trees::state::State,
) -> Option<&'a [typed_trees::signature::StateParameter]> {
    let required = program.state_signature_parameters(requirement);
    let mut actual = program.state_parameters(satisfier_state);
    if trait_definition.is_boundary
        && actual.len() == required.len() + 1
        && actual.first().is_some_and(|parameter| {
            let label = crate::value_custody::type_references::type_reference_label(
                program,
                parameter.type_reference,
            );
            let leaf = label.rsplit("::").next().unwrap_or(label.as_str());
            let trait_leaf = trait_definition
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(trait_definition.name.as_str());
            leaf == trait_leaf
        })
    {
        actual = &actual[1..];
    }
    (actual.len() == required.len()).then_some(actual)
}
