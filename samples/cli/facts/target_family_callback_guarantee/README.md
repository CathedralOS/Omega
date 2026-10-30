# Selected callback guarantee

Owning outcome: **BUILD-OWNED-TARGET-SELECTION** in `TASKS.md`.

Fact: a selected callback satisfies the public callable guarantee, and its bounded result permits subtraction from 64u64.

The nine places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [another selected implementation](target_body/main.omg).
Every place keeps input42 and forms64-result from the returned bound before
observing remaining22. Its `invalid/main.omg` twin removes only identity's
published result>=value guarantee; its body and consuming operation stay the
same. A concrete private body cannot supply a missing callable obligation.
All positives bind all four hosted targets; the target controls also bind x86macOS.
The positive/twin sources are pinned under
`tests/omega/{pass,fail}/facts/target_family_callback_guarantee/`.

One ordinary CallbackApply requirement explicitly retains the complete static
callback contract formerly demanded by all four implicit members: input<=64,
result<=64 and result>=input. SharedApply uses an independently named Action
binder and proves its public result<=64 obligation. Build selects this ordinary
checked implementation separately for each target. Selection grants no source
proof or authority, and duplicate target-prefixed bodies are removed.

The match place exercises both choices with the same42 and bound; both
subtractions form before checking distinct17/29arm markers. The const-name
place still names the selected call result itself. The target-values control
retains that same call constant, with Windows selecting41 and observing
remaining23, while the other targets select42 and observe remaining22.
The target-body place nests the selected callback directly inside the selected
subtraction call.

Additional witnesses retain the same fact and operation:

- [Interleaved bare binder](bare_interleaved/main.omg): outer entry/result clauses
  surround a complete nested callable requirement, which determines binder kind.
- [Repeated application](repeated_application/main.omg): two calls retain
  distinct42/41result snapshots despite sharing a canonical public application.
- [Single evaluation](evaluation_once/main.omg): observable inner and outer
  counters must each be1 after the nested callback and its enclosing consumer.

Each has a guarantee-sensitive reject twin. Other retained controls cover a
non-host invalid helper, a weaker disjunctive guarantee, a stronger callback
precondition and equal versus distinct captured constants. The captured-constant
formation control is checking-only.

The accepted contracts are [static callback applications](../../../../wiki/spec/language/generics.md#static-machine-binder-categories),
[named implementations](../../../../wiki/spec/language/conformances.md),
[Build selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection),
and [requirement scopes](../../../../wiki/spec/language/state_contracts.md#requirement-clauses).
Canonical requires-machine clauses own their nested contracts and terminating
semicolons. The parser stages an owner's clauses before publishing contiguous
arena spans, so interleaved nested clauses cannot become outer predicates.

Establishment comes from the callback's published contract and each checked
implementation's return. Full original static tuples are admitted independently
of executable specialization. Exact formal/result identities, captured
constants and call arguments survive substitution. Calls and joins transport
the bound directly into subtraction; no caller guard manufactures it. Invalid
unselected implementations and wrong static kinds/scopes still reject. Ordinary
boundary calls carry their scalar arguments through the same computation owner,
so a nested producer executes once. Private service-contract exposure follows
the authored declaration; public contracts still reject private constants.

Optimized producer3e904b0883480a9d77d894c302271d43fbdea979 has34proper Source
outcomes (15positives,19reject controls) and14actual sample native0/Terminal0
observations on macOSARM64. The pinned corpus has14ARM native/Terminal
agreements and one x86macOS/Rosetta agreement. The target-body and target-values
controls also execute one unchanged Terminal artifact across all five authored
target installations. ARM/x86captured products retain the actual portable bytes
and component inputs. Other-host native execution is not established here.
Terminal-only counterfactuals retain a positive0 and typedTraps for a wrong
payload, wrong match marker and duplicated inner-counter effect.

Against fixed Source base472bcd85026456652e414e1f834301b662558674, the534affected
fixture comparison has no losses and the six added fixtures have their intended
outcomes. All329samples changed by the static-clause migration retain their
original checking outcomes (155acceptances and174refusals); this comparison is
Source evidence, not329native runs. The Omega-written prefix parser's retained
native artifact passes503decoder cases. The original explicit `terminates;;`
control is pinned as
`tests/omega/pass/calls/requires_machine_explicit_operational_closers_exit`
and runs native0/Terminal0. Its full nested/interleaved requirement
implementation remains open under REQUIRES-UNIFICATION.

Original producerad909308cdb32b3c95b5cbf8093262e4630e0c51 checked all25old Source
controls and ran all11old executable programs with native0/Terminal0. The new
ordinary forms initially refused in P01, private review, public tuple transport
and nested computation lowering. Frozen baselines, every intermediate refusal,
actual products and raw observations remain under `build/content-provider/callback`;
archive manifests preserve closed compiler images with exact restoration metadata.

The parent target/native retirement and remaining requirement forms remain open.
The single-semicolon operational ownership question is narrowly recorded in
[OWNER_QUESTIONS.md](../../../../OWNER_QUESTIONS.md); explicit distinct terminators
remain engineering and are accepted by the Rust producer.
