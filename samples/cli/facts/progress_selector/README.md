# Finite progress subjects through selected operands

Requirement: `TERMINATION-RANKING-CHECKS` / TPR6 preserves every possible exact
progress subject when a reference is selected, passed to a call or transferred
to another state. Build-owned selection for this row is described below.

Each program selects shared references to two distinct `Token` values from a
fixed array, then calls `consume` using their existing `Fair` qualifications.
Both selected values must retain their qualification. The caller observes 37
and 43 with exactly two calls and authored exit 0. The returned-array helper
also declares the lifetime relation needed to return both references. All
positive programs bind `ProgramEntry` for the four hosted targets.

The nine places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [other machine](other_machine/main.omg),
and [target body](target_body/main.omg). Each invalid example removes only the
right token reference's `Fair` requirement:
[local](invalid_local/main.omg), [field](invalid_field/main.omg),
[parameter](invalid_parameter/main.omg), [call result](invalid_call_result/main.omg),
[match join](invalid_match_join/main.omg), [const name](invalid_const_name/main.omg),
[nested machine](invalid_other_state/main.omg),
[other machine](invalid_other_machine/main.omg), and
[target body](invalid_target_body/main.omg). The corresponding corpus fixtures
are `tests/omega/{pass,fail}/facts/progress_selector_through_*`. Four existing
controls replace a slot, replace the entire array, introduce an unknown
alternative or write the referenced contents; each must reject reuse of the
old qualification. Supplemental snapshot controls remain under
`tests/omega/*/termination`.

The original baseline at `d6cf554fd1cbf20551983d29bae26f5cf3b81664`
rejected the positive row because it could not prove the published termination
promise for the selected call. The repair checked every possible producer of a
selected reference rather than choosing one token at compile time. Array size,
lifetime annotations, equal scalar values and the callee's promise do not
supply `Fair`. Unknown producers reject the call, and existing permissions still
govern whether either token may remain borrowed. The historical Linux x86-64
run then executed all nine positives with native/Terminal exits 0/0, rejected
all nine missing-subject twins with `published termination contract does not
cover that exact subject`, and rejected all four stale/substituted controls.
That observation covered the original programs on Linux, not the later
Build-owned migration or the three other hosted targets. Mutation summaries
were unchanged. The separate owned-arrival programs below cover local token
storage that the original row did not establish.

The row exercises these obligations:

- Establishment: the accepted `Grants` provider supplies `Fair` on both token
  results. `select` declares both exact input requirements. Runtime equality
  checks are oracles and supply no static proof.
- Identity and lifetime: 37 and 43 distinguish the referents. Copies preserve
  the selected address; snapshot controls preserve earlier bindings and values.
  Replaced slots, replaced arrays and live writes cannot reuse old facts.
- Composition and soundness: calls and state transfers preserve every possible
  subject. Every alternative needs an existing qualification, and normal loan,
  access and termination checks still apply. Erased lifetime parameters create
  neither runtime arguments nor proof of a selected body's behavior.
- Terminal Psi and native execution: the produced Terminal artifacts execute,
  including state arrivals, and agree with native exit 0. Retained
  `program.oterm` files are replay inputs rather than compile-only evidence.
- Providers, layout and calls: explicit boundary acceptance and Build selection
  supply the tokens. Reference arguments and helper results carry real pointer
  words. Runtime checks observe both selected values and exactly two calls
  across nested calls and state transfers.
- Cleanup, storage and encoding: existing loan/escape controls retain their
  meaning. The historical Linux ELF images execute and agree with Terminal;
  distinct local token homes are covered by the owned-arrival programs below.

These observations establish this finite-selector operation. They do not close
all of TPR6: generic/dispatched helper origins, unresolved reference boundaries,
growing projections, recursive aggregate results, opaque boundary ABI and
additional layout/backend obligations keep their separate acceptance.

## Owned backing and additional descriptor boundaries

The `TERMINATION-RANKING-CHECKS` / TPR6 continuation is in
[`owned_arrivals/`](owned_arrivals/local/main.omg). It copies the qualified tokens
into separate local homes before selecting and transferring a reference. Its
nine places are local, field, parameter, call result, match join, const name,
nested machine (`other_state`), other machine and target body. Each retains
37/43, exactly two calls, exit 0 and an invalid example omitting only the right
input's `Fair` requirement. Corpus mirrors are
`tests/omega/{pass,fail}/facts/progress_selector_owned_arrival_through_*`.

The [`nested` production](owned_arrivals/nested/local/main.omg) first puts the
same reference array inside an owned envelope, reads the stored references
through two explicit loads, and transports them through the same nine places.
Its missing-subject twins are pinned as
`tests/omega/{pass,fail}/facts/progress_selector_nested_arrival_through_*`.
The call-result place passes the copied array through a checked helper and
preserves the two independently captured token references in the result.

Supplemental [`affine`](owned_arrivals/affine/main.omg),
[`owned parameter`](owned_arrivals/owned_parameter/main.omg),
[`direct nested arrival`](owned_arrivals/reference_boundary/main.omg),
[`nested reborrow`](owned_arrivals/reference_reborrow/main.omg),
[`borrowed ingress reborrow`](owned_arrivals/borrowed_reborrow/main.omg), and
[`owned input replacement`](owned_arrivals/owned_input_replacement/main.omg)
programs test cleanup and reference identity; their missing-subject twins are
beside them. The [`frozen ingress`](owned_arrivals/frozen_ingress/main.omg),
[`repeated rebinding`](owned_arrivals/repeated_rebind/main.omg),
[`replacement helper`](owned_arrivals/replacement_helper/main.omg), and
[`mixed ingress`](owned_arrivals/mixed_ingress/main.omg) controls check that an
earlier pointer copy keeps its original meaning when a formal is rebound.
Borrowing the formal's own pointer storage still prevents replacing that
storage. Both productions also reject replaced slots and stale contents under
their `controls/` directories. A later equality guard cannot restore the lost
progress qualification.

Baseline `717593f593c656f865c7a322fd2874b4bd168afc` rejected the positive that
saved local owned references with `state loan: distinct backing alternatives`.
The repair tracks each local home that a selected pointer could reference and
checks every home's lifetime separately. It neither chooses one home at
compile time nor grants permission to move or dispose of a home. Normal
formation, availability and cleanup checks still apply to local homes, owned
inputs and local results. A borrow of the pointer's storage remains distinct
from a borrow of the token it points to; a borrowed input stays caller-owned.
Public target-family signatures share their authored entry requirements with
their bodies, without publishing facts supplied only by a particular caller.
The compiler preserves exact stored reference identities, the selector
computation and every physical load. Copying an aggregate copies its selected
references rather than substituting the containing pointer. A returned
reference is checked against the storage version that supplied it.

These separate programs cover establishment by accepted boundary results,
identity after writes and rebinding, calls and all nine places, loan/custody
checks, Terminal lowering, pointer/aggregate layout, cleanup and frame storage.
Their retained execution evidence is Linux x86-64 native/Terminal agreement;
although they declare all four hosted entries, other-host execution remains
unverified. This migration's new host results do not extend that evidence.
Generic/dispatched helpers, arbitrary recursive arrivals, additional access
modes, ranking and cyclic execution remain on the parent task. These finite
productions do not close all of TPR6 or `TERMINATION-RANKING-CHECKS`.

## Build-owned target-body migration

The target-body place replaces the four target-prefixed `select` declarations
with the `SelectedProgress` boundary requirement and ordinary named
implementations chosen by explicit Build rows. This is the remaining
**BUILD-OWNED-TARGET-SELECTION** transport for this existing fact. The other
eight places keep their original programs.

The requirement retains both exact `Token::Fair` input subjects, `index < 2`,
and `terminates`. Every implementation performs the original array selection
and `consume` call. The caller still observes 37 and 43 with exactly two calls.
The twin removes only the right subject's qualification from the same public
contract and its implementations; the caller's two granted tokens cannot
justify that broader interface. Its original missing-subject diagnostic stays
unchanged. Build chooses code and grants no progress qualification.

The compiler records when `Fair` came from a call input. Copies, joins and
state arrivals preserve that dependency. The body may use the fact normally,
but cannot use it to remove a premise required by the helper's authored
termination promise.
Native parameter lowering carries progress requirements in the dedicated
progress representation; it keeps the ordinary signature comparison strict.

The programs and Build files stayed unchanged during validation, together with
the 37/43 oracle, exactly two calls, exit 0 and every diagnostic pin.
Validation uses the nine positive places,
their nine twins and four existing replacement/unknown/stale-content controls.
Each completed host below passed nine positive Source checks, all 13 intended
rejections, nine corpus native/Terminal 0/0 pairs, and nine positive/nine invalid
ordinary samples. The compiler check and four facts-owner algebra tests passed
on each prepared host. Corpus runs used one worker and 120-second limits;
ordinary samples retained their original 300-second stage limits.

| Actual host | Actual compiler/program observer | Validation run | Result |
| --- | --- | --- | --- |
| Linux x86-64 | `dfb79c300aae3917c2681b51c319f58741f2664c` | [42218867485](https://github.com/CathedralOS/Omega/actions/runs/42218867485) | Full scope passed |
| Linux ARM64 | `dcfb6f8937b6a739731fe5a2100e27b6cde363f6` | [42224888218](https://github.com/CathedralOS/Omega/actions/runs/42224888218) | Full scope passed |
| macOS ARM64 | `dcfb6f8937b6a739731fe5a2100e27b6cde363f6` | [42224888218](https://github.com/CathedralOS/Omega/actions/runs/42224888218) | Full scope passed |
| Windows x86-64 | `dcfb6f8937b6a739731fe5a2100e27b6cde363f6` | [42224888218](https://github.com/CathedralOS/Omega/actions/runs/42224888218) | Full scope passed |

These observers carry source `b6b156b5e97a04c1806b6312df5225048b9888ff`,
which integrates the repair with fixed published main `c417f61c`. Actual raw
Source/BOTH comparisons remain moved/exit 1; they are not rewritten as fresh
zero-exit comparisons. Updated Source records retain their actual measured
`dfb79c30` source, times and diagnostics. Corpus products include authenticated
native images and Terminal artifacts. Sample records contain explicit numeric
engine exits and intended-pin decisions; their individual products and stderr
are not in the upload.

The earlier passing target-declaration baseline and failed migration are retained
at runs [42199181794](https://github.com/CathedralOS/Omega/actions/runs/42199181794)
and [42200336267](https://github.com/CathedralOS/Omega/actions/runs/42200336267).
They keep their original programs and compiler identities. This migration does
not close broader BUILD-OWNED-TARGET-SELECTION or termination acceptance, and
does not restamp the separate owned-arrival productions above.
