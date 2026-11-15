#![forbid(unsafe_code)]

/*
We provide target-neutral conformance test suites that both `isa-aarch64` and
`isa-x86_64` run against their selected-form encoders, decoders, and validators.
Our three public modules cover IEEE floating-point result policies
(`float_result_policy`), narrow 8-bit and 16-bit zero-extending memory loads
(`narrow_load`), and Linux supervisor calls (`system_call`).
*/

/*
We state cross-architecture conformance assertions once in this crate so that
neither instruction set can accidentally test a weaker contract than the other.
Each architecture supplies only what is genuinely target-specific — its
canonical physical register model, admitted operand rosters, and emitted instruction
bytes — while we own the operand sweeps, NaN/infinity/subnormal/signed-zero
payload oracles, footprint checks, single-bit mutation refusals, and operand
aliasing rejections.
*/

/*
`isa-aarch64` and `isa-x86_64` depend on `isa-conformance` in their unit and
machine-effect tests:
- `float_result_policy`: expected IEEE 754 binary32/binary64 payloads and trap
  outcomes for `FloatResultSaturating` and `FloatResultTrapping` forms.
- `narrow_load`: `Footprint` verification, width/offset sweeps, and bit-flip and
  register-aliasing rejection for `Load8` and `Load16`.
- `system_call`: `Encoding` verification, arity 0..=6 register transport sweeps,
  effect catalog conformance, and kernel-entry mutation rejection.
*/

pub mod float_result_policy;
pub mod narrow_load;
pub mod system_call;
