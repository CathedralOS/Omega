# Owner Questions

Only the owner answers an entry. The owner's word in a session is the
ruling: write it into the specification, remove the entry, and build it; that
is what the owner asked for. Otherwise agents add questions and may research
existing ones, but do not settle a question themselves.

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap construction follows the owner's [variant mandate](bootstrap/MINIMIZATION.md).
Architecture exploration and implementation within that mandate are delegated.
Unresolved changes outside it belong in [OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md).
This file continues to own product-language questions.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

### 1. Does a V2 direct run select a neighboring build file?

A developer runs `omega run main.omg` in a project that also contains a build file selecting dependencies and providers. Should V2 run the named source with its default Build, or automatically use the neighboring project's Build? This determines whether placing the same script in a project changes which providers it runs with.

The [command vocabulary](wiki/spec/build/command_line.md#command-vocabulary) names the root operand. The [project-role contract](wiki/spec/build/declarations.md#project-role) defines both a selected Build and the default when none is selected. Neither specifies when a neighboring file is selected by this invocation. The [V2 driver plan](omega-rust-v2/PLAN.md#the-omega-application) records direct execution as the tentative design. The [default execution engine](wiki/spec/build/embedding.md#standalone-execution-selection) is already settled.

### 2. Does a copy read through a reference make a by-value parameter or operator applicable?

A machine holds `v: @u8` and calls `take(v)`, where the only overload is `take(n: u8)`. Should the call read the `u8` through `v`, or is it refused until the source writes the read some other way? The same choice decides `id::<u8>(v)`, `v + 1u8` and `v == true` with `v: @bool`. The answer also decides overload selection: when both `take(n: u8)` and `take(n: @u8)` are declared, `take(v)` either stays an exact match for `take(n: @u8)`, or the copy read makes `take(n: u8)` a second candidate and the call is ambiguous, as `take(x)` with an owned `x: u8` already is under the implicit shared lending rule.

The V2 typer treats the copy read as settled at a destination with a declared type: a return, an annotated `let`, a record field value and a value pattern on a reference subject. At call arguments and operator operands it matches the reference type exactly, so `take(v)` is refused with only `take(n: u8)` declared, and `take(n: @u8)` wins when both are declared.

[Establishment, transport and helper calls](omega-rust-v2/wiki/spec/language/domains.md#establishment-transport-and-helper-calls) says that reading a copyable primitive through a readable reference needs no reference-to-value cast, but not whether passing the reference to a by-value parameter counts as that read. [Calls, places and temporaries](omega-rust-v2/wiki/spec/language/expressions.md#calls-places-and-temporaries) allows implicit shared lending at a shared-reference parameter. [Operator declarations](omega-rust-v2/wiki/spec/language/expressions.md#operator-declarations) refuses a general search for borrowed variants and requires ambiguity, not a preference, when adaptations leave competing candidates. Both cover only the owned-to-borrowed direction. [Owned local mutability](omega-rust-v2/wiki/spec/language/ownership.md#owned-local-mutability) says copyable values may be copied by value, without naming argument admission.

### 3. Does a bare word in an arm pattern compare against a parameter or local of that name?

In `machine f(x: u32, v: u32) -> u32 { match v { x -> x, _ -> 0 } }`, should the arm compare `v` with the parameter `x`, or bind a new name `x` that holds `v`? The same question applies to a local from `let x = 3;` and to a generic value binder such as `<const N: u32>`. The answer decides whether adding a parameter can silently turn a catch-all arm into an equality test.

The [V2 parser grammar](omega-rust-v2/layers/01_tokens-to-syntax-trees/GRAMMAR.md) says a bare-word pattern either "binds a new name or names a constant or a case", and the resolver decides which. The [patterns specification](omega-rust-v2/wiki/spec/language/patterns.md#pattern-subjects-and-bindings) allows a computed value as a pattern, and the [resolution order](omega-rust-v2/wiki/spec/language/modules.md#resolution) puts local bindings before package declarations. Neither says whether a word that resolves to a local, parameter or generic value binder is a comparison. The V2 resolver currently binds a new name in all three cases and compares only against a `const` declaration that no local of the same spelling hides.
