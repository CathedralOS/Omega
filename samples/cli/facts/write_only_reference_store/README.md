# Replacing a stored reference through write-only access

Each program replaces a reference stored in a record through `&write`, which
permits writing the destination without reading its old contents. The new
reference must point to a value of 85. The previous referent must remain 11,
and the neighboring stamp must remain 17.

Fact: a `&write` store into a record's reference leaf installs the incoming
descriptor — the stored target reads the new referent's 85 while the displaced
referent's 11 and the sibling stamp's 17 stay untouched.

The owning tasks are **WRITE-ONLY-BORROW**, for replacing stored references
without reading old contents or losing their borrow obligations, and
**BUILD-OWNED-TARGET-SELECTION**, for choosing the forwarding implementation
through explicit Build configuration.

`Held` contains `target: &Cell` and a neighboring `stamp: i64`. Each positive
program creates `held` pointing at `left`, carries a write-only view of `held`
through one place, and assigns `&right` to `slot.target`. It then reads the
record through its original owner and checks `held.target.value == 85`,
`left.value == 11`, and `held.stamp == 17`. The invalid program instead reads
`slot.target` through the write-only view to supply its own replacement.
That read must reject during source checking.

Acceptance is `omega run --both <place>/main.omg`: native execution and the
Terminal Psi interpreter must both exit 0. Each `invalid/main.omg` must reject
for observing write-only contents. The nine places are `local`, `field`,
`parameter`, `call_result`, `match_join`, `const_name`, `other_state` (a nested
machine), `other_machine`, and `target_body`. All four hosted `ProgramEntry`
bindings are present. Matching programs and reject controls live under
`tests/omega/{pass,fail}/facts/write_only_reference_store_through_<place>_{exit,rejected}`.

## What the row establishes

- **Establishment:** the incoming reference points to a valid `Cell`, and the
  write-only view covers the exact `Held` record being changed.
- **Identity and lifetime:** the view keeps the same destination through all
  nine places. A returned view still borrows the caller's record; it does not
  refer to a helper's temporary copy.
- **Composition:** ordinary helpers forward the view. The assignment replaces
  the stored reference without reading either referent or changing a neighboring
  field. Any still-live borrow obligations remain enforced.
- **Soundness:** the invalid programs read `slot.target` under write-only access
  and reject. Other controls reject readable widening and overlapping exclusive
  borrows. Forwarding does not add permission to read.
- **Psi lowering:** the checked program keeps the returned view's original
  destination and the old and new stored references' borrow relationships.
- **Layout, providers, and ABI:** the record has two ordinary fields. Calls pass
  the view using the ordinary pointer calling convention; Build chooses a
  checked implementation for each hosted target.
- **Instructions and frames:** the store changes the reference field's address
  without loading its old referent or overwriting `stamp`.
- **Native image and execution:** the emitted program must produce the same
  exit-0 result as Terminal Psi. Omitting the store, changing the wrong field,
  or returning a view of a copy makes the final check trap.

## Build-owned target selection

The `target_body` program passes the destination and incoming reference to
`SelectedStore::install`. Four explicit Build rows choose ordinary named
implementations for the four hosted targets. Each body performs the same
`slot.target = incoming` assignment. `Held<'source>` and the incoming parameter
share an explicit lifetime, so the contract keeps the new reference valid for
its use after the call. Build chooses code; it does not choose another
destination or grant readable access.

The selected body performs the store directly. A requirement that only returns
`&write Held` with an input-related lifetime does not promise that it returns
the same reference: a lifetime constrains validity, not the pointer's identity.
The earlier forwarding attempt and its refusals are retained with the PR
records, rather than presented as repaired by this migration. The ordinary
`call_result` place continues to test returning the original view through a
checked helper.

The compiler repair distinguishes the final field being replaced from pointer
reads needed to reach it. Locating that final field uses its receiver's existing
write permission; it must not activate a new permission for reading the field's
old reference. Intermediate pointer reads keep their existing checks. Unproved opaque boundaries
also remain rejected: a declaration without a checked body needs admitted
evidence that it will not observe write-only contents. The governing rules are
[write-only authority](../../../../wiki/spec/terminal-psi/structural_access.md#write-only-authority),
[returned-view lifetimes](../../../../wiki/spec/language/lifetimes.md#returned-views),
and [checked boundary providers](../../../../wiki/spec/terminal-psi/boundary_calls.md#checked-providers).

## Evidence and remaining scope

Existing row qualification records native and Terminal exit 0 for all nine
places on Linux x86-64, with all nine observation twins rejected. The new
Build-selected target-body migration has separate focused validation pending;
its run records and producer identities stay in the claim and PR evidence.
No execution on the other three hosted targets is claimed here.

This row establishes replacement of a stored reference through write-only
access. It does not establish whole-record replacement, readable widening,
loan splitting, opaque foreign outputs, or preservation of every routed domain
constraint. Those remaining requirements stay on **WRITE-ONLY-BORROW**; broader
source and library target migration stays on **BUILD-OWNED-TARGET-SELECTION**.
