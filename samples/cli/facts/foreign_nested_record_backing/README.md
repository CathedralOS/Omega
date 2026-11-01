# Retained original nested record backing

Parent: **NORMALIZED-ABI-LOWERING** in `TASKS.md`.

A mutable foreign registration retains the exact original nested Buffer backing until completion consumes Pending; completion writes its nested cell to 73 while marker 91 and an earlier copy 37 remain unchanged.

`Buffer` is an ordinary finite record containing marker `91` and a nested `Cell`. The C provider uses the corresponding 16-byte nested C struct; static assertions pin the outer size and field offsets. Registration writes nested value `41` and retains the original pointer. Completion writes `73` and verifies marker `91` before consuming the registration. The original copied value stays `37`.

The nine places preserve the same operation through local, field, parameter, call result, match join, const name, nested machine, other machine and selected target body. Every entry binds all four hosted targets. Each invalid twin inserts one nested scalar read before its linear Pending completes.

The [array continuation](arrays/README.md) exercises a fixed array of nested records with adjacent element canaries and the same retained original allocation.

This row exercises the recursive original-backing codec. Candidate `f0c9d9694a74` passes all nine places and the repeated-child composition control with actual native and Terminal exit 0 on macOS ARM64. The intended reject twins also pass; the other hosted targets are being qualified. The existing capacity and Pending content contracts and exact normal `Buffer::Owned` guarantee are preserved. Nested record shape adds no qualification or pointer-authority exception.

## Required behavior

- Establishment: the boundary takes a qualified exact mutable Buffer loan and returns linear Pending custody; `ensures buffer in Buffer::Owned` is the same explicit normal guarantee as the flat retained row. The nested `Cell` is an ordinary unqualified record.
- Identity and lifetime: both C writes reach the original nested scalar through the retained outer pointer, and completion consumes Pending before Omega reads the original again. Marker 91 and the copied scalar 37 are independent observations.
- Composition: the nine places transport Pending custody through local binding, an enclosing field, a parameter, call result, join, const-named construction, nested machine, another machine and selected target body.
- Soundness: each twin inserts one read of `buffer.cell.value` while retained exclusive custody remains live. Qualification and content projection are unchanged; nested shape grants no extra access.
- Psi 05–07: checking retains exact lifetime and access; lowering carries the original structural path through the boundary and Terminal representation. Terminal must execute the same nested storage semantics, including normal restoration coverage.
- Omega 00–02: the public foreign binding selects the same C ABI pointer lane, with the finite outer/nested layout joined to its original semantic scalar path. C asserts size 16, marker offset 0, cell offset 8 and nested scalar offset 0.
- Omega 03–06: materialized original storage and its nested scalar remain stable across registration, custody transport and completion; separate calls cannot replace that backing with independent snapshots.
- Omega 07–09: actual matching-host native execution and Terminal execution must both exit 0 for every positive. Source reject twins and the provider's real lifecycle traps retain separate oracles. Four authored target bindings alone do not establish four-host execution.

The authority is the existing [foreign storage contract](../../../../wiki/spec/build/foreign_storage.md), [original structural access](../../../../wiki/spec/terminal-psi/structural_access.md), and [loan restoration](../../../../wiki/spec/terminal-psi/loans.md). This row exercises recursive semantic field paths in a finite embedded record. [Owned arrays](arrays/README.md) and [borrowed fixed-array views](views/README.md) extend this mechanism. Recursive pointer graphs, variants, dynamic views, independently qualified descendants and owned aggregate result publication remain separate acceptance.

## Measured baseline

Optimized macOS ARM64 producer `a96bc885ffebee4ebd1a35845700c05c6fe0c668` runs every positive natively with exit 0, while Terminal declines each with `native import needs a materialized non-scalar field codec`. Every reject twin fails for the intended read of nested original storage while its Pending remains live. The actual CLI returns 0 for the native-success/Terminal-decline pair; acceptance therefore requires the explicit Terminal exit-0 observation as well.

The real C fixture self-test exits 0 and all nine lifecycle controls reach their intended deliberate trap on macOS ARM64. These observations establish the baseline gap and control validity, not completed compiler acceptance or other-host execution.

## Canonical nested declarations

Every completion branch is a nested `machine`, and the main body is the free machine `run_main`. The bad branch publishes its own `crashes Trap` contract; nesting does not inherit runtime contracts. The `other_state` place declares a lexical `forward<'a>` helper inside non-generic `run_main`. Its lifetime and Pending argument are explicit, so it captures no enclosing static telescope or runtime state. This is the same retained-token transport and original-backing operation.

The historical baseline with legacy state declarations is preserved separately. Canonical focused checking and native execution establish the same missing Terminal codec. Enclosing generic/static telescope capture remains an existing unrelated implementation limitation rather than a feature this record-layout row claims to establish.

## Current qualification

[PR #229](https://github.com/CathedralOS/Omega/pull/229) carries the implementation and its qualification. The semantic codec walks the complete embedded record, preserving every containing field identity and recursive erasure. Each scalar leaf joins its exact target-layout path, logical/stored codec, offset and extent; overlaps and incomplete coverage refuse. Reusing a child type in siblings is valid, while a cycle in active storage ancestry refuses.

The [repeated-child control](controls/repeated_child_type/main.omg) uses the same `Cell` type twice, with distinct parent paths. Its C provider retains typed original/repeated pointers under a private discriminator and checks both actual layouts. The same source and runtime obligations hold for its one-read reject twin.

Owned value ABI and scalar aggregate publication authenticate their separate direct semantic field roster before provider loading. Equal flattened bytes cannot replace a nested semantic field, including an empty nested field with no scalar leaves. The focused Rust admission test preserves its flat positive and rejects nested paths, missing empty fields, changed scalar class, offset and field span.

All nine places and the repeated-child control pass actual native/Terminal corpus execution and public `omega run --both` on macOS ARM64, Linux x64, Linux ARM64 and Windows x64. Each host also passes the ten intended source rejections and the C self-test with nine deliberate lifecycle traps. macOS and both Linux observations use producer `f0c9d9694a74`; Windows uses `4e01aaa87b6`, whose Rust/compiler, library and fixture inputs are identical. The historical baseline keeps its original producer and outcomes.

The macOS and Linux regression cohorts include 33 actual native/Terminal cases, the same 33 public CLI executions, 32 intended rejections, three C self-tests and 27 deliberate traps. Windows also passes the existing shared row. Its existing mutable `mixed_issuers` case times out natively while Terminal returns zero, so the complete Windows gate remains failed. The gate now completes independent cohorts and preserves a failed overall result instead of hiding the later nested row.

The original Windows observation in run `40282992153`, attempt 1, fails on mutable `mixed_issuers`. Its one bounded unchanged-source/deadline follow-up fails on shared `const_name`; run `40288218879` fails on mutable `mixed_issuers` while measuring the new row successfully. Actual Windows products and cross-produced products from unchanged base `a96bc885ffeb` and candidate `f0c9d9694a74` are byte-identical for those timed-out programs. These failures remain failures; no environmental cause is inferred.

The broader foreign-record gate in Linux runs `40282992153` and `40282994800` fails before native execution on the older independent opaque rows. The unchanged base reproduces its `Build::select_representation` declaration refusal: Product sources are deferred until after Build, while selection demands their declarations before execution. That separate Build migration regression remains open under the parent outcome. These observations qualify this finite nested borrowed-record codec, not a complete release or the wider resident/view codecs.

## Applied lifetime checking

The 2026-11-01 baseline producer `fbf567b724db78dbc5eab2dc4f30e35ed979a703` rejects all ten original positives at `Retained::content`: `enclosing callable lifetime telescopes have different arity`. The ten negative controls also stop there instead of reaching their active-loan obligation. Under [exact requirement edges](../../../../wiki/spec/language/conformances.md#exact-requirement-edges), `'source` inside `Content<Pending<'source>, ...>` specializes the trait argument; it is not an extra invocation binder.

Producer `d3b71168d9303e04894993fbacfd640d8e1bc111` consumes the existing applied input-region judgment when checking nested callable bounds and completes output refinement with the same prepared comparison. All ten positives now check and execute with actual native and Terminal exit 0 on macOS ARM64; all ten twins again reject the original early nested read. The unchanged C fixture passes its self-test and nine deliberate lifecycle traps. Other-host observations above remain historical.

The match join now exercises both Boolean inputs. Its ordinary transfer helper writes distinct route markers 17 and 23, and the caller reads the marker immediately after the join while `Pending` still holds the buffer loan. Completion then checks the same 73/91/37 observations and the selected route. Reversing an arm or omitting its marker cannot satisfy this oracle. The invalid twin differs only by the original forbidden nested-buffer read.

The witnesses and their matching rejects are [local](local/main.omg) ([reject](local/invalid/main.omg)), [field](field/main.omg) ([reject](field/invalid/main.omg)), [parameter](parameter/main.omg) ([reject](parameter/invalid/main.omg)), [call result](call_result/main.omg) ([reject](call_result/invalid/main.omg)), [match join](match_join/main.omg) ([reject](match_join/invalid/main.omg)), [const name](const_name/main.omg) ([reject](const_name/invalid/main.omg)), [nested machine](other_state/main.omg) ([reject](other_state/invalid/main.omg)), [another machine](other_machine/main.omg) ([reject](other_machine/invalid/main.omg)), and [target body](target_body/main.omg) ([reject](target_body/invalid/main.omg)). The [repeated-child control](controls/repeated_child_type/main.omg) has its own [reject twin](controls/repeated_child_type/invalid/main.omg). All twenty have exact Source pins under `tests/omega/{pass,fail}/facts/foreign_nested_record_backing_*`.

This repairs checking of the existing retained-storage contract. The finite nested-record row's Build migration is recorded below; the array/view continuations and wider parent acceptance are separate.


## Build-owned native bindings

Owning migrations: **BUILD-OWNED-TARGET-SELECTION** and **BUILD-OWNED-NATIVE-BINDINGS** in `TASKS.md`, under [native binding](../../../../wiki/spec/build/foreign_bindings.md#build-owned-native-binding) and [ordinary provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

Each project binds the original `Writer` requirements directly to the same sixteen target/locator associations in `build.omg`. Locator lengths are inferred from ordinary fixed-byte values. The target-body place transports the exact qualified `Pending` through an ordinary private `PendingTransferCode` implementation selected by four Build rows. Public contracts, content issuers, lifetimes, access, C ABI and the 73/91/37 operation are unchanged. All twenty sample projects and their Source pins have matching main/build inputs; no target-prefixed native helper or `via` forwarding declaration remains in this finite cohort.

Optimized producer `4e45c0b749edca0e816a4c322b009c3af3a33ec5`, on published custody prerequisite `b6a1d6d678bae9a13753f002f7f875a60e02f448`, freshly checks ten positive pins and rejects ten twins for their original active nested read. All ten pins and all ten authored programs execute with actual native and Terminal exit 0 on macOS ARM64. The selected-body project also checks under all four hosted target profiles; those are Source checks, not four-host executions. Changing either match-join route marker independently produces native signal 5 and Terminal `Trap`, while the unchanged match program passes both Boolean inputs.

The exact unchanged C provider retains its earlier self-test and nine lifecycle-trap observations: source SHA `1230033a221c32131cc96e8acffed9da6c20150a338c65f39641a2eb3592693f`, library SHA `7a38c83e51f9e2f742b00faa2959be61ff8605f33f5c92b16570f3e9c79edb0c`. The new native/Terminal executions use that same library. Four authored Build rows do not replace other-host execution evidence. Evidence is retained in `build/nested-binding-source-migration-20261101/`; historical producers above keep their original identities. This closes this cohort's Source migration only; the parent migrations retain the remaining repository-wide Source and compiler deletion work.
