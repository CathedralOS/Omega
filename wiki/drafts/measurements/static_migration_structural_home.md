# The static migration needs a realizable home for structural values

**STATIC-MACHINE-MIGRATION** moves every program off machine/state `self`
receivers. Ten samples have migrated and run. The remaining sample work is
mostly blocked behind one missing capability, and this file records exactly
what it is so the next slice is not spent rediscovering it.

## What was measured, 2026-09-29 at `5685b2942c5`

Under a receiver, a structural value lives in a field of the entry's data and
the call reads `self.line`. That realizes today. Under the static contract the
data is gone, so the same value must live somewhere else — and **none of the
alternatives realize.**

A three-line probe, identical except for where the carrier lives:

| where the carrier lives | `--check` | native |
| --- | --- | --- |
| field of the entry data (`self.line`), i.e. today's form | passes | **realizes** (`text/text_padding` runs) |
| ordinary local (`let mut line: [u8; 2] in Utf8 = "ab";`) | passes | `InvalidUnitMachinePlan` |
| state parameter (`state show(line: [u8; 2] in Utf8)`) | passes | `InvalidUnitMachinePlan` |

Every failing case stops at the same phase, `call operation: structural
arguments: caller structural result`. So the refusal is not about locals
specifically: it is that a structural value has no realizable home other than
a data field once it is handed to a machine.

## The same wall, from three directions

| customer | argument | result |
| --- | --- | --- |
| `dutch_flag` (board note) | `&mut pause`, a local `[u8; 256]` to `Console::read_line` | `UnknownStructuralArgumentPlace` |
| `generic_counters` | `&mut score`, a whole local record | stops at `structural arguments: caller structural result` |
| `nested_diagnostics` | `&dash.engine`, a **shared** borrow of a nested member of a local | the same stop |
| this probe | a carrier passed by value to `Console::write_line` | the same stop |

The board's note describes the mutable case. `nested_diagnostics` shows the
gap is **not specific to mutable arguments, nor to whole locals** — a shared
borrow of a projection hits it too, and so does an ordinary by-value carrier.
Whoever completes the claimed `SCALAR-CALL-ELEMENT-WINDOW` transport should
cover all four shapes, not only `&mut` whole-local views.

## How much rides on it

Of the **136** samples still carrying a receiver, **119** hand a structural
value to a machine — a printed carrier, a `read_line` buffer, or an explicit
`&`/`&mut` argument. Those cannot complete the migration until a structural
value has a home outside a data field.

**17** remain that do not, and two of those (`generic_counters`,
`nested_diagnostics`) are already proven blocked at the deeper level above.

## Sequencing

Migrating a blocked sample anyway would trade a program that runs today for
one that only checks. The board says not to demote coverage to avoid exposing
a gap, and this does not: `dutch_flag` already carries this gap as a tracked
customer, so a second red sample adds no coverage and costs a working program.

The order that keeps programs running: land the structural-argument transport,
then migrate the 119 in bulk. Until then the migration advances on canaries
and on samples whose state is entirely scalar.

## Three distinct blockers, not one

Measured 2026-09-29 while migrating canaries. Each has its own signature, and
conflating them would send a repair at the wrong layer.

| # | trigger | signature | evidence |
| --- | --- | --- | --- |
| 1 | a structural value handed to a machine from anywhere but a data field | `InvalidUnitMachinePlan`, stopping at `call operation: structural arguments: caller structural result` | the carrier probe above; `generic_counters`; `nested_diagnostics` |
| 2 | a static entry that applies a boundary operator | `BoundaryRealizationMismatch(BoundaryMachineId(1))` | `pass/providers/external_boundary_requirement_via_exit` checks with a receiver, rejects as a free machine |
| 3 | a checked canary whose data held only the Console binding | native `Lowering(Unsupported("composed Unit control has unsupported graph custody"))` | `pass/arithmetic/runtime_integer_casts_exit` |

Blocker 3 is worth stating carefully, because the obvious reading is wrong.
The migration is not a checking failure: after it, `--check` passes on
`macos_arm64`, `linux_x86_64` **and** `windows_x86_64`. With the receiver the
fixture reports `native exit 70 / interp exit: 70`; migrated, native
compilation fails outright. Thirteen of eighteen fixtures in that class
regressed the same way and none was landed.

Nor is it "static entries cannot call Console": `samples/cli/collections/slice_maximum`
already uses `machine main()` with `Console::write_line` and
`Console::exit_process` and realizes natively today. Something narrower about
this shape is unsupported, and it was not isolated further here.

Nor is it the entry shape. `entry_roots.md`'s `NoneOrProvisionedZii` "admits
an entry without a context or with one ordinary `context: &mut Context`
parameter", so a parameterless `machine main()` is admitted by contract.

## A hosted entry cannot take a context parameter, so entry state has nowhere to go

This is the reason only entries with an EMPTY data have migrated, and it is a
fourth blocker distinct from the three above.

`entry_roots.md` describes `NoneOrProvisionedZii` as admitting "an entry
without a context or with one ordinary `context: &mut Context` parameter".
Taking the second option on a hosted entry is refused:

```
machine main(context: &mut Main) reaches Console { ... }
  error: hosted `ProgramEntry` exposes no arrival parameters, but `main` declares `context`
```

So for a hosted `ProgramEntry` the entry must be parameterless. An entry that
holds state therefore has only one place left to put it: ordinary locals.

**And that is not a mechanical rewrite.** Measured over 35 canaries with one
to four plain fields, a single body and no console binding: 3 refused the
rewrite, and of the 32 that applied, **24 moved** — 15 regressed outright and
9 moved their diagnostic. One stayed checked. All were reverted.

A field and a local are not interchangeable to the checker. A field is
ZII-established and owned by the data; a local is not. Ownership, loan and
establishment facts differ, and those facts are exactly what many fixtures
pin.

### What this leaves

| migration | mechanical? | landed |
| --- | --- | --- |
| empty `data Main` + entry receiver | yes | 98 canaries |
| operation that never reads its receiver | yes | 25 canaries |
| operation that does read it, named as a parameter | yes | 11 canaries and packages |
| entry holding state | **no** | blocked above |

Either the hosted entry shape gains the provisioned context parameter its own
spec describes, or every stateful entry is a per-fixture rewrite whose subject
must be read first. The first would unblock the bulk mechanically; nothing
below it will.
