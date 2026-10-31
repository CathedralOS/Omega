# Selected-implementation invocation lifetimes

Owning task: **BUILD-OWNED-TARGET-SELECTION** in [TASKS.md](../../../../TASKS.md).
This row tracks exact requirement/implementation lifetime identity and
captured-operand transport under ordinary target-keyed Build selections.

**Every selected implementation retains a fresh invocation lifetime and the
returned item reference independently of captured context storage.**

The public `SelectedApply::apply` requirement and its four ordinary inline
implementations keep independently named own lifetime and static telescopes.
Each selected callback returns the item while its context contains a reference
to different storage. The caller observes item value 42; context points to independently initialized
value 17. This comparison tests returned storage rather than establishing a
static fact.
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

Current progress is established by [focused run 41228657620](https://github.com/CathedralOS/Omega/actions/runs/41228657620)
at source `e7ab78cfde9789da0edb6d141e09af553d0ca9e9`: local, field,
parameter, match join, const name, nested machine, another machine and selected
implementation all check and exit 0 through both Linux x86-64 native execution
and Terminal, with agreement. The retained optimized compiler SHA-256 is
`bdf58ae1044cc32f3a1d6ffb0a6bae992cf5796f57fb6716ebfa7236c27c6d58`.
The [captured-body reject](../../../../tests/omega/fail/borrows/invocation_lifetime_captured_body_return/main.omg)
keeps its item-result signature but returns captured storage; its authored
body-origin diagnostic also matches.

The two-lifetime case is established separately by
[run 41234754042](https://github.com/CathedralOS/Omega/actions/runs/41234754042)
at source `312b9f34a983180519eea3e9441a9f574c944b81`. It checks and exits 0
through Linux x86-64 native execution and Terminal, with agreement. Its return
relation and telescope twins both match their specific refusal diagnostics.
The optimized compiler SHA-256 is
`013c9842dd3efbc9b79feefd755f818945fb84bb6b21f54491ab8e26ecad5418`.
The repair includes final-call transition expressions in ordinary lifetime
inference. The invalid telescope program is unchanged; its expectation now
names the exact requirement/owned-realizer own-region telescope mismatch.

The ordinary call-result forwarding case is established by
[run 41238127524](https://github.com/CathedralOS/Omega/actions/runs/41238127524)
at source `db6c80fc4d2345c0406dc66f10cba81537598ef6`. Call result, local and
two-lifetime positives check and exit 0 through Linux x86-64 native execution
and Terminal, with agreement; all five selected ownership/region rejection
controls match. The canonical call-result sample also passes its ordinary
review and native/Terminal gate. The optimized compiler SHA-256 is
`23706c88bec26b5e60e55f6a30e65e69a67ca3ad4d776919724a83ef0c8f62dd`.
The repair composes origins through every exactly associated closed provider
body and retains each body's exact formal-to-actual correspondence.

That run remains failed because the sample invalid twin expected an older
input-source diagnostic. The byte-identical corpus twin in
[run 41239954082](https://github.com/CathedralOS/Omega/actions/runs/41239954082)
uses the same compiler and runner bytes and rejects during result validation:
the nested fresh invocation output disagrees with the enclosing captured item
region. Both expectation files now require that result-validation phase and
its explicit fresh-versus-captured comparison. Matching is verified against
the retained actual diagnostic; neither old wrong-fragment report is relabeled
as a passing run, and no new compiler execution is claimed for the text change.
The invalid program is unchanged.

Full-row acceptance remains open. The historical
[run 41233740591](https://github.com/CathedralOS/Omega/actions/runs/41233740591)
at source `19ba4aa0df0ac916ee6b20fe8d56b4600dcc9c05` retains its original
call-result incompleteness and no native/Terminal acceptance. The historical
[run 41227086012](https://github.com/CathedralOS/Omega/actions/runs/41227086012)
recorded eleven signature/telescope refusals with wrong diagnostic fragments.
The two-lifetime pair and call-result twin are now attributed specifically;
the other eight ordinary signature-twin reason frontiers remain open. These
results do not establish other native hosts or the complete family.

The parent task remains open for its remaining source migrations, native and
callback consumers, distinct-target execution and complete legacy model,
parser, codec and dispatch retirement.
