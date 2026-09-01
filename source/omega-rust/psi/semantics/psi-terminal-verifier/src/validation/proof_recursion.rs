//! Canonical source-free proof-recursion component validation.

use super::*;

pub(super) fn validate_proof_recursive_components(
    module: &TerminalModule,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    if module
        .proof_recursive_components
        .windows(2)
        .any(|components| components[0] >= components[1])
    {
        return Err(ModuleError::NonCanonicalProofRecursiveComponents);
    }

    for component in &module.proof_recursive_components {
        if component.rank_type_identity.is_empty()
            || component.types.is_empty()
            || component.members.len() < 2
            || component.edges.is_empty()
            || component
                .members
                .windows(2)
                .any(|members| members[0].contract >= members[1].contract)
            || component.edges.windows(2).any(|edges| edges[0] >= edges[1])
        {
            return Err(ModuleError::InvalidProofRecursiveComponent);
        }

        let mut types = BTreeMap::new();
        for proof_type in &component.types {
            if proof_type.identity.is_empty()
                || proof_type
                    .fields
                    .windows(2)
                    .any(|fields| fields[0] >= fields[1])
                || types
                    .insert(proof_type.identity.as_str(), proof_type)
                    .is_some()
            {
                return Err(ModuleError::InvalidProofRecursiveComponent);
            }
            let mut field_identities = BTreeSet::new();
            for field in &proof_type.fields {
                if field.identity.is_empty()
                    || field.type_identity.is_empty()
                    || !field_identities.insert(field.identity.as_str())
                {
                    return Err(ModuleError::InvalidProofRecursiveComponent);
                }
            }
        }
        if component.types.windows(2).any(|types| types[0] >= types[1])
            || !types.contains_key(component.rank_type_identity.as_str())
        {
            return Err(ModuleError::InvalidProofRecursiveComponent);
        }

        insert_unique(
            &mut registry.recursive_components,
            crate::proof_recursive_component_identity(component),
            |_| ModuleError::InvalidProofRecursiveComponent,
        )?;
        for obligation in crate::proof_recursion::proof_recursive_obligation_ids(component) {
            insert_unique(
                &mut registry.obligations,
                obligation,
                ModuleError::DuplicateObligation,
            )?;
        }

        let mut members = BTreeSet::new();
        for member in &component.members {
            if member.machine_identity.is_empty()
                || member.rank_parameter_identity.is_empty()
                || !members.insert(member.contract)
            {
                return Err(ModuleError::InvalidProofRecursiveMember(member.contract));
            }
            insert_unique(
                &mut registry.contracts,
                member.contract,
                ModuleError::DuplicateContract,
            )?;
        }

        let mut exact_sites = BTreeSet::new();
        let mut outgoing = component
            .members
            .iter()
            .map(|member| (member.contract, Vec::new()))
            .collect::<BTreeMap<_, _>>();
        for edge in &component.edges {
            let site_identity = match &edge.site {
                psi_terminal::TerminalProofRecursiveCallSite::Statement {
                    state_identity, ..
                }
                | psi_terminal::TerminalProofRecursiveCallSite::Expression {
                    state_identity, ..
                }
                | psi_terminal::TerminalProofRecursiveCallSite::Transition {
                    state_identity, ..
                } => state_identity,
            };
            if !members.contains(&edge.caller)
                || !members.contains(&edge.callee)
                || site_identity.is_empty()
                || edge.strict_member_path.is_empty()
                || edge.strict_member_path.iter().any(String::is_empty)
                || !exact_sites.insert((edge.caller, edge.site.clone()))
            {
                return Err(ModuleError::InvalidProofRecursiveEdge {
                    caller: edge.caller,
                    callee: edge.callee,
                });
            }
            let mut current_type = component.rank_type_identity.as_str();
            for field_identity in &edge.strict_member_path {
                let Some(proof_type) = types.get(current_type) else {
                    return Err(ModuleError::InvalidProofRecursiveEdge {
                        caller: edge.caller,
                        callee: edge.callee,
                    });
                };
                let Some(field) = proof_type
                    .fields
                    .iter()
                    .find(|field| field.identity == *field_identity)
                else {
                    return Err(ModuleError::InvalidProofRecursiveEdge {
                        caller: edge.caller,
                        callee: edge.callee,
                    });
                };
                current_type = field.type_identity.as_str();
            }
            if current_type != component.rank_type_identity {
                return Err(ModuleError::InvalidProofRecursiveEdge {
                    caller: edge.caller,
                    callee: edge.callee,
                });
            }
            outgoing
                .get_mut(&edge.caller)
                .expect("validated recursive caller is a component member")
                .push(edge.callee);
        }

        for start in &members {
            let mut reached = BTreeSet::new();
            let mut pending = vec![*start];
            while let Some(member) = pending.pop() {
                for next in &outgoing[&member] {
                    if reached.insert(*next) {
                        pending.push(*next);
                    }
                }
            }
            if !members.iter().all(|member| reached.contains(member)) {
                return Err(ModuleError::InvalidProofRecursiveComponent);
            }
        }
    }
    Ok(())
}
