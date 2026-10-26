//! Close explicit domain index telescopes without mutating source declarations.
//! Only retained type/fact occurrences request views. Open arguments keep their
//! authored theory until ordinary substitution supplies the complete tuple.

use super::{Diagnostic, SymbolHandle, TypedTrees};
use typed_trees::data::TypeParameterKind;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn materialize(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    loop {
        let mut requests = Vec::new();
        for occurrence in typed_trees::references::collect_program_type_occurrences(program) {
            if let TypeReferenceNode::Constrained { constraints, .. } = program
                .type_reference_table
                .type_reference(occurrence.reference)
            {
                requests.extend(
                    program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .filter_map(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain) => Some((
                                occurrence.lexical_owner,
                                domain.symbol,
                                domain.arguments.clone(),
                            )),
                            _ => None,
                        }),
                );
            }
        }
        for (owner, _, fact) in typed_trees::references::program_scoped_proof_facts(program) {
            if let Some(membership) = fact.as_membership() {
                requests.push((
                    owner,
                    membership.domain_symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments)
                        .to_vec(),
                ));
            }
        }
        let mut changed = false;
        for (lexical_owner, symbol, arguments) in requests {
            let Some(domain) = program.domain_definition(symbol).cloned() else {
                continue;
            };
            if domain.type_parameters.is_empty()
                || typed_trees::domain::has_generic_carrier(program, &domain)
            {
                continue;
            }
            let parameters = program.domain_type_parameters(&domain).to_vec();
            if arguments.len() != parameters.len() {
                return Err(vec![Diagnostic::error(
                    "domain application has an incomplete explicit telescope",
                )]);
            }
            if arguments
                .iter()
                .any(|argument| !typed_trees::domain::index_argument_is_closed(program, *argument))
            {
                continue;
            }
            let mut canonical = true;
            let mut closed_arguments = arguments.clone();
            for (index, (parameter, argument)) in parameters.iter().zip(&arguments).enumerate() {
                if let TypeParameterKind::Const {
                    type_reference: carrier,
                } = parameter.kind
                {
                    match program.type_reference_table.type_reference(*argument) {
                        TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid() => {}
                        TypeReferenceNode::ConstExpression(expression)
                            if lexical_owner.is_valid()
                                && typed_trees::domain::index_argument_is_closed(
                                    program, carrier,
                                ) =>
                        {
                            // Keep the source recipe and its exact instance ID.
                            // Checking a completed value grants no permission to
                            // erase the supplying operation or borrow the receiving
                            // domain's assumptions. Dependent carriers defer until
                            // their supplying telescope has actually closed.
                            // Successful normalization currently excludes calls
                            // and lexical implementation-dependent operations.
                            // If that cohort expands, derived theory traversal
                            // must also retain each recipe's supplying scope.
                            let Some(value) = validation::check_closed_const_expression(
                                program,
                                lexical_owner,
                                *expression,
                                carrier,
                            )?
                            else {
                                canonical = false;
                                break;
                            };
                            let spelling = match value.decode_encoding() {
                                Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer { value, .. }) => value.to_string(),
                                Some(_) => value.atom(),
                                None => return Err(vec![Diagnostic::error("closed domain argument lost its canonical value")]),
                            };
                            // Only the derived theory consumes this checked
                            // value. The application retains the source recipe
                            // and its instance identity independently.
                            closed_arguments[index] =
                                program
                                    .type_reference_table
                                    .insert(TypeReferenceNode::Named {
                                        symbol: SymbolHandle::invalid(),
                                        name: typed_trees::name::Identifier::generated(spelling),
                                    });
                        }
                        _ => {
                            canonical = false;
                            break;
                        }
                    }
                }
            }
            if !canonical {
                continue;
            }
            let identity = typed_trees::domain::indexed_domain_instance_name(
                program,
                &domain,
                &parameters,
                &arguments,
            )
            .map_err(|error| vec![error])?;
            let identity = program.semantic_domains.intern(&identity);
            changed |= program
                .prepare_closed_domain_application(symbol, &arguments, &closed_arguments, identity)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        }
        if !changed {
            return Ok(());
        }
        // Facts copied by the shared application operation retain their exact
        // argument packs; publish their instance identities by the ordinary
        // reference-driven normalizer before walking inherited requests.
        super::finalize_domain_instance_identities(
            program,
            &typed_trees::references::DeclarationSelection::all(),
        )
        .map_err(|error| vec![error])?;
    }
}
