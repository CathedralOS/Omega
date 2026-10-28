# Whole-data case predicate production

Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.

This is the canonical whole-data predicate production of `case_where_equality`, owned by **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. The sentence names Point's semantic equality. Source expresses it in data-level `where`: Empty, or Point together with the ordinary checked `same(lo, hi)` predicate. Cases introduce no case-local `where`, hidden binders or runtime subject. The earlier sibling row retains its historical evidence separately.

Construction must prove every clause before the value is established. The Point guard forms its payload projections; that pending predicate never supplies its own nominal admission. Matching an independently admitted value combines its complete predicate with the exact selected case to prove `hi - lo` directly. Empty returns77, making tag handling observable independently from Point's zero difference.

| Place | Positive | Fact-sensitive reject twin |
| --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| Field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| Parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| Call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| Match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| Const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| Another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| Selected target implementation | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

The target place uses four ordinary named implementations, selected by the exact ProgramEntry root and its explicit implementation argument in Build. Retired target-prefixed declarations are retained only in historical evidence. Each twin removes only the data predicate. The same produced values remain well formed, but the generic consuming subtraction loses its equality proof. The subtraction executes before a late observer compares the received lower payload with17 (or23 for the other join arm); the observer supplies no prior bound. Dropped or swapped payloads and join arms therefore fail the runtime oracle. Both match choices are exercised. Every positive binds ProgramEntry for all four hosted targets and declares Expected exit0.

The source-to-native obligations are:

- Establishment: full captured common/active-payload constructor valuation, exact applied receiving fields and independently legal omitted zeros; ordinary helper-call formation remains required.
- Identity and lifetime: admitted entry/call receipt, exact subject/application and storage version; payload writes open the relevant default-domain window until a complete post-store snapshot proves restoration. Saved subjects retain their own identity.
- Composition: parameter/return substitution and arm-local case facts transport the full clause; no caller equality guard substitutes for it. Runtime Point/Empty outcomes and both join choices observe the carried value.
- Soundness: pending whole self cannot satisfy its own nominal formal/domain, payload projection requires a formed case guard, repeated expression handles revalidate under each branch scope, and cycles refuse. Equality consequences use selected builtin fixed-integer/Bool semantics and exclude IEEE.
- Checked-to-Terminal Psi: the logical subject has no runtime argument; the complete default predicate is retained in the existing Terminal declaration requirement roster, including exact tag and payload paths. Actual tagged payloads, matches, calls and scalar subtraction remain ordinary checked operations. No classifier is manufactured as a value.
- Layout and ABI: existing tagged aggregate layout and ordinary parameter/return placements carry Interval; Build selects each hosted ProgramEntry normally.
- Instruction and frame realization: existing u64 subtraction, case branching, call preservation and target-body realization implement the same checked operation.
- Encoding and emission: the ordinary native image and Terminal Psi engines must independently produce the authored exit0. Actual host observations and exact producer identities belong in the final evidence record.

Fresh-ebf helper and literal controls wrongly refused; the missing-predicate twin correctly refused u64 subtraction. Historical400f/3d acceptance retains its original target-prefixed source and is not relabeled after incoming canonical target migration. Fresh optimized3ee integration ran all nine current places, the direct control and the bounded nested-payload control with native0 / Terminal0; all ten twins rejected the intended subtraction. Its focused corpus records32checking outcomes (14accepted,18intended rejects, no wrong-reason failures) and14actual engine agreements. The incoming retained-constructor pair checks1accept/1reject and runs its positive0/0. Ten current affected consumers preserve the expected3accept/7refusal pattern and three generic positives run0/0; this is current-oracle qualification, not an old-compiler/new-Std baseline comparison.

The complete Terminal roster descends through owned payload data under exact case guards and refuses unsupported descendant predicates. Three direct owner-model tests pin that carriage/completeness boundary, including unresolved extents. The nested Pair/Empty source control already passed on pre-fix172; it is coverage, while the model tests distinguish the missing roster/fence. Exact source, producers, raw streams, scoped goldens and restore objects belong to the separate integrated review/qualification evidence. Earlier retired-syntax and proof-only IEEE refusals remain explicitly uncredited for later obligations.

The restoration controls retain ordinary completed-case metadata across local initialization and whole-value replacement, expose raw independently valid scalar contents while a coupling window is open, and require exact Current case/field observations to close it. Arithmetic before restoration still rejects.

The parent task remains open for its other migration and composition requirements. This production makes no claim of arbitrary generic GADT/type-equation support, deferred recast establishment, or native execution on unobserved hosts.

A second actual PR conflict against pinned 4a9 changed only the checking golden text, but incoming compiler dependencies required a fresh 6d4ed producer. That exact producer repeated all 11 public engine agreements, 10 intended twins, 32 checking / 14 corpus agreements, the 3 owner tests and retained-constructor pair; two incoming computed-call controls also checked and ran 0/0. The prior 3ee / 433 evidence stays historical. Scoped 32/14 rows retain every pinned 4a9 outsider unchanged.
