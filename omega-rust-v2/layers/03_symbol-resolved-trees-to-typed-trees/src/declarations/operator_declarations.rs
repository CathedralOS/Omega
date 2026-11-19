use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::type_system::overload_identity::same_operands;
use crate::{TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, ResolvedStage, SymbolResolvedTrees};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub(crate) struct TypedOperatorBinding<'a> {
    pub declaration: &'a omega_resolver::OperatorDeclaration,
    pub home: Option<DeclarationId>,
}

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
        if let Some(omega_resolver::Symbol::Declaration(home)) = &binding.declared_home
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
        let refusal = |message: &str| TypeError::at(tree.file, node, message);
        let mut home = binding
            .declared_home
            .as_ref()
            .map(|symbol| catalog.types.selected_declaration(symbol))
            .transpose()
            .map_err(refusal)?;
        if home.is_some_and(|owner| owner.kind == NodeKind::Conformance) {
            return Err(refusal("a conformance supplies the trait's token and cannot bind one"));
        }
        let signature = items
            .signatures
            .get(&binding.machine)
            .ok_or_else(|| refusal("operator has no typed signature"))?;
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
                        return Err(refusal("operator declaration has multiple possible semantic homes"));
                    }
                    home = Some(&domain.declaration);
                }
            }
        }
        if home.is_some_and(|home| home.package != binding.machine.package) {
            return Err(refusal("only the declaring package may bind an operator for this home"));
        }
        if home.is_none() {
            let mut nominal_operands = signature
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(position, (_, ty))| nominal_operator_operand(&binding.token, position, ty))
                .peekable();
            if nominal_operands.peek().is_some() && !nominal_operands.any(|operand| operand.package == binding.machine.package) {
                return Err(refusal("only a nominal operand's declaring package may bind this operator"));
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
            let sealed_pair = library.is_some_and(|core| {
                core.collection_primitive(&binding.machine).is_some()
                    && core.collection_primitive(&b.declaration.machine).is_some()
                    && binding.machine != b.declaration.machine
            });
            !sealed_pair
                && b.home.as_ref() == home
                && b.declaration.machine.package == binding.machine.package
                && same_operands(
                    &items.signatures[&b.declaration.machine].parameters,
                    &b.declaration.machine,
                    &signature.parameters,
                    &binding.machine,
                )
        });
        if !valid || duplicate || primitive {
            return Err(refusal(if !valid {
                "incorrect operator arity"
            } else if primitive {
                "cannot bind a compiler-owned primitive operator family"
            } else {
                "duplicate normalized operator binding"
            }));
        }
        family.push(TypedOperatorBinding {
            declaration: binding,
            home: home.cloned(),
        });
    }
    Ok(())
}
