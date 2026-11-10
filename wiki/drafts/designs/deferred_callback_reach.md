# Deferred callback reach

Ratified 2026-11-07. A collection or wrapper should accept a callback using an
application-defined service without naming that service in the library.
Omitted reach on a bodyless non-boundary requirement therefore defers to the
selected implementation's published row. Explicit clauses constrain selections.

The normative owners are [effect declarations](../../spec/language/effects.md#deferred-requirement-reach),
[static reach dependencies](../../spec/language/effects.md#static-callback-reach-dependencies),
and [generic requirements](../../spec/language/generics.md#requirements-and-conformance-evidence).
This implementation record belongs to `STATIC-MACHINE-MIGRATION` in
[TASKS.md](../../../TASKS.md), alongside [static machines](../../spec/language/machines.md#static-declarations-and-explicit-state),
[named implementations](../../spec/language/conformances.md#data-and-code-are-independent)
and `dropped by`. Delete it when implementation and acceptance are complete.

## Decisions

- Checked bodies infer omitted reach from calls. Bodyless non-boundary
  requirements defer it to selected implementations. Bare `reaches` promises
  empty; a named row supplies a finite ceiling. No originating `reaches _`
  form or explicit service-row generic parameter is added.
- Structural and nominal static machine binders, and conformance evidence
  members, all contribute row projections. Conformance projections retain
  the evidence binder, exact member and substitutions independently per member.
- Generic summaries are finite unions of fixed contributions and projections.
  An unresolved open projection cannot fit a finite authored ceiling in its
  owning generic body. A bounded projection may fit. Closed callers substitute
  the selected published rows before checking their ceilings.
- Every public API publishes a checked summary; it may infer service reach.
  Boundary requirements explicitly declare stable ceilings across all providers.
  Installation resolves every static row and checks the receiving policy.
- Forwarding preserves the exact originating contract and whether its reach is
  open, empty or finitely bounded. Omission does not infer a callable signature,
  choose an implementation, grant authority or expose a private body.
- Runtime machine-value types keep finite ceilings and state them; an omitted
  clause rejects under `open-callable-contracts`. Suspension/blocking bounds, public declarations and dependency rules
  are unchanged, as are termination, proof admission, custody and crashes.
  [Conditional callback carry](conditional_callback_carry.md) separately extends
  suspension-permission checking; it does not widen service reach or other axes.
- Changing an inferred published row can invalidate ceilings, proof contexts
  and installation policy. Record the interface change even when all current
  concrete selections happen to remain service-free.

## Implementation

The documentation sweep at base `2f26d51bdf25b2a351d7a1a69cc01ab00e4aae2c`
does not implement these rules. Complete the migration in both compilers,
published summaries, library customers and existing suites before closing it.

1. Represent the reach policy explicitly: open or finite, including finite
   empty. Preserve the distinction through parsing, requirement elaboration,
   generic body checking, checked representations and published interfaces.
   A missing or unresolved row cannot become empty by fallback.
2. Generalize dependency projection/substitution to all three static forms.
   Preserve exact structural/nominal origins, evidence-binder member identities,
   nested substitutions, forwarded conjunctions and recursive-call fixed points.
   Read selected published contracts; opaque supplies need admitted rows.
3. Keep the abstract body valid for every admitted selection. Check finite
   generic ceilings against bounds, then specialize dependencies for closed
   caller admission, evaluation and installation. A ceiling contributes no reach.
4. Carry the summaries through specialization and lowering without scanning
   source again. Update canonical identities and the
   [reach encoding](../../spec/terminal-psi/encoding.md#deferred-static-reach-summaries),
   schema versions, wire tables and codecs together. Existing empty-bound
   metadata must never be silently reinterpreted as open. This docs change
   assigns no new wire tags and changes no serialized bytes.
5. Audit omitted library and example requirements by their contracts. Add bare
   `reaches` wherever generic proof/evaluation or API guarantees require an
   empty row. Inspect algebra and relation laws, float and ordering operations,
   equality, conversions, content predicates and layout requirements. Do not
   assume every conversion or callback must be service-free. `core/algebra.omg`
   demonstrates proof-used operations needing this audit; `core/iteration.omg`
   keeps omission on `ElementStep::apply` so clients can supply services.
6. Migrate affected examples and positive/negative controls with the checker.
   Retain explicit service-free contracts where rejection was intentional;
   identify old rejection pins whose only cause was the superseded empty default.
   Use focused existing suite coverage and native/Psi comparison for execution.

## Acceptance

| Case | Required result |
| --- | --- |
| Application-defined service | Core `for_each` accepts a fitting step reaching `MotorControl`, and publishes that reach. Include Console and service-free selections too. |
| Static forms | Exercise structural and nominal machine binders and `S satisfies Trait<T>` member calls; two members of one S retain different rows. |
| Forwarding | Public/private and multi-layer wrappers preserve openness, exact origins and substitutions. Missing callable origins and origin-free cycles still reject. |
| Union | A fixed Console call plus a callback retains both contributions; two callbacks contribute independently. Uncalled members add nothing. |
| Generic ceilings | An unbounded unresolved callback under a finite ceiling rejects; a bounded callback whose bound fits passes. An authored ceiling adds no service. |
| Closed callers | Substitute first: a Console-only selection fits `reaches Console`; the same call fails under empty or incompatible ceilings. |
| Requirement defaults | Omission defers; `reaches none` requires empty; finite rows bound selections. Boundary omission and originating `reaches _` reject. |
| Proof use | Generic propositions and evaluation cannot assume omitted reach is empty; explicit service-free requirements preserve existing admission. |
| Runtime values | Close static dependencies before fitting a finite callable type. Omitted runtime reach stays empty; unresolved row leakage rejects. |
| Other axes | Preserve suspension/blocking ceilings and markers, termination, crashes, access, custody and actual authority requirements. |
| Publication | Renaming binders preserves normalized identity; changing open/finite policy or a selected published contract invalidates dependent records. Test codec round trips and stale/missing dependencies. |
| Installation | Resolve every static projection to a finite row; reject unresolved or disallowed services. Do not manufacture provider selection or capability grants. |

Extend the existing generic/effect, proof-admission and codec suites; keep their
reject twins and pinned outcomes. Validate executable cases with
`omega run --both` on the supported hosts, including the Omega-written compiler's evidence
binder path. No new test subsystem or general row algebra is part of this task.
