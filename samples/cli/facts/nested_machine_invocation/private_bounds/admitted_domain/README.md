# Admitted input domains at private reads

Owner: [NESTED-MACHINE-UNIFICATION](../../../../../../TASKS.md).

Every private arrival carries index < table.bytes.len, so the helper can read table.bytes[index].

This production establishes the bound from two independent facts: the admitted table's `count <= 4` default domain and the caller's `index < count` guard. The private helper reads the fixed array directly under the transported argument facts. A pending constructor or destination annotation supplies no input premise.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [inclusive guard](local/invalid/main.omg) |
| field | [source](field/main.omg) | [inclusive guard](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [inclusive guard](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [inclusive guard](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [inclusive guard](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [inclusive guard](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [inclusive guard](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [inclusive guard](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [inclusive guard](target_body/invalid/main.omg) |

Each twin changes only `<` to `<=` in the helper's selecting guard, admitting the out-of-range endpoint. Positives exercise two distinct readable elements, the full-table boundary fallback and an empty table; distinct canaries remain unchanged. Copies are pinned under `tests/omega/{pass,fail}/facts/nested_machine_invocation/private_bounds/admitted_domain/`.

The incoming table's admitted type establishes its default domain before the guard. Exact formal/actual substitution transports the two facts without lexical capture. Only this invocation's admitted inputs and requirements become premises; ordinary formation and storage-write invalidation retain their separate owners. Terminal Psi and native calls must preserve the selected values and fallback paths through their ordinary scalar/reference calling plans, frame homes, instruction selection and final image encoding. This does not add layout, cleanup, ABI or target-specific authority.

The frozen minimal read and eight well-formed initial places fail on the staging-only comparator (source `1f41b1eee4549d72c27e8efa424a43923dfe6318` plus its recorded staging diff, SHA `ed7c0c235871e0048d1b5cabcf0eda0e3241a1ca1501e73aa1a0271c551e7081`) because the private read loses the admitted table bound. Literal-bound guards and an explicit helper requirement pass as diagnostic variants. A malformed initial target-body spelling is retained as an authoring failure and supplies no defect evidence; its corrected form needs its own recorded comparison.

The optimized reference producer at source `e12c034a9fc810e7f371bf6513d35a6923eedd59` (CLI SHA `3c8ceb14773e48665bb278fc97662da7d3480eadc2636830ff79c2e493c69c6e`, pinned nightly, opt-level 1) executes all nine positives and the unrelated-write control with native exit 0 and Terminal exit 0 on macOS ARM64. Nine twins reject the missing strict bound; the changed-index control rejects its new storage version, and the pending-constructor control rejects its own default-domain formation. Current target-body recipes select the named implementation explicitly for each target.

The source and native/Terminal corpus records retain those pinned outcomes. Exact commands, source hashes, raw engine observations and producer receipts remain under `build/product-front/`. All four hosted bindings are authored; other-host execution and full product compiler parity are not established here. The parent outcome remains open for its remaining product and invocation-contract acceptance.
