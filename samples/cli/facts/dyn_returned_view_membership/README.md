# Dynamic returned view membership

**Fact: A selected implementation returns one of its Utf8 inputs, and the returned view keeps the Utf8 membership its consumer requires.**

Owning task: **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** in
[TASKS.md](../../../../TASKS.md), the Omega realization and native execution
requirement. Its acceptance runs `--filter traits/`, which includes
`tests/omega/pass/traits/runtime_dyn_qualified_byte_result`, red at base on
every target. Contract:
[dynamic dispatch](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#call-and-forwarding-roles)
(structured results keep the requirement's exact type and qualifications) and
[byte views](../../../../wiki/spec/terminal-psi/byte_views.md).

`Payload::window` takes two `&[u8] in Utf8` views and returns one in `Utf8`.
`First` returns its first input and `Second` its second. The caller passes the
returned view to `classify`, which requires `Utf8` and compares contents:
`"é"` gives 2, `"abc"` gives 3, anything else 9. `First` must give 2 and
`Second` 3; a swapped row, a wrong descriptor pointer or a wrong length traps.

## Places

The places vary where the returned view comes from inside the selected row.

| Place | Returned view | Positive | Reject twin |
| --- | --- | --- | --- |
| local | a local alias of the input | [main](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | a field of a local record holding the input | [main](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | the input parameter itself | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | the result of `keep`, which hands back its argument | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | a match joining both inputs; `First` takes the false arm, `Second` the true arm | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| const name | **open**: see below | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | a nested `deliver` machine's local alias | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| another machine | `relay`'s local alias | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| selected implementation | Build selects `Selected` for `Reader`; it returns the dispatched view through a local alias | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

The const-name place does not carry the fact. A constant cannot hold the
returned view: at base `const WORD: &[u8] in Utf8 = "abc";` rejects with
`constant type must permit copying and contain no cleanup, shared ownership,
or interior mutability`. Its program therefore keeps the local-place alias and
only names the selectors as constants. It stays as selector-constant coverage;
the row is filled in 8 of 9 places until a constant form can supply a view.

Each twin removes only `in Utf8` from the requirement's result and from both
implementations' results. Eight twins reject at `classify`
(`cannot prove requires contract for call classify ... in [u8]::Utf8`); the
selected-implementation twin rejects at `Selected::read`'s qualified local
(`cannot prove initializer of bytes ... in domain [u8]::Utf8`). Positives are
pinned as `tests/omega/pass/facts/dyn_returned_view_membership_through_<place>_exit`
and twins as `tests/omega/fail/facts/dyn_returned_view_membership_through_<place>_rejected`.

## Nearest row

`dyn_dispatch_selector_payload_bound` and `dynamic_trait_dispatch` carry a
scalar result bound and vary where the selector lives. This row returns a
two-word view descriptor, varies where the returned view comes from, and its
twins name the returned view's `Utf8` membership. The repaired layer is Omega
02 descriptor return lowering, which neither scalar row reaches.

## Evidence

Base `4f87c9e08b1`, macOS x86-64: seven programs (local, field, match join,
const name, nested machine, another machine, selected implementation) refuse
native lowering with `descriptor return source has no matching live carrier`;
parameter and call result already pass. All nine twins reject.

Omega 02 compared the source home's root memberships with the declared
result's. A local alias carries its occurrence memberships plus the checker's
decision (two domain rows for one declared), and a field projection carries
none at the root. Psi already proved the declared memberships at the return,
and the literal-view arm and owned homes never compared them. The descriptor
arms now choose the carrier by structural type and multiplicity.

Requirements:

- Establishment: the string literals establish `Utf8`; each implementation's
  declared result carries it to `classify`. No caller guard adds it.
- Identity: each row returns its own input, so the selected table row and the
  descriptor contents are observable as 2 or 3.
- Composition: the join executes both arms across the two rows; the field
  place keeps its marker beside the view.
- Psi 06-08: `--both` runs the Terminal interpreter on every positive.
- Omega 00-09: the returned descriptor uses the two-word result placement and
  executes natively on macOS x86-64.

Open: the const-name place (above). Windows, Linux and macOS arm64 native
execution is not observed by this row's producer run; every positive binds
those entries.
