# Codec writes through a stored reference

Encoding through an owned reference field preserves a disjoint index bound.

The codec encodes `200` and writes cursor `4` through `Counter.slot`. The other
input stays at `1`, below the array bound `4`, so indexing `[4, 7, 8, 9]` reads
`7`. Every positive exits with `0`. Each twin points the cursor at that index
instead; its earlier bound is stale and the index operation must reject.

`BUILD-OWNED-TARGET-SELECTION` in `TASKS.md` owns this row’s source migration.
One ordinary `StoredCursorProbe` implementation replaces four identical
platform-prefixed bodies. Four explicit Build rows select it. The public
requirement retains both original input bounds; the caller receives a byte,
not a new promise about reference aliasing or the cursor’s value.

The value owner supplies the free `packet_fields` visitor for its numbered field.
The free `encode_packet` machine adapts the existing core-library entry.
The core `CompactBinary` library performs the encoding with explicit output,
scratch storage, field storage and fuel. Runtime checks observe bytes
`[0, 0, 200, 1]` as well as the original cursor and indexed result. The match
witness runs both choices and observes an independent path marker.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [overlap](local/invalid/main.omg) |
| field | [field](field/main.omg) | [overlap](field/invalid/main.omg) |
| parameter | [parameter](parameter/main.omg) | [overlap](parameter/invalid/main.omg) |
| call result | [call_result](call_result/main.omg) | [overlap](call_result/invalid/main.omg) |
| match join | [match_join](match_join/main.omg) | [overlap](match_join/invalid/main.omg) |
| const name | [const_name](const_name/main.omg) | [overlap](const_name/invalid/main.omg) |
| a nested machine | [other_state](other_state/main.omg) | [overlap](other_state/invalid/main.omg) |
| another machine | [other_machine](other_machine/main.omg) | [overlap](other_machine/invalid/main.omg) |
| another target body | [target_body](target_body/main.omg) | [overlap](target_body/invalid/main.omg) |

Root [main.omg](main.omg) carries the same operation. Each positive binds all four
hosted `ProgramEntry` targets. Matching fixtures live under
`tests/omega/{pass,fail}/facts/may_write_codec_through_<place>_{exit,overlapping}`;
their source and Build files match the samples exactly.

Run `omega run --both samples/cli/facts/may_write_codec/<place>/main.omg`.
Each twin is intended-invalid and requires its authored `cannot prove` fragment.
No caller equality guard establishes the index bound after encoding.

## Requirements and evidence

1. **Establishment.** The independent index starts at `1`, or arrives under
   `other < 4`, before encoding. The codec’s selected cursor is established by
   the owned `Counter.slot` reference; reflection supplies no borrow authority.
2. **Identity and lifetime.** The cursor writes its captured referent. The other
   input retains its own storage identity. Output and scratch loans remain
   disjoint, and no reference to local codec context escapes.
3. **Composition.** The preserved bound feeds a local, field, parameter, returned
   value, match join, constant bound, nested machine, other machine or selected
   implementation. Both match choices must keep the same index fact.
4. **Soundness.** Redirecting the cursor invalidates the index bound. Unknown
   writes remain conservative; the public requirement does not expose a
   selected implementation’s private guarantees.
5. **Checked-to-Terminal Psi.** The authored visitor and library codec use the
   ordinary checked call and reference paths. Mathematical and type information
   is erased while the real encoding and cursor write execute.
6. **Provider selection and calling plans.** Four target-keyed Build rows select
   one checked implementation. Shared input, exclusive output/cursor, slices and
   result bytes retain their declared access and ordinary calling placements.
7. **Instructions and storage.** Native code must encode `200`, write cursor
   `4`, and reload the untouched input for indexing.
8. **Image emission and engines.** Both Terminal and native execution must agree
   on the authored exit `0`, including byte checks and both match choices.

## Validation

The original programs and oracles are frozen against published base
`e4617438fb66df39eeb1fa6605b8fcacba19df62`. README-only source
`d0c7f056bee383c45a8836a63cefa796d7679219` has the same compiler and library.
[Baseline run 41608807527](https://github.com/CathedralOS/Omega/actions/runs/41608807527)
refuses every place because legacy `Packet::encode` has no callable declaration.
The twins stop at the same formation error; that is no evidence of their index
obligation. No engines or public projects ran.

The accepted [codec rules](../../../../wiki/spec/layouts/codecs.md) require ordinary
checked library bodies and owner-authorized reflection. The migrated sources
use that idiom, with stable field number `0` on the actual visited value.
[Build-owned selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
selects the ordinary probe, and [mutation contracts](../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity)
require preserving the exact disjoint storage fact. Compiler code is unchanged.
The complete row is validated by two measured revisions. At
`6ab94efb1d699133108fa31bd704636c9613b4b9`,
[run 41612327184](https://github.com/CathedralOS/Omega/actions/runs/41612327184)
checked eight positive places and their eight intended bound rejections, then
observed native/Terminal exit `0` for those eight positives. Its target project
stopped at an authored Build syntax error; that refusal is not fact enforcement.

Only the four target Build files changed afterward, to the documented
`Build::select_provider` operation. At `77d5742e003ce607ca3856dfa76cb17d0679aa17`,
[run 41614342092](https://github.com/CathedralOS/Omega/actions/runs/41614342092)
checked the corrected target, rejected its stale-bound twin, and observed target
native/Terminal exit `0`. It also ran all 18 public place/twin projects: nine
positive engine agreements and nine intended rejections. The root passed
ordinary CLI admission and both engines at exit `0`. Independent review accepts
reuse of the unchanged eight corpus projects at their original source stamps.

Source checks use macOS arm64; all runtime observations use Linux x86_64.
The exact source archive, including both measured revisions, is published in
[part 1](https://github.com/CathedralOS/Omega/pull/1655#issuecomment-6600701813),
[part 2](https://github.com/CathedralOS/Omega/pull/1655#issuecomment-6600702345), and
[part 3](https://github.com/CathedralOS/Omega/pull/1655#issuecomment-6600702935).
The posted bytes were retrieved and verified against the retained bundle.
Source retention and actual engine observations are separate evidence.

Other runtime targets and the wider target-source migration remain open. This
row does not close the codec library’s general schema agreement, decoding or
wire-protocol customer acceptance.

## Free-helper source migration

`STATIC-MACHINE-MIGRATION` also owns removing the newly introduced helper
namespace from these examples. The client now declares ordinary free
`packet_fields` and `encode_packet` machines, rather than declaring code under
an empty data type. Their explicit arguments, encoding bodies and runtime checks
are unchanged. The [machines contract](../../../../wiki/spec/language/machines.md#static-declarations-and-explicit-state)
requires this separation; passing engines do not make the retired spelling valid.

The existing public `CompactBinary::encode` and `CompactBinary::scalar_field`
entries in `source/library/core/wire.omg` still use the library’s older
qualification. That remains separate library migration debt under the same
task, with no syntax exemption claimed. This client correction adds no such
declaration and leaves the existing library API unchanged. Fresh validation of
the renamed helpers is pending; the earlier results above retain their original
source identities.
