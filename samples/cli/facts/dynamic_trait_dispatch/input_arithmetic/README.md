# Arithmetic at the public selector contract

a runtime dyn trait selector dispatches an inline conformance member over explicit data arguments to establish 100u64 when subtracting 100u64.

Owning outcome: **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** in
[TASKS.md](../../../../../TASKS.md), supported integer implication at selector calls.

Difference from the original row: `probe` receives `59 <= a <= 60` and
`a + 1 >= 61`, rather than the public requirement's exact `a == 60` bounds.
The independent upper bound makes the Exact addition representable before its
comparison contributes a premise. Ordinary integer entailment derives the
public left-operand contract, and the unchanged right input is 40. Each place
consumes the result by subtracting 100 before checking zero.

This bounded-addition probe already checks on fixed base
`eb9acb01452251ddb0c338fa400fb9cd476c8bc4`, optimized CLI
`68e71955520e14f15e686f4cb7c867d04d15246c178d5f22cfc3d29f89098ec7`.
It verifies the existing common integer engine; the accompanying IEEE repair
does not introduce an arithmetic exception or stronger provider facts.

Every twin changes only `a + 1 >= 61` to `a + 1 >= 60`, admitting 59.
The actual entry remains 60, but the generic caller must now reject its public
`WeightedCombine::combine` requirement. All four hosted entries are authored.
The two providers use different formal names and marker results 11 and 13;
the match place checks both selector arms, the field place checks neighbors
17 and 29, and target selection uses ordinary named Build providers.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [positive](local/main.omg) | [weakened input](local/invalid/main.omg) |
| field | [positive](field/main.omg) | [weakened input](field/invalid/main.omg) |
| parameter | [positive](parameter/main.omg) | [weakened input](parameter/invalid/main.omg) |
| call_result | [positive](call_result/main.omg) | [weakened input](call_result/invalid/main.omg) |
| match_join | [positive](match_join/main.omg) | [weakened input](match_join/invalid/main.omg) |
| const_name | [positive](const_name/main.omg) | [weakened input](const_name/invalid/main.omg) |
| other_state | [positive](other_state/main.omg) | [weakened input](other_state/invalid/main.omg) |
| other_machine | [positive](other_machine/main.omg) | [weakened input](other_machine/invalid/main.omg) |
| target_body | [positive](target_body/main.omg) | [weakened input](target_body/invalid/main.omg) |

- **Establishment:** independently formed addition and prefix bounds imply the public precondition; the implementation separately establishes the public result.
- **Identity and lifetime:** exact captured ordinary operands substitute the public formals, independently of private binder spelling or selected implementation.
- **Composition:** the same selector and arithmetic implication survive local/field/parameter storage, call result, both joins, constant, nested machine, forwarding machine and Build-selected target.
- **Soundness:** widening the caller admits a counterexample; the selected requirement remains mandatory even for the actual valid entry.
- **Psi 05–07:** precondition arithmetic erases after proof; selector/data lanes, addition and consuming subtraction reach executable Terminal Psi.
- **Omega 00–02:** ordinary u64 arguments/results, two-row selector tables, record neighbors and four hosted entry plans retain their layouts and call shapes.
- **Omega 03–06:** dynamic calls, integer register homes and live marker/neighbor values must preserve 100 and the selected marker.
- **Omega 07–09 and emission:** complete images and row relocations must execute with matching native and Terminal exit/crash oracles.

Source and Build files are pinned byte-identically under
`tests/omega/{pass,fail}/facts/dynamic_trait_input_arithmetic_through_*`.
Other operational envelopes, Product parity and matching-host table
qualification remain separate parent acceptance.

## Recorded acceptance

At source checkpoint `297562733b94b15caabac5dd979ed34bbf74ba51`, the scoped
Source gate records all nine positives checked and all nine twins rejected at
the intended `dyn WeightedCombine::combine` requirement. All nine positives
execute on Linux x86-64 with native exit 0 and Terminal exit 0. Both selector
arms, distinct markers and the field neighbors retain their authored oracles.
Source/Build mirrors and the one-threshold twin differences were verified.

The compiler inputs are identical to the accompanying IEEE checkpoint
`95f6bd2b44300afdc37712cf585b1f3515adc819`; the freshness build confirms the
same optimized CLI SHA-256
`26d0ed4d3fd0e3f49252a81b8a46f12c7a3547d0918ad4d8584912b4c521848c`.
The [replay archive](https://chatgpt.com/api/library/files/libfile_fd3245e81d7c8191aefaa782e1a8073f/download)
is retained in the author's Library, version 1 (access required), with both
source bundles, each original observation revision, commands and raw records.
SHA-256: `570d55dcdf4075b6d1473e9b39fabf61b81e6760aaef826e46444edd0fa4966e`.
This supplements the 48 IEEE/control Source observations and 21 native/Terminal
executions without relabeling them as new runs. Other-host and Product compiler
execution remain unmeasured.
