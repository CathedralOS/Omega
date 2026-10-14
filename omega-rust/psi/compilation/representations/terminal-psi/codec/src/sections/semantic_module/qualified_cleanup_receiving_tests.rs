//! Consume a real canonical artifact; do not run the Source compiler here.

use super::module_wire::encode_raw;
use crate::{CanonicalTerminalArtifact, CodecError, decode_module};
use terminal_psi::{StructuralAccess, TerminalAffineCleanupAction, Terminator};

#[test]
#[ignore = "explicit qualification supplies OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT"]
fn qualified_owned_cleanup_rejects_overlapping_whole_discard() {
    let path = std::env::var_os("OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT")
        .expect("supply actual public CLI canonical output");
    let bytes = std::fs::read(path).expect("read canonical artifact");
    let artifact = CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("intact qualified cleanup must cross canonical receiving");
    let mut module = artifact.semantic_module().clone();
    let mut mutated = false;
    for machine in &mut module.machines {
        let qualified_roots = machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .filter(|parameter| {
                parameter.access == StructuralAccess::Owned
                    && (!parameter.qualifications.is_empty()
                        || !parameter.projected_qualifications.is_empty())
            })
            .map(|parameter| parameter.place)
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| {
                        operation
                            .result
                            .as_structural()
                            .filter(|result| {
                                !result.qualifications.is_empty()
                                    || !result.projected_qualifications.is_empty()
                            })
                            .map(|result| result.place)
                    }),
            )
            .collect::<Vec<_>>();
        for block in &mut machine.blocks {
            let rosters: Vec<_> = match &mut block.terminator {
                Terminator::Jump {
                    cleanup_actions, ..
                } => vec![cleanup_actions],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    vec![
                        &mut when_true.cleanup_actions,
                        &mut when_false.cleanup_actions,
                    ]
                }
                Terminator::StructuralCase { cases, .. } => cases
                    .iter_mut()
                    .map(|case| &mut case.cleanup_actions)
                    .collect(),
                _ => Vec::new(),
            };
            for actions in rosters {
                let root = actions.iter().find_map(|action| match action {
                    TerminalAffineCleanupAction::DiscardResidual(discard)
                        if qualified_roots.contains(&discard.place) =>
                    {
                        Some(discard.place)
                    }
                    _ => None,
                });
                if let Some(root) = root {
                    actions.push(TerminalAffineCleanupAction::DiscardRoot(root));
                    mutated = true;
                    break;
                }
            }
            if mutated {
                break;
            }
        }
        if mutated {
            break;
        }
    }
    assert!(
        mutated,
        "fixture must exercise a qualified owned residual root"
    );
    // Raw encoding deliberately preserves the malformed data so the public
    // decoder, rather than a producer-side validator, decides the refusal.
    let wire = encode_raw(&module).expect("encode intentional malformed wire");
    assert_eq!(
        decode_module(&wire).unwrap_err(),
        CodecError::MalformedStructuralFoundation(
            "affine region cleanup overlaps a whole-root discard"
        )
    );
}
