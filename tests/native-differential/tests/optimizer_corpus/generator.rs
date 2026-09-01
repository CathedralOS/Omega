pub(super) const FORMAT: &str = "omega.optimizer-corpus.v2";
pub(super) const GENERATOR: &str = "omega-test-lcg64-v1";
pub(super) const SEED: u64 = 0x4f4d_4547_415f_4f50;
pub(super) const CASE_COUNT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LaneInput {
    pub(super) left: u64,
    pub(super) right: u64,
    pub(super) expected: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CorpusCase {
    pub(super) ordinal: usize,
    pub(super) x86: LaneInput,
    pub(super) aarch64: LaneInput,
}

pub(super) fn cases() -> Vec<CorpusCase> {
    let mut state = SEED;
    (0..CASE_COUNT)
        .map(|ordinal| {
            let x86_expected = (next(&mut state) as u32).max(1) as u64;
            let x86_left = next(&mut state);
            let lane = (next(&mut state) & 3) as u32;
            let complement = ((next(&mut state) as u16) | 1) as u64;
            let aarch64_expected = u64::MAX ^ (complement << (16 * lane));
            let aarch64_left = next(&mut state);
            CorpusCase {
                ordinal,
                x86: LaneInput {
                    left: x86_left,
                    right: x86_expected.wrapping_sub(x86_left),
                    expected: x86_expected,
                },
                aarch64: LaneInput {
                    left: aarch64_left,
                    right: aarch64_expected.wrapping_sub(aarch64_left),
                    expected: aarch64_expected,
                },
            }
        })
        .collect()
}

fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}
