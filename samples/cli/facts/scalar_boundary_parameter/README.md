# Routed scalar boundary parameters

An issued scalar is at least 100; a boundary parameter preserves membership and
justifies exact subtraction. Every place uses the same `AtLeast100` domain,
exact `issue` route and `Consume::subtract` operation. Positives use
`START = 100`; each reject twin changes only that input to 99, violating the
same bound before it can be issued. No caller guard reconstructs the fact.

Owning migration: **BUILD-OWNED-TARGET-SELECTION**, alongside the filesystem
prerequisite below. The four identical issuer bodies now share one ordinary
`issue` machine with the same signature, `value >= 100` precondition, `{ value }`
body, and concrete issuer route selected by the domain's access policy through
`reflect::callable(issue)`. This preserves exact-machine
origin authority; it does not replace it with a trait route or let arbitrary
providers mint the qualification. Every caller still receives `u64 in AtLeast100`
before `Consume::subtract` consumes the bound.

The target-body place selects ordinary `TargetSubtractCode` through a private
`TargetSubtract` requirement, preserving the original qualified input, subtraction
call and `Consume` ceiling. The existing `Consume`/`Actual` selections and all four
hosted entries are unchanged. The old target-family result and authorization
repairs described below retain their historical producer identities.

This is the next prerequisite of **FILESYSTEM-PLATFORM-SERVICES**: the Windows
transfer leaf requires an already issued synchronous-handle classification.
This row tests compiler transport, not Windows mode bits, native lifetime or
the truth of a foreign contract.

The places are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[a nested machine](other_state/main.omg), [another machine](other_machine/main.omg)
and [target body](target_body/main.omg). Each place has an `invalid/main.omg`.
Matching fixtures are under
`tests/omega/{pass,fail}/facts/scalar_boundary_parameter_through_<place>_{exit,rejected}`.
Recorded at `abda3e427bc9+wt`: nine `_exit` positions carry `checked`
(`check-target:macos_arm64`), `built ... exit:0` and
`interpreter-exit:0 interpreter:agrees` rows; nine `_rejected` twins carry the
`99 >= 100` structural rejection. 0 keys lost, 0 timeout rows.

Each positive binds all four hosted targets and expects native and Terminal
exit zero. Run `omega run --both <place>/main.omg`; check its twin with
`omega --check <place>/invalid/main.omg`.

Requirements and limits:

- Establishment: `issue` independently owes `value >= 100`; naming a route
  does not prove that predicate or give another machine issuer authority.
- Identity and lifetime: scalar membership remains on the exact issued value
  through the selected place; equal numeric bits are not routed provenance.
- Composition: the boundary parameter carries membership to the checked
  provider, where `value - 100` consumes the retained fact directly.
- Soundness: numeric predicate rows cannot replace the independent domain
  identity. Provider selection cannot weaken the requirement or mint an input.
- Psi 06–08: retain ordered scalar-domain admission rows and the complete
  operand type through statement and nested calls. Terminal artifacts retain
  those rows without adding a runtime tag or replaying source proof.
- Omega 00–02: select the checked `Actual` provider with an unchanged
  primitive carrier and exact boundary identity; classification adds no ABI word.
- Omega 03–06: preserve the scalar through actual calls, register homes and
  activation frames, including target-scoped bodies.
- Omega 07–09 and emission: emitted native execution must agree with Terminal
  execution at zero. Producer/target-bound records belong to qualification.

The original positive checked source but native production refused a missing
`Consume::subtract` boundary plan. The Windows customer reported `structural
precondition representation` for the equivalent routed scalar formal.
The repaired row executes all nine places at zero in native and Terminal Psi
on macOS ARM64; all nine reject twins fail their unchanged `99 >= 100`
obligation. Supporting fixtures cover explicit `requires` forwarding, an
immutable snapshot surviving field replacement, and a field passed through
another machine. Producer-bound qualification records retain those observations.

Field membership is a receiving contract, not permission to create an alias
that forgets it. A primitive reference currently has no representation for
these nominal scalar memberships; writable formation and call presentation
must reject that erasure. A reference to the record retains its declared field
contract, and shared bare observations cannot write or manufacture membership.
Explicit byte-offset aggregate re-views do not yet retain scalar-field origin
correspondence; they cannot introduce membership or erase a writable invariant.
Shared views into a bare aggregate may forget those observations only when
the target permits no shared mutation. Developmental wire checks exercised
zero provisioning, detached initializers, wrong domain identities and stronger
provider requirements; their producer-bound observations remain qualification
evidence, not permanent compiler-internal Rust tests. Permanent language
controls live beside the executable fact fixtures.

This is macOS ARM64 execution evidence, not a four-host runtime claim. Windows
runtime acceptance and the full portable provider remain unfinished under the
parent task; the owner will arrange a later Windows agent for execution.

The migration also observes both match paths: the unchanged START=100 produces
subtraction0 and marker17, while the same issuance/qualification operation on101
produces subtraction1 and marker23. The target-body place likewise observes100/0
and101/1. These checks follow formation and consumption; they do not prove the
qualification for the compiler. All nine invalid twins still differ only by
START100→99 and fail the issuer precondition before issuance.

The current matching-host baseline is the scoped macOS15 run41323737111 with
optimized source08798a08b8d2ef02bc438ece549d6006539d0d5f: all18original Source
pins (nine positives/nine intended rejections) and nine native/Terminal agreements
pass. Its two Source/two runtime historical timing slowdowns remain reported,
not semantic failures. Fresh migrated-row observations use optimized producer
`13c1304502d60141ce3403a0c5367d45e0792846` with its matching immutable SDK:
all nine public programs exit0 in native and Terminal execution, all nine invalid
projects refuse the exact99>=100 issuer obligation, and the eighteen Source pins
and nine runtime pins pass. The target-body project prepares/checks for all four
hosted targets. Six Terminal counterfactuals produce two unchanged exit0 controls
and four typed traps after changing only either marker, the issuer body to100,
or the selected subtraction body to0. These distinguish both branch outcomes,
exact input behavior and selected operation without adding a public postcondition.

The producer includes the fully qualified retained-loan compiler published in
PR1213. It is not relabeled as the later Source base: the twenty-file production
delta is reviewed against this row. Mixed data-where predicates/restoration windows,
generic-instance carriers, recasts and static provider applications are absent;
the scalar comparison/subtraction leaf laws and normal contracts remain the same.
The SDK and Cargo inputs are identical. The initial marker probe used unsupported
Rust-style `*route` syntax; the corrected Omega `route = marker` source passes,
and that parse-only setup failure is retained separately.

Fresh runtime is macOS ARM64 only. No Windows runtime or full filesystem/Build
capability closure is claimed; the remaining scalar-reference, byte-view and
parent transport limits above stay open.
