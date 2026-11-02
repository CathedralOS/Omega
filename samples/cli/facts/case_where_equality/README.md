# Case-where equality

Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.

This row advances [BUILD-OWNED-TARGET-SELECTION](../../../../TASKS.md): its target body is an ordinary `SelectedPointWidth` implementation of the `PointWidth` requirement, selected by four explicit Build rows. `ProgramEntry` still binds the ordinary entry for Windows x86-64, Linux x86-64, macOS ARM64, and Linux ARM64. Broader library/product migration and removal of the compiler's retired target-family paths remain open.

The complete `Interval` default domain admits Empty or Point with `lo == hi`. Construction and call admission establish that domain independently; a selected tag alone does not establish its predicate. Every place retains the exact `main.width = hi - lo` operation. Setup runs in an ordinary wrapper before an independently checked body, so literal 17/17 setup cannot supply the body's general proof. The const place retains its original 4/4 named constant and declared `const INTERVAL: Interval` header.

| Place | Positive | Fact-only twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | [invalid/main.omg](invalid/main.omg) | original non-Copy Interval |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | copied local observation |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | borrowed aggregate field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | original owned Interval parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | ordinary identity result |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | both original fields and actual true/false routes |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | exact static header value |
| nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | original nested-machine arrival |
| other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | original ordinary invocation |
| target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | public requirement and explicit Build selection |

Each new twin is byte-identical to its positive except removal of ` && (self.lo == self.hi)`. It keeps every setup assignment, payload, entry, wrapper, helper and runtime observation. The independently checked subtraction must refuse because the declared domain no longer carries the equality. The old twins also omitted setup and runtime code; their historical rejection is not the new fact-only enforcement evidence.

Caller-owned initialized storage observes width changing from 99 to 0, neighboring canaries 101/107, and the original hits/flag behavior after return. Match executes true and false, checking the first caller-owned route17 and width before the second call, then route23 and the original caller home. The original Point17/17 fields are retained. These observations establish this row's subtraction and writeback, not every possible scalar instruction width or complete payload identity after every call.

The nested [data_predicate](data_predicate/README.md) row is an independent existing production and is unchanged by this migration.

## Requirements

- **Establishment:** constructors prove the full declared default domain before completion; entry arguments and Const/Value headers are independently admitted operands. Removing the equality cannot borrow the caller's literal payload as a body premise.
- **Identity and lifetime:** constructor and completed-read receipts name fresh exact values. Live reads retain current-storage and overlapping-write fences; detached snapshots keep their own meaning. No reference descriptor supplies an unobserved referent.
- **Composition:** ordinary calls and nested arrivals substitute exact actuals. Both match branches retain the complete same domain, and copied/synthetic transition homes carry the completed value's admission. Case payload access consumes an independently proved classifier through the existing typed projection owner.
- **Soundness:** malformed Point4/3 construction and const admission refuse; overlapping payload mutation cannot reuse the old nominal predicate, and Empty cannot provide another case's payload. The repair adds no copy permission, caller guard, receiving-type premise, or tag-from-equality rule.
- **Checked to Terminal Psi:** the unchanged exact-u64 subtraction, tags, case fields, ordinary calls, and crash routes lower and execute in both engines. Admission metadata is checking context, not a new runtime representation.
- **Provider and physical realization:** the target body has the original `&mut Main`, empty reach, and Trap ceiling. Four Build choices select the same ordinary body. Caller-home observations exercise the realized record fields, case payload, parameter/result transport and native writeback. Four-target artifacts are distinguished from actual host execution.

## Evidence

Qualification remains unfinished while this draft is prepared. The fixed original source is bb7/published b68. Actual optimized 08b Linux baseline passed eight original positive Source/runtime pins; const-header, root, and selected-provider prototype refused. Actual dd14 retained constructor/header admission but still refused their subtraction. Its follow-up trace isolated current payload Case projection and copied-value admission loss. All red observations remain in the claim record.

The final record must name the actual repaired producer, Source and engine outcomes, relevant invalid controls, counterfactuals, artifact identities, pin correspondence, independent exact review and publication. No execution of a later integration or a foreign artifact is inferred from these historical observations.
