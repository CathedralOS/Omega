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
                .any(|argument| !is_closed(program, *argument))
            {
                continue;
            }
            let mut canonical = true;
            for (parameter, argument) in parameters.iter().zip(&arguments) {
                if let TypeParameterKind::Const {
                    type_reference: carrier,
                } = parameter.kind
                {
                    match program.type_reference_table.type_reference(*argument) {
                        TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid() => {
                            validation::validate_closed_const_argument(
                                program,
                                domain.name.as_str(),
                                parameter,
                                *argument,
                            )?;
                        }
                        TypeReferenceNode::ConstExpression(expression)
                            if lexical_owner.is_valid() && is_closed(program, carrier) =>
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
                            if validation::check_closed_const_expression(
                                program,
                                lexical_owner,
                                *expression,
                                carrier,
                            )?
                            .is_none()
                            {
                                canonical = false;
                                break;
                            }
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
                .prepare_closed_domain_application(symbol, &arguments, identity)
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

fn is_closed(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    struct Closure<'a> {
        program: &'a TypedTrees,
        types: std::collections::HashSet<TypeReferenceHandle>,
        expressions: std::collections::HashSet<typed_trees::expression::ExpressionHandle>,
        open: bool,
    }
    impl Extend<SymbolHandle> for Closure<'_> {
        fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
            self.open |= symbols
                .into_iter()
                .filter(|symbol| symbol.is_valid())
                .any(|symbol| {
                    matches!(
                        self.program.symbols.get(symbol).kind,
                        symbols::SymbolKind::TypeParameter
                            | symbols::SymbolKind::MachineParameter
                            | symbols::SymbolKind::ConformanceParameter
                            | symbols::SymbolKind::PropositionParameter
                            | symbols::SymbolKind::Parameter
                            | symbols::SymbolKind::Local
                    )
                });
        }
    }
    impl typed_trees::references::TypedReferences for Closure<'_> {
        fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
            self.types.insert(reference)
        }
        fn expression(&mut self, expression: typed_trees::expression::ExpressionHandle) -> bool {
            self.expressions.insert(expression)
        }
    }
    let mut closure = Closure {
        program,
        types: Default::default(),
        expressions: Default::default(),
        open: false,
    };
    typed_trees::references::visit_type_reference(program, &mut closure, reference);
    !closure.open
}
