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

A value parameter (`const Tag: u64`, `const Flag: bool`) is given an integer
literal, `true` or `false`. A number written as a calculation over literals,
`configure<(8u64 + 9u64)>(..)`, reaches this pass already as the literal of
its value: typing folds it where it reads the argument
(`typed_trees::const_integer::closed_literal`), so `17u64`, `(8u64 + 9u64)`
and `(20 - 3)` are one application. A calculation typing does not close (two
integer types, a result outside its type, a name or a call) stays a request
on the machine that holds the call.

A Build selection inside a generic helper,
`Build::select_provider<Reading<Tag>, FirstReading<Tag>>(builder, target)`, is
copied as written. Its operands are the request the call was authored with,
not static arguments of a machine to generate, so the generated helper holds
the same request, and the reader of the selection takes each parameter's
value from the generated machine's application
(`typed_trees::product_operand::capture_static_environment`).

Checked-body machines nested inside a generic machine are copied with its
closed application if they have no static parameters of their own. The pass
validates the whole tree first, then gives each machine, state,
formal and local a fresh symbol before rebinding any body. A nested
declaration keeps its own source template and its enclosing application's
static bindings, so two outer applications never share a
helper's identity or signature. Calls between the copies use that one symbol
map. Runtime values still need explicit arguments; no enclosing
activation is captured. If a child has its own static parameters,
the whole tree is left unserved.

A fully applied named map with a default type-only telescope uses the same
copier through [named conformance preparation](src/passes/specialization/named_conformances.rs).
Its call keeps the public requirement. An existing `CapturedMachineApplication`
keeps the actual constructor bindings, complete tuple and lexical owner,
mapped to the fresh body, states and formals. An unused binder
still belongs to that capture's source application. The copier keeps written
reach rows, `reaches none` included; installation-bound rows stay unserved,
as do captured members with nested declaration trees, map
lifetimes or non-type map binders. This metadata proves nothing about the
template.

The record-pattern pass resolves a subject that names a parameter or a typed
local whose declared type is a plain, non-generic record. A field path, a call
result, an applied generic record, a quotient and a sum stay unresolved, and
the checker refuses the pattern where a program reaches it.

## What is not here yet

The deleted preparation ran about forty passes of five kinds. Three kinds
belong to this stage and come back as passes: annotations the checker reads,
specialization of generic machines and requirement applications, and
selection (calls bound through a value receiver). Typing in
[Psi 03](../03_symbol-resolved-trees-to-typed-trees/src/expression_meanings.rs)
prepares operator calls with their authored selection and static arguments.
The first annotation pass and specialization by type arguments exist. Until
the rest are rebuilt, the requests they would serve stay in the trees as
typing left them, and structural elaboration refuses each by name where a
program needs it (`generic application`, `receiver call`,
`static requirement dispatch` and so on).

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
refused by name. [Product operand preparation](src/product/mod.rs) builds the
application of a provider choice whose slot or provider takes static
arguments ([applications](src/product/applications.rs)): it closes the
request's operand graph with the static arguments of the generic helper that
made the choice, and checks the resulting argument tuple against the
declaration's parameters. It refuses a requirement applied to static
arguments (a native binding's operand).

This stage sees the program together with the whole bundled library and
does not know what an entry needs. So a pass that refuses refuses every
program, even one that never reaches the construct. A pass whose requests
occur in the library must either serve them all or leave the refusal to the
reader of the declaration.
