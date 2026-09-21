# AP-BRINGUP — verify + record (z142)

Marked row (TASKS.md:1463) — Cathedral secondary-processor startup
under `wiki/spec/build/external_roots.md#secondary-processor-startup`.

## Re-verified at `97be15c1b5` (linux x86-64)

Both delivered legs hold:

- **Ledger** — `external-roots/src/platform_bringup/secondary_processor`
  complete: three-way `SecondaryProcessorStartupVerdict`,
  `complete_secondary_processor_startup` (DefiniteNondispatch withdraws,
  DispatchUnconfirmed stays invoked-held, ConfirmedArrival starts),
  `settle_secondary_processor_startup` two-premise release naming the
  exact outstanding carrier, replay/foreign/stale receipt rejection,
  overlapping stack-class/state refusal, quiescence-bound retirement —
  witnessed in `secondary_processor/tests.rs` +
  `tests/omega/pass/memory/secondary_processor_canary/` authored
  surface.
- **Emission (leg 1)** — landed `d71c5ad8a6`:
  `machine-emission/src/startup_trampoline.rs`
  emit/resolve/validate x86-64 startup trampoline; the test installs
  emitted bytes, not the authored array.

Fresh witness: `cargo nextest run -p compiler --test
secondary_processor_startup` — **4/4 PASS** (47.8s):
nondispatch-withdrawal+late-arrival, installed-entry reach,
stale-evidence/resource-conflict rejection, terminal-and-native
contract survival.

## Residuals (not this item's slice)

- (2) real receipt ingress — owned by BOUNDARY-ISSUANCE's issuance
  review.
- (3) Windows/macOS/QEMU host legs — host-unavailable on this lane.
- The `uefi_bootstrap` neighbor subtree stays fenced to
  UEFI-PHYSICAL-SEMANTIC-ENTRY; it is not this ledger.

## Verdict

**Open-but-attributed / record-only.** Every implementable ledger+emission
leg under this name is landed and freshly witnessed; the two residuals
belong to BOUNDARY-ISSUANCE (receipt ingress) and unavailable hosts.
No code change.
