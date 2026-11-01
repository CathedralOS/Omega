# Native failure preserves its successful target sibling

One ordinary package request produces one canonical Terminal artifact. Linux
executes the authored zero-materialization check; macOS enters Omega stage 06
and refuses the selected x86-only optimization with its existing target mismatch
obligation. The refusal must not discard the successful Linux image or replay
source, Build, Terminal publication, or a native stage.

Run the optimized `omega-build` example with `OMEGA_PHASE_PROFILE=1`:

```sh
XDG_CACHE_HOME="$(mktemp -d)" OMEGA_PHASE_PROFILE=1 package_batch --native-evidence --compare-single-targets --run \
  --reject macos_arm64 --reason 'TargetMismatch { optimization: X86SelectXorZeroI64MaterializationV1, required: X86_64, actual: Aarch64 }' \
  --reject-through-stage 6 --targets macos_arm64,linux_x86_64 \
  tests/omega/fail/targets/native_pipeline_failure_isolation/main.omg 1 1 1 2
```

The explicit whole-invocation counts start before package preparation. Independent
single-target byte comparisons start afterward. The probe retains the failed
native prefix and successful image, and checks native/reloaded-Terminal agreement
on the Linux host. It does not establish numbered Psi invocation counts or
foreign-host execution. The original single-target optimizer refusal fixture is
unchanged.

The private empty cache fixes the fresh-check precondition; valid retained proof
reuse may reduce that count on a warm cache.
