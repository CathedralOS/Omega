# Result context and qualified scalar observations

The initial sample landed at `dd2fe591441716bb07d5cc6caf570ffb694c5941`.
Its physical-domain generalization exposed two general compiler failures.

On compiler sources `58857ba614b660fe0ccc7ca31339053d3c881695`, an implicit
final call to a named machine family with plain `f64` and `f64 in TravelTime`
results refused, even though both overloads declared their result types and
there was no guard. The terminal-call capture discarded the receiving result
context before overload binding. The frozen witness interprets 2500 milliseconds
as 2.5 seconds; selecting the empty result projection instead returns 2500.

Stage 02 now preserves the current state's declared result type on its existing
terminal capture; absent/inferred contexts retain ordinary inference. Stage 04
passes the declared result context to final expression statements through the
existing expectation walker, including match arms. Guarded calls are not moved.
Exact result-domain selection remains separate from predicate, compatibility,
custody, effect and provenance admission, as required by
[result-domain overloads](../../../../../wiki/spec/language/domains.md#result-domain-overloads).

A separate minimal witness constructs `Observation { time: f64 in Time }`
with value 2.5, then explicitly erases its field to `f64`. Terminal inspection
retained an exact `OperationResult` produced by `EstablishRecord`, a relevant
field identity, and `RefinedScalar(Binary64, Truth)`. Native lowering refused
`InvalidPrimitiveScalarRead(OperationId(3))`: read geometry excluded every
static restricted field. The read-only field query now rejoins that same
unique declaration for static paths as well as selected/reference paths.
Writable projection rules, atomic exclusions, field predicates, view formation,
producer identity and availability/access checks remain intact.

The sample uses separate SurfaceOffset, Velocity and TravelTime domains and a
result-overloaded direct-time helper. Its numerical formula, bounded input,
exits and frozen independent Fermat/crossover oracle are unchanged. Regression
fixtures independently cover the minimal field read, shared and nested reads,
conditional result selection, named states, transitions, fields and arguments.
Refusal controls require missing result projections, routed establishment,
positive predicates, exact carriers and valid receiving-field bounds.

Qualification follows current-main integration and is recorded with exact
compiler, native and Terminal hashes in the preserved recovery evidence.
Independent reviewer `/root/independent_review` (`gpt-6.1-sol`, medium) reviewed
the integrated source against the canonical spec, including the writable-loan
and RepresentationView interactions. Existing filesystem/runtime authority,
proof and custody checks remained enabled. The user explicitly authorized the
bounded host route after the sandbox's UID65534 ancestor caused a custody
refusal and the first automatic approval review rejected that route. The exact
287-row package inventory approval was reused only after inventory inspection;
source-risk declarations do not grant host access.

Behavioral qualification used compiler tree
`94d0a22e0a7e8d585a0a6d64504e6f60d0c0e4b3` on main
`292c990572a9eb284068ca901053debf44993761`. Compiler SHA-256:
`119e3c1a50dab1fb013fbcf077a1db21b20cf6d05a83907641b94e24127d34a5`.
Native image: `29ae920f0fdb6b2a696991e8c131ca7677259137d94bdf8d1ffd57eba27e9a5c`.
Captured Terminal: `1ba9c9ea5d85812311aeb1a01743d1cf52cb96740726935c0f133bb00feda620`.
Frozen oracle: `d88a8ecaa3b4a8a2494458af2395cbec93020f30702bc4ca91a02bc1465ba61e`.

All 31 native and 31 actual captured Terminal numerical/refusal records passed,
including the invalid interface bound. Source corpus: 58 selected fixtures,
24 checked and 34 intended refusals; zero wrong reasons or lost successes.
The 24 runtime-leg observations contain 20 actual paired executions, two
build-only fixtures, one declaration-only check and one custody-audit refusal.
All executed authored exits and stdout oracles passed, including the new
result-context stdout in both engines. Included controls cover guarded call
effects, result overloads, restricted loans/handback, atomics, returned views,
mutable IEEE parameters and closed-sum dispatch. Both sample discovery gates
passed. The observer's 159 exact recorded library hashes were independently
reviewed alongside the numerical streams; exit parity alone was not acceptance.

The additional `post_mutation_guard_alias` control produces native exit 0 on
the normal CLI but refuses the corpus's independent retained-target audit.
The refusal already occurred with candidate `9b02f7d` on main `20ad71d`, before
this qualification's incoming integrations. `reference_custody.rs:2424` rejects
terminal `DiscardRoot` cleanup while its referent remains suspended by a live
loan. The witness has no result-overloaded final call or qualified scalar field;
its Boolean literal result and unrestricted u64 field do not exercise these
repairs. The affected custody/descriptor cleanup readers are unchanged. This
source-reader attribution establishes an unrelated audit failure; it does not
claim standalone baseline execution or a false positive. Both observations are
preserved, and the audit refusal is excluded from executed counts. The normal
CLI intentionally avoids replaying the audit performed by the corpus runner.
No custody check or unrelated golden was weakened or rewritten.

Main `74916c95ae323f8f0e9cb2e4936517dea0ca6537` subsequently changes expression
fact admission and delays unavailable intrinsic-row refusals until entry scope
selection. Every row admitted by the recorded successful producer already had
a closed execution identity. Independent review confirmed the same completion,
catalog and authority checks apply to those rows, so the above numerical
records remain applicable as evidence of the recorded producer. New source
qualification uses tree `718f1f0c9eb81c7f6d0733bf38c010fb8fa02d71` on that main,
compiler SHA-256
`5abdec170a081e4c95bd973bd2eec2d97aeb0dc4006e2a2ff08b9f4e3ddf7fd8`.
The fresh source matrix again checked 24 fixtures and rejected 34 intended
controls, with zero wrong reasons or lost successes. Compiler-custody package
review was renewed through the normal reviewed route before the sample gate.
The subsequent main changes through `1877bf177f2e7d1099689a82008acff01a523a75`
touch unrelated samples and task bookkeeping; governing inputs are unchanged.
Qualification records retain their actual producer identities. Final publication
requires an exact-SHA workspace-aware landing receipt for the integrated tree.

After integrating Terminal format 152 and module-owned service catalog identities,
fresh qualification uses compiler tree `8d3a0acb500a7f180e4d397683da5ed40e0db8dc`
on main `77cde1c51efdee32386859b1d723c18909ba2a67`. Compiler SHA-256:
`fa6d7f8c5578ac7beea2d12358202fce064804033b9a7f7eaacf56b6e9701039`.
Native image remains `29ae920f0fdb6b2a696991e8c131ca7677259137d94bdf8d1ffd57eba27e9a5c`;
the new captured Terminal artifact is
`125fef238ac0045e00e60b43a9bf228edeec03be7f2210ddf5be298e82bee72d`.
All 31 native and 31 Terminal records again satisfy the unchanged oracle.
The private observer now decodes the exact serialized native-report artifact,
checks canonical round-trip equality, and executes that decoded artifact with
the report's unchanged runtime provisions. Both new runtime witnesses pass
in native and Terminal, including authored stdout. All seven new source
controls again produce two acceptances and five precise refusals. Normal
package update and both sample gates pass with the fresh compiler identity.
The incoming range-access rules are unexercised by this application and these
record/overload fixtures; earlier broader source/control evidence remains
at its recorded producer. The landing lease was released for the codec and
Console compatibility rebuilds; no gate was waived to meet its deadline.

Main e5e3af49604c31217eb4b591b95be8fb6a12dc94 subsequently changes primitive
locals with omitted initializers and their scalar ordinals. Independent review
found none in the application, seven new fixtures or relevant core/std sources.
Omitted library declarations are aggregates; aggregate entry initialization
and the negative fixture's explicit zero literal use unchanged paths. The above
format-152 execution evidence remains applicable at its recorded producer,
with a fresh exact-commit landing receipt for the final integration.

Executor preflight passed eight doctor checks with no failures/warnings; Linux
x86-64, 4-CPU/16-GiB quota, pinned Rust 1.100.0-nightly and mbx 1.21.0. One Rust
build ran at a time. The first repair rebuild took 4m31s; the later main
integrations took 2m30s and 1m48s with runner builds of 37.58s and 32.72s.
The observer links 159 exact recorded libraries directly, avoiding a separate
Cargo build, and all native numerical cases reuse one retained executable.
