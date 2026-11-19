//! Bind the policy's argument and returned issuers to the domain's actual typed source.

use crate::compile_time::BuildTimeValue;
use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionNode;
use crate::source_state::references::StaticArgumentCall;
use language_core::core_vocabulary::{CoreVocabulary, ReflectionDeclaration};
use schema_reflection::{CallableDescription, SemanticSchemaGraph};
use source_declarations::DomainEstablishmentRoute;
use symbols::SymbolHandle;

/// The data the core reflection vocabulary names `which`.
pub(super) fn core_reflection_data(
    typed: &TypedTrees,
    which: ReflectionDeclaration,
) -> Option<SymbolHandle> {
    typed
        .data_definitions()
        .iter()
        .find(|data| typed.core_vocabulary(data.symbol) == Some(CoreVocabulary::Reflection(which)))
        .map(|data| data.symbol)
}

/// The `TypeSchema` of the domain's subject, asked for by the domain's
/// author: the same graph `reflect::schema::<Subject>()` returns there.
pub(super) fn subject_schema(
    typed: &TypedTrees,
    definition: &crate::source_state::domain::DomainDefinition,
    schema_type: SymbolHandle,
) -> Result<(SemanticSchemaGraph, BuildTimeValue, Vec<u8>), String> {
    let subject = crate::source_state::reflection::construction::schema_subject_symbol(
        typed,
        definition.target_type,
    )?;
    let authority = crate::source_state::reflection::construction::schema_query_authority(
        typed,
        definition.symbol,
        subject,
    )?;
    let graph =
        crate::source_state::reflection::construction::construct_semantic_schema_graph_for_type(
            typed,
            definition.target_type,
            authority,
        )?;
    let revision =
        crate::source_state::reflection::construction::schema_subject_revision(typed, subject)?;
    // Ordinary result snapshots use the canonical record identity, not its
    // short source name. Construct the input with that same identity so the
    // complete returned value can be compared without discarding its type.
    let schema_identity = typed
        .normalized_nominal_type_identity(schema_type)
        .and_then(|identity| typed.type_identity_text(identity))
        .filter(|identity| !identity.is_empty())
        .ok_or("the core TypeSchema has no canonical record identity")?;
    let schema = BuildTimeValue::Struct {
        type_name: schema_identity,
        fields: vec![(
            "storage".to_owned(),
            BuildTimeValue::Text(graph.runtime_image()),
        )],
    };
    Ok((graph, schema, revision))
}

/// One description a `reflect::callable(..)` query of this program returns,
/// with the route it names.
pub(super) struct Description {
    route: DomainEstablishmentRoute,
    callable: CallableDescription,
}

/// Every callable description the program's checked `reflect::callable`
/// queries produce. A plan's route is accepted only as one of these: the
/// bytes alone grant nothing (`reflect.omg`, `CallableInfo`).
pub(super) fn callable_descriptions(typed: &TypedTrees) -> Result<Vec<Description>, String> {
    let mut descriptions = Vec::new();
    for (expression, node) in typed.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        let query = typed
            .machine_by_symbol(call.target_symbol)
            .or_else(|| typed.machine_holding_state(call.target_symbol));
        if !query.is_some_and(|query| {
            typed.core_vocabulary(query.symbol)
                == Some(CoreVocabulary::Reflection(ReflectionDeclaration::Callable))
        }) {
            continue;
        }
        // Read the query as Psi 05 does (`read/reflection.rs`,
        // `declaration_query`): one declaration path as its only argument,
        // described for the call's lexical author.
        if call.authored_machine_application.is_valid()
            && !typed
                .expression_table
                .static_machine_application_is_valid(call.authored_machine_application)
        {
            continue;
        }
        let [operand] = typed.expression_table.expression_handles(call.arguments) else {
            continue;
        };
        let Ok(selected) = typed.declaration_operand_from_expression(*operand) else {
            continue;
        };
        let owner = StaticArgumentCall::Expression(expression, call).lexical_owner(typed);
        let in_scope = typed
            .symbols
            .symbol_provenance_source_span(owner)
            .is_some_and(|author| {
                selected.symbol.is_valid()
                    && typed
                        .symbols
                        .source_reference_may_select_symbol(author, selected.symbol)
            });
        // A query Psi 05 would refuse produces no description either.
        let Ok((requirement, identities)) =
            crate::source_state::reflection::construction::callable_description_identity(
                typed, &selected, owner,
            )
        else {
            continue;
        };
        if !in_scope {
            continue;
        }
        let route = if requirement {
            let (owner, _) = typed
                .trait_machine_signature(selected.symbol)
                .ok_or("a requirement description lost its trait")?;
            if owner.is_boundary {
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait: owner.symbol,
                    requirement: selected.symbol,
                }
            } else {
                DomainEstablishmentRoute::CheckedRequirement {
                    trait_definition: owner.symbol,
                    requirement: selected.symbol,
                }
            }
        } else {
            let machine = typed
                .machine_by_symbol(selected.symbol)
                .or_else(|| typed.machine_holding_state(selected.symbol))
                .ok_or("a machine description lost its declaration")?;
            DomainEstablishmentRoute::ExactMachine {
                machine: machine.symbol,
            }
        };
        descriptions.push(Description {
            route,
            callable: (requirement, identities),
        });
    }
    Ok(descriptions)
}

/// The declaration a returned `CallableInfo` names, matched on every byte
/// it carries.
pub(super) fn authenticated_route(
    descriptions: &[Description],
    route: &CallableDescription,
) -> Result<DomainEstablishmentRoute, String> {
    let mut routes = descriptions
        .iter()
        .filter(|description| &description.callable == route)
        .map(|description| description.route);
    let route = routes
        .next()
        .ok_or("a plan route names no callable this program describes")?;
    if routes.any(|other| other != route) {
        return Err("a plan route names more than one declaration".into());
    }
    Ok(route)
}
