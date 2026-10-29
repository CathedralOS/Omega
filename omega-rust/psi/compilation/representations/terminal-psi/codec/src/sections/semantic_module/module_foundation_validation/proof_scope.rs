//! Structural ingress checks for retained proof scope. These do not establish
//! any proof condition or authorize a declaration's execution.

use crate::codec_error::{CodecError, malformed};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    ProofApplicationScope, ProofDeclarationId, ProofScopeDeclaration, TerminalIdentityIndex,
    TerminalIdentityTable,
};

pub(super) fn validate(
    scope: &ProofApplicationScope,
    identities: &TerminalIdentityTable,
) -> Result<(), CodecError> {
    let declaration = |identity: ProofDeclarationId| {
        if scope.declaration(identity).is_some() {
            Ok(())
        } else {
            malformed("proof scope references an absent declaration")
        }
    };
    let text = |identity: TerminalIdentityIndex| {
        if identities
            .text(identity)
            .is_some_and(|text| !text.is_empty())
        {
            Ok(())
        } else {
            malformed("proof scope declaration lost its portable identity")
        }
    };
    for (_, row) in scope.declarations.iter() {
        match row {
            ProofScopeDeclaration::Missing => {
                return malformed("proof scope declaration is absent");
            }
            ProofScopeDeclaration::Unavailable(reason) => {
                if reason.is_empty() {
                    return malformed("unavailable proof declaration has no reason");
                }
            }
            ProofScopeDeclaration::Nominal(identity) => text(*identity)?,
            ProofScopeDeclaration::Callable(row) => {
                for identity in [
                    row.machine.identity(),
                    row.state.identity(),
                    row.semantic_declaration,
                    row.callable,
                    row.result_type,
                ] {
                    text(identity)?;
                }
                for identity in &row.parameter_types {
                    text(*identity)?;
                }
                if let Some(specialization) = &row.specialization {
                    for identity in [
                        specialization.template.identity(),
                        specialization.template_callable,
                        specialization.template_semantic_declaration,
                    ] {
                        text(identity)?;
                    }
                    if let Some(capture) = &specialization.capture {
                        text(capture.template_identity)?;
                    }
                }
            }
        }
    }
    let plan = &scope.plan;
    for (_, identity) in plan.symbols.iter() {
        declaration(*identity)?;
    }
    let mut machines = BTreeSet::new();
    let mut state_owners: BTreeMap<_, _> = plan
        .machines
        .iter()
        .map(|(_, row)| (row.machine, row.machine))
        .collect();
    for (_, row) in plan.machines.iter() {
        declaration(row.machine)?;
        if !machines.insert(row.machine) {
            return malformed("proof scope repeats a machine");
        }
        let mut local_states = BTreeSet::new();
        for state in
            plan.symbols
                .span(row.states)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "proof scope lost its state span",
                ))?
        {
            if !local_states.insert(*state)
                || state_owners
                    .insert(*state, row.machine)
                    .is_some_and(|owner| owner != row.machine)
            {
                return malformed("proof scope state has ambiguous ownership");
            }
        }
        let binders =
            plan.symbols
                .span(row.binders)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "proof scope lost its binder span",
                ))?;
        if binders.iter().collect::<BTreeSet<_>>().len() != binders.len() {
            return malformed("proof scope repeats a static binder");
        }
        for target in [row.authored_target.as_deref(), row.source_target.as_deref()]
            .into_iter()
            .flatten()
        {
            if target.is_empty() {
                return malformed("proof scope has an empty target");
            }
        }
        let mut targets = BTreeSet::new();
        for member in plan.target_members.span_or_empty(row.family_members) {
            declaration(member.machine)?;
            let Some(body) = plan
                .machines
                .iter()
                .map(|(_, candidate)| candidate)
                .find(|candidate| candidate.machine == member.machine)
            else {
                return malformed("proof scope family member has no machine");
            };
            if body.authored_target.as_deref() != Some(member.target.as_str())
                || body.family_interface
                || plan.symbols.span_or_empty(body.binders).len()
                    != plan.symbols.span_or_empty(row.binders).len()
            {
                return malformed(
                    "proof scope family member has a different target or binder roster",
                );
            }
            if member.target.is_empty() || !targets.insert(&member.target) {
                return malformed("proof scope repeats or omits a family target");
            }
        }
    }
    for (_, row) in plan.declaration_dependencies.iter() {
        declaration(row.declaration)?;
    }
    for (_, (requirement, realization)) in plan.provider_candidates.iter() {
        declaration(*requirement)?;
        declaration(*realization)?;
    }
    let graph = &scope.calls;
    let mut summaries = BTreeSet::new();
    for (_, row) in graph.machines.iter() {
        if !machines.contains(&row.machine) || !summaries.insert(row.machine) {
            return malformed("proof scope has an unknown or repeated call summary");
        }
    }
    for (_, call) in graph.calls.iter() {
        for identity in [call.binder, call.callee, call.state, call.operator] {
            if identity.0 != 0 {
                declaration(identity)?;
            }
        }
    }
    for arguments in graph
        .calls
        .iter()
        .map(|(_, call)| call.arguments)
        .chain(graph.bindings.iter().map(|(_, binding)| binding.arguments))
    {
        let mut parameters = BTreeSet::new();
        for binding in
            graph
                .bindings
                .span(arguments)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "proof scope lost its binding span",
                ))?
        {
            if !parameters.insert(binding.parameter) {
                return malformed("proof scope static application repeats a parameter");
            }
        }
    }
    for (_, binding) in graph.bindings.iter() {
        declaration(binding.parameter)?;
        declaration(binding.selected)?;
    }
    for (_, cleanup) in graph.cleanup.iter() {
        declaration(*cleanup)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::HandleSpan;
    use semantic_vocabulary::proof_applications::{
        ProofApplicationCall, ProofApplicationMachine, ProofApplicationTargetMember,
        ProofStaticBinding,
    };
    use terminal_psi::TerminalIdentityTableBuilder;

    fn scope() -> (ProofApplicationScope, TerminalIdentityTable) {
        let mut identities = TerminalIdentityTableBuilder::default();
        let identity = identities.intern("same nominal name");
        let mut scope = ProofApplicationScope::default();
        for _ in 0..4 {
            scope
                .declarations
                .insert(ProofScopeDeclaration::Nominal(identity));
        }
        let members = scope
            .plan
            .target_members
            .insert_many([ProofApplicationTargetMember {
                target: "macos_arm64".into(),
                machine: ProofDeclarationId(2),
            }]);
        scope.plan.machines.insert(ProofApplicationMachine {
            machine: ProofDeclarationId(1),
            family_interface: true,
            family_members: members,
            product_selected: true,
            ..Default::default()
        });
        scope.plan.machines.insert(ProofApplicationMachine {
            machine: ProofDeclarationId(2),
            authored_target: Some("macos_arm64".into()),
            product_selected: true,
            ..Default::default()
        });
        (scope, identities.build())
    }

    #[test]
    fn proof_scope_rejects_missing_wrong_target_and_mismatched_family_members() {
        let (scope, identities) = scope();
        validate(&scope, &identities).unwrap();
        let mut missing = scope.clone();
        missing
            .plan
            .target_members
            .for_each_mut(|_, member| member.machine = ProofDeclarationId(3));
        assert!(matches!(
            validate(&missing, &identities),
            Err(CodecError::MalformedStructuralFoundation(
                "proof scope family member has no machine"
            ))
        ));
        let mut wrong = scope.clone();
        wrong.plan.machines.for_each_mut(|_, row| {
            if row.machine == ProofDeclarationId(2) {
                row.authored_target = Some("windows_x86_64".into());
            }
        });
        assert!(validate(&wrong, &identities).is_err());
        let mut binders = scope;
        let binder = binders.plan.symbols.insert_many([ProofDeclarationId(3)]);
        binders.plan.machines.for_each_mut(|_, row| {
            if row.machine == ProofDeclarationId(2) {
                row.binders = binder;
            }
        });
        assert!(validate(&binders, &identities).is_err());
    }

    #[test]
    fn proof_scope_rejects_ambiguous_state_owners_and_static_bindings() {
        let (mut scope, identities) = scope();
        let state = scope.plan.symbols.insert_many([ProofDeclarationId(3)]);
        scope
            .plan
            .machines
            .for_each_mut(|_, row| row.states = state);
        assert!(matches!(
            validate(&scope, &identities),
            Err(CodecError::MalformedStructuralFoundation(
                "proof scope state has ambiguous ownership"
            ))
        ));
        scope
            .plan
            .machines
            .for_each_mut(|_, row| row.states = HandleSpan::default());
        let arguments = scope.calls.bindings.insert_many([
            ProofStaticBinding {
                parameter: ProofDeclarationId(3),
                selected: ProofDeclarationId(1),
                ..Default::default()
            },
            ProofStaticBinding {
                parameter: ProofDeclarationId(3),
                selected: ProofDeclarationId(2),
                ..Default::default()
            },
        ]);
        scope.calls.calls.insert(ProofApplicationCall {
            arguments,
            ..Default::default()
        });
        assert!(matches!(
            validate(&scope, &identities),
            Err(CodecError::MalformedStructuralFoundation(
                "proof scope static application repeats a parameter"
            ))
        ));
    }
}
