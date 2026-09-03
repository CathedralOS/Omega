# Design Brief: Build-Time Evaluation — Const Evaluation + Trait Generators

Omega uses hermetic **semantic evaluation** for constants, proofs, plans, and
trait generators. It has no `comptime`, macro, or `#run` keyword.

Do not confuse this facility with [`build.omg`](build_and_package_model.md).
`build.omg` is build orchestration in a build-host world with explicitly
injected capabilities. Semantic evaluation is the compiler's target-semantic,
hermetic evaluator. They happen before runtime for different reasons and
have different admissibility and reproducibility contracts.

## Governing laws

> Evaluating an eligible runtime-capable machine during compilation produces
> exactly the value that executing the same machine, with the same arguments,
> selected conformances/providers, and target semantics, would produce on the
> target.

Evaluation changes *when*, never *what*. There is no `is_build_time()`
predicate or other phase observation. A proof-only machine may have no runtime
representation, but it follows the same core value semantics and cannot observe
that the compiler is evaluating it.

> The position requests evaluation. The concrete invocation's complete
> normalized contract decides whether evaluation is legal.

No declaration marker restates that judgment.

Three adjacent concepts remain distinct:

1. an ordinary machine invocation may be evaluated before runtime;
2. `const` gives the resulting pure value a compile-time name but no storage
   identity; and
3. one addressable immutable image occurrence, if later admitted, is a storage
   feature rather than another meaning of `const`.

They compose directly:

```omega
machine Imports::write_file() -> DllImport<12, 9, 0> {
    DllImport::PeByName {
        library: "kernel32.dll",
        export: "WriteFile",
    }
}

pub const WRITE_FILE: DllImport<12, 9, 0> = Imports::write_file();
```

No `const machine`, `comptime`, macro language, or declaration-phase predicate
is introduced. Local mutation, allocation, recursion, and temporary borrows are
ordinary implementation techniques inside an admitted evaluation; only inputs,
effects, resource use, and escaping results are restricted.

## Two pre-runtime execution worlds

| Property | Semantic evaluation | `build.omg` orchestration |
|---|---|---|
| Purpose | constants, proofs, plans, generators | dependencies, target selection, staging |
| World | selected target semantics | build-host services |
| Reach | hermetic | explicit scoped capabilities |
| External input | compiler-materialized values | provider operations and receipts |
| Result | value/evidence consumed by compilation | `Build`, staged artifacts, lock inputs |
| Reproducibility | semantic cache identity | operation ceiling + realized receipts |

`build.omg` may read files or use an admitted network provider. A proof machine
cannot. The handoff is explicit:

```text
build.omg observation
  -> staged value/artifact plus receipt
  -> recorded build input
  -> compiler materializes an ordinary value
  -> semantic evaluation consumes that value
```

There is no route from an ambient host observation directly into a proof,
layout, type, or constant.

## Target-semantic world

The evaluator process runs on the host but interprets the selected target's
Omega world. Its sealed semantic input includes:

- language/evaluator semantics version;
- target primitive semantics required by the program;
- selected conformance and provider-plan identities;
- normalized machine implementations; and
- explicit compiler-materialized arguments.

It cannot observe host filesystem, environment, clock, randomness, network,
pointer width, floating-point behavior, or process state. Target facts used as
ordinary data arrive as explicit values, selected requirement inputs, or the
closed typed target-semantic projections defined below. The sealed target
capsule is an evaluator/cache input, not a general source-visible `BuildWorld`.

Structured proof/static values used by future indexed domains evaluate in this
same sealed world. Their eligibility is narrower than general evaluator output:
each index value must have decidable structural equality and one unique
canonical encoding. The resulting canonical value, not the evaluation trace,
enters a closed domain's semantic identity. Open symbolic index expressions
remain normalized artifact data rather than arbitrary evaluator programs.
Until quotient-backed canonical rationals land, a `Rat` used in an index must
also prove at that site that its denominator is positive, its signed
coordinates are cancelled, and its numerator magnitude and denominator are
gcd-reduced.

Canonical compile-time values retain one atom across generic,
monomorphized, and structural identity. Eligible values are integers, Booleans,
fixed arrays, records, and cases; declaration order determines aggregate
identity. Floating/text values, references, slices, dynamic identities, and
boundary-opaque data are not canonical generic atoms. Indexed qualification and
constrained calls preserve the same closed atom or const-binder identity.
Quotients and constrained records fail closed unless their canonical
representative and required facts are proved at the use site. Arbitrary machine
evaluation never participates in type equality.

Target equivalence is mandatory. Build-time floating-point operations use the
same executable `FloatSemantics` meanings as runtime operations, including
format-specific rounding, classification, conversions, square root, and fused
versus unfused arithmetic. A target-specific realization may refine raw NaN
bits, but the portable promise is equality of `FloatMeaning`; observing
computed NaN payload bits requires a proof or selected realization that fixes
them. Cache identity includes every selected realization and semantic control
state that can affect the result.

### Target-semantic observations

One compiler-owned, versioned, typed target capsule is the primitive bridge
between target selection and hermetic semantic evaluation. `build.omg` selects
that capsule; it does not mint individual proof facts. The evaluator and backend
consume the same closed capsule so source interpretation and emitted target
meaning cannot drift.

The capsule exposes a closed vocabulary of subject-qualified observations, not
a first-class runtime reflection object and not an ordinary replaceable
provider. Language/toolchain schema owns the primitive projections. Packages
may derive constants, propositions, and plans from those projections but cannot
append a new primitive observation or claim what an existing carrier means.
The vocabulary contains language-semantic facts such as carrier formats,
selected floating semantics, and fixed layout interpretations. Deployment
facts such as installed memory, page geometry, devices, or current CPU features
remain explicit provider, capability, installation, or admitted-hardware input.

Conceptually, core can derive declarations such as:

```omega
pub const addr::Bound: Int =
    TargetSemantics::address_bound<addr>();
```

The projection spelling is pending the target-capsule implementation; the
semantic shape is settled. Subjects are explicit. A target with several address
spaces, data layouts, or execution modes does not invent one global answer:

```omega
TargetSemantics::address_bound<addr>()
TargetSemantics::endianness<NativeDataLayout>()
TargetSemantics::guaranteed_entry_stack<UefiX86_64>()
```

If target closure has not fixed the named subject, evaluation rejects. Values
and interpretation selectors use the same heterogeneous typed projection
mechanism; endianness need not be forced into a numeric encoding.

An entry-profile guarantee is a canonical observation about the selected
target contract, not proof that the executing firmware satisfies it. For
example, `guaranteed_entry_stack<UefiX86_64>()` supplies the symbolic byte bound
used by entry WCSU composition and private-stack sizing. The physical-arrival
provider separately admits that the actual invocation conforms to that selected
profile. Changing the profile or guarantee changes target closure; accepting a
profile never turns its real-world conformance into a derived fact.

For the closed UEFI x64 application profile, target closure selects the x64
boot-services minimum from UEFI Specification 2.11 section 2.3.4: 128 KiB of
available stack with 16-byte alignment. The closure retains that numeric value
and alignment beside the exact symbolic application. They remain target-
contract observations; physical-arrival admission must still establish that
the executing firmware invocation conforms to the selected profile.

Target observations are canonical compile-time values. They may participate
wherever an equivalent ordinary constant may participate, including array
lengths, const-generic applications, proof expressions, further semantic
evaluation, and evaluated layout or calling plans. This permission adds no
conditional declaration grammar: an ordinary constant cannot add a record
field, remove a case, alter multiplicity, or splice a declaration, so neither
can a target observation. A future `UInt<const Bits>` constructor would decide
its own admitted widths, lowering, and identity; target dependence neither
creates nor forbids that separate feature.

An unscoped constant or public type application may therefore be
target-polymorphic. Its declaration path is one source identity, while each
closed application retains its type/const/machine substitutions and target
dependencies. A target-neutral intermediate exports the symbolic recipe rather
than pretending to own one concrete value. For example,
`[u8; TargetSemantics::address_bits<addr>()]` remains symbolic until closure and
then becomes an ordinary exact array application for that target.

Target-dependent native geometry normally remains a plan quantity rather than
an array merely because the latter is legal. Calling-plan staging, or an extent
reserved from a validated plan's size and alignment and then used through
placement, preserves padding, alignment, and provenance. This is modeling
guidance, not a type-system prohibition. Genuinely different native field or
case sets are a hard boundary: define distinct nominal ABI schemas and let each
exact target realization privately select its schema behind one stable portable
requirement. `build.omg` never splices fields into an existing declaration.

Target dependence is retained as two normalized dependency kinds:

```text
ObservationApplication {
    projection, subject, projection_semantics_version,
    selected_value_or_interpretation
}

SelectedRealizationApplication {
    requirement_or_slot, exact_realization_application, target_scope,
    normalized_contract_plan_or_binding_fingerprint
}
```

The second kind is required even when selected target-scoped code contains only
literals and never reads a capsule projection. Derived constants, proofs,
plans, public signatures, caches, and artifacts inherit both kinds
transitively. Before target closure an observation remains symbolic. After
closure it may fold to a concrete value, but folding must never erase either
dependency or its target-closure receipt. A verifier reconstructs the
projection or selected realization rather than trusting a folded scalar.

Exact used dependencies are the normative compatibility identity. Keying by
the complete target capsule is a sound conservative implementation because it
only over-rejects reuse; replacing it with fine-grained replay must account for
both dependency kinds before removing that conservative key. Independently
closed artifacts compose only when their applications agree. Adding, removing,
or changing a target dependency in a published signature is a breaking
semantic-API revision. A private dependency changes target artifact identity
and forces rebuilding or relinking without changing the public contract.

Every dependency union also retains a compact origin DAG through aliases,
constants, generic applications, projection calls, and selected plans. This is
required diagnostic data once target-dependent public types are legal: a
composition failure must identify the producer and consumer closures and trace
the mismatching type argument back to the observation or realization that
introduced it.

Two target applications never share a cached value or artifact identity merely
because their declaration path matches. A later exact-replay implementation may
share them only after proving every retained dependency application compatible.

## Admission uses the complete invocation contract

An empty service-reach row is necessary and nowhere near sufficient.
Suspension, blocking, failure/control outcomes, authority, trust, escaping
mutation, and termination are independent contract axes.

The compiler specializes the selected machine contract at the concrete
arguments and available facts, then requires:

- ordinary checked termination;
- empty runtime service reach;
- no possible suspension or blocking;
- no unhandled failure, trap, or abort route for the demanded value;
- no runtime authority acquisition/consumption;
- no escaping runtime mutation; and
- only proof/build-admissible trust and resource inputs.

This is invocation-sensitive. A generally trap-capable `divide` may evaluate at
`divide(10, 2)` when the nonzero obligation is discharged. A position demanding
`divide(10, 0)` rejects before evaluation and names the undischarged route and
call chain. If the evaluator nevertheless reaches a forbidden terminal route,
it reports the trace as a checker/accepted-assumption consistency failure.
Handled result sums remain ordinary values.

Escaping mutation is excluded by the semantic-evaluation bridge itself. Every
compiler-materialized argument is converted into a fresh owned interpreter
value graph, the machine instance is freshly instantiated for the invocation,
and the only value crossing back is a recursive value snapshot. Interpreter
cells and references cannot cross that boundary. Local mutation that computes
the returned value is therefore legal, while no mutation can reach compiler
state, another invocation, or runtime state. Augmenting `build.omg` evaluation
uses a separate API that deliberately returns snapshots of its mutated
arguments and is not this hermetic world.

Fixed arrays, records containing them, and other ordinary recursively owned
values may cross this bridge. A byte literal in an exact fixed-array result or
constant position copies its bytes into that array; a width mismatch rejects.
It does not export a slice into evaluator storage. Equality and hashing remain
the ordinary structural operations of the value's type, and constant-pool
interning is an unobservable emission optimization. Temporary references and
slices remain legal inside the evaluation. A genuinely dynamic owned sequence
uses the ordinary collection model when that model is const-evaluable; semantic
evaluation introduces no special byte-blob type.

## Evaluation and materialization are separate judgments

The compiler derives two internal properties; neither is a user-satisfiable
trait or a source keyword:

```text
ConstEvaluable(T, value)
    the complete result type is pure/copy-eligible and the semantic value may
    cross the hermetic evaluator as an owned snapshot

ConstMaterializable(value, layout)
    the selected representation determines every observable emitted bit
```

The first bounded internal `ConstMaterializable` carrier is live for closed
non-generic `[copy]` records whose recursive values contain only integers,
Booleans, non-NaN binary32/binary64 values, literal fixed arrays, and records of
the same kind. Binary32 values must already retain one exact binary32 value;
materialization never grants narrowing authority. Exact IEEE bits, signed zero,
infinity, and target byte order remain in custody. The carrier retains the exact
typed owned value, schema name and identity, complete validated layout and
normalized fingerprint, exact zero-initialized staged bytes, and a deterministic
identity. Independent replay rechecks exact member placements, derived offsets,
fixed extent and alignment, value shape, byte order, and staged bytes through
the existing atomic aggregate writer. Applying the carrier copies only after
replay succeeds, so malformed evidence or a short destination leaves the
destination unchanged.

The normalized schema/layout fingerprints and deterministic materialization
identity are compact, non-authoritative report coordinates. Only explicitly
named non-authoritative report accessors expose them. Fixed-record and
conventional-sum replay authority comes
from the retained typed value, complete layout report, target byte order, and
exact staged bytes. Even if a substituted layout is assigned the same compact
report coordinate, the hash-free structural comparison rejects it before
materialization or copying.

This slice deliberately rejects every NaN without canonical or selected exact
raw-representation evidence, plus generic/opaque/quotient records, references,
slices, Text, dynamic values, atomics, non-copy records, and malformed shapes.
A separate first pure-sum rung consumes the compiler-owned conventional runtime
layout rather than weakening programmable `LayoutPlan`: `omega-layout` reports
the fixed four-byte tag, authored-order case ordinals, complete all-case payload
overlay, and total extent/alignment. Psi independently rejoins every reported
case and relevant payload field to the typed schema, recomputes the supported
target-independent geometry, and validates only the selected value payload.
It emits the selected ordinal and payload into fresh zero-initialized staging,
so inactive overlay bytes, intra-payload gaps, and tail padding remain zero.
The non-clone carrier retains the selected case identity/ordinal, exact report
and report fingerprint, byte order, bytes, and deterministic report identity.
Replay uses an exact hash-free comparison that treats stable-numbered
case/payload names as presentation while retaining authored ordinal and
complete geometry; replay and short-destination rejection leave the destination
unchanged.

The direct nested rung admits the complete nonempty authored-order set of
direct runtime-relevant conventional pure-sum fields in one closed non-generic
`[copy]` record. `omega-layout` projects the whole-field outer
`LayoutPlanReport` and one per-occurrence field-identity plus complete
conventional nested-sum report from the same built target runtime layout;
source still gains no tag/case placement vocabulary. Repeated occurrences of
the same sum type remain distinct and may select different cases. A distinct
non-clone carrier retains the outer typed value, exact outer report, every
ordered direct field identity, every complete nested report and selected case
identity/ordinal, target byte order, and final staged bytes. Replay revalidates
the current typed program, rejects missing, extra, reordered, or duplicated
occurrence rows, compares every report hash-free, reconstructs every selected
nested buffer, reconstructs the zero-padded outer bytes, and performs one copy
only after every check succeeds. Erased direct sum fields remain outside the
runtime occurrence set.

The direct array rung admits the complete nonempty authored-order set of direct
nonzero literal fixed-array-of-sums fields in that same outer record cohort.
Each compact target row retains one exact outer field identity, literal count,
element stride, and complete conventional all-case element layout. Value-
sensitive custody separately retains each field occurrence and literal index
with its selected sum, so different elements may select different cases without
copying the complete all-case report per element. Projection rejoins every
exact array descriptor, element symbol, count, extent, alignment, and the outer
layout from one target plan. Fragmented, stored-integer, or target-dependent
repeated placement on any outer field rejects. Replay checks the outer and
compact reports hash-free, reconstructs every selected element into contiguous
fixed-array staging, reconstructs the zero-padded outer record, and performs one
atomic copy. The singular surfaces remain exact-one wrappers and reject a
plural cohort.

The first one-level record-path rung admits the complete nonempty authored-order
set of direct outer fields whose exact types are closed non-generic `[copy]`
records with direct conventional-sum sets. Its compact plural report retains the
outer whole-record plan once and one exact outer occurrence, inner whole-record
plan, and complete child row set per field, all from the same target runtime
plan. Repeated occurrences of one inner type remain separate and may select
different cases. A distinct non-clone carrier owns one existing validated
direct-sum record carrier per occurrence. Construction and replay independently
rejoin every schema, layout, occurrence identity, complete child row, selected
case, and byte; every inner and outer image starts zeroed and only the finished
outer image is copied. The singular surfaces are zero-allocation exact-one
wrappers and reject a plural cohort. Exact hash-free comparison makes stable-
numbered field/case/payload renames presentation-only. A fallible memoized graph
walk with explicit record and edge bounds plus a linear authored-order cursor
classifies excluded recursive and deeper sum paths without host recursion,
repeated traversal of shared subgraphs, or occurrence cross-products.

The complete plural fixed-depth rung admits the nonempty authored-order set of
`Outer -> Middle -> Leaf -> direct conventional sums` record chains. Its report
retains the outer whole-record plan once and one exact row per authored-order
outer occurrence; each row's unchanged plural one-level report owns the
complete middle-to-leaf path set. A distinct non-clone carrier composes one
existing plural one-level carrier per outer occurrence, reconstructs every leaf
and middle image plus the outer zero-initialized image in order, replays every
identity and layout hash-free, and performs one final atomic copy. Repeated
nominal middle and leaf types stay occurrence-distinct and may select different
cases. Direct sums at an enclosing layer, arrays containing sums, recursion,
and shallower or deeper paths reject. Target-dependent placement at any of the
three record layers also rejects. The earlier singular producer and consumer
remain exact-one wrappers and reject a plural outer or middle cohort; existing
direct and one-level APIs remain unchanged.

One further singular fixed-depth rung admits exactly one
`Outer -> First -> Middle -> Leaf -> direct conventional sums` chain and no
other sum-reachable field anywhere in the outer schema. Its compact report
retains the new outer whole-record plan and exact field occurrence beside the
unchanged singular depth-two report. A distinct non-clone carrier similarly
retains the complete existing depth-two carrier, independently rebuilds the
inner and outer zero-padded images, replays every layout and occurrence identity
hash-free, and performs one final atomic copy. Shallow, deeper, plural,
recursive, array-mediated, or direct-sum-coexisting paths reject, as does
target-dependent placement at any of the four record layers. Existing direct,
one-level, singular depth-two, and plural depth-two APIs remain unchanged.

The complete plural depth-three fixed-depth rung admits a nonempty
authored-order set of `Outer -> First -> Middle -> Leaf -> direct conventional
sums` chains. Its compact report retains the outer whole-record plan once and
one exact row per outer occurrence; each row owns the unchanged plural
depth-two report for that occurrence. A distinct non-clone carrier composes the
corresponding plural depth-two carriers, independently rebuilds every nested
image and the outer zero-padded image, replays every layout and occurrence
identity hash-free, and performs one final atomic copy. Repeated nominal types
remain occurrence-distinct. The singular depth-three API remains exact-one. A
shared memoized bounded reachability walk, fallible storage, and a global
leaf-occurrence ceiling bound both production and replay. Shallower, deeper,
recursive, array-mediated, or direct-sum-coexisting paths reject, as does
target-dependent placement at every layer.

The complete plural depth-four fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-three
report for that occurrence. A distinct non-clone carrier composes the
corresponding plural depth-three carriers, preserves repeated nominal
occurrences separately, independently rebuilds every nested zero-padded image,
replays layouts and occurrence identities hash-free, and performs one final
atomic copy. One shared memoized bounded reachability walk and global
leaf-occurrence ceiling cover the whole cohort. Paths deeper than four records,
recursion, arrays containing sums, direct sums at an enclosing layer, and
target-dependent placement remain rejected; shallower APIs are unchanged.

The complete plural depth-five fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-four
report for that occurrence. A distinct non-clone carrier composes those
depth-four carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than five records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-six fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-five
report for that occurrence. A distinct non-clone carrier composes those
depth-five carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than six records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-seven fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-six
report for that occurrence. A distinct non-clone carrier composes those
depth-six carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than seven records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-eight fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-seven
report for that occurrence. A distinct non-clone carrier composes those
depth-seven carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than eight records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-nine fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-eight
report for that occurrence. A distinct non-clone carrier composes those
depth-eight carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than nine records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-ten fixed-depth rung admits a nonempty authored-order
set of
`Outer -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-nine
report for that occurrence. A distinct non-clone carrier composes those
depth-nine carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than ten records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-eleven fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-ten
report for that occurrence. A distinct non-clone carrier composes those
depth-ten carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than eleven records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-twelve fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-eleven
report for that occurrence. A distinct non-clone carrier composes those
depth-eleven carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than twelve records, recursion, arrays containing
sums, enclosing direct sums, and target-dependent placement remain rejected;
every shallower API keeps its exact boundary.

The complete plural depth-thirteen fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-twelve
report for that occurrence. A distinct non-clone carrier composes those
depth-twelve carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than thirteen records, recursion, arrays
containing sums, enclosing direct sums, and target-dependent placement remain
rejected; every shallower API keeps its exact boundary.

The complete plural depth-fourteen fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-thirteen report for that occurrence. A distinct non-clone carrier
composes those depth-thirteen carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than fourteen records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-fifteen fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-fourteen report for that occurrence. A distinct non-clone carrier
composes those depth-fourteen carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than fifteen records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-sixteen fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-fifteen report for that occurrence. A distinct non-clone carrier
composes those depth-fifteen carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than sixteen records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-seventeen fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-sixteen report for that occurrence. A distinct non-clone carrier
composes those depth-sixteen carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than seventeen records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-eighteen fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-seventeen report for that occurrence. A distinct non-clone carrier
composes those depth-seventeen carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than eighteen records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-nineteen fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-eighteen report for that occurrence. A distinct non-clone carrier
composes those depth-eighteen carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than nineteen records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-twenty fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-nineteen report for that occurrence. A distinct non-clone carrier
composes those depth-nineteen carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than twenty records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-twenty-one fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural depth-twenty
report for that occurrence. A distinct non-clone carrier composes those
depth-twenty carriers, keeps repeated nominal occurrences distinct, rebuilds
every nested zero-padded image in authored order, replays all layouts and
occurrence identities hash-free, and performs one final atomic copy. One shared
memoized bounded reachability walk and global leaf-occurrence ceiling cover the
complete cohort. Paths deeper than twenty-one records, recursion, arrays
containing sums, enclosing direct sums, and target-dependent placement remain
rejected; every shallower API keeps its exact boundary.

The complete plural depth-twenty-two fixed-depth rung admits a nonempty
authored-order set of
`Outer -> Twentieth -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-twenty-one report for that occurrence. A distinct non-clone carrier
composes those depth-twenty-one carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than twenty-two records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

The complete plural depth-twenty-three fixed-depth rung admits a nonempty
authored-order set of
`Outer -> TwentyFirst -> Twentieth -> Nineteenth -> Eighteenth -> Seventeenth -> Sixteenth -> Fifteenth -> Fourteenth -> Thirteenth -> Twelfth -> Eleventh -> Tenth -> Ninth -> Eighth -> Seventh -> Sixth -> Fifth -> Fourth -> Third -> Second -> First -> Middle -> Leaf -> direct conventional sums`
chains. Its compact report retains the outer whole-record plan once and one
exact row per outer occurrence; each row owns the unchanged plural
depth-twenty-two report for that occurrence. A distinct non-clone carrier
composes those depth-twenty-two carriers, keeps repeated nominal occurrences
distinct, rebuilds every nested zero-padded image in authored order, replays
all layouts and occurrence identities hash-free, and performs one final atomic
copy. One shared memoized bounded reachability walk and global leaf-occurrence
ceiling cover the complete cohort. Paths deeper than twenty-three records,
recursion, arrays containing sums, enclosing direct sums, and target-dependent
placement remain rejected; every shallower API keeps its exact boundary.

Zero-length or nested sum arrays, coexistence with direct sum fields,
plural paths deeper than twenty-three records, paths deeper than twenty-three
records, mixed common-field/case shapes, and sums whose all-case geometry
requires a target capsule remain later rungs. The same NaN, reference, slice, Text,
dynamic, atomic, and non-copy fences remain in force.
None of these slices narrows the legacy typed-owned materialization API or
establishes evaluator admission, quotient canonicalization, producer-origin
chains, or proof authority. Carried quotient representatives, richer origin
diagnostics, and target-dependent const application remain later slices.

The second judgment is value-sensitive and structural over the realized value,
not merely its outer type. It traverses the active sum case and its actual
fields; an inactive case cannot make the current value fail. When a nested
component fails, the diagnostic retains a compact origin chain from the runtime
materialization site through the field/index/case path to the evaluated
operation that produced it. Layout padding is emitted as zero under ZII and
remains outside program semantics.

A float whose meaning is NaN but whose payload is not fixed remains usable in
proof and compile-time positions through `Float::meaning`. Materializing that
value rejects unless the author canonicalizes it, constructs explicit bits, or
selects a realization publishing exact NaN representation. This prevents an
arbitrary evaluator choice from becoming accidentally reliable image data.

A quotient is different. Quotient construction carries one exact operational
representative, and zero-cost evaluation preserves it just as runtime execution
does; ordinary materialization may emit that carried representative without a
canonicalization proof. Canonical form is required only when a consumer demands
representative-independent identity, such as stable serialization, public ABI,
canonical const-index use, structural hashing/interning, or reproducible raw
bytes. Equivalent quotient values may otherwise materialize different opaque
representatives.

Until proof/build-admissible resource inputs have a sealed admission artifact,
the common gate rejects every declared linear runtime carrier in a reachable
checked body: attached machine data, machine-owned data, state and callable
parameters/results, and local bindings. The judgment uses typed structural
multiplicity, not type names. This is a monotone fail-closed input/resource
rung; it does not claim that the remaining authority, trust, or
resource-admission artifacts exist.

Pre-check evaluation currently has no concrete-argument proof context. The
common gate therefore rejects an authored `requires` premise anywhere in the
reachable machine/callable closure rather than assuming it before the checker
has discharged it. This is the first fail-closed trust-input rung. A later
invocation-sensitive gate may admit the call by supplying the ordinary checked
proof of that exact premise; it must not add a build-time-only proof rule.

Trait requirements pin the public floor. A conformance cannot grow an
incompatible axis unnoticed; it fails at the conformance declaration.

## Ordinary termination, not compile-time totality

Semantic evaluation requires the existing `terminates` guarantee. There is no
second compile-time completion concept:

```text
TerminationGuarantee = NoGuarantee | Terminates
```

An acyclic checked body supplies a local `Terminates` summary without source
annotation. A cyclic body supplies `terminates by ...`. An open or separately
compiled contract writes `terminates;` when consumers may rely on it. This is
the normal infer-when-closed, declare-when-open split.

Termination proves eventual completion, not affordability. A deliberately
hours-long terminating proof is semantically legal, while the build sponsor may
still refuse to spend the resources required to evaluate it.

## Deterministic work observation and optional policy

The evaluator maintains a deterministic usage record independent of host speed,
thread scheduling, optimization, and target cycle cost. The initial normalized
record should retain canonical counts rather than one permanently weighted
scalar:

```text
EvaluationUsage {
    fuel_units;
    build_log_bytes;
    filesystem_operation_attempts;
    peak_live_cells;
    peak_live_text_bytes;
    result_cells;
    result_text_bytes;
}
```

`fuel_units` is ultimately charged by the canonical Terminal Psi fuel schedule
in [`canonical_ir_fuel_and_resource_provisioning.md`](canonical_ir_fuel_and_resource_provisioning.md).
The remaining counts are attributed telemetry rather than interchangeable work
currencies. The evaluator's local schedule charges one unit for each entered
state, executed statement, and evaluated expression. Semantic evaluation and
ordinary interpreted outcomes retain the resulting usage, and equal
invocations reproduce it. This telemetry is not Terminal Psi fuel and cannot
support an IR fixed-work certificate.

Compiler build-machine evaluation retains that precursor usage alongside the
computed build configuration in checked and full compilation reports. It does
not enter `BuildConfig`, terminal semantics, or artifact identity. Once build
machines lower through terminal Psi, the canonical schedule replaces this
precursor count rather than being inferred from it.

Package review additionally owns one unobservable version-7 evaluation sponsor
across the complete resolved closure. The current compiler policy grants
100,000,000 deterministic evaluator fuel units, 16 MiB of compiler-owned
BuildLog output, 65,536 canonical filesystem operation attempts, 4,096
concurrently live compiler-owned filesystem resources, 1,048,576 concurrently
live semantic interpreter cells, 1,048,576 successful result cells, and 64 MiB
of concurrently live interpreter Text backing payload plus 64 MiB of
successful result Text payload while
retaining the ordinary 100,000-unit ceiling for an effect-free invocation and
10,000,000-unit ceiling for a granted invocation. Initial evaluation and
automatic provider-free replay debit the same shared sponsor;
dependencies cannot raise it. The version-7 usage receipt binds the step
schedule, per-invocation ceiling, optional sponsor schema and session ceilings,
and distinct initial/replay fuel, BuildLog, filesystem-attempt, result-cell,
and result-Text charges plus distinct initial/replay live-cell and live-Text
peaks. It also retains the shared session's peak live-handle, live-cell, and
live-Text counts. Successful closure review rejects unless all retained charge
totals and peaks exactly
equal the sponsor's counters and no live reservation remains. The ambient
interpreter development override does not alter package-policy evaluation.
These are deterministic compiler-resource limits, not claims about CPU time,
resident memory, `Vec` capacity, temporary non-Text copies, the process-wide
descriptor table, or hostile-process containment.

Package build filesystem authority enters through the one `Build` activation,
which exposes an immutable `BuildSource` capability and a fresh writable
`BuildOutput` capability. Their compiler-owned operations perform sponsored
reads, writes, and explicit generated-source publication directly; no runtime
standard-library `FilesystemHost`, `Console`, or `Path` declaration is part of
the build protocol. A checked facet operation joins one exact root occurrence
to canonical relative bytes before host access. No compiler-host absolute path,
process working directory, virtual-prefix test, package role, or service name
participates in authorization.

The resolved value, not an erased qualification over bare path bytes, carries
the operational root identity. Resolution rejects absolute input, traversal,
ambiguous root membership, and symlink escape. Authorized path-returning
operations preserve the same root or reject. `read_link` returns inert stored
bytes; following them requires another checked resolution, so an outside target
may be inspected but not traversed. `/source/...` and `/output/...` are reserved
canonical evidence renderings and never package-facing authority spellings.

Writing under `BuildOutput` and publishing generated input are distinct. A successful
evaluation first closes its observations and output-tree custody; only an
explicit `Build` handoff may then introduce selected staged content into
compilation. Failure discards the staging occurrence. Root capabilities,
root-bound paths, and open handles are activation-scoped and never enter the
durable build value or runtime package data.

The package-aware checked path now freezes the ordinary source closure before
execution, runs the selected build once, and admits only explicitly handed-off
ordinary non-executable `.omg` files from the retained staged tree. Their exact
bytes are parsed under compiler-owned logical paths and pass through the full
final frontend/checker as ordinary candidate code. Generated imports cannot
expand the frozen dependency/source closure, and a staged `.omg` file without
an explicit handoff rejects instead of entering compilation by filename.

Host observation is retained separately from this meter. The granted evaluator
reports whether the filesystem host family was actually invoked; the compiler
joins that fact to the exact statically reachable toolchain service. The
current scoped real-filesystem provider has no replay transcript, so reachable
or realized filesystem use is conservatively `Volatile`. A pure or console-only
build is `Hermetic`, and console-only execution is not supplied real filesystem
authority. Both statement- and value-position dispatch require an exact
canonical toolchain filesystem requirement symbol before the provider is
entered; a package-authored lookalike remains an ordinary unsupported call even
under granted execution. The selected canonical signature then maps to a
closed, explicitly tagged operation identity exhaustively handled by both
filesystem providers. ABI aliases remain distinct. Package-rooted execution
rejects `canonicalize` and any `final_path_name_by_handle` call whose validated
logical handle could return a path because that result is host-absolute. An
exact `Native/Unknown` failure may proceed because it cannot return path bytes.
`read_link` payload remains inert and acquires no rooted
authority without checked resolution. Observation
summary schema v20 carries operation-attempt schema v18, retaining in call-start order
each completed operation's exact provider, stable tag, normalized result,
post-operation error state, and every direct scoped path authorization for a
successful build evaluation. Each authorized path retains its exact operand
ordinal, read/write access, closed Source/Output root, and canonical
slash-separated relative UTF-8 bytes without a host absolute prefix. Grant-gate
denials remain distinct; ordinary host errors do not fabricate one and retain
any authorization that preceded the host failure. Descriptor, Native, and Find
operands are normalized immediately to exact Resolved, Null, or Unknown logical
lifetimes, so later preparation failure keeps the completed prefix. A fully
prepared call must reproduce that exact plan. Successful created and duplicated
outputs mint monotonic logical lifetime IDs; a repeated borrowed native
conversion reuses its live alias, while duplicate and borrowed views retain
their source. Successful closes retain every invalidated lifetime, raw provider-
token reuse never reuses an ID, and failed closes retire nothing. A token live
in another logical domain rejects before provider access; provider acceptance
of an otherwise Unknown token traps instead of publishing contradictory lineage.
The hermetic duplicate model shares one open-file cursor with its source. Real
descriptors retain rooted write authority across duplicate and borrowed views,
and deny content, extent, metadata, ownership, or host-lock mutation before
sponsor or host access when opened under source-read authority alone.
`open_at`/`unlink_at` accept only one nonempty portable relative component, and
real-provider path-byte conversion is lossless or rejects.
Successful descriptor/find/native-handle results retain only their logical
identity in observation evidence; provider token integers do not survive.
Non-handle results and failed handle-result sentinels remain exact scalar
values, with both lanes type-tagged by package commitment framing.
Every successfully typed non-handle scalar, immutable payload, non-rooted path-
like operand, compiler-rooted path, and mutable carrier is retained as argument
preparation advances. Later failure keeps that exact ordinal prefix. Fully
prepared calls cross-check compiler-private semantic sidecars before provider
access. Scalars retain explicit I32/U32/I64/U64 identity; immutable write and
FILETIME payloads retain complete authored bytes; at-family components,
directory-entry names, symlink targets, and find patterns retain exact portable
bytes in role-specific lanes. Raw rooted/path-alias spellings never enter the
payload lane. Rooted input rows instead carry closed Source/Output identity and
canonical relative bytes before physical provider-path lowering. They are not
authorization rows: a later grant check separately carries access and may
select a different canonical rooted location after symlink or nested-root
resolution. Mutable byte and i64 carriers retain distinct resolution-time and
provider pre/post state. Provider pre-state follows every authored argument
because a later argument may alias an earlier carrier; post-state follows
provider return or halt. Input-only mutable ABI carriers remain explicit. A
separate 256 MiB aggregate operand-evidence sponsor covers immutable, path-like,
rooted-resolution, exact returned-path, and all mutable byte copies. Exhaustion halts that call;
prior or nested staging effects remain cleanup-contained. Package commitment
framing hashes every lane without rendering bytes as text.
A granted evaluation failure
retains partial usage and observations with an explicit returned/evaluator-halt
outcome; worker creation or panic marks evidence unavailable. Omega emits only
fixed non-admission counts and no review row on failure. Duplicate identities,
conflicting equal roots, unresolved roots, unrepresentable rooted paths, and
the 16 MiB aggregate authorized-path ceiling reject before host access.
Successful provider write branches retain exact meaningful `read_link`,
canonical, and final-path bytes without terminators or stale tails, including
closed kind and Complete/LimitReached disposition. Provider-known target length
distinguishes exact-fit from truncated `read_link`; failures and insufficient-
capacity returns add no row. Package-rooted execution rejects canonical and
final absolute outputs, while `read_link` remains inert. Content custody remains
incomplete. Successful `read`/`read_at` calls designate the
exact returned prefix of the already-retained mutable post-carrier as
sequential or positioned file content. Length equals the nonnegative result;
EOF retains an empty row and failure retains none. The zero-copy designation
adds no sponsor charge. `read_dir` similarly designates exact
`DirectoryRecords`; `find_first` and entry-producing `find_next` designate
complete 320-byte `FindEntry` records. Directory EOF and no-entry find returns
retain empty rows, while failed enumeration retains none. Successful path,
descriptor, and no-follow metadata operations retain one target-neutral
canonical row containing all 14 `StatRecord` fields. After target selection,
the compiler extracts and validates the already-checked
`StatLayout<StatRecord>` from its earliest coherent private typed/layout state
and passes only that closed descriptor to the Psi evaluator. The evaluator
zeroes and serializes the complete authored ABI carrier (whose API minimum is
144 bytes) through the descriptor and checks it against the semantic row.
Filesystem-reaching builds load and
check the standard filesystem layout policy before execution; console-only
builds need no such layout. This is an internal checker seam, not a public IR
contract or reason to add nominal Chi. Complete replay remains absent. It is
an incomplete operation trace, not a transcript or receipt, and makes no
replayability or source-rebuildability claim.
The first bounded replay executor accepts one or more complete, non-interleaved
Source-rooted source-read chains. Each chain contains one flags-zero `open`, one
or more `read`/`read_at` calls on its distinct created descriptor, and its exact
retiring `close`. It reruns the build without a filesystem provider, uses inert
rooted coordinates, supplies recorded scalar/logical results and mutable read
bytes, reconstructs descriptor lifetimes, and rejects the first extra,
reordered, changed, or missing event. Each chain's sequential cursor starts at
zero and advances by exact sequential-read results; positioned reads bind an
exact nonnegative offset and do not advance it. Ordered operations, counts,
offsets, results, carriers, and observed regions determine cursor semantics
without separately trusted fields. Zero reads, failed reads, descriptor reuse,
cross-chain operations, interleaving, and incomplete chains reject. Exact
result and complete-record equality are required. Summary v22 binds successful
partial replay. Compiler replay-record v4 retains the complete ordered chains
in canonical binary form, rejects stale semantic schemas and operation-
inapplicable or internally inconsistent lanes, and survives restart inside
review-baseline capsule v2.
The record is opaque, bounded, and review-
only; custody alone establishes neither authenticity, admission, nor a receipt.
An explicit checked-compilation entry now strictly rehydrates the canonical
record into the PSI executor's exact typed source-read chains and evaluates the
build machine with no host filesystem provider. The replay supplies retained
source bytes even if that host file has changed and rejects changed authored
paths, counts, positioned offsets, operation or region kinds, and event
structure through the same exact checks. This uses an existing
compiler-private checked/evaluator seam rather than exposing an IR contract or
adding nominal Chi. The build remains `Volatile` until all operations, output
mutation and staged-output reproduction, package-command integration, and the
complete replay verdict are implemented.

Observation summary v23 and compiler replay-record v5 generalize that bounded
record into an ordered source-input event stream. Successful Source-rooted
`read_metadata` and `read_symlink_metadata` calls may occur before, between, or
after closed read chains. Replay retains the authored rooted input separately
from the authorized target selected after follow/no-follow resolution, all 14
target-neutral metadata fields, and the complete target-specific mutable
carrier. Recovery rejects an inapplicable lane, noncanonical relative path, or
structural mismatch. Replay reconstructs the complete zero-filled carrier from
the semantic row and selected checked `StatLayout`, compares every field,
padding, and tail byte, then requires exact event and result equality. Failed
metadata and descriptor-backed `read_file_metadata` are not admitted by this
rung. The extension is still provider-free, review-only, `Volatile`, and below
`Receipted`.

Raw filesystem byte-valued inputs are evaluated once by the shared preparer and
reject above 16 MiB before provider cloning/allocation. Read/count capacities
use one checked evaluator conversion and reject negative,
host-unrepresentable, or above-ceiling values. The ceiling is current compiler
sponsorship policy, not an Omega API limit. One shared closed preparer checks
exact arity before evaluation, consumes all authored operands once from left to
right, rejects wrong scalar/byte kinds, and retains validated mutable cells and
capacities, including fixed ABI inputs such as Win32 `OVERLAPPED`, before either
provider or grant gate. It covers otherwise-unused ABI
operands, and its exact operand/result schema is checked against the canonical
50-operation trait. Canonicalize enforces its declared 1024-byte `PATH_MAX`
carrier at that gate. This does not replace process memory, CPU, or transport
quotas.
Scoped hard links require write authority on both names, preventing a package
from aliasing a read-only source inode into writable staging. Namespace
mutations canonicalize the parent but preserve the final directory entry, so an
outside symlink pointing into a write root cannot lend its target's authority
to removal, replacement, rename, linking, or `unlink_at` of the outside name.
Operations that semantically follow the leaf, such as open/truncate, continue
to authorize the resolved target. Package review
uses one compiler-owned staging sponsor across the complete closure. Its
initial ceiling is 4,096 namespace entries, 256 MiB total logical bytes, and
256 MiB for any one object extent. Entries count names, regular-file extents
count once per object, symlink spellings count as bytes, and open-but-unlinked
objects remain charged through their final descriptor. Package build roots are
entries in the same account. Provider mutations reserve account state before
touching the OS and commit only after success; a ceiling refusal is reported as
resource exhaustion. Per-package and path-summed accounting are intentionally
rejected designs.

After successful sponsored package evaluation releases the provider and all
descriptors, Omega captures a versioned commitment to the complete fresh Output
tree before orchestration removes the disposable session. The canonical tree
sorts portable Output-relative UTF-8 slash paths and binds empty directories,
canonical directory/ordinary/executable/symlink modes, file lengths and content
digests, and validated self-contained relative symlink spelling. Ambient host
metadata, absolute roots, inode identity, and hard-link topology are omitted.
The physical walk is cross-checked against a quiescent sponsor namespace,
including kinds, extents, and object groups; mismatch, unknown kinds, external
symlinks, or bounded-resource excess reject. A successful empty tree has an
explicit commitment. Package observation identity retains the tree digest,
entry count, and unique byte count after cleanup, but not its content. This is
not replay evidence or a generated-output handoff and does not claim protection
from a hostile same-user process racing the private session.

The usage record carries a schema identity independently from evaluator-step
identity: adding telemetry does not change what one step means. It records
`result_cells` and `result_text_bytes` for successful semantic evaluation.
Each returned scalar, unit, text, or aggregate root contributes one cell, and
aggregate fields, case payload values, and array elements contribute their
recursive cell counts. Text does not inflate the cell count; its exact retained
payload bytes are counted separately. Structural type/member/case names and
Rust allocator overhead are compiler metadata and implementation details, not
invented semantic byte size. Augmenting-machine results sum both measures over
every returned argument. The evaluator computes them with checked arithmetic
and rejects accounting overflow rather than publishing a partial record.
`peak_live_cells` counts semantic storage-cell allocations from reservation
until the final alias drops. It does not estimate their Rust allocation size or
resident memory.

This meter supports three policies without becoming program semantics:

1. live progress attribution;
2. deterministic large-work warnings; and
3. optional root-selected hard ceilings.

An evaluation is resource-admissible by either of two routes:

1. a certified maximum-logical-work ceiling fits the compiler-side grant, so
   completion within that grant is known before execution; or
2. the deterministic evaluator meters an otherwise eligible invocation against
   an explicit sponsor budget.

The evaluated machine cannot observe its budget, catch exhaustion, or change
behavior when the grant changes. Raising a budget can change whether the build
obtains a result, never which result it obtains. Published reproducible builds
either carry the certified ceiling or record the admitted work budget and usage
receipt. Peak live cells receive the same treatment. Temporary bytes are not
one honest generic domain: additional hard ceilings are introduced only for a
named compiler-owned payload whose complete allocation lifetime can be charged
before it becomes interpreter state and released exactly. Evaluator `Text`
backing payloads now have that account. Complete synchronous directory
enumeration truncates retained components to Omega std's existing 255-byte
`DirEntry` carrier and caps packed dirent payload at 16 MiB per operation
before retaining each snapshot name and before allocating the packed buffer.
That extent strictly dominates the complete retained source-name payload, so a
duplicate name account adds no invariant. Retained find-cursor names have a
16-MiB per-snapshot name ceiling in the ambient/differential interpreter, but
the unrooted find trio is rejected in rooted package-build mode. It therefore
does not justify package sponsor, usage, or manifest fields before a root-aware
Build-facet protocol is admitted. Synchronous file-read transfer buffers
already have one exact pre-provider 16-MiB per-operation bound; another peak
field would add no enforceable invariant. Allocator capacity, RSS, and process
memory remain deployment policy rather than an Omega claim.

The root may raise or remove ceilings. Dependencies may publish expected usage
but cannot grant themselves more. Evaluation code cannot inspect remaining
work, branch on policy, catch exhaustion, or request an increase. Exhaustion is
a build-resource error, never divergence, a failed termination proof, or a
machine result.

Progress reporting names the stable invocation, sponsor package, accumulated
usage, and largest active call path. Wall-clock elapsed time may be displayed
but never affects admission or accounting. Parallel scheduling changes neither
the canonical counts nor aggregate verdict.

Observation summary v52 and filesystem replay-record v33 use one version-1
closed replay disposition: `NotReplayed`, `SourceInputsOnly`, or `Complete`.
The partial disposition means that provider-free execution consumed the exact
retained Source-input record and reproduced the build result and observations.
`Complete` additionally requires exact attempted-operation and
generated-source-handoff equality, reconstructed virtual Output state, clean
replay teardown, and a matching staged-output commitment or sponsored custody.
The compiler fails closed rather than issuing `Complete` without source replay
or staged-output custody. Observation identity binds the verdict schema and
disposition with the attempts, handoffs, and tree. The verdict is execution
evidence, not package admission or an audit attestation. Host CPU and RSS
controls are deployment availability policy; they do not strengthen the
verdict or turn review into authority.

Cloning admitted filesystem operation attempts into interpreter custody uses
explicit, host-independent retention weights instead of Rust-layout sizes.
Fixed typed rows use their current canonical fixed-width upper bounds and
retained variable payload bytes contribute one unit each. Separately bounded
generated-source handoffs are outside this account. This is an implementation
availability guard, not canonical record length, heap or RSS measurement,
replay evidence, or package admission.

Summary v53 and filesystem replay-record v34 add one exact no-effect failure
row: an optional Source prefix followed by tag-10 `seek` on an unknown
descriptor. Replay binds the authored `i64` offset and `i32` origin alongside
the fixed scoped-real provider, scalar `-1`, post-error `9`, and
`Descriptor/Unknown` input. Every other evidence lane and generated-source
handoff is empty. Provider-free execution against a fresh virtual descriptor
table must reproduce the complete attempt and teardown before the compiler
issues empty staged-output custody.

Summary v54 and filesystem replay-record v35 add the exact write-gated scalar
unknown-descriptor family: tag-17 `set_file_permissions(u32)`, tag-41
`set_len(i64)`, tag-46 `lock_file(i32)`, and tag-49
`change_file_owner(i32, i32)`. The compiler binds each authored scalar by type,
ordinal, and value with fixed scoped-real provider, scalar `-1`, post-error `9`,
and `Descriptor/Unknown`; all other lanes and handoffs are empty. The real
evaluator rejects at descriptor grant lookup before host mutation, and virtual
replay must reproduce the complete attempt and teardown before empty
staged-output custody issues.

Summary v55 and filesystem replay-record v36 add the corresponding exact
mutable-carrier row for tag-42 `set_file_times` on an unknown descriptor. The
compiler binds operand one's complete authored carrier as equal resolution and
provider pre/post bytes, requires at least the 32-byte timespec pair, and fixes
scoped-real provider, scalar `-1`, post-error `9`, and
`Descriptor/Unknown`; all other lanes and handoffs are empty. Descriptor grant
lookup rejects before host mutation. Virtual replay restores the exact carrier
and must reproduce the complete attempt and teardown before empty staged-output
custody issues.

Summary v56 and filesystem replay-record v37 add exact failed tag-4 `read` and
tag-6 `read_at` rows on an unknown descriptor. The compiler binds the authored
`u64` count, the positioned read's `i64` offset, and operand one's complete
unchanged mutable carrier, with the count bounded by that carrier. Scoped-real
provider, scalar `-1`, post-error `9`, and `Descriptor/Unknown` are fixed; no
failed transfer region, other lane, or handoff is present. Compiler-owned
descriptor lookup rejects before a host read, and virtual replay must reproduce
the exact attempt and teardown before empty staged-output custody issues.

Summary v57 and filesystem replay-record v38 add exact failed tag-5 `write` and
tag-7 `write_at` rows on an unknown descriptor. The compiler binds operand
one's complete authored immutable payload and the positioned write's
operand-two `i64` offset. Scoped-real provider, scalar `-1`, post-error `9`, and
`Descriptor/Unknown` are fixed; every other lane and handoff is empty.
Compiler-owned write-grant lookup rejects before sponsor accounting or host
mutation, and provider-free replay must reproduce the exact attempt and
teardown before empty staged-output custody issues.

Summary v58 and filesystem replay-record v39 add exact failed tag-39
`read_file_metadata` on an unknown descriptor. The compiler binds operand
one's complete authored mutable carrier as equal resolution and provider
pre/post states after the preparer's 144-byte metadata-ABI minimum. Scoped-real
provider, scalar `-1`, post-error `9`, and
`Descriptor/Unknown` are fixed; no metadata observation, other lane, or
handoff is present. Compiler-owned descriptor lookup rejects before host
metadata access, and provider-free replay must reproduce the exact attempt and
teardown before empty staged-output custody issues.

Summary v59 and filesystem replay-record v40 add exact failed tag-30
`get_osfhandle` on an unknown descriptor after the optional Source prefix. The
row fixes scoped-real provider, scalar `-2`, unchanged post-error `0`, and
operand-zero `Descriptor/Unknown`; every other lane and handoff is empty. Both
evaluators consult compiler-owned synthetic descriptor tables, so provider-free
replay checks only Omega's modeled bridge. It proves no native-handle custody or
Windows security property.

Summary v60 and filesystem replay-record v41 add exact failed tag-29
`close_handle` on an unknown native handle after the optional Source prefix.
The row fixes scoped-real provider, scalar `0`, post-error `6`, and operand-zero
`Native/Unknown`; every other lane and handoff is empty. Provider-free replay
checks only the compiler-owned synthetic handle model, not native-handle
custody or a Windows security property.

Summary v61 and filesystem replay-record v42 add exact failed tag-31
`final_path_name_by_handle` on an unknown native handle after the optional
Source prefix. The row binds the complete unchanged mutable carrier, its
bounded `u64` capacity, and `u32` flags, while fixing scoped-real provider,
scalar `0`, post-error `6`, and `Native/Unknown`. No returned path exists.
Provider-free replay checks only the compiler-owned synthetic handle model,
not native path/handle custody or a Windows security property.

Summary v62 and replay-record v43 close one additional modeled family after an
optional Source prefix: a single failed tag-32 `set_file_time`, tag-33
`lock_file_ex`, or tag-34 `unlock_file` on `Native/Unknown`. The row retains
every authored scalar and complete byte carrier, including minimum FILETIME and
OVERLAPPED preparation sizes, while fixing scalar `0` and post-error `6`.
Both evaluators reject before sponsor accounting or host mutation. This claims
only synthetic invalid-handle replay, not native handle, lock, timestamp, or
Windows security custody. `get_last_error` remains separate ordered provider
state.

Summary v68 and replay-record v48 admit one exact ordered use of that state:
the same optional Source prefix, one exact tag-32/33/34 unknown-native-handle
mutation failure, then its immediate tag-35 `get_last_error`. The second row is
operand-free, scoped-real, scalar `6`, post-error `6`, and empty in every
evidence and handoff lane. Replay reproduces the modeled failure before reading
the modeled error slot. Standalone, delayed, reordered, or altered error-state
reads remain non-receipted. This is compiler evaluator sequencing, not custody
of Windows error state, native handles, credentials, or host policy.

Summary v69 and replay-record v49 admit one POSIX-shaped ordered error-state
sequence after the optional Source prefix: an exact operand-free tag-8
`close`, tag-43 `sync`, tag-44 `sync_data`, or tag-45 `duplicate` failure on
`Descriptor/Unknown`, followed immediately by tag-50 `errno`. The second row
is operand-free, scoped-real, scalar `9`, post-error `9`, and empty in every
evidence and handoff lane. Standalone, delayed, reordered, or altered reads
remain non-receipted. Other descriptor operand families are not implied, and
tag-30 `get_osfhandle` is excluded because its modeled post-error is `0`.
This is compiler evaluator sequencing, not custody of host thread-local state,
descriptors, credentials, or operating-system policy.

Summary v70 and replay-record v50 extend the same immediate tag-50 `errno`
receipt to every already-admitted exact unknown-descriptor failure whose
modeled post-error is `9`: seek; `open_at` and `unlink_at`; directory reads;
scalar write operations; file-time mutation; sequential and positioned reads
and writes; and descriptor metadata reads, in addition to the operand-free
family. The failure row retains its existing exact authored operands,
unchanged mutable carriers, scoped-real provider, scalar `-1`, and sole
`Descriptor/Unknown` input. The immediate `errno` row remains operand-free,
scoped-real, scalar `9`, post-error `9`, and empty in every evidence and
handoff lane. Tag-30 `get_osfhandle` remains excluded because it does not
establish modeled error `9`; native-handle failures remain in the separate
error-`6` grammar. Standalone, delayed, reordered, or altered reads remain
non-receipted. This binds compiler-evaluator sequencing only; it claims no
custody of host thread-local state, descriptors, credentials, or
operating-system policy.

Summary v71 and replay-record v51 extend the ordered tag-35
`get_last_error` receipt to every already-admitted exact unknown-native-handle
failure whose modeled post-error is `6`: tag-29 `close_handle`, tag-31
`final_path_name_by_handle`, and the tag-32 through tag-34 mutation family.
The failure retains its existing exact authored operands and unchanged
carriers; the immediate error-state row remains operand-free, scoped-real,
scalar `6`, post-error `6`, and empty in every evidence and handoff lane.
Provider-free replay reconstructs the selected typed failure before the read.
Standalone, delayed, reordered, repeated, or altered reads remain
non-receipted. This binds compiler-evaluator sequencing only; it claims no
custody of Windows error state, native handles, credentials, or host security
policy.

Summary v64 and replay-record v44 generalize the failure-only Output sequence
to exact absent tag-9 `remove` and tag-12 `remove_dir` attempts. Every row binds
the selected operation, canonical compiler-rooted Output path, matching write
authorization, scalar `-1`, and post-error `2`; mixed ordered file/directory
sequences replay against a fresh namespace and retain empty staged-output
custody. This receipts those observed attempts only. It is not a claim that a
host path is globally or durably absent.

Summary v75 and replay-record v55 extend the exact grant-policy denial to one
tag-1 `create` or tag-9 `remove` through a compiler-issued Source coordinate.
Both bind scalar `-1`, post-error `13`, and one operand-0
`Write`/`OutsideGrantedRoots` refusal; create additionally fixes mode `438`,
while remove has no scalar operand. The rooted operand is retained before
physical provider lowering, so no compiler-host absolute path participates in
identity. Provider-free replay validates that coordinate and injects the
compiler-owned denial without executing a virtual mutation. Source rows are
handled before generic Output-create or Output-remove classification and leave
the replayed Output tree empty. They carry no BuildLog output. Other grant
failures, multiple or mixed attempts, alternate modes, and host-derived denial
text remain non-receipted. This receipts Omega's grant-policy decision only;
it does not claim host filesystem containment.

Runtime WCET and target instruction cost remain a different resource theory.
A fixed-IR logical-work certificate does not alter native execution and its
scalar does not predict the target's worst-cycle path.

## Result caching and usage accounting

Semantic identity and accounting identity are separate:

```text
ResultKey =
    normalized implementation closure
  + arguments
  + selected conformances/providers
  + observed target-semantic applications
  + selected target-realization applications
  + evaluator semantics version

UsageRecord =
    ResultKey
  + usage-schema version
  + canonical usage counts

PolicyCharge =
    interpret(UsageRecord, selected cost policy)
```

Using the complete target semantic capsule plus the complete selected
realization closure for the two target-dependency rows is the initial
conservative implementation. Fine-grained replay must not retain observations
while accidentally dropping selected target-scoped plans that read none.

Changing accounting weights must not invalidate a result computed under
unchanged semantics. If a future policy needs counts an older usage schema did
not retain, usage may need remeasurement; the semantic result remains valid.

When a cache hit substitutes for an evaluation in the current build graph, it
receives the recorded logical charge. Warm and cold builds therefore make the
same hard-ceiling decision. Merely linking an already-built dependency does not
charge the historical cost of producing that artifact.

## Published evidence is not a consumer cache

An expensive producer-side proof may publish carrierless selected-conformance
evidence under the
[law-bearing relation model](law_bearing_relations_and_quotients.md). The
producer performs the witness search while building its artifact; a consumer
projects the retained published proposition term and never reruns that search.
Cheap artifact/kernel verification may remain.

That is separate compilation of proof evidence, not an evaluator-cache escape
hatch. A local cache substitutes for work still belonging to the current build
graph; published evidence moves the proved contract into the producer's
artifact.

## Implementation boundary

The reference interpreter already serves constant, domain, layout, wire, and
calling-policy positions under deterministic termination, reach,
suspension/blocking, snapshot, and invocation-specific crash checks. `TASKS.md`
owns the remaining semantic capsule, result/usage identity split, telemetry,
progress policy, const parameters, recursive evaluation, and generator
expansion. This brief defines their contract; tests and Git history record which
individual sites have landed.

## Trait-body and reflection rules

- A trait machine body is its overridable default; there is no `default`
  keyword.
- Reflection iterates only over the conforming `Self` inside its own machine
  body. It grants no visibility, boundary access, compiler access, type
  descriptor, or type-as-value capability. Build-time evaluation still runs on
  admitted values in the sealed semantic world.
- Generated equality follows the current field set. Hand-written equality owns
  its own coverage and does not become substitutable equality; quotient laws
  remain the explicit route for that claim.
- Record destructuring is exhaustive: each field is bound, renamed with `as`,
  or visibly waived with `as _`. It is snapshot sugar for ordinary field `let`
  bindings, supports only copy-eligible implicit bindings, and introduces no
  reference patterns or binding modes. Larger fields are waived or borrowed by
  an explicit ordinary expression.
- Body-shape coverage is not a value proposition and is not exposed as a
  requirable fact. A separate trait-level coverage gate remains deferred unless
  real drift demonstrates a need for code-effect tracking.
- The generator-body unroll syntax remains unsettled. Combiner examples such as
  `all(self.[field] == other.[field])` are illustrative, not approved syntax.
