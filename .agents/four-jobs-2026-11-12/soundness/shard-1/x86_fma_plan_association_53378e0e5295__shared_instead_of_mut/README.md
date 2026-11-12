# Admitted x86 FMA execution

The exit oracle makes this fixture execute in the native and `--both` corpus
gates. Its success exit is 70. Previously the corpus stopped at publication.
Repeated F64 demand, a field write, and F32 cancellation retain the selected
single-rounding operation; multiply-then-add remains a separate operation.

The bounded `X86-FMA-PROVIDER-TRANSPORT` acceptance is Linux x86-64 hardware
execution of these authored oracles:

| Fixture in `pass/float/` | Native exit | Terminal exit |
| --- | --- | --- |
| `fused_multiply_add_single_rounding_exit` | 0 | 0 |
| `x86_fma_plan_association` | 70 | 70 |
| `named_provider_fused_multiply_add_exit` | 70 | 70 |
| `admitted_numeric_imports_preserve_semantics` | 70 | 70 |
| `named_provider_multiply_then_add_exit` | 70 | 70 |

The fresh baseline producer is `d6cf554fd1cbf20551983d29bae26f5cf3b81664`, built
with pinned Rust `1.100.0-nightly (a69a63265 2026-09-03)`, opt-level 1. The
cloud host exposes an AMD EPYC 9V74 under KVM. CPUID reports AVX, FMA and
OSXSAVE; XCR0 is `0xe7`, including enabled XMM/YMM state. The std-free direct
`omega run --both --keep` run exits 0 in both engines, and its retained ELF
contains `VFMADD213SS` and `VFMADD213SD` alongside separate scalar multiplies.
These are native host executions, with no instruction emulator.

The std-free witness compares positive fused residuals with zero unfused
results for both formats, passes runtime parameters through helpers, and
consumes a previous FMA result. The named unfused control also checks overflow
remains infinity. The negative
[`x86_fma_missing_deployment_features`](../../../fail/float/x86_fma_missing_deployment_features/main.omg)
uses the exact std-free source, binds Linux, omits deployment admission, and
requires both F32 and F64 missing-AVX+FMA3 diagnostics. It is never executed.

Reproduce using an optimized current producer:

```sh
python3 tools/corpus_gate.py --both --filter pass/float/fused_multiply_add_single_rounding_exit,pass/float/x86_fma_plan_association,pass/float/named_provider_fused_multiply_add_exit,pass/float/admitted_numeric_imports_preserve_semantics,pass/float/named_provider_multiply_then_add_exit --json build/fma-both.json
python3 tools/corpus_gate.py --filter fail/float/x86_fma_missing_deployment_features --json build/fma-negative.json
```

The checked-in rows retain their measured producer stamps. The normal corpus
harness supplies its documented repository test acceptance, rather than
claiming a package source audit. Host execution uses the unchanged compiler
custody checks; this environment's sandbox maps `/` to UID65534, while an
approved read-only host check reports UID0. A target-only `omega run --target`
publishes without execution and does not establish these runtime oracles.

This closes the named hardware-evidence acceptance without a compiler behavior
change. A new nine-place compiler fact row and wider release-matrix closure
are separate obligations. The governing contracts are
[numeric values](../../../../../wiki/spec/language/numeric_values.md)
(single versus double rounding and explicit ISA admission) and
[provider selection](../../../../../wiki/spec/build/provider_selection.md).
