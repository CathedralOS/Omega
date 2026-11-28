# Typed preparation (Psi 04)

Every program's typed trees cross this stage on their way to the checker, by
one path: [`prepare_typed_trees`](src/lib.rs) runs an explicit, ordered
list of passes over the typed trees, then returns
[`PreparedTypedTrees`](../../representations/prepared-source/lib.rs). That
representation checks that computation inputs belong to this source and retain
their exact declarations before freezing them. Stage 05 depends on this data
crate, not on the code that runs preparation. Its readers still refuse missing
specializations, unresolved fields and unsupported shapes.

Specialization reports an invalid static argument at the authored call. A
request it cannot resolve stays on the owning declaration for lowering to
report if the program needs it. The subsequent passes attach domain theories
and field identities to the trees. Stage 06 checks the resulting lowered form.

## The list today

| Pass | What it writes | Who reads it |
| --- | --- | --- |
| [specialization](src/passes/specialization.rs) | For a call that applies static arguments (`relay::<u8>(..)`, `advance::<Hold>(..)`): which parameter each argument is for, on the call's application; and, when the arguments are closed, the machine the application selects, generated from the generic machine with each parameter replaced, which the call then enters | Structural elaboration: an open application in a generic machine is read from its bindings (`read/calls.rs`, `type_arguments`), a generated machine as any closed machine, and its declaration states the generic machine and the application (`declare.rs`, `application`) |
| [domain applications](src/passes/domain_applications.rs) | The declaration’s carrier, proof facts and cleanup selection with its retained type and value arguments substituted, or a refusal on that application | Structural elaboration reads the theory for the exact declaration and semantic domain identity |

Record patterns already carry their record and field identities from typing.
Preparation preserves those selections; lowering checks the actual subject,
complete field coverage and ordinary ownership obligations.

For a reflected sum visitor, specialization checks both callback interfaces and
creates a candidate helper for each declared case. When stage 05 reads the
dispatch, it tests the applied sum's type contracts and admits callback
applications only for cases that can still occur. The generated helpers enter
through that dispatch. [Active-case expansion](src/passes/specialization/reflected_fields/active_cases.rs) · [Dispatch lowering](../05_prepared-typed-trees-to-lowered-program/src/read/transitions.rs)

Specialization serves type parameters given plain named types (a builtin
type, a data declaration with no parameters, a type parameter in scope) and
implementation parameters given plain named implementations (one with no
parameters, of exactly the bound's trait at the types supplied for the
binders before it, such as `CoinByBoth` for `E satisfies
EqualImpliesIdentity<T>` with `T` given `Coin`, or an implementation
parameter in scope), on a generic machine of one state with a body. Both kinds are bound
through one operand shape. In the machine generated for an implementation,
each call through the parameter, in the body or a clause, enters the machine
the implementation selects for that requirement and records the requirement it was written against
(`StaticRequirementDispatch`), which structural elaboration reads
(`read/calls.rs`, `requirement_call`). Two names of one implementation value
are one application. A call that names a trait's requirement and applies the
requirement's own parameters (`Probe::take::<Reading>(..)`) is served the same
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
`configure::<(8u64 + 9u64)>(..)`, reaches this pass already as the literal of
its value: typing folds it where it reads the argument
(`typed_trees::const_integer::closed_literal`), so `17u64`, `(8u64 + 9u64)`
and `(20 - 3)` are one application. A calculation typing does not close (two
integer types, a result outside its type, a name or a call) stays a request
on the machine that holds the call.

A Build selection inside a generic helper,
`Build::select_provider::<Reading<Tag>, FirstReading<Tag>>(builder, target)`, is
copied as written. Its operands are the request the call was authored with,
not static arguments of a machine to generate, so the generated helper holds
the same request, and the reader of the selection takes each parameter's
value from the generated machine's application
(`typed_trees::product_operand::capture_static_environment`).

Closing a generic machine copies its nested declarations with it. The pass
validates the tree and allocates fresh symbols for every machine, entry,
formal and local before rebinding any body. Calls between siblings use the
same symbol map. Each copied declaration retains its own source and receives
the enclosing application's static arguments. A helper's independent type
parameters remain available for its own calls. Runtime values pass through
ordinary arguments.

A fully applied named map with a default type-only telescope uses the same
[copier](src/passes/specialization/named_conformances.rs). The call identifies
the public requirement. One `CapturedMachineApplication` retains the map's
constructor bindings, complete argument tuple and supplying owner. It names
the selected member as its root and records the source and entry of each
copied private machine. An unused map parameter still contributes to the
identity of every copy. An ordinary named call still refuses map lifetimes,
non-type map parameters and installation-bound reach rows.

When a Build chooses a generic provider, the same copier prepares each method
the implementation supplies. For example, the type and value arguments in
`FirstReading<[Holder<u64>; 2], 17>` become the bindings for its methods.
Target choices by the same owner can share those method bodies when the
implementation and all its arguments agree. The lowered-form checker also
checks the original method against its public contract with the value
parameter still free.

## What is not here yet

The deleted preparation ran about forty passes of five kinds. Three kinds
belong to this stage and come back as passes: annotations the checker reads,
specialization of generic machines and requirement applications, and
selection (calls bound through a value receiver). Typing in
[Psi 03](../03_symbol-resolved-trees-to-typed-trees/src/expressions/expression_meanings.rs)
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

The prepared result owns the frozen source and the inputs used to settle its
named constants, computed type arguments and array extents. Each computation
retains the constants it names and the deployments that admitted its evaluation.
A pending deployment stays explicit. Build reads its constant dependencies from
this result after lowering. Source admission refuses unsupported wire demands
before evaluation.

An array type such as `[u8; WIDTH]` names the constant that supplies its length.
Preparation reads the evaluated value from that declaration for both a literal
initializer and one that calls a machine. A numeric extent such as `[u8; 4]`
stores its length directly.
[Extent handoff test](tests/computed_array_extents.rs)

The Product driver forms Build-selected operands against the authored typed
declarations before specializing the program. Source calls and applied Product
requirements share the same static argument binding and closed requirement header.
[Product policy](../../driver/compile_product/product_policy.rs) · [Static operand binding](../../representations/typed-trees/callables/static_operand_binding/mod.rs)

This stage sees the program together with the whole bundled library and
does not know what an entry needs. So a pass that refuses refuses every
program, even one that never reaches the construct. A pass whose requests
occur in the library must either serve them all or leave the refusal to the
reader of the declaration.
