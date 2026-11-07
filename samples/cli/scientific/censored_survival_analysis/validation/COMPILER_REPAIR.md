# Contracted collection measurements

This workload now exercises a general compiler repair through its reusable
slice row accessor. A checked helper declares `ensures result == values.len`;
its caller captures the returned count and guards a variable index with it.
Previously this valid composition rejected with `cannot prove index ... within
unknown slice length`. The direct length guard checked successfully.
The retained before witness is `build/w2-04/probes/slice_row.before.stderr`;
its producer CLI SHA256 is
`ae780e51d7b32aa8dec24e2a497995d0c2eeded024bbeedd16010851a1835f40`.

The canonical [contract rule](../../../../../wiki/spec/proofs/contracts.md)
requires verified conclusions to retain exact argument/result substitution and
occurrence scope; intersecting writes invalidate revisioned facts. The repair
projects a selected declaration's unconditional builtin result/length equality,
then captures the measured collection descriptor and selector dependencies when
that actual argument finishes evaluation. Later argument and callee writes retire
the relation through the existing normalized write/alias owner. Neither a helper
name, its body, a same-spelled field, nor a private implementation establishes it.
Direct guards, locals, copied counts and scalar assignments consume retained
measurements; they never reread an initializer after effects.

Independent review found an unsafe provisional implementation: a later argument
could change the index selecting the measured slice. Both the inline-guard and
local-binding forms are now rejection controls. The retained result measurement
is captured before that sibling argument executes and retired on its selector
write. The existing descriptor, filesystem, proof and runtime authority checks
remain in force.

The focused executable `tests/omega/pass/collections/contract_length_results_exit`
checks nonempty and empty views, direct calls, copies, assignments, reversed
contract equality, reordered parameters, state transport and reuse after an
element write. Ten invalid controls cover false and conditional guarantees,
fake members, wrong subjects, missing preconditions, replaced counts/descriptors,
both later-argument selector writes, and private machine-binder strengthening. Existing captured length/range and
returned range extent controls are rechecked. Full native and captured Terminal
survival validation continues to use the unchanged independent rational and
event-subset enumeration oracles.

Producer identities, numerical results and final integration are recorded in
`producer.json`; raw source, runtime and numerical observations remain in the
supported recoverable evidence archive.
