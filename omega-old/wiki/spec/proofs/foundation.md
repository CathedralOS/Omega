# Mathematical foundation

## Decision and scope

Omega uses one dependent mathematical core with universe-polymorphic types,
proof-relevant identity, and a separate definitionally irrelevant logical layer.
The purpose is general mathematics and independently checkable program proofs,
without making authors use a second declaration language.

The reference design is Gilbert, Cockx, Sozeau and Tabareau,
[Definitional Proof-Irrelevance without K (POPL 2019)](https://doi.org/10.1145/3290316).
Its [published text](https://jesper.sikanda.be/files/definitional-proof-irrelevance-without-K.pdf)
is the rule reference, not whichever behavior a current proof assistant exposes.
The starting core is the predicative sMLTT presentation in sections 3.1–3.6:
stratified relevant and strict universes, dependent functions and pairs, strict
empty propositions, squash and boxing. Section 4 supplies the metatheoretic
discussion. Section 5's derived elimination criterion is not unrestricted
singleton elimination. The optional UIP extension in section 4.4 is excluded.
The impredicative variant and implementation-specific extensions are not silently
part of this selection.

The [W-based inductive profile](inductive_profile.md) supplies the selected
typed function eta, relevant identity, two-element type, W rules and derived
indexed encoding.
[Set quotients](quotients.md#set-quotient-foundation) use an explicit assumption-bearing
interface, without new kernel reduction rules. These choices are settled;
their combined metatheory, checked encodings and implementation remain required
before claiming a complete verified profile. The reference paper does not supply
those results automatically. [Typed function eta](inductive_profile.md#typed-function-eta)
is an explicit conversion extension, not function extensionality.

## Separate judgments

Each judgment answers a different question; a successful answer to one cannot
stand in for another.

| Judgment | What it establishes | What it does not establish |
| --- | --- | --- |
| Mathematical formation and checking | A term has its stated type under an exact context and calculus. | That it computes executable bits, or that its assumptions are consistent. |
| Definitional conversion | Two well-typed expressions agree under the fixed computation rules. | Arbitrary theorem-proved equality, provider agreement, or identical provenance. |
| Logical identity and transport | A checked identity supports its permitted substitution/transport. | A new conversion rule or equality of all identity proofs. |
| Executable realization | Relevant data and control have a checked implementation of the selected meaning. | That a mathematical inhabitant automatically has an algorithm. |
| Constant materialization | Admitted computation or a checked executable realization supplies the exact representable value to emit. | That having a runtime layout is sufficient. |
| Assumption admission | The receiver accepts the exact assumptions for this claim and role. | Statement fidelity, axiom consistency, or unrelated runtime authority. |

## Logical proofs and mathematical witnesses

The following is kernel notation, not additional Omega syntax:

```text
A : Type u                     a mathematical type
P : Strict v                   a strict logical proposition
p : P, q : P                   proofs of the same proposition
p ≡ q : P                      strict proof irrelevance
w : Σ (x : A). B(x)             a witness package with an accessible first field
Nonempty A = squash A          existence without ordinary witness extraction
Id A x y                       proof-relevant mathematical identity
```

The sealed core mathematical declaration `core::Identity<T>(left, right)`
exposes `Id T left right` explicitly under
[data identity in facts](contracts.md#data-identity-in-facts). Value `==` retains
its selected comparison in all contexts. Its relation to identity requires
the appropriate checked bridge law; no contextual conversion supplies one.

Sort formation and elimination determine the distinction. Compiler-generated
versus handwritten, runtime versus erased, and a domain's weakening class do not.
The statement's complete application must agree before strict irrelevance
applies. It does not identify different statements, Type witnesses, selected
declarations, resource occurrences, or assumption records.

General identity is selected to be intensional and proof-relevant, with
reflexivity and dependent identity elimination. This is the relevant extension
discussed in the paper's section 4.3, not a rule already supplied by sections
3.1–3.6. Its rule presentation is in the
[inductive profile](inductive_profile.md#primitive-rules).
There is no equality reflection, automatic UIP/K, or rule
making every proof-relevant equality proof irrelevant. Systems type identity,
IEEE comparisons, and mathematical identity retain their separate meanings.
An arithmetic certificate must interpret an existing operation's actual meaning;
it cannot replace floating-point comparison with reflexive mathematical equality.

Ratified 2026-11-06: identity of a raw IEEE `f32` or `f64` value preserves its
exact format and every representation bit, including signed zero and NaN sign,
payload and encoding distinctions. The carrier model and identity eliminator
must justify substitution in bit-observing operations. Equality of payload-erased
`FloatMeaning` projections is weaker and cannot establish this source identity.
Aggregate identity remains typed value identity, not bytewise equality of
record padding, addresses or unrelated storage. Its float component identities
follow the raw-carrier rule. See [float relations](../terminal-psi/mathematical_values.md#float-equality-and-source-identity).

Strict elimination uses the selected calculus's exact rules. Neither "no strict
proof ever eliminates into Type" nor "one constructor permits elimination" is
an adequate implementation rule. In particular, squashed existence does not
provide ordinary witness extraction. A separately admitted choice assumption
may supply a mathematical witness without providing executable computation.

`[erased]` is a binding's runtime relevance, not its logical sort. An erased
Type witness retains its subjects, identity, multiplicity, validity and custody.
Logical irrelevance cannot manufacture or duplicate an owned capability. The
interpretation of program ownership into mathematics must preserve those
obligations; an unrestricted mathematical function is not itself a runtime
capability producer.

## Universes, functions and recursion

Mathematical functions and predicates range over arbitrary eligible values,
including types at declared universe levels. They are not limited to executable
machine symbols. Universe-polymorphic declarations retain explicit, checked
levels in the core; an elaborator may solve level constraints but the checker
does not trust its guesses. There is no self-typing universal type.

Dependent function application, capture-avoiding substitution, mathematical
conversion and dependent pairs belong to this core. General data declarations
must lower through the [W/indexed encoding](inductive_profile.md), not a trusted
assertion that the source recursion looked reasonable. The encoding scheme needs
semantic justification and each application needs independent checking; a
round-trip structure comparison alone supplies neither induction nor computation
correctness. This mathematical encoding does not prescribe runtime storage.

For termination, distinguish the measure value, a proof of decrease,
accessibility evidence used in a mathematical definition, and executable control
flow. Runtime tail recursion still lowers to iteration with no frame growth.
That fact chooses neither the denotation's recursion principle nor the sort of
accessibility evidence. If an eliminator computes by inspecting evidence, it
must have the relevant rules required by that computation.

A decreasing measure establishes termination, not the computed function or the
correctness of lowering. A denotation defined by well-founded recursion and an
emitted loop require checked correspondence in addition to termination.

## Mathematical values are not executable promises

An admitted mathematical declaration may have a Type-valued result without an
algorithm. This includes a chosen member of a nonempty subset of `u32`; using a
runtime-representable carrier does not forbid ordinary mathematics. Conversely,
that carrier does not authorize materializing the chosen member.

The checked representation must distinguish mathematical assumptions from
executable declarations and checked executable realizations. Reusing an existing
boundary spelling is a source-elaboration choice, not permission to report an
axiom as an ordinary missing provider. Diagnose computational misuse at its
source use; retain the invariant through evaluation and lowering.

Proof-only references create no executable demand. Relevant data and control
dependencies do: a branch whose condition depends on a noncomputable choice
does not become executable because both branches contain ordinary constants.
The check must compose through calls and control flow, not just arithmetic
operands. No `Noncomputable` domain, cast-away annotation, or blanket ban on
mathematical values of representable types is introduced.

An independently checked simplification or executable realization can remove a
computational dependency or implement a particular use. It must establish the
required correspondence, not merely assert that some implementation exists.
The resulting executable still retains the appropriate assumption closure.
Normal mathematical terms stuck at opaque axioms are not evidence of looping;
valid proof checking and failed materialization can coexist.

## Assumptions and calculus identity

The calculus fixes formation, conversion and elimination. Additional axioms are
named assumptions with exact statements, not switches that modify conversion.
Choice, extensionality or axiomatic univalence require their own precise types
and receiving grants. These names do not establish mutual consistency or
authorize unrestricted variants. Native cubical computation is not selected;
admitting an axiom of univalence does not make its transports compute.

For each checked root declaration, compute assumption closure over the complete
reachable checked declaration graph: its statement and type, body/evidence,
referenced declarations, and their types and dependencies. Collecting names
only from the final normalized proof or only when unfolding occurs is wrong.
Unused package declarations and discarded proof-search attempts are not roots.

Retain that closure independently of conversion, strict irrelevance, erasure and
optimization. Those operations cannot silently remove a recorded dependency.
Any replacement proof with a different dependency record is checked and published
as that evidence, not used to rewrite the provenance of existing evidence.
Receivers apply policy to the exact closure and claim role. Acceptance for
mathematical exploration does not automatically accept a safety/refinement claim.

Axiom count is not a trust ordering. A checked derivation replacing an assumed
law retains its actual closure; acceptance depends on the receiver and claim role.
That closure need not be a literal subset of the replaced law's dependencies.
Conversely, fewer named assumptions do not establish a weaker commitment.
Closure computation makes dependencies and policy acceptance checkable, not
arbitrary logical strength or consistency. A derivation of quotient effectivity
using extensionality or univalence retains those dependencies; a direct assumed
effectivity law retains its own. Neither route silently replaces the other.

Unknown calculus identities reject. A checked interpretation between concrete
profiles may establish a bridge; similar syntax, shared digests or successful
tests cannot. No general checker-plugin interface is implied.

## Implementation and independent checking

Psi owns mathematical terms, declaration checking, source elaboration and
target-neutral proof evidence. Elaboration, LLM authorship, tactics and search
are untrusted producers. They supply evidence checked against an independently
established question. The kernel checks formation, typing, permitted conversion
and declaration rules; it does not accept a producer's Boolean success flag.

Keep specialized arithmetic and contract automation as useful producers or
explicitly justified checked rules. They must not grow a parallel general
notion of truth. Preserve their numeric and operational meanings when mapping
them into the common core. Terminal obligations and native refinement retain
their distinct subjects; [fact validity](../terminal-psi/fact_validity.md)
governs which premises a Terminal obligation may use.
Selecting a mathematical core does not prove an operational interpretation or
establish a general proof kernel merely by reusing a bootstrap checker.

### Admitting an arithmetic rule

Named decision: `kernel-rule-admission`, settled 2026-11-12.

An arithmetic fact belongs in the kernel as a theorem whose proof the core
checks; that adds nothing to what is trusted and needs no approval. Until the
core can check such a proof, a lane may add the fact to the certificate kernel
as a provisional rule, as ordinary implementation work, when all of these hold:

1. **A program needs it.** The change names a program whose proof cannot be
   checked without the rule. No rule is added on speculation.
2. **A general method is preferred.** Where a definition or one method covers
   the whole class, add that instead of a single fact: division and remainder
   by their defining equation, bitwise operations by evaluation at their
   width. A one-off fact is for a class with no such method, such as a product
   of two non-literal values.
3. **Its conditions are exact.** The rule states every side condition it
   depends on: sign, width, a nonzero divisor, the absence of overflow.
4. **It is tested against arithmetic.** A test evaluates the rule's statement
   over every value of a small width and compares it with computed results.
   A program the rule proves and a near-miss it must refuse accompany it.
5. **It lands alone.** One rule per pull request, with its row in the kernel's
   rule table.
6. **It is recorded as trusted.** The row is marked provisional: believed, not
   derived. Each provisional row is replaced by a checked proof when the core
   can check one, and none may remain when the kernel is claimed verified.

Owner approval is still required to change the mathematical core, or to accept
a new kind of evidence the kernel does not check itself, such as an external
solver's verdict or a new class of assumption.

Independent checking removes the need to trust the proof's producer. Humans
still review whether the statement is the intended one and whether its
assumptions are acceptable. Neither LLM confidence nor kernel acceptance proves
those two things. Kernel size, conversion cost and bootstrap feasibility require
measurement; no performance guarantee follows from the sort choice.

## Bootstrap and foreign proof producers

Bootstrap construction evidence and application proof checking have distinct
subjects. The bootstrap
[encoding acceptance contract](../../../bootstrap/proofs/beta_encoding/ACCEPTANCE.md)
fixes a concrete source-to-tape equation, not universal compiler correctness or
application safety. Application proof checking neither enables nor waives
required bootstrap evidence.

Sharing a checker, format or isolated kernel subset is justified only when it
reduces whole-chain audit and implementation burden while covering the required
claims. Isolation and dependencies must themselves be justified. Do not expand
the bootstrap checker into general mathematics merely for uniformity; do not
preserve separate machinery merely because it exists. Construction correctness
and the general kernel's logical soundness remain distinct obligations.

Foreign tools may translate proofs into the common calculus without becoming
trusted checkers. A compatible statement is not sufficient: definitions, rules,
conversion and assumptions need a checked interpretation. Demonstrate one
concrete import before claiming interoperability. No full-corpus import or new
foundational axiom is implied by this contract.

## Migration acceptance

Each example needs a valid derivation and an invalid control:

- A universe-polymorphic theorem over arbitrary predicates, plus dependent
  witness packages, identity transport and inductive reasoning.
- Two strict proofs of the same statement convert; differing subjects and
  distinct relevant witnesses do not collapse. Erased ownership remains checked.
- Squashed existence and an admitted choice over `u32` work mathematically;
  unjustified executable extraction and choice-dependent control reject.
- A theorem whose statement references an axiom-dependent definition retains
  that assumption even when its final proof does not unfold the definition.
- A terminating machine with a well-founded denotation lowers to a loop with
  checked correspondence. A wrong accumulator update that still decreases the
  measure fails correctness, rather than inheriting success from termination.
- A theorem-dependent program obligation checks under the exact profile and
  assumptions. Goal substitution and missing dependencies reject. Quotient-based
  analysis additionally exercises the selected quotient interface and its exact
  admitted or proved laws.

These are discriminating controls, not a proof of consistency or mathematical
completeness. [Mathematical source forms](mathematical_bindings.md) are selected;
their checked elaboration remains required. No anonymous machine or optional
formula-naming proposal is a prerequisite.
