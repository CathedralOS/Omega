# Case-where equality

Case Point(lo, hi) where lo == hi makes hi - lo exact in u64, without a guard or Wrapping.

This row advances [BUILD-OWNED-TARGET-SELECTION](../../../../TASKS.md): its target body is an ordinary `SelectedPointWidth` implementation of the `PointWidth` requirement, selected by four explicit Build rows. `ProgramEntry` still binds the ordinary entry for Windows x86-64, Linux x86-64, macOS ARM64, and Linux ARM64. Broader library/product migration and removal of the compiler's retired target-family paths remain open.

The complete `Interval` default domain admits Empty or Point with `lo == hi`. Construction and call admission establish that domain independently; a selected tag alone does not establish its predicate. Every place retains the exact `main.width = hi - lo` operation. Setup runs in an ordinary wrapper before an independently checked body, so literal 17/17 setup cannot supply the body's general proof. The const place retains its original 4/4 named constant and declared `const INTERVAL: Interval` header.

| Place | Positive | Fact-only twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | [invalid/main.omg](invalid/main.omg) | original non-Copy Interval |
| local | [local/main.omg](local/main.omg) | [invalid_local/main.omg](invalid_local/main.omg) | copied local observation |
| field | [field/main.omg](field/main.omg) | [invalid_field/main.omg](invalid_field/main.omg) | borrowed aggregate field |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid_parameter/main.omg](invalid_parameter/main.omg) | original owned Interval parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [invalid_call_result/main.omg](invalid_call_result/main.omg) | ordinary identity result |
| match join | [match_join/main.omg](match_join/main.omg) | [invalid_match_join/main.omg](invalid_match_join/main.omg) | both original fields and actual true/false routes |
| const name | [const_name/main.omg](const_name/main.omg) | [invalid_const_name/main.omg](invalid_const_name/main.omg) | exact static header value |
| nested machine | [other_state/main.omg](other_state/main.omg) | [invalid_other_state/main.omg](invalid_other_state/main.omg) | original nested-machine arrival |
| other machine | [other_machine/main.omg](other_machine/main.omg) | [invalid_other_machine/main.omg](invalid_other_machine/main.omg) | original ordinary invocation |
| target body | [target_body/main.omg](target_body/main.omg) | [invalid_target_body/main.omg](invalid_target_body/main.omg) | public requirement and explicit Build selection |

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

The original source was frozen at `bb7ed1722584` / published `b68e4aa32a7b`. Actual optimized Linux producer `08b7c22717e5` passed eight original Source/runtime places, while the const-header, root, and selected-provider prototype refused the exact subtraction. Traces separated missing nominal admission, copied/current case projection, and the missing scalar `BoundProjection` equality rewrite. The repair retains completed independently admitted values, uses the existing proved-case projector, and lets immutable bound projections use the existing typed equality owner. Construction still proves its own complete predicate before any new receipt exists.

Actual optimized producer `70036176871c` (`opt-level=1`) then checked all ten positives and properly refused all ten equality-only twins; all ten runtime pins and ten ordinary public programs produced native exit 0 and Terminal exit 0. These are the actual historical results from [the row capture](https://github.com/CathedralOS/Omega/actions/runs/41424258406) and [the public continuation](https://github.com/CathedralOS/Omega/actions/runs/41427117249), not execution claims about a later compiler. The continuation also preserves two harness attribution errors: an auxiliary fixture's stale diagnostic label and a missing-provider test asked at Source checking instead of native realization.

The actual integration against `e90267933c4a` changed reaching-storage versions, Boolean evaluation, and backend/Terminal consumers. Fresh optimized producer `3c9cfd4b5e2d` checked all twenty pins with intended outcomes and ran all ten positive pins at native 0 / Terminal 0. Ordinary public root, target, const and strong-match runs also returned 0 / 0; six focused preservation controls passed, and three runtime mutants trapped in both engines. [The integrated capture](https://github.com/CathedralOS/Omega/actions/runs/41429826043) retains its actual source, SDK, binary profiles, locks, raw observations and four artifact hashes.

That capture's aggregate is red only because an auxiliary missing-Linux-row control still found the compiler's existing sole-covering-candidate fallback and published an image. The unchanged provider-policy owner accounts for that observation; it is not counted as missing-supply rejection or parent completion. Separate [absent/competing candidate controls](https://github.com/CathedralOS/Omega/actions/runs/41433671965) properly reject on the same immutable compiler. Malformed construction/header observations are recorded in their [Source addendum](https://github.com/CathedralOS/Omega/actions/runs/41433976838), separately from the original full row.

Malformed construction/header, inactive payload and overlapping payload writes remain refusal controls. Independently materialized equal literals remain valid without a data predicate; their literal proof is separate from this row's domain transport. The previously recorded maximum-u64 construction gap and unused uninstantiated binder-only scratch refusal were not requalified here and are not claimed repaired. Broader routed/view/ownership behavior and the parent task's remaining acceptance stay open. Four-target artifact preparation establishes image production, with actual execution claimed only for the recorded Linux host.
