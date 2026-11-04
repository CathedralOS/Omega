# Psi 05 numeric observations: where a cold Dungeon spends them

2026-11-03, main `7051a9e0e`, Linux x86-64 cloud VM (4 vCPU, 15 GB). Compilers
built at `opt_level` 1 (the dev profile), so absolute seconds do not compare
with the Windows release figures in the [DoD baseline](../dod_baseline_2026_11_02/README.md).
**Decision: the observation owner now walks each state once; the scope cannot
be halved there, because almost all of it is one state's structural judgment.**

The numeric-observation owner (`captured_numeric_observations`) costs about
34 s of a cold Dungeon compile on this VM. The suspected cause was repeated
walks of the same state: the owner kept only completed rows, so a statement
whose row came back incomplete was walked again on every later question.
That waste is real, but it is small. 168 of 1,741 state walks were repeats.
PR #1621 keeps every row a walk produces, which removes the 54 repeats whose
walks did not meet an enclosing query's recurrence cut. Cold Dungeon CPU does
not move measurably (paired ratio 0.996).

What does dominate is one state. `CombatSystem::fight` in
`samples/cli/games/dungeon_crawler_cli/combat/combat.omg` accounts for 21 s of
the 34 s. Its first three statements chain `max_i32`, wrapping casts and a
`Random::range` result. Two questions about its two transitions each walk
that prefix (8.5 s together). Then a single visit at the second transition
builds the arm judge for the case `round.enemy_defeated == false` and takes
about 10 s. Stack samples put that time in the structural judge's term
resolution (`resolve_with_at` → `resolve_uncached` →
`selected_requirement_judgment`, `selected_guarantees`, `completed_predicate`).
Neither part is a repeated observation walk. The next repair belongs to the
structural judge's resolution of nested call results, not to this owner.

## How the 34 s divides

Counters from instrumented builds of base (probes 2 to 5; none landed).
Times are exclusive per query kind, so nested queries are not counted twice.

| Part | Calls | Time |
| --- | ---: | ---: |
| Whole-state batch walks (statement and arrival demand) | 1,741 (168 repeats) | 11.6 s |
| of which association building (`assoc.build_preds`) | 801 | 5.4 s |
| Single-site walks for earlier completed reads (`StatePrefix`) | 249, no repeats | 22.1 s |
| of which `CombatSystem::fight`, 2 transition sites | 2 | 21.1 s |
| of which other transition sites | 127 | 1.0 s |
| of which local, call and assignment sites | 129 | 0.4 s |
| Memo hits | 9,299 | < 0.01 s |

Inside the `StatePrefix` walks, 107 visits took 12.6 s. 33 of them carried a
case equation from an earlier guard and took 10.0 s. In those, the visitor
rebuilds the arm judge from the incoming judge plus the whole equation list,
then re-judges every premise. The walker has already admitted those equations
into that same incoming judge (`next_judge` in the transition loop of
`recognize_structural_state_leaves_from`). Admitting them again looks
idempotent, but this lane did not prove that, so the rebuild was left alone.

Rejected alternatives, with the measurement that rules them out:

- **One walk for every `StatePrefix` site of a state.** No `StatePrefix` site
  was ever asked twice; queries cover about 3.4 sites per state. A shared
  walk saves at most the prefix walked twice in `fight`, about 2.2 s. Walking
  every site would add visits nobody asked for, and visits are the expensive
  part.
- **Serving `StatePrefix` from the batch walker** (`recognize_structural_state_prefixes`).
  That walker admits citation guarantees into each path as it goes, and it
  rejoins dead locals by a different rule. The solo walker admits guarantees
  only at visit time. The two are not interchangeable without a proof that
  their observations agree.

## Measurement

`omega-bench compare`, cold Dungeon (`linux_x86_64`, its own accepted lock per
compiler), one warmup and six alternating AB/BA pairs. Both compilers produced
the same native image in all 14 compiles (`d2176b5d0f2cf11e`), and each image
printed Dungeon's exact `expected_stdout.txt`.

| Metric | Base | Candidate | Paired change |
| --- | ---: | ---: | --- |
| Process CPU, median (user + system) | 184.3 s | 183.5 s | ratio 0.996 (range 0.983-1.027) |
| Wall, median | 140.7 s | 140.2 s | -0.61%, 95% interval [-2.8 s, +2.3 s], inconclusive |

Phase scopes from one `OMEGA_PHASE_PROFILE=1` compile each:

| Scope | Base | Candidate |
| --- | ---: | ---: |
| `psi.05.numeric.scan` (one per whole-state batch walk) | 1,738 | 1,684 |
| `psi.05.numeric.observe_prefixes` | 13.3 s, 1,093 calls | 12.5 s, 1,039 calls |
| `psi.05.numeric.visitor_visit` | 6.4 s | 6.1 s |
| `prepare_numeric_observations` (5,147 calls each) | 11.5 s | 11.1 s |

The candidate removes 54 of the 168 repeated walks. The other repeats belong
to walks that met an enclosing query's recurrence cut, whose incomplete rows
correctly stay out of the table.

Oracle for the change: `windows_x86_64` images of an empty program, a one-call
`hello` and Dungeon are byte-identical to base. Dungeon's Terminal Psi artifact,
compiled from one shared project path by each compiler in turn, is
byte-identical (`660be53d456f98d4`).

Raw journals, phase dumps and stack samples stay on the measuring VM and are
not retained in git.
