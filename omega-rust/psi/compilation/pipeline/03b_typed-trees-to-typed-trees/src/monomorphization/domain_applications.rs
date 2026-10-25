//! Close explicit domain index telescopes without mutating source declarations.
//! Only retained type/fact occurrences request views. Open arguments keep their
//! authored theory until ordinary substitution supplies the complete tuple.

use super::{Diagnostic, SymbolHandle, TypedTrees};
use typed_trees::data::TypeParameterKind;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn materialize(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    loop {
        let references = typed_trees::references::collect_program_references(program);
        let mut requests = Vec::new();
        for reference in references.type_references {
            if let TypeReferenceNode::Constrained { constraints, .. } =
                program.type_reference_table.type_reference(reference)
            {
                requests.extend(
                    program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .filter_map(|constraint| match constraint {
                            TypeConstraintNode::Domain(domain) => {
                                Some((domain.symbol, domain.arguments.clone()))
                            }
                            _ => None,
                        }),
                );
            }
        }
        for handle in references.proof_facts {
            if let Some(membership) = program.proof_facts.get(handle).as_membership() {
                requests.push((
                    membership.domain_symbol,
                    program
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments)
                        .to_vec(),
                ));
            }
        }
        let mut changed = false;
        for (symbol, arguments) in requests {
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
                if matches!(parameter.kind, TypeParameterKind::Const { .. }) {
                    if !matches!(program.type_reference_table.type_reference(*argument),
                        TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid())
                    {
                        canonical = false;
                        break;
                    }
                    validation::validate_closed_const_argument(
                        program,
                        domain.name.as_str(),
                        parameter,
                        *argument,
                    )?;
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
