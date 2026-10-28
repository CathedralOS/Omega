# Terminal value completion

Owner: `CHECKED-PACKAGE-REUSE` in [TASKS.md](../../../../../TASKS.md), through its ordinary std-review prerequisite.

A terminal invocation forwards its normal value and explicit mutable backing without introducing a caller continuation.

Baseline `85e1ce7073884a9d1ea83aeef055030d0f44daff` rewrites an unconditional terminal value call into a synthetic local for mutable callers. Its checked SCC then correctly refuses the resulting non-tail call. Scalar callers and Unit results have separate legacy exceptions. The target-body positive and its pending-result twin both checked on that baseline because nested target helpers lost their source target context; neither baseline acceptance establishes the twin's obligation. The repair retains lexical target context without making helpers authored family members. The repair removes that terminal hoister and leaves operand evaluation and guard subject capture with their existing owners. Mathematical completion uses the ordinary selected-denotation judgment while proof submachines retain checked body entry. Computed successor operands publish the same exact callee-entry coordinates as pure operands.

- [local](local/main.omg), [pending-result twin](local/invalid/main.omg)
- [field](field/main.omg), [pending-result twin](field/invalid/main.omg)
- [parameter](parameter/main.omg), [pending-result twin](parameter/invalid/main.omg)
- [call_result](call_result/main.omg), [pending-result twin](call_result/invalid/main.omg)
- [match_join](match_join/main.omg), [pending-result twin](match_join/invalid/main.omg)
- [const_name](const_name/main.omg), [pending-result twin](const_name/invalid/main.omg)
- [other_state](other_state/main.omg), [pending-result twin](other_state/invalid/main.omg)
- [other_machine](other_machine/main.omg), [pending-result twin](other_machine/invalid/main.omg)
- [target_body](target_body/main.omg), [pending-result twin](target_body/invalid/main.omg)

Every place executes two 100,000-step cycles, preserves the explicit caller-owned counter, and forwards distinct scalar values. The zero-step call leaves backing unchanged. The Match place exercises 41 on the selected arm and 17 on the fallback; either always-selected branch contradicts an authored result. Each twin moves the cyclic call under an identity computation, leaving pending caller work and requiring the non-tail refusal. All four hosted entry bindings are authored. Identical fixtures live under `tests/omega/{pass,fail}/facts/nested_machine_terminal_value_*`.

Establishment: source terminal position, exact result type and exact block acknowledgement justify forwarding; argument computations finish first. Identity/lifetime: every transfer retains the same explicit mutable backing, while scalar actuals keep their values. Composition: a complete SCC must consist of classified terminal occurrences; a tail call elsewhere cannot cover a nested operand. Soundness: callback, proof and authority admission remain independent. Psi/Terminal and Omega/native execution must demonstrate the counter and returned values before this row is closed, including constant-stack realization on the long cycle. Other hosts are unverified here. Parent persisted checked/native products and timing acceptance remain open.

Precursor evidence: the recorded `run --both --target macos_arm64` CLI calls took the explicit-target compilation route; they are compile observations, not runtime agreements. Their positives compiled and their nine twins refused for the intended diagnostic. Fresh corpus records must report actual native and interpreter exits before runtime acceptance is established. Parent persistence/timing requirements and other hosts remain open.
