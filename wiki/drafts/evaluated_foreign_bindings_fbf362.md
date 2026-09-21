# EVALUATED-FOREIGN-BINDINGS — re-verification at fbf36233c9

Re-verified the row's recorded state on linux x86-64. TASKS.md is under live
claims this wave, so this ledger carries the stamp.

## Recorded verdict, re-confirmed

The row's scope verification (`201d58c5915`) still holds: the consuming
machinery is landed and pinned; the producing chain is still absent.

- `image-emission/src/function_fragments/production.rs:226` still hardcodes
  `port_effects: Vec::new()`.
- `native-realization/src/native_realization/callback_thunks.rs:171` still
  hardcodes `port_effects: Vec::new()`.
- Nothing constructs a `machine_code::PortEffectRecord` outside the
  consumer-side test/evidence surfaces; no selected instruction or
  machine-emission fragment emits port-write bytes.

## Witness runs at fbf36233c9 (linux x86-64)

```text
cargo nextest run -p native-artifact --lib
  # 43/43 PASS — derivation/replay pins green, including the
  # port-effect custody, substitution and role-swap negatives in
  # physical::derivation::tests (PortEffectRecord-driven cases at :574-634).

OMEGA_FAIL_CANARY_FILTER=ports \
  cargo nextest run -p compiler --test canary_suite -E 'test(~fail_canaries)'
  # 3/3 PASS — ports/asm_port_in_unsettled still rejects at planning:
  # "no checked unit operation arm exists for a port read yet", which is
  # exactly the recorded producer gap made observable.
```

## Disposition

No bounded slice. The remaining work is the whole producing half: a
port-write emission fragment in the selected-instruction/machine-emission
lane plus `PortEffectRecord` production in the fragment writers — a
multi-crate leg that is the substance of PRIVILEGED-PORT-EFFECT-SETTLEMENTS
(its fence has expired from the registry this wave, but the scope is
unchanged). Fence map re-checked at claim time: `terminal_authority_policy`
under FILESYSTEM-RELEASE-CONTRACT (exp 14:20Z), `machine_code/calls` +
selection construction under CALLBACK-PRIVATE-MATERIALIZATION (exp 09:13Z);
the fragment-writer files are momentarily unfenced but nothing they could
produce exists upstream to record.
