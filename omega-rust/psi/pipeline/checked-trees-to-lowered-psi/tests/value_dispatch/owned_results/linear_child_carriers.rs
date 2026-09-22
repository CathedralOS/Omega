//! Whole affine carriers with `[linear]` children join the owned selection:
//! the transfer's claim set discharges each child's exact claim, and the
//! joined destination re-establishes the frontier under its own symbol so its
//! children stay consumable.

use crate::value_dispatch::check_source;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use language_semantics::{PermissionClaimIdentity, PermissionProvenance};

/// A whole-carrier arm consumes the joined root's entire linear frontier: one
/// claim row per frontier child, each naming the exact source-relative field
/// path and the consumed place's own identity and provenance. Settling every
/// re-established child after the join is ordinary linear use.
///
/// Lowering is bounded at the same consumer gap as the primitive borrowed
/// referent: no machine whose body holds claim-bearing custody has a
/// source-independent checked scalar control plan yet, so `lower_machine`
/// stops before the receipt replay. The claim-set replay itself is pinned by
/// the lowerer crate's structural replay unit tests, and the checker-side
/// independent replay by
/// `carrier_owned_selection_replay_rejects_mutated_claim_evidence`.
#[test]
fn carrier_match_moves_the_whole_claim_frontier_and_reconsumes_each_child() {
    for (name, source, expected_fields, shared_source) in [
        (
            "one linear child, uniform source",
            "data Token [linear] { code: u64; }
             data Holder { left: Token; }
             machine Token::settle(self) {}
             machine choose(selected: bool, x: Holder) -> u64 {
                 let picked: Holder = match selected { true -> x, false -> x };
                 let code: u64 = picked.left.code;
                 Token::settle(picked.left);
                 code
             }",
            vec!["left"],
            true,
        ),
        (
            "two linear children, uniform source",
            "data Token [linear] { code: u64; }
             data Holder { left: Token; right: Token; }
             machine Token::settle(self) {}
             machine choose(selected: bool, x: Holder) -> u64 {
                 let picked: Holder = match selected { true -> x, false -> x };
                 let code: u64 = picked.left.code;
                 Token::settle(picked.left);
                 Token::settle(picked.right);
                 code
             }",
            vec!["left", "right"],
            true,
        ),
        (
            "two linear children, distinct sources",
            "data Token [linear] { code: u64; }
             data Holder { left: Token; right: Token; }
             machine Token::settle(self) {}
             machine choose(selected: bool, x: Holder, y: Holder) -> u64 {
                 let picked: Holder = match selected { true -> x, false -> y };
                 let code: u64 = picked.left.code;
                 Token::settle(picked.left);
                 Token::settle(picked.right);
                 code
             }",
            vec!["left", "right"],
            false,
        ),
    ] {
        let checked =
            check_source(source).unwrap_or_else(|errors| panic!("{name} checks: {errors:#?}"));
        let ownership = &checked.facts.flow.ownership;
        let (_, receipt) = ownership
            .owned_selections
            .iter()
            .next()
            .unwrap_or_else(|| panic!("{name}: carrier selection receipt"));
        let mut arm_identities = Vec::new();
        for transfer in ownership
            .selection_transfers
            .span_or_empty(receipt.transfers)
        {
            assert!(
                ownership.segments.span_or_empty(transfer.path).is_empty(),
                "{name}: a whole carrier leaf records an empty moved path: {transfer:?}"
            );
            let claims = ownership
                .selection_transfer_claims
                .span_or_empty(transfer.claims);
            let fields = claims
                .iter()
                .map(|claim| match ownership.segments.span_or_empty(claim.path) {
                    [facts::PlaceSegment::Field { symbol }] => {
                        checked.typed.symbols.name(*symbol).to_string()
                    }
                    path => panic!("{name}: each edge discharges the exact field claims: {path:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                fields, expected_fields,
                "{name}: the claim set is the leaf's whole linear frontier"
            );
            for claim in claims {
                assert_ne!(
                    claim.claim_identity,
                    PermissionClaimIdentity::Unknown,
                    "{name}: each claim carries the consumed place's identity: {claim:?}"
                );
                assert!(
                    matches!(claim.provenance, PermissionProvenance::Established { .. }),
                    "{name}: each claim carries the consumed place's provenance: {claim:?}"
                );
            }
            arm_identities.push(
                claims
                    .iter()
                    .map(|claim| claim.claim_identity)
                    .collect::<Vec<_>>(),
            );
        }
        if shared_source {
            assert_eq!(
                arm_identities[0], arm_identities[1],
                "{name}: uniform arms discharge the same source claims on every edge"
            );
        } else {
            assert_ne!(
                arm_identities[0], arm_identities[1],
                "{name}: distinct sources keep their own claim identities"
            );
        }
        // Every frontier claim is consumed on every edge and re-established at
        // the join, so per-child `Token::settle` use checks. The claim-bearing
        // local itself still waits on unit-control local construction: the
        // machine stops at the documented consumer gap rather than lowering.
        assert_eq!(
            format!(
                "{:?}",
                checked_trees_to_lowered_psi::lower_machine(
                    &checked,
                    TerminalMachineSelection::Name("choose")
                )
                .unwrap_err()
            ),
            r#"Unsupported("machine has no source-independent checked scalar control plan")"#,
            "{name}: a claim-bearing local has no scalar control plan yet"
        );
    }
}

/// The joined carrier's claims are settled at the result frontier, so a
/// caller-side use of a moved source — or a second consumption of a joined
/// child — keeps hitting the dead-claim fences at checking.
#[test]
fn carrier_match_rejects_post_join_source_and_double_consumption() {
    for (name, tail) in [
        (
            "moved source reuse",
            "let again: Holder = x; Token::settle(again.left); Token::settle(again.right);",
        ),
        (
            "double consumption of a source child",
            "Token::settle(picked.left); Token::settle(picked.right); Token::settle(x.left);",
        ),
    ] {
        let errors = check_source(&format!(
            "data Token [linear] {{ code: u64; }}
             data Holder {{ left: Token; right: Token; }}
             machine Token::settle(self) {{}}
             machine choose(selected: bool, x: Holder) -> u64 {{
                 let picked: Holder = match selected {{ true -> x, false -> x }};
                 {tail}
                 0
             }}"
        ))
        .expect_err(&format!("{name} must reject"));
        assert!(
            errors.iter().any(|error| {
                error.message.contains("may have been transferred")
                    || error.message.contains("already transferred or consumed")
            }),
            "{name}: {errors:#?}"
        );
    }
}
