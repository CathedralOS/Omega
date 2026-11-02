# Ordinary-call distance endpoints

Owner: [NESTED-MACHINE-UNIFICATION](../../../../../../../../../../TASKS.md).

A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

This production keeps the existing seven/empty counter operation. The upper endpoint is the ordinary `endpoint(stop)` call, whose published contract proves `result == stop + 1` under `stop < 7`. Starting at cursor 0 and stop 6 performs three cursor advances and four upper-input decrements, yielding `Counts { value: 10, climbs: 3, shrinks: 4 }`. The idle invocation preserves all three fields. No caller guard re-establishes the rank. Each twin changes only `ceiling - 1` to `ceiling - 0`.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](../../../../../local/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../local/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |
| field | [source](../../../../../field/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../field/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |
| parameter | [source](main.omg) | [source](invalid/main.omg) |
| call_result | [source](../../../../../call_result/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../call_result/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |
| match_join | [source](../../../../../match_join/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../match_join/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |
| const_name | [source](../../../../../const_name/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../const_name/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |
| other_state | [source](../../../../../other_state/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../other_state/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |
| other_machine | [source](../../../../../other_machine/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../other_machine/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |
| target_body | [source](../../../../../target_body/bounded_distance/moving_upper/computed_upper/call/main.omg) | [source](../../../../../target_body/bounded_distance/moving_upper/computed_upper/call/invalid/main.omg) |

Fixed optimized base `f45f51d86f5591a3d731f4b855ee4d09187857bb` accepts the original arithmetic-endpoint parameter witness. The frozen ordinary-call positive and its nondecreasing twin both refuse with `a bounded cursor rank lacks independent entry formation`; that twin therefore supplies no baseline evidence of non-descent enforcement. The composed endpoint control also refuses at this earlier boundary. Raw observations and the retained producer digest are in `build/advance/` in the owning worktree.

The repaired optimized producer accepts the unchanged ordinary-call and composed endpoints. Its shared formation path also preserves the existing anonymous-literal and indexed endpoint controls. The latter retains entry selector receipts; a call result does not replace those read obligations. A nested call is reserved under its completed actual values before its denotation crosses a query.

Outer acceptance command:

```sh
python3 tools/sample_native_gate.py --filter bounded_distance/moving_upper/computed_upper/call
```

- Establishment: endpoint formation visits children before publishing result facts, proves each selected call requirement under the captured actuals, and keeps its exact finite carrier and arithmetic policy. The [missing-premise control](controls/missing_call_premise/main.omg) tightens only the helper requirement to `value < 6`.
- Identity and lifetime: the endpoint reads the invocation input roster. Arriving values substitute all actuals simultaneously; current storage or an enclosing invocation supplies no replacement facts. The retained application names its source owner, exact expression, call/cast decisions and coordinate inputs.
- Composition: [the composed endpoint](controls/composed_endpoint/main.omg) nests an identity call and places the endpoint under addition. Every component edge still proves nonincrease, and removing strict edges must leave an acyclic graph.
- Soundness: call premises, independent carrier formation, selected operational contracts, ordinary loans and whole-cycle descent remain separate obligations. A result guarantee cannot justify forming its own partial argument. The [effectful endpoint](controls/effectful_endpoint/main.omg) additionally writes caller storage and must reject denotational use. Ranking expressions enter the same selected-call operational collector as proof facts and measure bodies.
- Psi 05–07: checked endpoint applications carry the admitted calls and casts into the ordinary scalar computation graph, including their exact checked target contracts. There is no synthesized statement call or special endpoint evaluator.
- Omega 00–02: the same four hosted `ProgramEntry` rows and explicit calling plans are retained. This production changes no public Terminal wire schema or target ABI.
- Omega 03–06: the caller-visible three-field counter distinguishes the two branches and the idle path; this does not establish a portable physical stack limit.
- Omega 07–09: every positive and the composed control must produce explicit native and Terminal exit-zero agreement. Their counter checks trap on a wrong result or changed idle path. The source and Linux both-engine corpus rows retain the observed acceptance of the pins below. Other-host execution and the broader parent migration/product implementation remain open.

Pins: `facts/nested_machine_joint_rank_endpoint_call_<place>_{exit,rejected}` and `termination/private_rank_endpoint_{composed_endpoint_exit,missing_call_premise_rejected,effectful_rejected}` under `tests/omega/{pass,fail}/`.
