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
