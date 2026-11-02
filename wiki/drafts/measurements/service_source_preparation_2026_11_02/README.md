# Shared service declaration ownership

The existing frozen service census now retains each record's nominal declaration
owner and each ordered requirement's nominal owner as deferred `Result` values.
It captures them when the record is constructed, using the sealed toolchain
source roster. The existing identity operation moved from Omega to this Psi
owner; it was not duplicated or replaced with callable overload identities.

Per-target `ServiceDeclarations::prepare` now takes only that descriptive roster
and the target's evaluated Calling evidence. It performs no checked-tree lookup,
service-context binding or nominal-owner reconstruction. Generic applications,
phantom/region arguments and root/inherited Calling attachment use the unchanged
attachment operation. There is no new cache, map, IR or target selector.

Original owner authentication remains at selected-execution ingress. Unsupported
surfaces and missing ownership stay deferred; the consumer's refusal order is
still owner, surface, correspondence, then ordered requirement identities.
Independent package review freshly constructs and binds its supplied candidate
catalog using the original custody's sealed source roster. It never borrows the
producer's Source-coordinate catalog. Early Build's absent toolchain roster
produces inert identity refusals, unused by its existing derive/select path.

## Exact evidence

Fixed author base: `eeb9ca6c5a99cbdc5339409fc7073881c501fb3b`, published through
[PR #1462](https://github.com/CathedralOS/Omega/pull/1462) as
`96defff0759c5210a7708d2d155bdf5d17ec9242`.
Tested source: `083a172200e3a8b5c0651f75e57372fc65ffa970`.
Host: macOS ARM64; rustc `1.100.0-nightly (a69a63265 2026-09-03)`.

The verified opt-level-1 corpus runner SHA-256 is
`4a90eadb78303f771c779620c6f44d7c8ef1658a00137be3ffa721fb1e8f692c`.
The existing shared-target integration executable SHA-256 is
`62c2b7a44ac2c3a240ce2cba4d892403b8d91fac852b58c4490295e95d98ee8d`.
[Raw reports, commands and profile receipt](https://github.com/CathedralOS/Omega/releases/tag/evidence/service-source-083a172200e3)
retain these actual observations:

| Exact fixture | Mode | Observation |
| --- | --- | --- |
| `pass/build/terminal_calling_policy_execution` | `--both` | Native 0, Terminal 0; inspections match |
| `pass/build/terminal_private_callback_calling` | `--both` | Native 0, Terminal 0; callback controls and inspections match |
| `pass/host/runtime_console_low_byte_exit` | `--both` | Native 0, Terminal 0; both engines' binary stdout and stderr match |
| `fail/build/uefi_entry_schema_wrong_owner` | check | Exact dependency entry schema requires accepted binding |
| `fail/build/uefi_entry_schema_accepted_wrong_convention` | check | SystemVAMD64 refuses the MicrosoftX64 entry obligation |
| `fail/build/calling_policy_missing_target_member` | check | Intended missing Linux target-member refusal |

All three reject controls matched their authored fragments. Commands used
`tools/corpus_gate.py --runner <verified-runner> --filter <exact names> --json`;
the three positives added `--both`. No fixture, oracle or golden changed.
The positive gate exited 1 for an already-observed inspection fact difference
and historical timing movement, not an authored oracle failure. The reject gate
exited 1 for timing movement only. These are scoped behavior observations, not
a claim that either golden comparison was clean.

The existing shared-target test passed 1/1 in 129.73 s. It asserts one Source
check, one Build execution, one Terminal publication and shared artifact pointer
identity. macOS ARM64 and Linux ARM64 images equal their respective single-target
images byte-for-byte; an earlier refused target leaves supported output intact.
Only the macOS image executes. The existing source architecture guard
`product_boundary::provider_derivation_consumes_the_prepared_service_census`
passed 1/1; its optimized rebuild plus execution took 117.185 s. Scoped compiler
check completed in 32.66 s before the final inline capture threading, which the
subsequent exact-source builds compiled. Rule and whitespace checks passed.

## Honest limits

`terminal-runtime.retained_plan` is `unrequested`, not a passing runtime control.
The same fixture already had that observation with the preceding verified
`bd6c07b138bd5c1d8d91ec3c794bc8d27f463b29` producer, preserved in the
[Calling capture report](https://github.com/CathedralOS/Omega/releases/tag/evidence/calling-source-bd6c07b138bd).
The service change did not create that difference from the older golden, and
does not claim to repair or qualify that separate inspection consumer.

Positive program times were 47.658–118.558 s; reject times were 2.988–38.152 s.
Historical goldens have different producer/workload contexts. These are not
controlled performance comparisons or a speedup claim. Uniform owner metadata
also captures keys on currently unconsumed catalog records; no separate pass or
additional per-target Source derivation is introduced.

Independent source review found no semantic blocker. Final source changes after
the tested producer only record this evidence. Provider settlement, program-entry
preparation, intrinsic declaration preparation and remaining Source-qualified
review are still migration work. This does not close the parent task or prove
current Cathedral boot.
