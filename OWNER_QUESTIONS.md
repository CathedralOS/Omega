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

<a id="build-network-and-subprocess-grants"></a>
### 1. Should an interpreted build be able to request network access or launch a host tool?

A release build might need to fetch a pinned firmware image or run a vendor image packer. For each operation, should the caller perform it outside Omega and supply the captured bytes, or may the build request it through a scoped host service? If admitted, what does the consumer approve and the executor enforce for the remote resource or the executable, its inputs and its outputs?

The default build has neither service. The [network reach rules](wiki/spec/build/platform_services.md#network-reach) classify a networking call and its provider. The open choice is whether a Build may receive a live provider or launch a host tool. The existing rules distinguish a machine's transitive service reach, the consumer's acceptance of a restricted operation and the actual resource the executor supplies. Package retrieval already runs under the resolver's separate authority. [Build host restrictions](wiki/spec/build/execution.md#restricted-build-requests) · [Service reach](wiki/spec/language/effects.md#declarations) · [Restricted-build acceptance](wiki/spec/packages/acceptance.md#restricted-build-acceptance) · [Resolver transport](wiki/spec/packages/resolver_security.md#host-routed-network-transport)

<a id="decrease-of-a-computed-subject"></a>
### 2. What must a proof-only machine show when it calls itself at a computed value?

`mod(a, b)` in `core/nat.omg` is `terminates by a` and calls itself at `saturating_sub(a, b)`. [Termination](wiki/spec/language/termination.md#ranking) says "computed next subjects use ordinary decrease obligations and cited lemmas", and `mod` cites `sub_lt`, whose conclusion is `saturating_sub(Succ(saturating_sub(a, b)), a) = Zero`. That is the library's own way of writing `a - b < a`. Which proposition is the decrease obligation for a `Nat` subject, so that the compiler can check that this citation meets it, and what shows that the order it names is well founded?

It blocks nothing today, because the mathematical kernel needs no answer to define the machine. Its only recursion is on the structure of a value, so the compiler defines `mod` over one more argument, the fuel, by recursion on it, and takes the fuel from `a`. That definition is total whatever the calls do, and nothing is assumed. But two things follow that the owners should decide on. First, the compiler does not check the decrease for such a machine: a body whose ranked argument did not get smaller would still be accepted, as a definition that stops when the fuel does. Second, a law about such a machine can only be proved if it holds for every amount of fuel. `mod`'s bound does. `div(a, b) * b + mod(a, b) = a` does not, because it needs the fuel to be enough, and that is exactly what a checked decrease would give.

The choices I see: (a) name the obligation as the structural order on the data, "the next subject is a proper subtree", and have the library prove once that its `saturating_sub` form implies it; (b) let a data declaration name its order and a lemma that it is well founded, which the termination page calls an admitted ranking theory; (c) leave it as built, and refuse laws that need enough fuel. I recommend (a): it adds no trusted rule, and one library lemma would make every such law provable. [Kernel audit, "Step 4 as built"](wiki/drafts/designs/proof_kernel_core_audit.md#plan-order) describes the construction, and [Recursive data encoding experiments](wiki/drafts/designs/recursive_data_encoding_experiments.md) verifies constructive `Acc<u, v>` well-founded recursion under Option C (0 kernel changes, 0 assumptions: `nat_wf` in 93 steps, `div(a, b) * b + mod(a, b) = a` in 168 steps, `div(17, 5) == 3` in 521 steps).

<a id="quotient-mere-proposition-evidence"></a>
### 3. Where does a quotient declaration select the proof that its relation is a mere proposition?

[Quotient formation](wiki/spec/proofs/quotients.md#formation-and-representation) "consumes a carrier family `C`, relation family `R`, and explicitly selected `Equivalence<C,R>`, together with the selected mere-proposition evidence". The declaration syntax selects only the equivalence: `data Sign = Representative % same_sign requires same_sign satisfies Equivalence<Representative, same_sign> as SameSign;`. No library trait states "any two proofs of `R(a, b)` are equal", and nothing in the declaration could name a conformance of one.

It blocks one direction today. The compiler forms the quotient and proves `a as Sign` equal to `b as Sign` from `same_sign(a, b)`; that direction (soundness) holds for any relation. It does not use the other direction, effectivity (equal classes give related representatives), which is the one the evidence protects.

The choices I see: (a) a second selected conformance on the declaration, of a new `core` trait whose one row proves any two proofs of the relation equal; (b) no syntax: the compiler derives the evidence when the relation's formula is an `Identity` at data it can show is a set, and refuses effectivity for any other relation; (c) drop the requirement for relations written as a `proposition`, on the ground that a `proposition` is erased and so already proof-irrelevant, and say so in the specification. I recommend (b) first, because every relation in the recovered corpus is an `Identity` of two computed values, and (a) when a relation that is not one arrives.

<a id="supplying-build-library-bindings"></a>
### 4. How does an invoker supply an external library to a root Build or to one child Build?

A project wants its Build to call a shared topology generator on its first line. A parent may also need to give one child the generator without giving its other children access. What does the invoker write to bind the root’s import alias to a package source? What does the parent write on a particular child’s dependency description to supply that child’s aliases and sources?

The specification settles who chooses each binding, when checking happens, and which program can call the library. The published command vocabulary describes package installation aliases. The Build-input design asks for the root and child spellings. Resolving local files, checking a supplied library and registering a Product dependency can proceed under the existing rules. [Dependency declarations and binding](wiki/spec/build/scoped_execution.md#dependency-declarations-and-binding) · [Command vocabulary](wiki/spec/build/command_line.md#command-vocabulary) · [Build library and service inputs](wiki/drafts/designs/build_library_and_service_inputs.md)
