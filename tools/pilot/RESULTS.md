# Checker-soundness property pilot (not for main)

The lowered-form checker was never caught accepting a false fact on current
main. Out of 1898 mutants of 289 accepted programs, 395 were accepted, and
the oracle found no false `requires`/`ensures` and no interpreter fault. Both
fixed checker bugs were rediscovered from passing twins on their pre-fix
commits. The lowering bug in #3859 (stale match-arm payload) is out of reach
of any oracle that runs the lowered program.

Oracle: `soundness_oracle.rs` (interpreter, env `OMEGA_SOUNDNESS_ORACLE`)
evaluates callee `requires` at entry and `ensures` at scalar/unit return,
plus any interpreter fault in an accepted program.

| Known bug | Oracle on failing program (pre-fix) | Mutator from passing twin (pre-fix) | main |
| --- | --- | --- | --- |
| #3826 store to items[slot] keeps items[0] case | fault VerifiedOperationMalformed | 1 of 11 mutants (pick_index) | refused |
| #3856 &mut field loan skips record requires | fault ScalarObligationViolated | 4 of 80 mutants (poke, swap) | refused |
| #3859 stale match-arm payload (lowering bug) | Psi exit 0: not visible | 0 of 6 (mutator cannot reach arm expressions) | refused |
| `&mut` held in a lent record (OWNER_CODE_CONCERNS) | not reachable: main refuses as "not supported yet" | 0 (all 300+ carrier mutants refused this way) | refused |

Main campaign refusals: 603 by proof/ownership checks, 683 "not supported
yet", 217 other errors (mostly type errors the mutator introduced).
Median 20 s per mutant on a 4-core container (about 13 CPU-hours total).
Commands: see soundness_run.py docstring; pin OMEGA_LIBRARY_ROOT to the
binary's own source/library.
