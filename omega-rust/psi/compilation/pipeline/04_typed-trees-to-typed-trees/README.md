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
| [specialization](src/passes/specialization.rs) | For a call that applies static arguments (`relay<u8>(..)`, `advance<Hold>(..)`): which parameter each argument is for, on the call's application; and, when the arguments are closed, the machine the application selects, generated from the generic machine with each parameter replaced, which the call then enters | Structural elaboration: an open application in a generic machine is read from its bindings (`read/calls.rs`, `type_arguments`), a generated machine as any closed machine, and its declaration states the generic machine and the application (`declare.rs`, `application`) |
| [record patterns](src/passes/record_patterns.rs) | For `let { a, b as _ } = subject;`, the subject's record symbol on the marker local and each named field's symbol on its roster row | Structural elaboration (`read/places.rs`, `record_pattern`), which refuses a marker without them |

Specialization serves type parameters given plain named types (a builtin
type, a data declaration with no parameters, a type parameter in scope) and
implementation parameters given plain named implementations (one with no
parameters, of exactly the bound's trait, or an implementation parameter in
scope), on a generic machine of one state with a body. Both kinds are bound
through one operand shape. In the machine generated for an implementation,
each call through the parameter enters the machine the implementation selects
for that requirement and records the requirement it was written against
(`StaticRequirementDispatch`), which structural elaboration reads
(`read/calls.rs`, `requirement_call`). Two names of one implementation value
are one application. A call that names a trait's requirement and applies the
requirement's own parameters (`Probe::take<Reading>(..)`) is served the same
way, except that a requirement has no body to copy: the application selects
the requirement's signature and contract at the arguments
(`TypedTrees::instantiate_requirement_application`), one closed requirement
per application, which the call then names and structural elaboration reads
as any requirement (`read/calls.rs`, `boundary`). A machine argument, an applied type such as
`Pair<u8>` or an applied implementation, a requirement with parameters of its
own, and a generic machine that owns data or declares lifetimes are not served: the
call stays as typing wrote it and the machine holding it carries a deferred
refusal (`DeferredRequest::StaticApplication`), raised only when a program
demands that machine. The pass generates what applications ask for and judges
nothing: the lowered-form checker proves a generic machine once as written
and re-decides those proofs in each generated machine.

A generic machine's lexically nested checked-body machines are copied with
its closed application when they have no independent static telescope. The
whole tree is validated first, then each machine, state, formal and local is
given a fresh symbol before any body is rebound. A nested declaration retains
its own source template and the actual enclosing application's static bindings;
two outer applications therefore cannot share a helper's identity or signature.
Calls between the copied declarations use that one symbol map. Runtime values
still require explicit arguments; no enclosing activation is captured. A child
with its own static parameters is left unserved with the whole tree.

The record-pattern pass resolves a subject that names a parameter or a typed
local whose declared type is a plain, non-generic record. A field path, a call
result, an applied generic record, a quotient and a sum stay unresolved, and
the checker refuses the pattern where a program reaches it.

## What is not here yet

The deleted preparation ran about forty passes of five kinds. Three kinds are
this stage's work and return as passes: annotations the checker reads,
specialization of generic machines and requirement applications, and selection
(operator calls, authored selections, calls bound through a value receiver).
The first annotation pass and specialization by type arguments exist. Until
the others are rebuilt, the
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
