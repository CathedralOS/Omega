# Arithmetic at the public selector contract

a runtime dyn trait selector dispatches an inline conformance member over explicit data arguments to establish 100u64 when subtracting 100u64.

Owning outcome: **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** in
[TASKS.md](../../../../../TASKS.md), proving the required integer input at a runtime-selected call.

This production proves the required left input of 60 from `59 <= a <= 60`
and `a + 1 >= 61`. The upper bound independently makes the addition safe;
the comparison then rules out 59. The right input remains 40. Each place
subtracts 100 from the result before checking zero.

The earlier bounded-addition check used fixed base
`eb9acb01452251ddb0c338fa400fb9cd476c8bc4`, optimized CLI
`68e71955520e14f15e686f4cb7c867d04d15246c178d5f22cfc3d29f89098ec7`.
It verifies the existing integer proof rules; the accompanying IEEE repair
does not add a special rule for this call or change the provider contract.

Every twin changes only `a + 1 >= 61` to `a + 1 >= 60`, admitting 59.
The actual entry remains 60, but the generic caller must now reject its public
`WeightedCombine::combine` requirement. All four hosted entries are authored.
The two providers use different parameter names and marker results 11 and 13;
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

- **Establishment:** the independently safe addition and input bounds prove the public precondition; each implementation proves the public result.
- **Identity and lifetime:** the public requirement applies to the actual arguments, regardless of the implementation or its parameter names.
- **Composition:** the same selector and arithmetic implication survive local/field/parameter storage, call result, both joins, constant, nested machine, forwarding machine and Build-selected target.
- **Soundness:** the weaker caller contract permits 59, which violates the requirement. Passing 60 at entry does not excuse that invalid caller contract.
- **Psi 06–08:** precondition arithmetic erases after proof; selector/data lanes, addition and consuming subtraction reach executable Terminal Psi.
- **Omega 00–02:** ordinary u64 arguments/results, two-row selector tables, record neighbors and four hosted entry plans retain their layouts and call shapes.
- **Omega 03–06:** dynamic calls, integer register homes and live marker/neighbor values must preserve 100 and the selected marker.
- **Omega 07–09 and emission:** complete images and row relocations must execute with matching native and Terminal exit/crash oracles.

Source and Build files are pinned byte-identically under
`tests/omega/{pass,fail}/facts/dynamic_trait_input_arithmetic_through_*`.
Broader operational contracts, product-compiler parity and other-host execution
remain parent acceptance.

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
executions without relabeling them as new runs. Those historical runs measured
Linux x86-64; they did not measure other hosts or the product compiler.


The unchanged eighteen projects here also pass the full-source migration runs at
`0a53590a2991a39c810225fade12860d577192df` on Linux x86-64 and macOS ARM64:
nine positives check and exit 0 in both engines, and nine weakened-input controls
reject their public call requirement. The [parent validation record](../README.md#validation-and-remaining-scope)
links those runs and explains the retained observations. Windows x86-64,
Linux ARM64 and product-compiler execution remain unmeasured.

The complete integrated replay at `281c0d530cb7a5c108e8301ea6b5851f441244e1`
checks and executes these same nine positives, with native and Terminal exit 0,
and rejects all nine weakened-input twins on Linux x86-64 and macOS ARM64.
The [parent validation record](../README.md#validation-and-remaining-scope)
links the fresh runs and preserves the earlier observations separately.
