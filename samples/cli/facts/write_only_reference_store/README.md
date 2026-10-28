# Write-only reference-carrying store

Fact: a `&write` store into a record's reference leaf installs the incoming
descriptor — the stored target reads the new referent's 85 while the displaced
referent's 11 and the sibling stamp's 17 stay untouched. Parent:
`WRITE-ONLY-BORROW` in `TASKS.md`, whose aggregate replacement must carry
conserved displaced custody without observing old contents through write-only
access. This row establishes replacement of a reference-carrying leaf through
write-only authority, not readable widening or loan splitting.

`Held` pairs a `target: &Cell` descriptor with a `stamp: i64` neighbor. Every
positive forms `held` pointing at `main.left`, transports a `&write Held`
through the place, then stores `Held { target: &main.right, stamp: 9 }` through
it; the runtime falsifier observes `held.target.value == 85` (from
`main.right.value + 80`), `main.left.value == 11` untouched, and `stamp == 9`
written beside the displaced leaf. The observation twin reads the write-only
referent (`slot.target`) to compute its own replacement and must reject at
checking.

Acceptance is `omega run --both <place>/main.omg` with native/Terminal exit 0,
plus refusal of each `invalid/main.omg` for write-only observation. All four
hosted `ProgramEntry` bindings are present. Places: local, field, parameter,
call_result, match_join, const_name, other_state, other_machine, target_body.

## Evidence and remaining scope

Optimized main `b45b43803e` refused the positive store as an unsupported
write-only projection and could not summarize `forward`/`lend` bodies that
return `&write Held`. The repair admits a displaced reference leaf as a
non-observing assignment target in `write_only_borrows.rs` (the incoming
descriptor installs its own backing loan; the displaced descriptor retires
through the retained replacement custody transaction) and exports argument
storage backing for write-only parameters in the descriptor-results replay:
a `&write` loan fixes its referent at formation, so `Parameter` origins naming
that storage are formation facts, not observations. All nine positives execute
with native and Terminal exit 0 on Linux x86-64; their nine observation twins
reject. Exact producer identities and raw runs are retained in the owning
qualification.

The row is pinned under
`tests/omega/{pass,fail}/facts/write_only_reference_store_through_<place>_{exit,rejected}`.
Adjacent controls under `tests/omega/fail/borrow/write_only_*` refuse readable
escalation, direct referent observation, and overlapping exclusive loans.
No execution on the other three hosted targets is claimed.

This closes reference-carrying leaf replacement under `&write`, not the parent
write-only capability: boundary declarations, opaque foreign outputs, and
routed-membership preservation remain open under `WRITE-ONLY-BORROW`.

## Required evidence

- Establishment: incoming `&Cell` validity, exact `Held` layout, write-only
  authority over the whole referent including its descriptor leaves.
- Identity/lifetime: retained `&write` identity across locals, fields,
  parameters, call results, match joins, constants, nested machines, other
  machines, and per-target bodies; the returned descriptor names the argument
  storage it was formed from.
- Composition: the same replace-and-observe sentence through all nine places;
  `forward`/`lend` are ordinary forwarding bodies, never special-cased.
- Soundness: the invalid twins read `slot.target` under `&write` and reject
  before lowering; no readable widening is admitted.
- Psi lowering: the returned `&write Held` carries a resolvable
  stored-reference result origin into the caller; the displaced `&Cell`
  retires without a contents observation.
- Layout/provider/ABI: ordinary two-field record geometry and pointer calls;
  four hosted entry bindings, with execution claims only for observed hosts.
- Instructions/frames: the store writes the new descriptor word beside `stamp`
  without a dead read of the old referent.
- Encoding/native: native exit 0 agrees with Terminal; the stored `&Cell`
  carries its referent address through call-result and join transport.

Return to `WRITE-ONLY-BORROW` acceptance after this prerequisite.
