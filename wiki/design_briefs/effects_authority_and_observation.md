# Design Brief: Effects, Authority, And Observation

Settled 2026-07-18 (frozen decision 22). This brief defines one kinded
effect-row surface. It settles the qualitative v1 row, its propagation/refinement laws,
and its relationship to authority and trust. Decision 23 has since settled
opaque boundary progress profiles; general trace theorems, quantitative
resource entries, effect polymorphism, and user-defined
operational members remain explicitly deferred.

## The surface

Omega has one effect-specific clause and adds no reserved words:

```omega
machine backup(
    src: [u8] in Utf8,
    dst: [u8] in Utf8
) -> BackupResult
    effects Readable + Queryable + Suspend
{
}
```

An effect row is a `+`-separated set of name-resolved members. In v1 a member
has one of two mechanically represented kinds:

- **Service reach**: a boundary-trait identity such as `Readable`,
  `Queryable`, `Clock`, or `ProcessExit`.
- **Operational possibility**: a small core identity such as `Suspend` or
  `Block`.

The members are identifiers, not keywords. There is no `may suspend`,
`budget`, `fails`, `uses`, `trust`, negative effect, or scoped mode syntax.
The uniform source row does not collapse the kinds: checked IR, diagnostics,
manifests, and provider admission retain them separately.

Boundary traits automatically contribute their service-reach identity. A
boundary operation declares any additional service or operational ceiling:

```omega
boundary trait Readable {
    machine read(
        path: [u8] in Utf8,
        out: &mut Vec<u8>
    ) -> ReadResult
        effects Suspend;
}
```

The effective row of that call is `Readable + Suspend`. A wake operation on a
scheduler may reach its scheduler service without carrying `Suspend`; reach is
trait-granular while temporal behavior is operation-granular.

## The seven contract axes

The complete machine contract retains seven independent axes even though only
one is spelled with `effects`:

| Axis | Durable home |
|---|---|
| Service/operational possibility | kinded `effects` row |
| Authority possession | capability values, domains, and parameters |
| Trust reach | provider/admission receipts and the trust ledger |
| Positive temporal guarantees | operation/provider contracts and context floors |
| Resource consumption | explicit capabilities and dependent contracts in v1 |
| Failure/control outcomes | return sums, traps, cancellation, and non-return |
| Mutation | ownership, borrows, and state contracts |

Frame size is compiler-derived and reported. Task activation capacity is
declared or proved through task-pool authority. Version retention is a
component/deployment budget. Heap/region capacity is an explicit resource
value. Only frame size is report-only; no unbounded retention becomes
invisible merely because it does not belong in the effects row.

## Polarity and ceilings

Rows describe **possible** behavior and therefore accumulate upward through
calls. Absence is the negative guarantee:

```text
Suspend absent  => the machine never parks
Block absent    => the machine never occupies a worker by blocking
Writable absent => the machine never reaches the Writable service surface
```

Presence is permission in the published ceiling, not proof that every
execution performs the effect. A declared `Readable + Queryable` ceiling may
contain a body whose inferred row is only `Readable`.

Internal machines may omit the clause and receive an inferred row. Exported
machines, boundary operations, and trait requirements write their row. An
omitted row on an export or requirement means the empty row: pure by default,
with the missing member named by the diagnostic.

Authority possession and permission remain separate. Holding a Writable
capability does not let a machine whose ceiling omits `Writable` exercise it.
Conversely, listing `Writable` does not mint the capability required by the
operation.

## V1 row algebra

For qualitative v1 members, normalization is deterministic, idempotent set
union plus trait-parent closure:

```text
R + R = R
R + empty = R
Readable <= Filesystem       when Filesystem inherits Readable
```

The checker enforces:

```text
inferred(body)  subset-of declared_ceiling
callee_ceiling  subset-of caller_ceiling
provider_row    subset-of pinned_slot_row
```

Private omitted rows are the least conservative fixed point over the checked
call graph. Recursive call components compute one finite fixed point. Calls to
local checked machines use checked callee summaries. Imports, generic/dynamic
requirements, and boundary calls use their pinned requirement ceilings, never
facts learned from an eventually selected provider.

Provider admission is deterministic. A machine compiled against a
`Suspend`-only scheduler slot remains `Suspend`-only; a provider whose checked
or accepted contract includes `Block` fails refinement. A slot that explicitly
permits `Suspend + Block` admits either behavior and its consumers carry both
possibilities honestly.

Completeness is relative to checked bodies and pinned/accepted contracts. A
boundary provider may lie about its implementation; that is a trust failure
recorded by provider receipts, not evidence that row inference was incomplete.

## Published identity and proof gating

The normalized authored row is part of an exported machine's semantic
contract identity, import-slot identity, and component compatibility surface.
The body inference only checks inclusion.

> **Published-identity law:** every published identity is owned by a small,
> deterministic normalizer. The prover may gate legality, discharge an
> obligation, or enable erasure/optimization; it may never redefine a
> published identity.

Proof strength therefore cannot silently shrink an exported row or change a
contract hash. Internal legality may improve when a stronger prover establishes
that a path is unreachable, but exported identity remains the authored
ceiling. Stable syntactic/CFG reachability feeds row normalization; heuristic
entailment does not.

This is the effect-row instance of decision 19's normalization-versus-
entailment law and the component import-slot admission law.

## No laundering, masking, or handlers

V1 has no effect subtraction, masking, scoped allowance, or algebraic effect
handlers. Under reach semantics, a call through `Readable` has reached that
abstract service even when a checked in-memory provider supplies it. Provider
substitution can remove trust expenditure and refine operational behavior; it
does not rewrite the caller's published reach history.

This is an intentional difference from algebraic-effect handler systems:
virtualizing a service does not make the abstract service call disappear from
Omega's row. The benefit is stable provider-agnostic contracts and no hidden
control-flow construct; the cost is that a checked provider does not make an
otherwise effectful abstract program row-empty.

An implementation cannot launder effects through an ordinary trait or helper.
A logger that reaches `Writable` propagates `Writable`; a provider that blocks
propagates `Block` into its checked contract and cannot satisfy a slot that
forbids it.

## Effects and the other axes

### Authority and trust

Effects say what services/operational events may be reached. Capability values
say what authority the machine possesses. Trust receipts say which selected
realizations crossed out of proved Omega. A checked provider and host provider
may satisfy the same service contract while spending different trust; the
effect row remains provider-agnostic.

There is no ambient-service category. Provider threading may gain ergonomic
sugar later, but authority remains possessed and transferred as values.

### Resources

V1 resource bounds remain contracts on explicit resource capabilities:

```omega
machine parse(heap: &mut HeapBudget, input: &[u8]) -> ParseResult
    requires heap.remaining >= required(input.len)
    ensures heap.remaining >= old(heap.remaining) - max_used(input.len)
{
}
```

Parameterized entries such as `Alloc<Peak, Retained>` are reserved for the
resource-algebra brief. A single `Alloc<N>` is not compositional: sequential
peak depends on both prior retained usage and the next peak. No quantitative
entry lands until branch, loop, parallel, cancellation, and resource-identity
composition are deterministic.

### Failure and control

Recoverable failure remains a return sum with case-specific guarantees
(decision 18). There is no `fails` clause. Traps, cancellation behavior, and
non-return still appear in the normalized complete machine contract through
their existing policies, sums, and totality/productivity rules.

### Totality and positive liveness

`terminates` guarantees eventual terminal progress conditional on explicit
requirements and the progress premises pinned by reached operation/provider
contracts. It does not prove no suspension, fairness, a deadline, eventual
wakeup, or starvation freedom by itself. Row absence proves only negative
operational guarantees such as never-parks and never-blocks.

Decision 23 represents v1 positive progress premises as opaque, sealed
profiles on boundary traits/providers/slots. They use grant receipts,
participate in admission, and never entail proof facts. General trace theorems
and profile entailment remain deferred. See
[Termination, Ranking, And Progress](termination_ranking_and_progress.md).

## Staging and extensibility

V1 recognizes service members from boundary-trait declarations and the small
core operational set (`Suspend`, `Block`). The declaration mechanism for
user-defined operational members is deferred. Quantitative row members and
effect-row polymorphism are also deferred until their algebras have real
customers.

The current compiler's lowercase names and `u64 EffectSet` are a compatibility
implementation, not the semantic model. Migration replaces string lookup with
symbol-resolved, kinded rows while retaining a bitset/cache projection where
useful. Unknown identifiers resolve normally; there is no global hard-coded
effect keyword table in the end state.

## Acceptance register

1. Calling `Readable::read` contributes `Readable` plus its declared
   `Suspend`; calling a wake-only operation does not acquire `Suspend`.
2. A machine whose published row omits `Suspend` cannot transitively call a
   suspending requirement.
3. A blocking provider cannot satisfy a `Suspend`-only import slot; it can
   satisfy a slot whose ceiling includes `Block`.
4. Possessing Writable authority does not permit a call when the machine row
   omits `Writable`; listing `Writable` does not create the authority.
5. A checked in-memory Readable provider produces no trust receipt but does not
   subtract `Readable` from the machine's row.
6. Trait inheritance normalizes `Filesystem` to include its Readable,
   Writable, and Queryable service parents without resolution ranking.
7. A provider or implementation may refine to a smaller row and may never
   widen its requirement/slot ceiling.
8. A stronger prover cannot change an exported normalized row or contract ID.
9. An internal omitted row reaches a finite fixed point across a recursive call
   component.
10. Recoverable errors remain result cases; no parallel failure effect is
    introduced.

## Deferred, explicitly

- General trace propositions, deadline/starvation contracts, and entailment
  between decision 23's opaque progress profiles.
- Quantitative resource entries and their sequential/branch/loop/parallel
  algebra.
- User-defined operational effect-member declarations.
- Effect-row polymorphism and higher-order/callback row variables.
- The suspension amendment's continuation storage and suspension-safe loans.
- Component-version budgets and admission mechanics beyond the pinned-row law.
- Byte-information units as a possible units-family-zero customer; decision 19
  still requires explicit scale conversion when a consumer expects canonical
  bytes.

## Review laws

Two general review laws were established repeatedly while deriving this model:

> Coincidence of projections in a blessed instance is not identity of axes.

> Every published identity is normalizer-owned; the prover only gates.

They are architecture tests, not rhetoric. The first prevents reach from being
identified with suspension, provider-origin with provider-locality, or
authority with trust. The second prevents solver evolution from changing
interface meaning.
