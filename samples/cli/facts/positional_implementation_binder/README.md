# Positional trait implementation binder

**Fact and operation:** Reader in the middle slot of Carrier<u64, Reader, i32> supplies at least seven before exact value - 7.

This row advances `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION` in
[TASKS.md](../../../../TASKS.md). Its distinct mechanism is one positional
evidence binder inside an ordered mixed trait telescope. `Reader satisfies
Reading` occupies the middle slot of `Carrier<Mark, Reader satisfies Reading,
Tag>`. The binder names an implementation. It is not a data subject, `Self`,
or a bodyless map. `Chosen satisfies Carrier<u64, Seven, i32>` is an empty
conformance: the trait default calls `Reading::floor<Reader>()`, and
instantiation substitutes `Seven`. The consuming operation is `value - 7`.

The witnesses are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[another state](other_state/main.omg), [another machine](other_machine/main.omg),
and [another target body](target_body/main.omg). Each has
`invalid/main.omg` beside it. Every twin changes only `Seven` to `Other`.
`Other` satisfies `Loose`, whose `floor` ensures `result >= 6`, and does not
satisfy `Reading`. The required minimum, the default call, and the consuming
subtraction stay the same. The [root](main.omg) copies the local transport.
Each positive binds all four hosted `ProgramEntry` targets and expects exit 0.

Applicable requirements:

- **Establishment:** `Seven satisfies Reading` and `Reading::floor`'s
  `ensures result >= 7` establish the bound. The trait default calls through
  the positional binder. A caller guard does not supply the subtraction premise.
- **Identity and lifetime:** `Seven` and `Other` are distinct named
  implementations. Substituting one for the other changes the trait the
  default call must satisfy. No data value or `Self` subject is introduced.
- **Composition:** the same binder, bound, and `value - 7` operation are
  carried through a local, a field, a parameter, a call result, a match join,
  a const name, another state, another machine, and another target body.
  Each twin differs only in the implementation supplied to that binder.
- **Soundness boundaries:** an implementation of `Loose` is not an
  implementation of `Reading`. The default call `Reading::floor<Other>()`
  is rejected. Data, conformance, domain, proposition, and machine telescopes
  still do not admit an evidence binder.
- **Psi 05–07:** the default call, the substituted implementation, and the
  scalar subtraction reach Terminal Psi and execute there.
- **Omega 00–02:** the result is an ordinary `u64`. This row adds no new ABI
  placement lane.
- **Omega 03–06:** scalar subtraction and the selected `floor` body keep the
  authored result.
- **Omega 07–09 and emission:** the native image exits 0 and agrees with
  Terminal Psi on the host that was run. Declaring the other three hosted
  targets does not execute them.

The governing clauses are
[conformances](../../../../wiki/spec/language/conformances.md) and
[generics](../../../../wiki/spec/language/generics.md).

Linux x86-64 `omega run --both` reports native exit 0 and interpreter exit 0
for the root and all nine places. All nine `<place>/invalid` twins reject.
The diagnostic fragment is `` `Other` implements `Loose<>`, which neither is
nor inherits `Reading` ``. Windows, macOS, and QEMU were not run.

The nine positive and nine reject mirrors are byte-identical to the place
programs and their Build pairs:

- `tests/omega/pass/facts/positional_implementation_binder_through_<place>_exit`
- `tests/omega/fail/facts/positional_implementation_binder_through_<place>_rejected`

This row removes product-producer parity for positional trait implementation
binders and their ordered mixed telescopes. It does not close the parent.
Const-expression closure and package projection outside the reference
qualified scalar inline member path, calling-policy and member kinds beyond
the closed inline path, machine evidence telescopes in the product, inline
reference-row migration, and cross-host execution remain open.
