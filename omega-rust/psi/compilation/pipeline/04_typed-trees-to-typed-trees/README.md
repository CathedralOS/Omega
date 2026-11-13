# Typed preparation (Psi 04)

Every program's typed trees cross this stage on their way to the checker, by
one path: [`prepare_typed_trees`](src/preparation.rs) runs an explicit, ordered
list of passes over the typed trees and freezes the result. There is no way to
freeze a program without running the list.

A pass takes typed trees and returns typed trees. It either does its work or
refuses, by name, a request it does not serve
(`no product route yet: typed preparation: <pass>: <request>`). A pass with
nothing to do for a program changes nothing; that is not a reason to skip the
stage. What a pass writes is input the checker reads. No pass judges a
program, and nothing here is an acceptance.

## The list today

| Pass | What it writes | Who reads it |
| --- | --- | --- |
| [record patterns](src/passes/record_patterns.rs) | For `let { a, b as _ } = subject;`, the subject's record symbol on the marker local and each named field's symbol on its roster row | Structural elaboration (`read/places.rs`, `record_pattern`), which refuses a marker without them |

The record-pattern pass resolves a subject that names a parameter or a typed
local whose declared type is a plain, non-generic record. A field path, a call
result, an applied generic record, a quotient and a sum stay unresolved, and
the checker refuses the pattern where a program reaches it.

## What is not here yet

The deleted preparation ran about forty passes of five kinds. Three kinds are
this stage's work and return as passes: annotations the checker reads,
specialization of generic machines and requirement applications, and selection
(operator calls, authored selections, calls bound through a value receiver).
Only the first annotation pass exists. Until the others are rebuilt, the
requests they would serve stay in the trees as typing left them, and
structural elaboration refuses each one by name where a program demands it
(`generic application`, `receiver call`, `static requirement dispatch` and so
on).

Two kinds do not return here. The old checker's judgments (wire protocol,
assembly discharge, domain membership carriers, mathematical declarations)
belong to the lowered-form checker. Evaluation is carried as a request and
refused downstream until the route's evaluator serves it.

Two annotations of the deleted stage have no reader on this route and were not
rebuilt: an assignment's receiving type and a local home for an assigned
parameter. Structural elaboration types an assignment and homes a binding from
its own slots.

Requested work beside the program (deferred layout applications, opaque
representation selections, wire compatibility demands) has no pass and is
refused by name. [Product operand preparation](src/product/mod.rs) keeps its
request shapes and refuses every operand.

This stage sees the program together with the whole bundled library and has no
notion of what an entry demands. A pass that refuses therefore refuses every
program, including ones that never reach the construct. A pass whose requests
occur in the library must either serve them all or leave the refusal to the
reader of the declaration.
