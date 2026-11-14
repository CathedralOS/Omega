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
scratch storage, field storage and work. Runtime checks observe bytes
`[0, 0, 200, 1]` as well as the original cursor and indexed result. The match
witness runs both choices and observes an independent path marker.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [overlap](invalid_local/main.omg) |
| field | [field](field/main.omg) | [overlap](invalid_field/main.omg) |
| parameter | [parameter](parameter/main.omg) | [overlap](invalid_parameter/main.omg) |
| call result | [call_result](call_result/main.omg) | [overlap](invalid_call_result/main.omg) |
| match join | [match_join](match_join/main.omg) | [overlap](invalid_match_join/main.omg) |
| const name | [const_name](const_name/main.omg) | [overlap](invalid_const_name/main.omg) |
| a nested machine | [other_state](other_state/main.omg) | [overlap](invalid_other_state/main.omg) |
| another machine | [other_machine](other_machine/main.omg) | [overlap](invalid_other_machine/main.omg) |
| another target body | [target_body](target_body/main.omg) | [overlap](invalid_target_body/main.omg) |

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

[Run 41620319587](https://github.com/CathedralOS/Omega/actions/runs/41620319587)
passed on optimized source `9637a7912c043bc9c8c27d151d8953c35b942516`.
All nine positive fixtures checked, all nine twins rejected their intended
stale bound, and all nine pinned native/Terminal comparisons agreed on exit `0`.
The 18 public place/twin projects produced nine explicit engine agreements and
nine intended rejections. Root `main.omg` passed ordinary CLI admission and both
engines at exit `0`; all 234 copied replay-input hashes match the measured source.

Source checks use macOS arm64; runtime observations use Linux x86_64.
The exact measured source archive is public in
[part 1](https://github.com/CathedralOS/Omega/pull/1662#issuecomment-6602051584),
[part 2](https://github.com/CathedralOS/Omega/pull/1662#issuecomment-6602052751), and
[part 3](https://github.com/CathedralOS/Omega/pull/1662#issuecomment-6602053934).
The posted bytes were retrieved and verified. The archive preserves source;
the run’s engine observations supply runtime evidence.

## Source migration and remaining work

`STATIC-MACHINE-MIGRATION` owns the client helper correction. The examples use
free `packet_fields` and `encode_packet` machines, rather than declaring code
under an empty data type. Explicit arguments, encoding bodies and runtime checks
are retained. The [machines contract](../../../../wiki/spec/language/machines.md#static-declarations-and-explicit-state)
requires this separation; earlier passing engines did not ratify the retired
helper spelling.

Original source at the earlier published base
`e4617438fb66df39eeb1fa6605b8fcacba19df62` used implicit `Packet::encode`.
[Baseline run 41608807527](https://github.com/CathedralOS/Omega/actions/runs/41608807527)
refused that missing callable before any engines or intended index checks.
[PR 1655](https://github.com/CathedralOS/Omega/pull/1655) migrated to the real core
codec and explicit target selection. Its successful executions retain their
historical source identities; the fresh run above validates the corrected free
helper declarations.

The existing public `CompactBinary::encode` and `CompactBinary::scalar_field`
entries in `source/library/core/wire.omg` still use the library’s older
qualification. That is separate migration debt under the same task, with no
syntax exemption or full library-closure claim. This client adds no such
declaration and leaves that public API unchanged.

Other runtime targets, the wider target-source/static-machine migrations, and
the codec library’s general schema agreement, decoding and customer acceptance
remain open.
