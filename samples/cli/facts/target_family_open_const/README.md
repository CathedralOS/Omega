# Open const through selected implementations

Owning outcome: **BUILD-OWNED-TARGET-SELECTION** in `TASKS.md`.

Fact: an alpha-renamed const bound survives an ordinary Build-selected implementation, permitting subtraction from 64u64.

The public requirement's `Bound`, the implementation's `Cap`, and forwarding
machine's `Maximum` are different lexical declarations. `forward<64>(42)` must
retain the returned bound and justify `64u64 - result` directly; its runtime
result is 22. Each reject twin changes only the forwarded bound to 40 and keeps
input 42, so it must reject `42 <= 40`. No caller guard establishes the bound.
All nine places use the same operation and sentence. There is no root program.

The retired source had four equivalent OS-prefixed bodies with differently named
const binders. One ordinary `PortableBoundedValue` implementation now supplies
an exact `BoundedValue` requirement through explicit target-keyed Build rows.
Its three distinct binder scopes preserve the original alpha-renaming mechanism
without four copies of the same body. The target-body place additionally selects
an ordinary subtraction implementation. Public contracts establish the bound;
Build selection supplies no additional source-level facts or conformance evidence.

The match place executes both choices and observes which arm ran. Both arms
carry the same bounded value; each choice forms the subtraction before checking
its distinct arm marker. Choosing either wrong arm fails the runtime oracle.
Those observations supply no arithmetic premise.

## Governing contracts

[Generic applications](../../../../wiki/spec/language/generics.md#applications-and-specialization)
retain exact static carriers and substituted results.
[Named implementations](../../../../wiki/spec/language/conformances.md#declaration-and-selection)
own their inline members and independent telescopes.
[Build-owned selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
replaces target-prefixed declarations with ordinary checked implementations.
[Invocation contracts](../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use)
require exact actual-argument substitution and independently proved returns.

## Requirements and evidence

- Establishment: each implementation proves `result <= Cap` from its input
  contract; the public method and forwarding machine retain their own binders.
- Identity and lifetime: a const selection keeps its exact carrier and lexical
  declaration identity. No borrowed storage or provenance is introduced.
- Composition: calls substitute the same actual64; joins retain the bound from
  every arm. The static control changes the argument obligation alone.
- Soundness: the operation forms from the transported bound, before runtime
  checks. Neither a selected body's private facts nor a post-hoc guard justify it.
- Psi05–07: exact static applications and ordinary boundary-call demands must
  survive checking, proof erasure, Terminal lowering and interpretation.
- Omega00–02: explicit Build rows realize the same public requirement for four
  hosted targets; actual const applications, ABI and calling policy stay exact.
- Omega03–09: native calls, value homes, instruction encoding and image emission
  must retain the authored values and agree with Terminal execution.

The current warm baseline `30641d4e0ddc80b1da0504511cc9574327a0bc59`, using
optimized producer `767215d6ee2d6f734d20c963f1919eae9565faf7` with byte-identical
compiler/library inputs, passes all nine original paired executions and their
nine intended rejections. The first well-formed ordinary replacement wrongly
refused its forwarding postcondition on that producer. The repair admits the
complete original static tuple before transporting it to the public invocation,
and recognizes an already-substituted exact supplying binder in that admitted
mapping. It does not use the selected implementation's private guarantees.

Optimized producer `ad909308cdb32b3c95b5cbf8093262e4630e0c51` checks all nine
migrated places, rejects their nine `42 <= 40` twins, and runs all nine with
native exit 0 and Terminal exit 0 on macOS ARM64. The target-body pin also runs
natively on macOS x86-64 with Terminal agreement. Its unchanged Terminal artifact
runs with the supplied Build admissions for all five recorded targets, including
the four required hosted targets. Two additional checking fixtures distinguish
the exact forwarded binder from an unrelated available binder; neither is a
runtime claim. Evidence retains the historical producer and source identities
under `build/content-provider/open-const/`; other native hosts remain unmeasured.

A focused same-input comparison of 139 nearby contract fixtures has no semantic
moves between the historical and repaired producers. One preexisting collateral
golden expected-diagnostic gap remains recorded on both sides. Terminal-only
counterfactuals keep the valid bound but change the payload or selected match arm;
both trap, while the positive exits 0. No native trap run is claimed.

The earlier Windows `e4d9c74577f9ed221697b73113b045220364f9de` sibling-binder
refusal remains historical; it is superseded by the actual current baseline.

Full parent acceptance still includes generic/static applications elsewhere,
callbacks, library/product/sample migration, native binding retirement and all
remaining parser/IR/codec/runtime family paths. This row does not close it.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [call_result](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [match_join](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [const_name](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [other_state](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [other_machine](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Target body | [target_body](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Matching pins remain under `tests/omega/{pass,fail}/facts/target_family_open_const/<place>/`.
