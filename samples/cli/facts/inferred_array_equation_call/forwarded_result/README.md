# Inferred generic result composition

Fact: an omitted `Capacity` is inferred as 2 from the actual `[u8; 2]` type and `Length == [u8; Capacity]`, independently of element values.

This is another production of the parent fact, under
`STRUCTURAL-GENERIC-MATCHING`. The original direct-call row stays intact.
Every program feeds the transported array through `forward<Value>(value: Value)
-> Value` before calling `capacity`. The result is determined by the selected
call's declared argument/result relationship, without inspecting its body or
runtime array contents.

## Reproduction and execution

At baseline `036605466510d2361fd579cf9fd97b722e070beb`, an optimized producer
rejects the root's `capacity(forward(items))` with
`generic machine capacity cannot determine Length from its where equations`.
The equation reader discarded the open result `Value` before composing the
callee's inferred tuple. The repair uses the existing structural solver for
that tuple and reconstructs the declared result through the same type
constructor traversal. Ordinary typing still checks arguments, constraints,
properties, invocation requirements and ownership afterward.
An early result observation can remain underdetermined when the outer call
supplies its context. That case defers to ordinary inference; it is not a
contradictory equation. Unavailable static-machine telescopes and ordinary
reference-access compatibility likewise remain the normal checker's obligations.
The observer retains its diagnostic and explains it if the consumer also fails
to determine a tuple. Pinned outer-context, static-machine-binder and reference
compatibility positives protect these previously valid compositions.

Run normal package preparation, then the public entrypoint:

```sh
omega update --project samples/cli/facts/inferred_array_equation_call/forwarded_result/local --target linux_x86_64
omega run --both samples/cli/facts/inferred_array_equation_call/forwarded_result/local/main.omg
```

Every positive declares exit 0, observes capacity 2, and binds all four hosted
targets. The root and nine places executed on Linux x86-64 with native exit 0
and Terminal Psi exit 0. Other matching hosts are unverified by this run.
Every twin changes only the outer call's explicit capacity to 3 and rejects
with `where equation binds Capacity to 2 but its explicit argument is 3`.
Actual diagnostics quote the binder names with backticks.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

## Requirement evidence

- **Establishment:** the selected generic signature binds `Value` from the
  declared actual type; the equation then establishes `Capacity = 2`.
- **Identity and lifetime:** the same closed array type crosses each transport;
  no runtime index, lifetime erasure, mutation-derived inference or bounds-based
  specialization is introduced.
- **Composition:** generic result substitution supplies the consumer's exact
  type. Nested forwarding and a declared `[Element; Count]` generic result
  additionally exercise the shared constructor traversal.
- **Soundness:** explicit equation conflicts and repeated inferred-result type
  conflicts reject. A callee's explicit wrong type remains an ordinary argument
  rejection; reconstruction does not waive that check. A false callee
  `requires okay` still rejects as a structurally false invocation fact, even
  when the consumer supplies a complete tuple.
- **Psi 05–07:** the inferred closed tuple monomorphizes through the ordinary
  lowering route; the emitted Terminal Psi executes the authored exit oracle.
- **Omega 00–02:** normal Build entry settlement, two-byte array layout and
  argument/result placement execute on Linux x86-64.
- **Omega 03–06:** generic forwarding and the array-consuming call preserve the
  scalar result through register/frame realization.
- **Omega 07–09 and emission:** native ELF execution agrees with the interpreter.
  Foreign-target source bodies are checked; matching-host native runs remain
  unverified here.

The nine positive and nine negative programs are pinned as
`tests/omega/{pass,fail}/facts/inferred_array_equation_forwarded_result_through_*`.
The focused generic-result variations and their conflicting counterparts are
pinned under `tests/omega/{pass,fail}/generics/inferred_equation_*`.

Full parent acceptance remains open: open-schema/retained-extension inference,
late receivers, conformance/operator/machine/evidence/value binders, named
lifetimes and unspecialized disjunctive body checking. The two named Std-backed
customers also reject on the recorded baseline at Std `FloatMeaning` ensures
contracts before reaching their authored program; this repair changes none of
those proof obligations.
