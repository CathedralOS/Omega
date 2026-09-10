# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

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

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

## Q1. Nonboundary direct operator executable supply

<a id="direct-operator-executable-supply"></a>

How does a nonboundary direct operator declaration bind the checked machine
body that executes each selected use?

The customer is ordinary library-authored data and domain arithmetic:
[`Vec2::add`](wiki/language_guide/chapter_5_expressions_evaluation.md#operators)
and qualified arithmetic such as `Degrees` need executable meanings for their
documented operator tokens. Existing named machine calls can compute the same
values, but replacing the token use with a machine call does not supply the
promised operator declaration. This is not a request for another arithmetic
syntax or an implicit implementation search.

[Expressions](wiki/spec/language/expressions.md#operator-declarations) specifies
operand-directed selection of the operator declaration, independent identity,
visibility, and stability under unrelated imports. It says direct operators
need no conformance selection, but does not define their executable body
binding. [Machines](wiki/spec/language/machines.md#supply) defines `satisfies` as
selecting and refining a requirement; that provider-to-requirement relation does
not select a provider at an operator use. The parser currently accepts only
semicolon-terminated operator signatures. `CheckedOperatorRealizationContract`
retains checked satisfaction, not a use-site choice. Existing
[provider selection](wiki/spec/build/provider_selection.md#declaration-and-choice)
selects boundary slots; reusing it for an ordinary operator would change that
contract.

Please settle whether executable supply is an explicit declaration-owned body
or binding, or a package-owned unique canonical realization with a specified
ownership and selection rule. In either case, define missing/ambiguous supply,
visibility and cross-package access, and preserve the guarantee that unrelated
imports cannot change existing meaning. Mere uniqueness among visible
`satisfies` machines is not currently authority to select one. Boundary-provider
selection and explicitly selected trait conformances remain unchanged.

The concrete compiler witness is
`tests/omega/pass/expressions/declared_operator_match_result/main.omg`: an
ordinary `u8::sum` signature and a checked satisfying machine returning `u64`.
Source checking passes, but executable selection is absent. Once supply is
settled, acceptance retains exact declaration/body association, ordered operands,
branch-local invocation and independent Terminal replay. The source supply
mechanism is undetermined; implementation of that binding waits for this answer,
not independent Match or numeric-result work.
