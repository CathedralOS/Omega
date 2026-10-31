# Selected-implementation invocation lifetimes

Owning task: **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).
This row establishes exact requirement/implementation lifetime identity and
captured-operand transport under ordinary target-keyed Build selections.

**Every selected implementation retains a fresh invocation lifetime and the
returned item reference independently of captured context storage.**

The public `SelectedApply::apply` requirement and its four ordinary inline
implementations keep independently named own lifetime and static telescopes.
Each selected callback returns the item while its context contains a reference
to different storage. The caller observes item value 42 and context value 17;
this comparison tests returned storage rather than establishing a static fact.
Each twin changes only one implementation's callback result to the enclosing
lifetime. Every supplied implementation must check even when another target's
Build row selects execution.

- [Local](local/main.omg) / [reject](local/invalid/main.omg)
- [Field](field/main.omg) / [reject](field/invalid/main.omg)
- [Parameter](parameter/main.omg) / [reject](parameter/invalid/main.omg)
- [Call result](call_result/main.omg) / [reject](call_result/invalid/main.omg)
- [Match join](match_join/main.omg) / [reject](match_join/invalid/main.omg)
- [Const name](const_name/main.omg) / [reject](const_name/invalid/main.omg)
- [Nested machine](other_state/main.omg) / [reject](other_state/invalid/main.omg)
- [Another machine](other_machine/main.omg) / [reject](other_machine/invalid/main.omg)
- [Selected target implementation](target_body/main.omg) / [reject](target_body/invalid/main.omg)

The match helper takes a runtime choice. Both paths call the same original
operation; the true call must return the original item 42 and the false call
an independent item 73. The captured context still denotes 17. Inverting a
branch or returning context storage therefore fails the runtime oracle.
Identical instrumentation remains in its static reject twin.

The [two-lifetime control](two_lifetimes/main.omg) separates captured context
and item lifetimes and uses them in reverse telescope order. Its
[return-relation twin](two_lifetimes/invalid/main.omg) names the captured
lifetime; its [telescope twin](two_lifetimes/invalid_telescope/main.omg) adds
one implementation lifetime. Both obligations remain independent.

Applicable requirements:

- Establishment and composition: the selected body must refine the complete
  public callback signature for every fresh invocation, including the returned
  input-source relation. Calls and joins transport that established relation.
- Identity and lifetime: receiving declaration/ordinal and supplying lexical
  declaration/ordinal remain distinct through copying and specialization.
  Captured regions cannot become fresh invocation binders through a shared name.
- Soundness: source checking retains universal callable refinement, complete
  own telescope correspondence, exact storage origins, access and loan debt.
  No signature, visibility or proof requirement is weakened by selection.
- Psi 05–07: one checked portable requirement call and its actual returned
  reference must survive Terminal lowering and execute in the interpreter.
- Omega 00–09: explicit four-hosted-target Build rows select ordinary
  implementations; matching-host native calls must preserve returned storage
  and agree with Terminal. These alpha-renamed implementations do not establish
  the separate distinct-output/two-target acceptance of the owning task.

See [invocation lifetime families](../../../../wiki/spec/language/generics.md#invocation-lifetime-families),
[lifetimes](../../../../wiki/spec/language/lifetimes.md), and
[multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation).
The exact source twins and positives are pinned under
`tests/omega/{pass,fail}/facts/target_family_invocation_lifetime`.

At fixed optimized baseline `e4601e395b752c092042ef5b1ddbbe17bc22f749`, all nine
ordinary-provider positives and the two-lifetime positive wrongly refuse;
the exact canonical sources and diagnostics are retained in qualification
records. Historical Linux/Windows observations of the former target-prefixed
sources keep their original producer identities and provide no execution
credit for this migration. Current native/Terminal acceptance is recorded
separately with its actual source, producer and host.

The parent task remains open for its remaining source migrations, native and
callback consumers, distinct-target execution and complete legacy model,
parser, codec and dispatch retirement.
