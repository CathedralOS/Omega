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

A compelling proposal for a new bootstrap language, replacement rung, or
alternate dialect belongs here before implementation, even as an experiment.
Apply the [bootstrap scope checkpoint](AGENTS.md#scope-checkpoints): identify
the concrete compiler customer or required proof obligation, compare existing
languages and simpler refactors, and account for total audit cost and machinery
displaced. Await an owner decision; a task or prototype is not approval.

## 1. Epsilon expanded-storage admission

<a id="epsilon-static-storage-policy"></a>

Should interpreted Epsilon retain a separate cap on fully expanded logical
storage, or bound only the resources its evaluator actually consumes?

**Standing requirement:** [Epsilon section 10](bootstrap/4_epsilon/LANGUAGE.md#10-resource-classification)
requires post-checking expanded-storage admission, deterministic array-length
attribution, and the bounded `ApplicationStaticStorageBytes` demand witness.
The switch to an evaluator explicitly retained unrelated resource decisions
(`c1f09eddaa`, historical decision D125). This is a proposed policy revision,
not an unanswered permission to choose a larger numeric capacity. The existing
requirement remains in force until answered.

The customer is the interpreted first Omega compiler D. Its
[`AlphaTapeBuffer`](bootstrap/5_omega/representations.epsilon) contains eight
fixed arrays. With a hypothetical one-byte `u8`, four-byte `i32`, no-padding
layout, those arrays alone represent 141,674,226 bytes, excluding scalar fields
and metadata. That is logical-size arithmetic, not measured memory or a selected
Epsilon layout. D's authored extents and capacity checks remain source semantics.
The evaluator's [sparse storage](bootstrap/4_epsilon/implementation_notes.md)
does not materialize those arrays in full; conversely, repeated updates to a
small array can consume substantial cumulative immutable Gamma allocation.
An expanded-size cap therefore does not replace actual resource containment.

A disposable probe at `f048ab4155` on macOS arm64 confirms this distinction:
a checked 529-byte program declares
`[[[u8; 2147483647]; 2147483647]; 2147483647]`, reads the unwritten last cell,
writes `65` there, checks a separate zero cell, and outputs `A`. The exact
current execution receipt (`dd4985c0eb6e1f30bc2178f90dd30e25ae7b842fb544137f606a44e622000f22`)
returns process zero and private observation `000000000041` in about 0.81 seconds.
Its logical byte-cell count is 9,903,520,300,447,984,150,353,281,023.
This is diagnostic execution, not final-profile conformance or evidence that a
selected static limit was violated. No permanent profiling tool was added.

1. **Recommended: remove the separate expanded-size admission requirement for
   interpreted Epsilon.** Retain exact source array bounds, zero homes, value
   semantics, evaluator refinement, and explicit fail-closed limits on actual
   request, storage, stack, output, and execution resources. This avoids adding
   a logical layout/sizing pass, its attribution rules, and corresponding proof
   and conformance obligations solely for an unrealized dense representation.
   Actual-resource containment and the final observation boundary still need
   implementation; this is not a proposal for unlimited execution or host policy.
2. **Retain the current requirement.** Select the profile's byte measure,
   storage-root accounting, and extent, then implement bounded type-graph sizing
   and canonical attribution before execution. Keep the sparse runtime: no
   Alpha backend, native ABI, or eager allocation is required. This preserves
   an independently predictable logical-size admission rule, at the cost of a
   separate analysis that does not bound cumulative interpreter allocation.

No implementation-size or speedup estimate has been established for either
choice. Only replacement of this policy awaits an owner ruling. The
[P3 resource task](TASKS_BOOTSTRAP.md#p3---delta-to-epsilon) remains open; checking
and runtime conformance, lower-chain resource analysis, and independent proof
work do not depend on this answer.

## Q2 — Callback contract forwarding

### Context

Named static callbacks use ordinary requirement checking. Fixed authored callback
ceilings already give a coherent language contract. Anonymous forms are entirely
unaccepted under [proposal 0000](wiki/proposals/0000_anonymous_machines.md);
their ergonomics are not a reason to introduce a contract system. The external
customer for forwarding
is a reusable traversal accepting either a pure calculation, a Console-writing
callback, or a suspending callback without making every use conservatively
effectful. Existing corpus use is not required to motivate that API.

[Operational contracts](wiki/spec/language/effects.md) currently require fixed
ordinary exported ceilings and exact call-site suspension/blocking markers.
Private summaries cannot narrow generic calls. A static callback's declared
contract therefore cannot vary its effects through a generic consumer without
an additional public composition and acknowledgement rule.

### Problem statement

How does a generic machine expose a selected callback's permitted reach and
operational possibilities, and acknowledge the corresponding call crossings,
without guessing from private bodies or granting unconstrained effects?
This is general machine-contract work, not a special lambda privilege. It does
not block named fixed-ceiling callbacks or their repeated-reborrow implementation.

### Proposed solution

Keep fixed declared consumer contracts as the baseline. First compare an ordinary
dedicated machine or a fixed-ceiling generic consumer against the annotation and
compiler cost of callback-dependent contracts. Declining the extension is a valid
outcome; no forwarding work is an implementation prerequisite.

If that comparison justifies an extension, one candidate is typed static
projections from a bound machine's public contract, such as
`reaches Step.reaches;`, `suspends Step.suspends;`, and `blocks Step.blocks;`.
Compose service rows by union and suspension/blocking independently by Boolean
disjunction. Selections must satisfy the binder's authored bounds and retain
their exact substitutions; opaque private body behavior cannot define an API.
Decide the binder syntax for those variable bounds along with the projections,
not merely the spelling used by the consumer.

For call acknowledgement, consider static conditional forms such as
`suspend(Step.suspends) block(Step.blocks) Step(context, item);`. Resolve markers
from the same pinned contract used for admission. Check carry/loans for every
admitted crossing; false conditions do not manufacture suspension. This syntax
is proposed, not part of the current spec.

Do not forward a whole contract mechanically. Each call proves its preconditions
and substitutes its post-state/crash routes; recoverable failure remains result
data. Termination includes the consumer's traversal argument, and resource
composition accounts for call count and overlap. The result must support named
callbacks and any separately accepted anonymous syntax identically, recursive
generic checking, and independent
replay of the same normalized contract dependencies.

### Alternates

- Fixed conservative callback ceilings or separately named pure/effectful
  interfaces remain viable. Compare their API duplication and loss of pure or
  no-suspend eligibility against projection complexity.
- Explicitly bound effect rows/operational parameters are viable if they stay
  coupled to the selected callback contract and define the same call-marker rule.
- Implicit insertion of crossing markers would change the acknowledgement
  policy. It is not a harmless implementation shortcut and needs its own rationale.
- Inferring public effects from whichever body or instantiations happen to be
  visible, weakening generic ceilings after specialization, or treating
  `invokes Step` as an unspecified all-axis forwarding wildcard is wrong.

## Q3 — Anonymous-machine scope and contracts

### Context

[Proposal 0000](wiki/proposals/0000_anonymous_machines.md) is entirely unaccepted,
including its simple lambda forms. Named machines and explicit environment data
already express the behavior. The proposed surface saves local names, data
declarations, and association boilerplate; it is not needed for reflection.

### Problem statement

Does that saving justify any anonymous surface, and if so, should it stop at simple
typed bodies and captures or admit full machine clauses, generic families, and
internal states? Repeating effect and proof headers inside a machine may defeat
the intended benefit. Inference, contextual permissions, and promised guarantees
must not be conflated.

### Proposed solution

Start with concrete named-machine/environment versus simple-lambda examples under
existing fixed callback requirements. Consider only sugar which elaborates to
ordinary declarations and data, without effect polymorphism or new execution
semantics. Decide whether custom clauses and state-bearing bodies justify their
own complexity before admitting them; their inclusion is not the default.

Private-body inference can avoid repeated local effect headers. A receiving
requirement supplies the contract to satisfy, not an assumed proof. Termination
must be derived or established for an invocation which promises it. Constructing
a callable and invoking it have different obligations. Captured loans and linear
debt remain ordinary obligations regardless of whether the body is ever called.

### Alternates

- No anonymous surface: use named machines and explicit context data.
- Restricted simple lambdas: accept a precisely bounded sugar without promising
  later support for full anonymous contracts or states.
- Full anonymous machines: accept only if their concrete usefulness outweighs
  the mid-machine annotation burden and implementation cost.
- Ambient caller authority, inherited termination as an assumption, or a new
  contract-forwarding system justified solely by lambda convenience are wrong.

## Q4. Gamma product checking

<a id="gamma-product-checking"></a>

For Gamma-written compiler data, should named products retain Gamma's current
dynamic value checking, introduce static nominal typing, or remain ordinary
pairs? This concerns a small addition within the selected rung, not replacing
Gamma with Delta or removing a rung.

The concrete customer is the seven-field argument continuation in
[Delta argument checking](bootstrap/3_delta/implementation/checking/types/calls.gamma).
Its producer and consumer manually agree on a nested-pair layout. Sequential
binding groups expose the decoding steps but do not check that agreement.
A disposable ordinary-Gamma constructor/accessor experiment against `355ea00d55`
on macOS preserved six exact compiler results. It increased the owning module
from 4,731 to 5,542 bytes and from four to twelve definitions. Independent field
accessors traverse 21 tail links instead of the existing shared decoding's six.
It provided names but no new layout guarantee; it was not retained. This result
does not establish the cost or benefit of a language-level product facility.

Recommended next comparison: one field declaration, a constructor, and one
destructuring operation, with opaque constructor identity and arity checked at
runtime. Ordinary pairs or matching integer bits must not forge that identity.
Field values could retain the current scalar/pair conventions rather than
requiring a whole-program nominal type checker. Construction order, lexical
scope, tail position, private allocation bounds, and failure mapping remain
explicit obligations. This is an unimplemented candidate, not accepted syntax.
Compare its Beta implementation, tests, and refinement obligations against the
pair-layout machinery removed from actual compiler consumers before retention.

Static nominal typing could instead reject more mistakes before execution, but
requires declared field types and judgments for construction, calls, and
destructuring. Do not introduce it implicitly as part of a readability change.
No product evaluator size, resource demand, or human defect-localization result
has been measured. The choice of guarantees needs a ruling; ordinary instruction
selection and storage design within an approved comparison do not.

## Q5. Runtime source in compiler artifacts

<a id="bootstrap-runtime-source-composition"></a>

May the Delta compiler artifact carry explicit immutable runtime source members,
with a specified composition/publication boundary, instead of reconstructing
those programs with [byte-emission instructions](bootstrap/3_delta/implementation/emission/bytes.gamma)?

The customer is the generated Delta application's byte runtime and
`ConformanceBytesV1` adapter. A disposable diagnostic against `355ea00d55` on
macOS replaced generated runtime definitions with a 73-line, 1,957-byte ordinary
Gamma module using descriptive locals and grouped bindings. Twelve baseline/module
execution observations agreed, including empty/NUL/high-byte data, concatenation,
indexing, and exact byte-range/index traps. It changed the emitted source bytes
and left the generated main adapter unchanged. It bypassed production composition,
so it establishes neither a closed artifact
nor correct whole-output resource accounting. The substitution tool was not
retained and must not become a host semantic stage.

Recommended direction: runtime code is authored once as normal source. The
compiler artifact binds the exact ordered support members as well as compiler
source. A specified generic composition operation includes the selected members;
no host parser, output patcher, filename discovery, or unbound dependency chooses
program meaning. Before publication the compiler must account for every support
byte and generated byte and preserve atomic refusal on the complete output.
Compare source-owned copying with strictly declared byte-only materialization;
neither route is selected or sized by this question.

This changes the current [composed-artifact](bootstrap/2_gamma/COMPOSED_ARTIFACT.md)
and [Delta receipt](bootstrap/3_delta/LANGUAGE.md) boundaries. Keeping a complete
source-only receipt is the baseline; changing it needs explicit subject custody,
dependency selection, output order, limit attribution, and reconstruction rules.
Audit code as code, not string fragments or per-character calls. No new file I/O,
general module language, or intermediate rung is implied. Implementation waits
for the boundary decision; proof and complete runtime conformance remain required.

## Q6. Nonboundary direct operator executable supply

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
