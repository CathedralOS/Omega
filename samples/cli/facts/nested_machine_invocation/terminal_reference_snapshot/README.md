# Terminal reference snapshots

Owner: [NESTED-MACHINE-UNIFICATION](../../../../../TASKS.md).

A terminal invocation captures the completed scalar observation before transferring its existing mutable reference; distinct mutable inputs remain disjoint.

This is the ordinary terminal invocation boundary, independent of lexical declaration placement. `forward` passes an existing mutable descriptor, observes its value through a completed shared call, and transfers it to `update`. The update changes caller-visible storage and preserves adjacent canaries; the returned scalar must remain 37 after that storage becomes 73. A second mutable input becomes 19 and keeps its own canary.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [duplicate mutable input](local/invalid/main.omg) |
| field | [source](field/main.omg) | [duplicate mutable input](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [duplicate mutable input](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [duplicate mutable input](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [duplicate mutable input](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [duplicate mutable input](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [duplicate mutable input](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [duplicate mutable input](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [duplicate mutable input](target_body/invalid/main.omg) |

Every twin changes only the terminal call's third actual from the distinct descriptor to its first mutable input. Additional controls reject [a fresh mutable borrow](controls/fresh_mutable_borrow/main.omg), [a returning call](controls/ordinary_returning_call/main.omg), [a later mutable access](controls/later_mutable_observation/main.omg), and [a retained shared input](controls/retained_shared_input/main.omg). The staging exception does not create authority or release a retained loan.

Establishment and composition come from the completed reference-free observation and terminal transfer of an existing descriptor. Exact storage identity and lifetime remain explicit; distinct input and neighboring canaries make writes observable. No proof-only value, implicit capture, or new authority is introduced. The checked calls lower through Terminal Psi and native calling plans with the copied result live across the update. Native register/frame homes, final encoding and image emission must preserve both mutations and all canaries. This row exercises hosted scalar/reference calling lanes, rather than every target ABI placement.

Frozen baseline `1f41b1eee4549d72c27e8efa424a43923dfe6318` (CLI SHA `ab98269f85bed75a32044a848ddf327d25201e6e3c60f8768fa2c106e9c7acbe`) refuses all nine positives as competing mutable/read arguments. The repaired comparator built from that revision plus the terminal staging change (SHA `ed7c0c235871e0048d1b5cabcf0eda0e3241a1ca1501e73aa1a0271c551e7081`, pinned nightly, opt-level 1) executes all nine on macOS ARM64 with native exit 0 and Terminal exit 0. Nine twins and four authority controls reject with the relevant overlapping-argument diagnostic. Exact commands, source hashes, producer receipts and raw observations are retained under `build/product-front/`. The row is pinned under `tests/omega/{pass,fail}/facts/nested_machine_invocation/terminal_reference_snapshot/`.

The integrated optimized producer at source `e12c034a9fc810e7f371bf6513d35a6923eedd59` (CLI SHA `3c8ceb14773e48665bb278fc97662da7d3480eadc2636830ff79c2e493c69c6e`) corroborates all nine positives with native and Terminal exit 0, and the twins and authority controls retain their intended refusals. Target-body recipes select each target explicitly. The pinned source and native/Terminal corpus records retain these observations.

All four hosted target bindings are authored; executable observations are macOS ARM64. The parent outcome remains open for its remaining product and complete invocation-contract acceptance. This row does not establish other-host runtime behavior or every ownership/termination contract.
