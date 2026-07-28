use omega_core::semantics::{DomainEstablishmentRoute, DomainPredicateBody};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionNode;

/// Bind semantic `as ... in Domain` sites to declaration identity and select
/// the unique canonical home route, if one exists. Validation owns the
/// diagnostic policy; this pass only publishes deterministic identities so
/// no checked consumer re-resolves a user spelling.
pub(crate) fn normalize_qualification_casts(program: &mut TypedTrees) {
    let updates = program
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            let [name] = program
                .expression_table
                .name_path_members(cast.semantic_domain)
            else {
                return None;
            };
            let matches = program
                .domain_definitions()
                .iter()
                .filter(|domain| {
                    same_semantic_name(domain.name.as_str(), name.as_str())
                        && program.normalized_type_identity(domain.target_type)
                            == program.normalized_type_identity(cast.target_type)
                })
                .collect::<Vec<_>>();
            let [domain] = matches.as_slice() else {
                return Some((handle, SymbolHandle::invalid(), SymbolHandle::invalid()));
            };

            let atoms = atomic_domain_symbols(program, domain.symbol);
            let satisfier = if let [atom] = atoms.as_slice() {
                let candidates = program
                    .domain_definitions()
                    .iter()
                    .find(|candidate| candidate.symbol == *atom)
                    .filter(|candidate| candidate.predicate_body == DomainPredicateBody::Bodyless)
                    .map(|candidate| {
                        candidate
                            .establishment_routes
                            .iter()
                            .filter_map(|route| match route {
                                DomainEstablishmentRoute::CanonicalQualification { satisfier } => {
                                    Some(*satisfier)
                                }
                                _ => None,
                            })
                            .fold(Vec::new(), |mut unique, symbol| {
                                if !unique.contains(&symbol) {
                                    unique.push(symbol);
                                }
                                unique
                            })
                    })
                    .unwrap_or_default();
                candidates
                    .as_slice()
                    .first()
                    .copied()
                    .filter(|_| candidates.len() == 1)
                    .unwrap_or_else(SymbolHandle::invalid)
            } else {
                SymbolHandle::invalid()
            };
            Some((handle, domain.symbol, satisfier))
        })
        .collect::<Vec<_>>();

    for (handle, domain, satisfier) in updates {
        let ExpressionNode::Cast(cast) = program.expression_table.expression_mut(handle) else {
            continue;
        };
        cast.semantic_domain_symbol = domain;
        cast.qualification_satisfier = satisfier;
    }
}

fn atomic_domain_symbols(program: &TypedTrees, domain: SymbolHandle) -> Vec<SymbolHandle> {
    fn expand(
        program: &TypedTrees,
        domain: SymbolHandle,
        stack: &mut Vec<SymbolHandle>,
        output: &mut Vec<SymbolHandle>,
    ) {
        if !domain.is_valid() || stack.contains(&domain) {
            return;
        }
        let Some(definition) = program
            .domain_definitions()
            .iter()
            .find(|candidate| candidate.symbol == domain)
        else {
            return;
        };
        let Some(alias) = definition.alias.as_ref() else {
            if !output.contains(&domain) {
                output.push(domain);
            }
            return;
        };
        stack.push(domain);
        for constituent in &alias.constituents {
            expand(program, constituent.domain_symbol, stack, output);
        }
        stack.pop();
    }

    let mut output = Vec::new();
    expand(program, domain, &mut Vec::new(), &mut output);
    output
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}
