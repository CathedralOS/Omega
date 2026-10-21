# Generic case subject

Matching `Choice<u64>` retains its exact payload type beside `Choice<bool>`, so
the selected payload returns as `u64`.

Parent: **BUMP-ALLOCATOR-CANARY**, specifically Cathedral's aligned real-backing
placement path. This repair lets distinct word/page placement results coexist;
it does not close allocator cleanup, return, or reset acceptance.

Cathedral's page preparation uses the same placement-result declarations for
both a word and a page. Case classifiers must follow the tested value, not the
first specialization prepared anywhere in the program. The generic `wrap`
helper here also creates applications during preparation; the Boolean result
and the empty numeric case are both exercised, not merely declared.

Each of `local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`,
`other_state`, `other_machine`, and `target_body` carries the same sum to a
numeric result operation. All projects bind the four hosted targets. The
runtime oracle requires numeric payload 17, Boolean payload true, and empty
numeric result 0. `omega run --both <place>/main.omg` requires native and
Terminal execution agreement. Runtime comparisons check the result; they do
not establish the payload's static type for the preceding return operation.

Each `invalid/main.omg` changes only the numeric case classifier to the
same-spelled `Other::Value`. A wildcard preserves coverage so the refusal must
reach exact declaring-type membership, not fail for an incomplete pattern
matrix. Positive and negative counterparts live under
`tests/omega/{pass,fail}/facts/generic_case_subject_through_*`.

The additional `generics/generic_case_subject_sibling_bounds_*` fixture pair
uses identical `u64` payload layouts with different const-argument bounds.
Borrowed matching must preserve the narrow application's bound while a computed
broad result receives its own bound. Widening only the selected application's
limit from 10 to 100 must reject the unchanged `result <= 10` obligation.
A nullary named constant is also observed by membership in this pair.

The requirement chain is:

- Establishment: construction selects the generic application and its payload.
- Identity/lifetime: matching retains that exact application through each place;
  constant folding must not replace it with the template or a sibling instance.
- Composition: a generic helper, numeric and Boolean instances, and empty and
  payload-bearing cases coexist; the selected payload has its actual type.
- Soundness: a different declaration with an identical case name is not evidence
  about this subject. Reject twins demand the exact-owner diagnostic.
- Psi lowering: the interpreted tag branch and payload return must preserve the
  checked type selection after proof erasure.
- Provider/layout/ABI: ordinary ProgramEntry bindings realize both payload types
  and generic sum return values through the existing calling-plan route.
- Instructions/frames: native dispatch, payload access, calls, and state
  transfers must preserve the same observed values.
- Image/execution: the emitted host image and Terminal interpreter must both
  exit zero; foreign target declarations are not foreign-host execution evidence.

Producer-bound observations and remaining acceptance are recorded in the
qualification and claim records. This row does not establish aggregate placed
access, page-table installation, or Cathedral's later interrupt/scheduler work.
