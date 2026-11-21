use crate::declarations::declaration_catalog::{DeclarationCatalog, PendingOperator};
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use crate::type_system::overload_identity::same_operands;
use crate::{OperatorDeclarationError, TypeError, ValueType};
use omega_resolved::{DeclarationId, ResolvedStage, SymbolResolvedTrees};
use omega_syntax::NodeKind;
use omega_typed::site_at;
use std::collections::{HashMap, HashSet};

pub(crate) fn nominal_operator_operand<'a>(
    token: &str,
    position: usize,
    operand: &'a ValueType,
) -> Option<&'a DeclarationId> {
    let carrier = match operand {
        ValueType::Reference(_, carrier) if position == 0 && matches!(token, "[]" | "[..]") => carrier.as_ref(),
        _ => operand,
    };
    carrier.nominal_parts().map(|(declaration, _)| declaration)
}

/// Before signatures exist, only an operator declared on a bound trait or an ancestor
/// can require evidence when forming a primitive integer index.
pub(crate) fn trait_operator_tokens<'a>(
    resolved: &'a ResolvedStage,
    imported: &HashMap<&'a DeclarationId, HashSet<&'a str>>,
) -> HashMap<&'a DeclarationId, HashSet<&'a str>> {
    let mut by_trait = HashMap::new();
    let mut operators_by_home = imported
        .iter()
        .map(|(trait_id, tokens)| ((trait_id.package.as_str(), trait_id.path.as_str()), tokens.iter().copied().collect::<Vec<_>>()))
        .collect::<HashMap<_, _>>();
    for binding in &resolved.operator_declarations {
        if let Some(omega_resolved::Symbol::Declaration(home)) = &binding.declared_home
            && home.kind == NodeKind::Trait
        {
            operators_by_home
                .entry((home.package.as_str(), home.path.as_str()))
                .or_default()
                .push(binding.token.as_str());
        }
    }
    if !operators_by_home.is_empty() {
        for id in resolved
            .declarations
            .iter()
            .filter(|id| id.kind == NodeKind::Trait)
        {
            let lineage = std::iter::once((id.package.as_str(), id.path.as_str())).chain(resolved.trait_ancestors(id));
            let tokens = lineage
                .filter_map(|home| operators_by_home.get(&home))
                .flatten()
                .copied()
                .collect::<HashSet<_>>();
            if !tokens.is_empty() {
                by_trait.insert(id, tokens);
            }
        }
    }
    by_trait
}

pub(crate) fn index_operator_declarations<'a>(
    resolved: &'a SymbolResolvedTrees<'_>,
    catalog: &mut DeclarationCatalog<'a>,
    library: Option<&omega_toolchain::CoreInterface>,
) -> Result<(), TypeError> {
    let items = &mut catalog.declarations;
    for binding in &resolved.operator_declarations {
        let tree = resolved
            .trees()
            .nth(binding.node.file)
            .expect("operator belongs to the resolved forest");
        let node = binding.node.node;
        let site = site_at(tree.file, node);
        let machine = tree.file.nodes[node].parent.expect("operator belongs to a machine");
        let (declared_home, waiting_domains) = match &binding.declared_home {
            None => (None, None),
            Some(symbol) => match catalog.types.selected_machine_home(symbol, tree.file, machine)
                .map_err(|message| TypeError::at(tree.file, node, message))? {
                Some(home) => (Some(home), None),
                None => {
                    let omega_resolved::Symbol::DomainCandidates { candidates, .. } = symbol else {
                        unreachable!("only a domain carrier comparison can wait")
                    };
                    (None, Some(candidates))
                }
            },
        };
        if declared_home.is_some_and(|owner| owner.kind == NodeKind::Conformance) {
            return Err(TypeError::for_reason(&site, OperatorDeclarationError::ConformanceBindsToken));
        }
        let signature = items.signatures.get(&binding.machine);
        if signature.is_none() && !(matches!(binding.machine.kind, NodeKind::Machine | NodeKind::AsmMachine) && catalog.types.indices.declaration_is_pending(&binding.machine)) {
            return Err(TypeError::at(tree.file, node, "operator has no typed signature"));
        }
        let pending_binding = |declared_home| PendingOperator {
            machine: &binding.machine,
            // A waiting generic binder can postpone even the partial parameter list.
            arity: tree.file.parameters(machine).count(),
            declared_home,
            public: binding.public,
        };
        if let Some(domains) = waiting_domains {
            // A qualified binding can participate only through one of its resolved homes.
            // Keep those possibilities private to this pass until L4 answers the width.
            catalog.types.indices.block_node(binding.node);
            let pending = catalog.pending_operators.entry(&binding.token).or_default();
            pending.extend(domains.iter().map(|domain| pending_binding(Some(&domain.declaration))));
            continue;
        }
        let Some(signature) = signature else {
            catalog.pending_operators.entry(&binding.token).or_default().push(pending_binding(declared_home));
            continue;
        };
        let mut home = declared_home;
        if home.is_none() {
            for (_, parameter) in &signature.parameters {
                let domains = parameter.domains();
                for domain in domains {
                    if domains.iter().any(|other| {
                        catalog
                            .types
                            .domains
                            .is_ancestor(&domain.declaration, &other.declaration)
                    }) {
                        continue;
                    }
                    if home.is_some_and(|previous| previous != &domain.declaration) {
                        return Err(TypeError::for_reason(&site, OperatorDeclarationError::MultipleSemanticHomes));
                    }
                    home = Some(&domain.declaration);
                }
            }
        }
        if home.is_some_and(|home| home.package != binding.machine.package) {
            return Err(TypeError::for_reason(&site, OperatorDeclarationError::ForeignHome));
        }
        if home.is_none() {
            let mut nominal_operands = signature
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(position, (_, ty))| nominal_operator_operand(&binding.token, position, ty))
                .peekable();
            if nominal_operands.peek().is_some() && !nominal_operands.any(|operand| operand.package == binding.machine.package) {
                return Err(TypeError::for_reason(&site, OperatorDeclarationError::ForeignNominalOperands));
            }
        }
        let arity = signature.parameters.len();
        let valid = match binding.token.as_str() {
            "!" | "~" => arity == 1,
            "-" => matches!(arity, 1 | 2),
            "[..]" => arity >= 2,
            _ => arity == 2,
        };
        let family = items.token_bindings.entry(binding.token.as_str()).or_default();
        let primitive = home.is_none()
            && signature
                .parameters
                .iter()
                .all(|(_, t)| t.numeric() || t == &ValueType::Bool);
        let duplicate = family.iter().any(|b| {
            // Sealed bracket primitives also dispatch on whether the caller requests a loan.
            let sealed_pair = library
                .is_some_and(|core| core.collection_primitive(&binding.machine).is_some() && core.collection_primitive(b).is_some() && binding.machine != **b);
            !sealed_pair
                && items.signatures[*b]
                    .operator
                    .as_ref()
                    .and_then(|op| op.home.as_ref())
                    == home
                && b.package == binding.machine.package
                && same_operands(&items.signatures[*b], signature)
        });
        if !valid {
            return Err(TypeError::for_reason(&site, OperatorDeclarationError::IncorrectArity));
        }
        if primitive {
            return Err(TypeError::for_reason(&site, OperatorDeclarationError::PrimitiveFamily));
        }
        if duplicate {
            return Err(TypeError::for_reason(&site, OperatorDeclarationError::DuplicateBinding));
        }
        let home = home.cloned();
        let signature = items
            .signatures
            .get_mut(&binding.machine)
            .expect("the operator signature was checked above");
        std::sync::Arc::make_mut(signature).operator = Some(crate::TypedOperatorDeclaration {
            node: binding.node,
            site,
            token: binding.token.clone(),
            home,
            public: binding.public,
        });
        family.push(&binding.machine);
    }
    Ok(())
}
