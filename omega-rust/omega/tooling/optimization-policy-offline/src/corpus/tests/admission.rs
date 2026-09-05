use optimization_core::{ExternalDecisionAction, ExternalDecisionContext, ExternalDecisionLog};
use optimization_core::{OptimizationDecisionSchemaIdentity, OptimizationReasonCode};

use super::fixture::{chosen_point, context, encoded_log, skipped_point, source};
use crate::{OfflinePolicyCorpusError, admit_external_decision_logs, decision_surface_identity};

#[test]
fn admission_canonicalizes_log_order_and_returns_identity_bound_receipt() {
    let first = encoded_log(source(b"source-a"), [chosen_point(b"a")]);
    let second = encoded_log(source(b"source-b"), [skipped_point(b"b")]);
    let ordered = admit_external_decision_logs([first.clone(), second.clone()]).unwrap();
    let reversed = admit_external_decision_logs([second, first]).unwrap();

    assert_eq!(ordered.encode(), reversed.encode());
    assert_eq!(ordered.identity(), reversed.identity());
    assert_eq!(ordered.receipt().log_count(), 2);
    assert_eq!(ordered.receipt().source_count(), 2);
    assert_eq!(ordered.receipt().decision_count(), 2);
    assert_eq!(ordered.examples().len(), 2);
    assert_eq!(
        ordered.receipt().schema(),
        optimization_core::external_psi_decision_schema_v2_identity()
    );
    assert_eq!(
        ordered.examples()[0].point().action(),
        ordered.examples()[0].recorded_action()
    );
}

#[test]
fn decision_surface_excludes_action_while_corpus_identity_retains_it() {
    let source_identity = source(b"action-source");
    let chosen = chosen_point(b"same");
    let skipped = skipped_point(b"same");
    assert_eq!(
        decision_surface_identity(context(source_identity), &chosen),
        decision_surface_identity(context(source_identity), &skipped)
    );

    let chosen_corpus =
        admit_external_decision_logs([encoded_log(source_identity, [chosen])]).unwrap();
    let skipped_corpus =
        admit_external_decision_logs([encoded_log(source_identity, [skipped])]).unwrap();
    assert_ne!(chosen_corpus.identity(), skipped_corpus.identity());
    assert!(matches!(
        chosen_corpus.examples()[0].recorded_action(),
        ExternalDecisionAction::Choose(_)
    ));
    assert_eq!(
        skipped_corpus.examples()[0].recorded_action(),
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable)
    );
}

#[test]
fn empty_wrong_schema_duplicate_log_and_conflicting_surface_reject() {
    assert_eq!(
        admit_external_decision_logs::<[Vec<u8>; 0], Vec<u8>>([]),
        Err(OfflinePolicyCorpusError::EmptyCorpus)
    );

    let source_identity = source(b"reject-source");
    let empty = ExternalDecisionLog::new(context(source_identity), [])
        .unwrap()
        .encode();
    assert_eq!(
        admit_external_decision_logs([empty]),
        Err(OfflinePolicyCorpusError::EmptyDecisionLog)
    );

    let ordinary = encoded_log(source_identity, [chosen_point(b"ordinary")]);
    assert_eq!(
        admit_external_decision_logs([ordinary.clone(), ordinary]),
        Err(OfflinePolicyCorpusError::DuplicateLog)
    );

    let authored = context(source_identity);
    let wrong = ExternalDecisionContext::new(
        OptimizationDecisionSchemaIdentity::from_canonical_bytes(b"foreign-schema"),
        authored.source(),
        authored.selections(),
        authored.phase_selections(),
        authored.target(),
        authored.rule_set(),
        authored.cost_model(),
    );
    let wrong = ExternalDecisionLog::new(wrong, [chosen_point(b"wrong")])
        .unwrap()
        .encode();
    assert_eq!(
        admit_external_decision_logs([wrong]),
        Err(OfflinePolicyCorpusError::WrongExternalSchema)
    );

    let chosen = encoded_log(source_identity, [chosen_point(b"conflict")]);
    let skipped = encoded_log(source_identity, [skipped_point(b"conflict")]);
    assert_eq!(
        admit_external_decision_logs([chosen, skipped]),
        Err(OfflinePolicyCorpusError::DuplicateDecisionSurface)
    );
}
