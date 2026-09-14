use sha2::{Digest, Sha256};

use super::generator::{GENERATOR, next};

/// Lane-local seed distinct from `generator::SEED` so each corpus lane draws
/// an independent stream under the same LCG.
pub(super) const SEED: u64 = 0x4f4d_4547_415f_5452;
pub(super) const FORMAT: &str = "omega.optimizer-corpus.transition.v1";
pub(super) const CASE_COUNT: usize = 64;
pub(super) const MAX_EDGE_ARGUMENTS: u8 = 3;
pub(super) const MAX_CARRIED_ARGUMENTS: u8 = 2;

/// The u64 fold applied on one level of the transition chain. All four
/// closures stay inside the admitted scalar instruction set, so a surviving
/// block parameter must carry the exact folded value through every edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransitionFold {
    SaturatingAdd,
    SaturatingSubtract,
    BitwiseXor,
    BitwiseAnd,
}

impl TransitionFold {
    pub(super) fn apply(self, left: u64, right: u64) -> u64 {
        match self {
            Self::SaturatingAdd => left.saturating_add(right),
            Self::SaturatingSubtract => left.saturating_sub(right),
            Self::BitwiseXor => left ^ right,
            Self::BitwiseAnd => left & right,
        }
    }
}

/// Edge-transition leaf. Each conditional edge binds `edge_arguments` seeded
/// literals into the target arm's block parameters — the false edge in
/// reversed order when `permute_false` — and each arm folds its parameters
/// before a `Jump` transports `carried` scalars (the fold result, plus the
/// first bound parameter forwarded untouched when `carried == 2`) into the
/// next block. `converge` selects one shared two-predecessor merge over a
/// private per-arm tail; `inner` lets that block dispatch on a computed
/// `IntegerEqual` whose successors transport a different operand each; and
/// `extend` replaces the last returns with a transported `Jump` into a shared
/// single-parameter final block — up to four predecessors — that returns its
/// bound value. Every parameter is bound only along an edge, so a dropped,
/// swapped, or invented edge transfer must diverge from the reference
/// interpreter before the native result can agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TransitionCase {
    pub(super) ordinal: usize,
    pub(super) edge_arguments: u8,
    pub(super) permute_false: bool,
    pub(super) converge: bool,
    pub(super) inner: bool,
    pub(super) extend: bool,
    pub(super) carried: u8,
    pub(super) fold_true: TransitionFold,
    pub(super) fold_false: TransitionFold,
    pub(super) fold_late: TransitionFold,
    pub(super) seeds: [u64; MAX_EDGE_ARGUMENTS as usize],
    /// `[when_false, when_true]` arm literals — the same arm ordering
    /// `CorpusExpected::UnsignedPerArm` and `expected(arm)` use.
    pub(super) arm_literals: [u64; 2],
    pub(super) late_literal: u64,
    pub(super) leaf_literals: [u64; 2],
}

impl TransitionCase {
    /// The exact argument vector one conditional edge transports into its
    /// target arm: `seeds[..edge_arguments]`, reversed on the false edge when
    /// `permuted` holds.
    fn edge_vector(&self, permuted: bool) -> [u64; MAX_EDGE_ARGUMENTS as usize] {
        let mut arguments = self.seeds;
        if permuted {
            arguments[..self.edge_arguments as usize].reverse();
        }
        arguments
    }

    /// The permuted false edge only changes the transported bindings when at
    /// least two scalars ride the edge.
    fn permutes(&self) -> bool {
        self.permute_false && self.edge_arguments >= 2
    }

    fn fold(&self, arm: bool) -> TransitionFold {
        if arm { self.fold_true } else { self.fold_false }
    }

    /// Simulate the generated machine's exact scalar transitions for one arm:
    /// the edge-bound parameter fold, the carried second-level bindings, and
    /// either the computed-equality dispatch or the late fold. `extend` only
    /// relocates the last observation behind another transported edge, so it
    /// does not appear here.
    pub(super) fn expected(&self, arm: bool) -> u64 {
        let arguments = self.edge_vector(!arm && self.permutes());
        let fold = self.fold(arm);
        let mut result = arguments[0];
        for argument in &arguments[1..self.edge_arguments as usize] {
            result = fold.apply(result, *argument);
        }
        result = fold.apply(result, self.arm_literals[arm as usize]);
        let forwarded = arguments[0];
        if self.inner {
            let other = if self.carried == MAX_CARRIED_ARGUMENTS {
                forwarded
            } else {
                self.late_literal
            };
            let (argument, literal) = if result == other {
                (result, self.leaf_literals[0])
            } else {
                (other, self.leaf_literals[1])
            };
            self.fold_late.apply(argument, literal)
        } else {
            let combined = if self.carried == MAX_CARRIED_ARGUMENTS {
                self.fold_late.apply(result, forwarded)
            } else {
                result
            };
            self.fold_late.apply(combined, self.late_literal)
        }
    }

    /// What the false arm would answer had its edge carried the identity
    /// order — a permuted edge must actually change the transported bindings
    /// for at least one generated case.
    fn unpermuted_false(&self) -> u64 {
        let mut unpermuted = *self;
        unpermuted.permute_false = false;
        unpermuted.expected(false)
    }
}

pub(super) fn cases() -> Vec<TransitionCase> {
    let mut state = SEED;
    let cases = (0..CASE_COUNT)
        .map(|ordinal| {
            // Low LCG bits cycle with period 2^k, so every selector reads the
            // high bits of a fresh draw.
            let edge_arguments = 1 + ((next(&mut state) >> 62) as u8 % MAX_EDGE_ARGUMENTS);
            let carried = 1 + ((next(&mut state) >> 62) as u8 % MAX_CARRIED_ARGUMENTS);
            let flags = next(&mut state);
            let fold = |state: &mut u64| match (next(state) >> 62) as u8 {
                0 => TransitionFold::SaturatingAdd,
                1 => TransitionFold::SaturatingSubtract,
                2 => TransitionFold::BitwiseXor,
                _ => TransitionFold::BitwiseAnd,
            };
            TransitionCase {
                ordinal,
                edge_arguments,
                permute_false: (flags >> 63) & 1 == 1,
                converge: (flags >> 62) & 1 == 1,
                inner: (flags >> 61) & 1 == 1,
                extend: (flags >> 60) & 1 == 1,
                carried,
                fold_true: fold(&mut state),
                fold_false: fold(&mut state),
                fold_late: fold(&mut state),
                seeds: [next(&mut state), next(&mut state), next(&mut state)],
                arm_literals: [next(&mut state), next(&mut state)],
                late_literal: next(&mut state),
                leaf_literals: [next(&mut state), next(&mut state)],
            }
        })
        .collect::<Vec<_>>();
    assert_coverage(&cases);
    cases
}

pub(super) fn validate_manifest(cases: &[TransitionCase]) {
    assert_eq!(cases.len(), CASE_COUNT);
    let rendered = format!(
        "format={FORMAT}\ngenerator={GENERATOR}\nseed={SEED:#018x}\ncase_count={CASE_COUNT}\npsi_shape=conditional_and_jump_edges_bind_scalar_block_parameters_through_folded_transports\nhost_native_shape=edge_transported_scalar_block_parameters_same_artifact\nhost_native_oracle=terminal_interpreter_equals_postallocation_optimized_native_u64_per_arm\noperand_lane=seeded_edge_argument_orders_permutations_forwardings_and_inner_condition_dispatches\nrecords_sha256={}\n",
        records_digest(cases),
    );
    let checked_in = include_str!("../../corpora/optimizer/v2/conditional_transition_lanes.txt");
    assert_eq!(
        checked_in, rendered,
        "transition optimizer corpus manifest drifted"
    );
}

fn records_digest(cases: &[TransitionCase]) -> String {
    let mut hasher = Sha256::new();
    for case in cases {
        hasher.update((case.ordinal as u64).to_le_bytes());
        hasher.update([
            case.edge_arguments,
            case.carried,
            case.permute_false as u8,
            case.converge as u8,
            case.inner as u8,
            case.extend as u8,
            case.fold_true as u8,
            case.fold_false as u8,
            case.fold_late as u8,
        ]);
        for seed in case.seeds {
            hasher.update(seed.to_le_bytes());
        }
        for literal in case.arm_literals {
            hasher.update(literal.to_le_bytes());
        }
        hasher.update(case.late_literal.to_le_bytes());
        for literal in case.leaf_literals {
            hasher.update(literal.to_le_bytes());
        }
        hasher.update(case.expected(false).to_le_bytes());
        hasher.update(case.expected(true).to_le_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_coverage(cases: &[TransitionCase]) {
    // Edge width extremes and every fold in every position.
    assert!(cases.iter().any(|case| case.edge_arguments == 1));
    assert!(
        cases
            .iter()
            .any(|case| case.edge_arguments == MAX_EDGE_ARGUMENTS)
    );
    for fold in [
        TransitionFold::SaturatingAdd,
        TransitionFold::SaturatingSubtract,
        TransitionFold::BitwiseXor,
        TransitionFold::BitwiseAnd,
    ] {
        assert!(cases.iter().any(|case| case.fold_true == fold));
        assert!(cases.iter().any(|case| case.fold_false == fold));
        assert!(cases.iter().any(|case| case.fold_late == fold));
    }
    // Both carried widths: the forwarded-parameter second binding must be
    // witnessed, and so must the single-result transport.
    assert!(cases.iter().any(|case| case.carried == 1));
    assert!(
        cases
            .iter()
            .any(|case| case.carried == MAX_CARRIED_ARGUMENTS)
    );
    // Permuted false edges, including one whose swapped order changes the
    // answer and one applied through the non-commutative subtract fold.
    assert!(cases.iter().any(|case| case.permutes()));
    assert!(
        cases
            .iter()
            .any(|case| case.permutes() && case.expected(false) != case.unpermuted_false())
    );
    assert!(
        cases.iter().any(|case| {
            case.permutes() && case.fold_false == TransitionFold::SaturatingSubtract
        })
    );
    // Shared two-predecessor merges and private per-arm tails.
    assert!(cases.iter().any(|case| case.converge));
    assert!(cases.iter().any(|case| !case.converge));
    // Computed-equality inner dispatches and plain late folds.
    assert!(cases.iter().any(|case| case.inner));
    assert!(cases.iter().any(|case| !case.inner));
    // Extended transports into a shared final block: one predecessor for the
    // converged merge, two for split tails or converged leaves, and four for
    // split leaves.
    assert!(cases.iter().any(|case| case.extend));
    assert!(cases.iter().any(|case| !case.extend));
    assert!(
        cases
            .iter()
            .any(|case| case.extend && !case.inner && case.converge)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.extend && !case.inner && !case.converge)
    );
    assert!(
        cases
            .iter()
            .any(|case| case.extend && case.inner && !case.converge)
    );
    // The two arms must disagree somewhere, or the per-arm oracle would never
    // observe a transition-sensitive answer.
    assert!(
        cases
            .iter()
            .any(|case| case.expected(false) != case.expected(true))
    );
}
