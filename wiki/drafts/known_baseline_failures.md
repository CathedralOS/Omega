# Known baseline failures

Attribution evidence for test failures that reproduce at a recorded revision
without a task diff. A worker that hits one of these may cite this table —
command, revision, and failure set — instead of re-running a stash baseline.
Refresh or remove a row when its failures are fixed or when a task's diff could
plausibly interact with them; a listed failure does not excuse an unexplained
failure in affected behavior, and this file is not validation policy (see
[AGENTS.md](../../AGENTS.md#validation-scope)).

Rows verified by independent stash-baseline reproduction at revision
`8220f55febc1` on 2026-09-13, macOS arm64, unless noted otherwise.

## typed-trees-to-checked-trees

`mbx nextest run -p typed-trees-to-checked-trees` — 3612/3624 pass; 12 fail in
boundary, attached-frame, borrow, terminal-unit, and contract test areas
unreachable from the `src/flow/` modules and the additive `product_pruning`
module. Sampled members (`erased_record_equality…`,
`discarded_calls_in_open_templates…`,
`accepts_static_persistent_copy_across…`) reproduce identically with the diff
stashed. Independently verified twice on this revision.

## checked-trees-to-lowered-psi

`mbx nextest run -p checked-trees-to-lowered-psi --lib` — 598/599 pass;
`byte_write_loop::byte_input_exact_narrowing_rejects_a_state_annotation_without_field_bounds`
fails with stale expected diagnostic text.

## compiler build-target activation

`mbx nextest run -p compiler --test build_target_activation` — 16/22 pass; 6
fail in FMA demand and service-reach fixtures (`admitted_x86_fma_demand…`,
`aarch64_fma_demand…`, `boundary_operator_and_float_adapters…`,
`exact_x86_fma_demand_fails_closed…`, `source_fma_then_attached_unit_call…mxcsr`,
`terminal_product_retains_exact_fma…`). Fixtures lack service-reach
declarations; x86 FMA provider transport is unimplemented on this host.

## native-realization

`mbx nextest run -p native-realization --lib` — 79/83 pass;
`source_ordered_calls_reach_executable_publication`,
`source_common_return_conditionals_use_the_shared_native_pipeline`, and two
`function_reporting` tests fail with
`Selection(Legalization(SourceCustodyMismatch))` before the physical gate.

## terminal-fixed-fuel

`mbx nextest run -p terminal-fixed-fuel` — 38/39 pass;
`fixed_entry::contextual_scalar_cleanup_proof_metadata_adds_zero_fixed_fuel`
fails with `RejectedEvidence { obligation: ObligationId(3), error:
Certificate(UnknownAssumption(2)) }`.

## Host note (macOS)

`rust-objcopy` emits `dyld: Library not loaded: @rpath/libLLVM.dylib` (SIGABRT)
during test-binary linking on the pinned `nightly-2026-09-04` toolchain on
this host. It is a warning in build output only and does not fail compilation
or tests; do not investigate it as a test failure.

## Intel macOS host gap (x86_64-apple-darwin)

Environmental, not a test regression, verified across the macw3 wave on
2026-09-14 (independent sessions reproduced identical sets on unmodified
bases including `d1b7165cd6`, `07782416b4`, and `1055e88f31`):

- `TargetProfile::host()`
  (`omega-rust/omega/representations/target/src/lib.rs`, `host()`) has cfg
  arms for macos-aarch64, linux-aarch64, linux-x86_64, and windows-x86_64
  only, so host-profiled tests panic with `unsupported host profile for
  Omega native planning`. Observed sets: ~20 `package-manager` ops tests and
  2 `compiler::request` tests in the workspace `--lib` run.
- `native_hosted_target()` in `compiler/tests/canary_suite.rs` has the same
  four cfg arms, so `mbx nextest run -p compiler --test canary_suite` does
  not compile on this host.

On this host, route `omega` invocations through an explicit `--target` (for
example `linux_x86_64`) and report native-host coverage as unavailable rather
than re-running the baseline; a `MacosX64` host profile is open board work,
not a fix to inline into a task.
