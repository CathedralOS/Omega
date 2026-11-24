# Triage packet: unit_022 (area 11, native stages 11-15)

This packet gives a disposition for each claim in [records/unit_022.md](../../records/unit_022.md).
That record covers lines 1-7800 of `image-emission/src/function_fragments.rs` and its three small child
files. Lines 7801-21313 belong to the gap packets (`unit_022_gap_009` onward) and are not judged here.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. Reviewed at `a0152d26`; nothing under
`15_objects-to-image/image-emission/` changed between the two. Line numbers below are unchanged from the record.
Paths are under `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/`;
`FF` is `function_fragments.rs`. Evidence is code reading; nothing was built or run.

Status: in progress (early receipt).

## Claims

### F1. Empty installation call set accepts (record §1 "Fail-open risks", bullet 2; digest 11 line 203, `FF:3760`)
- Code path: `source::installed_calls::PreparedCalls::new` (`FF:3731-3790`) and `validate` (`FF:3792-3929`).
- Contract: every selected call owned by `CallSiteOwner::Installation` must belong to an
  `InstallStaticInterrupt` target operation and match its validation or work helper.
- Trace: `PreparedCalls::new` checks every installation-owned call against `targets` (`FF:3755-3759`) before the
  early return at `FF:3760`. With no targets, any installation-owned call is refused with "installation call has
  no owning operation", so the early return is reached only when `calls` is empty too. Calls with other owners
  are outside this validator's role. Each target carries `calls: [TargetInstallationCall; 2]`
  (`representations/target-operations/src/operations.rs:55`), so the "position 0 validate, others work" loop
  (`FF:3841-3851`) sees exactly two rows, and `installed_interrupts` counts exactly one of each (`FF:4593-4602`).
- Outcome: **intended** (refuses, no accept path).

### F2. Reads inside the IDT interval skip guard checks (record §1 "Fail-open risks", bullet 4; digest 11 line 203, `FF:5291-5311`)
- Code path: `installed_interrupts::publication::reads` (`FF:5263-5373`), `within` (`FF:5232-5238`),
  `Contract::derive` (`FF:4043-4160`).
- Contract: an installation load must address an operand input, a declared control word, or an authenticated
  read interval; loop-indexed intervals need the loop bound and the entry guards.
- Trace: `within` succeeds only when the address is the base plus a *constant* offset whose end fits the extent
  (`offset(address, base)` then `end <= extent`). The early skip at `FF:5302-5308` therefore exempts only
  constant-offset reads that lie entirely inside `pointers[0]` (the table operand, `FF:4051`) and its
  `table_bytes`. `Contract::derive` declares the same base and extent as `ReadKind::Table`
  (`FF:4146-4150`), and that arm requires no guard (`FF:5368`). Without the early skip such a read would reach
  `fresh_observations` (a no-op here: the only loads in `pointers[0]+c` are the operand loads it exempts,
  `FF:5492-5498`) and then the `Table` arm, with the same acceptance. Indexed gate reads
  (`table + index*16 + 5`) have no constant offset, so they fail `within` and still take the guarded `Gate`
  path (`FF:4126-4130`, `FF:5326-5361`).
- Outcome: **intended**. The early skip duplicates the declared `ReadKind::Table` interval; it accepts nothing
  that the interval roster does not already accept. Optional cleanup: drop the `within(..., table_bytes)`
  clause and let the `Table` interval decide. Not filed as a defect.
