//! A projected transfer consumes one subtree, not its containing owner.

use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

#[test]
fn projected_record_argument_executes_without_source() {
    let checked = crate::front_end::checked_program(super::PROJECTED_RECORD_ARGUMENT_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("exercise"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("projected record argument")
    .into_artifact();
    super::execute(&artifact, 1, 1);
}

#[test]
fn projected_record_transfer_rejects_changed_permission() {
    let original = crate::front_end::checked_program(super::PROJECTED_RECORD_ARGUMENT_SOURCE);
    let transfer = original
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find(|(_, event)| {
            event.kind == PermissionEventKind::Transfer && !event.segments.is_empty()
        })
        .map(|(handle, _)| handle)
        .expect("projected owned transfer");
    for mutation in 0..7 {
        let mut changed = original.clone();
        let permissions = &mut changed.facts.flow.ownership.permissions;
        match mutation {
            0 => permissions.get_mut(transfer).segments = Default::default(),
            1 => permissions.get_mut(transfer).access = PermissionAccess::Shared,
            2 => permissions.get_mut(transfer).multiplicity = Multiplicity::Unrestricted,
            3 => permissions.get_mut(transfer).obligation_live = true,
            4 => {
                permissions.get_mut(transfer).source = PermissionEventSource::Statement {
                    statement_index: 99,
                }
            }
            5 => {
                permissions.insert(permissions.get(transfer).clone());
            }
            6 => permissions.get_mut(transfer).root = facts::PlaceRoot::Symbol(Default::default()),
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("exercise"),
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .is_err(),
            "changed projected permission {mutation}"
        );
    }
}

#[test]
fn projected_record_residual_cleanup_executes_and_rejects_forged_complements() {
    let source = super::PROJECTED_RECORD_ARGUMENT_SOURCE
        .replace(
            "data Outer { inner: View; }",
            "data Token { code: i32; } data Outer { inner: View; spare: Token; }",
        )
        .replace(
            "Outer { inner: View { body: value } }",
            "Outer { inner: View { body: value }, spare: Token { code: 12 } }",
        );
    let checked = crate::front_end::checked_program(&source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("exercise"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("projected record with remaining sibling")
    .into_artifact();
    super::execute(&artifact, 1, 1);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    for mutation in 0..4 {
        let mut changed = module.clone();
        let actions = changed
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .find_map(|block| match &mut block.terminator {
                terminal_psi::Terminator::Return {
                    cleanup_actions, ..
                } if cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_)
                    )
                }) =>
                {
                    Some(cleanup_actions)
                }
                _ => None,
            })
            .expect("scalar return carries the untouched sibling");
        let position = actions
            .iter()
            .position(|action| {
                matches!(
                    action,
                    terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_)
                )
            })
            .unwrap();
        let terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) =
            actions[position].clone()
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                actions.remove(position);
            }
            1 => {
                actions.insert(position, actions[position].clone());
            }
            2 => {
                actions[position] =
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(discard.place)
            }
            3 => {
                let terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) =
                    &mut actions[position]
                else {
                    unreachable!()
                };
                discard.path.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(&changed, &proof, &profile).is_err(),
            "forged residual {mutation}"
        );
    }
}
