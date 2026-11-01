# Stored shared byte-view element lend

An element of a fixed array of two shared byte windows keeps its backing and
one-byte extent when lent to a callee, so the callee reads byte 11 from the
first window and byte 29 from the second.

Owning task: **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**. The requirement
is the Terminal interpreter's custody of a shared byte descriptor that is copied
out of an aggregate. Its acceptance fixture `traits/runtime_dyn_byte_array_result`
(a `dyn` call returns `[&[u8]; 2]`, then the first window is lent to a reader)
ran natively with exit 0 and was declined by the Terminal interpreter with
``StructuralArgumentPreparation ... prepare byte backing loan:
VerifiedOperationMalformed``. `dyn` is not needed to reach it: a local array of
two windows does the same.

Before the repair, copying `windows[0]` into a fresh place left that place with
two custody records for one descriptor: the frame binding every copied view gets,
and a copy of the array's stored descriptor row, keyed at the new identity and
stamped with the element's type. A call presents the same identity under its
formal's type. The binding lookup consults a stored row at the selected identity
before the frame binding, found the row, saw a different type, and refused
instead of rejoining the frame binding. The same copy out of a record's view
field never created a row, which is why `traits/runtime_dyn_byte_record_result`
already ran.

`ConstructorStorage::stage_subtree` now leaves a shared descriptor row behind
when it copies to a whole-home destination, because the copy's producer supplies
that place's frame binding. Rows keep their meaning for descriptors that live in
an aggregate, and a move (custody transfer) still relocates them.

Every place builds the same packet `[11, 23, 29, 31]`, the windows
`packet[0..1]` and `packet[2..3]`, and lends the first element through a copied
local and the second directly to `read(bytes: &[u8]) -> u8 requires bytes.len == 1`.
`main` crashes unless the two reads are 11 and 29, so a lend that reached the
wrong window, or the wrong extent, changes a byte. Only the transport of the
array changes.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [call result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [match join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [const name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [other state](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [other machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| another selected implementation | [target body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The field place stores the array in `Bundle`; the parameter place passes it to
`observe`; the call-result place returns it from `relay`, whose `ensures` names
both windows' extents; the const-name place indexes by named constants; the
nested and other-machine places pass it to a machine that states
`requires windows[0].len == 1 && windows[1].len == 1`. The match-join place
settles a flag through `&mut` so the checker cannot fold the `match`; its second
arm holds the windows `packet[1..2]` and `packet[3..4]`, which read 23 and 31, so
a join that took the wrong arm crashes `main`. The target-body place calls the
boundary requirement `Observe::observe`, backed by four named providers with
different binder spellings, one selected per hosted target through `Build` rows.

Each twin changes one window bound: `packet[0..1]` becomes `packet[0..2]`, and in
the match join the second arm's `packet[1..2]` becomes `packet[1..3]`. The array
literal alone proves the callee's `requires bytes.len == 1`, so the lent window
carries its extent from the literal with no caller guard. With a two-byte window
the checker refuses ``cannot prove requires contract`` at the call that lends it
(`read`, or `observe`, `inspect` and `forward` where the array crosses a machine
boundary first).

On Windows x86_64, with the optimized producer built from `83ae4b83d7b` plus this
change, all nine positives report native exit 0 and interpreter exit 0 from
`omega run --both`, `traits/runtime_dyn_byte_array_result` now agrees, and all
nine twins reject with the pinned fragment. The same tree with the change
reverted, built separately and optimized, ran all nine positives natively with
exit 0 and the Terminal interpreter declined each with the ``prepare byte backing
loan`` refusal above; the twins rejected identically, since the checker is
untouched. Pinned fixtures:
`tests/omega/pass/facts/stored_shared_byte_view_element_lend_through_<place>_exit`
and `tests/omega/fail/facts/stored_shared_byte_view_element_lend_through_<place>_rejected`.
Linux, macOS, and QEMU were not run; the other hosted targets are bound, not
executed, and only the Windows provider of `Observe` executes here.

- **Establishment:** the array literal establishes each window's extent and the
  callee's `requires bytes.len == 1` is proved from it at the call. No caller
  guard re-establishes it.
- **Identity and lifetime:** each element is its own descriptor over the packet's
  bytes. The second lend reads `packet[2]`, not `packet[0]`, and the neighbors 23
  and 31 are never read.
- **Composition:** every place hands the array, or a copy of one element, to the
  call through a local, a field, a parameter, a call result, a match join, a named
  constant index, a nested machine, another machine, or a Build-selected
  implementation.
- **Soundness:** a window longer than the callee requires is refused before
  execution in every place.
- **Psi 05–07 through Omega 07–09:** the lend lowers to the same call on both
  engines and they agree on the byte read. A lend that reached the wrong window
  leaves a different byte and `main` crashes.

Open: the sibling row `stored_exclusive_byte_view` reads exclusive windows in
place and does not lend them, so lending an element of an array of exclusive
windows is not covered. Moving a window out of an array, rather than copying it,
still relocates its row and is not exercised here.
