# slice_0110

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0847 | ELF/imports.rs:178-185 | REACHABLE | Second-loop import consistency skips relocations whose symbol handle fails `is_valid`, so a crafted `FinalImage` can evade the orphan-import error. |
| A0848 | ELF/file_assembly.rs:2217-2337 | CLOSED | Resolved-linkage derive and `validate_contents` run back-to-back in `plan_stage`; no input path sits between them to mutate `.rela.dyn` bytes. |
| A0849 | ff:3988-4112 | CLOSED | Production `ObjectArtifact` always retains `fragment_replay`; without replay, only six listed shapes are rejected before `Ok`. |
| A0850 | ff:18745 | REACHABLE | Several `source::admit` arms return `true` without comparing abstract operations to selected instructions (e.g. atomic events, constants, arithmetic family). |
| A0851 | ff:18988 | REACHABLE | `Return` / `ReturnUnit` with empty `cleanup_actions` admit without `control_flow::retained` when cleanup is empty. |
| A0852 | ff:18729 | REACHABLE | `PortWrite` uses `.any` on matching instructions while port publication requires exactly one span. |
| A0853 | ff:16982-16984 | REACHABLE | `runtime_footprint_retained` returns `true` when `runtime_indices()` is empty, without checking memory-access rows. |
| A0854 | ff:4717-4731 | CLOSED | Skipped descriptor hardware routes to `installed_interrupts::validate`, which runs in the same `source::admit` pass before the operation loop. |

## A0847

Digest item 17 (`area_02`). Opened `omega-rust/omega/compilation/backend/images/image-elf/src/imports.rs` at `178-185` (read); line numbers match this checkout.

Enclosing function: `canonical_referenced_imports` (`43-200`, read). The first loop builds import requests from `symbol_table.imports` with strict validity (`57-64`, read). The second loop (`178-197`, read) walks every relocation; when `symbols.is_valid(relocation.symbol_handle)` is false it `continue`s (`178-184`, read), so no orphan-import diagnostic runs for that relocation.

Search `canonical_referenced_imports` in `omega-rust`: **4** paths — definition plus **3** production call sites (read): `import_sections.rs:88`, `shared_component.rs:95`, `static_executable.rs:184` (tests in `imports.rs` are additional).

Callers reach this from ELF static/shared/dynamic import planning on a `FinalImage` already held in the ELF writer (read). The compiler builder downgrades invalid relocation symbol handles when copying from `ObjectPlan` (`image/src/lib.rs:15-16`, `builder.rs:97-114`, read), which limits but does not prove absence of a hand-built `FinalImage`.

**Input that reaches the branch:** A `FinalImage` passed into `canonical_referenced_imports` where an import row exists in the symbol table but the only relocation tying that import to linkage uses a symbol handle for which `is_valid` is false; the second loop skips it and returns `Ok` without the error at `192-195`.

**Verdict REACHABLE (read):** The skip is explicit; a non-builder `FinalImage` can satisfy the branch and avoid the orphan-import check.

## A0848

Digest item 18 (`area_02`). Opened `omega-rust/omega/compilation/backend/images/image-elf/src/file_assembly.rs` `2217-2337` and fingerprint block `2409-2413` (read); `GeneralRelocation` storage exists at `2109`, `3345-3347`, `3377-3378` (read).

Enclosing function: `resolved_procedure_linkage::candidates::validate_contents` (`2217-2336`, read). Length `require` at `2227-2251` compares text, data, procedure linkage, got, and procedure relocation lengths (read) but **not** `general_relocation_bytes` (read). The `validate_unchanged_bytes` loop at `2323-2335` lists four storages and omits `ElfAppliedProcedureLinkageStorage::GeneralRelocation` (read). Fingerprint at `2409-2413` hashes procedure-linkage-related bytes and applications, not `general_relocation_bytes` (read).

`apply_elf_procedure_linkage_fixups` uses `checked::plan_stage!` with `derive_contents` and validation via `plan_candidate` (`3543-3553`, `dynamic_executable.rs:428-450`, read): derived contents are validated immediately in one call; no separate consumer mutates bytes between derive and `validate_contents`.

Search `apply_elf_procedure_linkage_fixups` in `omega-rust`: **2** matches (definition + tests) (read). Upward: dynamic ELF file assembly chain holds prior sealed envelopes before this stage (inferred from `ValidatedElfDynamicFileEnvelope` typing, not fully read).

**Verdict CLOSED (read):** Every production path applies fixups through `plan_stage`, so `validate_contents` always runs on the same derived snapshot; the digest’s “not reachable in normal flow” matches the `plan_candidate` coupling.

## A0849

Digest item 19 (`area_02`). `ff` = `omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs`. Opened `replay::validate` at `3988-4112` (read).

When `artifact.fragment_replay` is `None`, the function returns `Err` only if one of six artifact-shape checks fails (`3991-4105`, read); otherwise it returns `Ok(())` without calling `validate_function_fragment_object_artifact` (`4107-4109`, read). When `fragment_replay` is `Some`, full replay validation runs (`4107-4109`, read).

Search `fragment_replay: Some` / `fragment_replay: None` in `omega-rust`: **one** construction sets `fragment_replay: Some(...)` at `3827` (read); **no** `fragment_replay: None` assignment found (read). `replay::validate` is called from `image-emission/src/image.rs:224` and `:1894` (read) on emitted artifacts.

**Verdict CLOSED (read):** Shipped object emission always retains replay evidence; replay-less acceptance applies only to artifacts lacking those six replay-mandatory shapes, not to the normal emission path.

## A0850

Digest item 20 (`area_02`). Opened `source::admit` operation match at `18735-18746`, `18864-18865`, `18995-19007` (read); enclosing function `source::admit` starts `18534` (read).

`AbstractOperation::AtomicEvent` for listed events returns `true` without inspecting `selected` (`18735-18746`, read). `IntegerConstant` / `BooleanConstant` and the comparison/arithmetic/shift/cast family at `18864-18865` and `18995-19007` likewise return `true` without selected comparison (read). Non-admitted operations fail at `19026-19031` with `UnaccountedOperation` (read).

Search `source::admit` in `omega-rust`: **1** production call site at `24432` inside `validate_function_fragment_object_artifact` (read). `attribution::validate` (`294-457`, read) checks that attributed byte intervals cover operations **on instructions that exist**, but does not re-derive correctness of lowering for ops admitted with `true`.

**Input that reaches the branch:** A `StagedOptimizedRelocationFreeObjectContainer` whose abstract function lists an atomic load/store (or integer compare, etc.) while the selected plan’s instructions for that `psi_operation` are wrong or duplicated; the admit arm still returns `true`.

**Verdict REACHABLE (read):** The cited arms do not consult `selected`; wrong machine lowering can pass `source::admit` if the container reaches `24432`.

## A0851

Digest item 21 (`area_02`). Opened `18986-19007` (read).

For `AbstractOperation::Return` and `ReturnUnit`, admission is `cleanup_actions.is_empty() || control_flow::retained(...)` (and `read_result_cleanup_actions_match` for `ReturnUnit` only) (`18986-19094`, read). When `cleanup_actions` is empty, the first disjunct is `true` without calling `control_flow::retained` (`10314+`, read).

Search `ReturnUnit` / `Return {` in this match: only this admit site (read). Caller path: same `source::admit` at `24432` (read).

**Input that reaches the branch:** An abstract return with empty cleanup roster while the target control graph’s return terminator for that edge does not match the abstract return; empty cleanup short-circuits to admitted.

**Verdict REACHABLE (read):** Empty cleanup bypasses `control_flow::retained` entirely.

## A0852

Digest item 22 (`area_02`). Opened `18728-18732` (read).

`PortWrite` admission uses `selected.instructions().any(|instruction| ...)` (`18729-18732`, read). In `ports::produce`, the same abstract port write requires exactly one selected span and errors on two (`3302-3314`, read).

Search `PortWrite` in `function_fragments.rs` admit match: **1** site (read). Search `AbstractOperation::PortWrite` in produce module: **1** (read).

**Input that reaches the branch:** A function whose abstract plan includes `PortWrite` and the selected function has **two or more** instructions matching port/value/`psi_operation`; admit passes if **any** matches, including an incorrect duplicate.

**Verdict REACHABLE (read):** `.any` is strictly weaker than produce’s exactly-one rule.

## A0853

Digest item 23 (`area_02`). Opened `structural_fields::runtime_footprint_retained` at `16937-16985` (read); admit use at `18787-18789` for `StructuralByteSequenceFieldLength` (read).

When `operation.runtime_indices()` yields no elements, the function returns `true` at `16982-16983` before comparing `rows` to obligations (`16985+`, read). Other call sites: only `18789` (grep `runtime_footprint_retained`, read).

**Input that reaches the branch:** A byte-sequence length operation with no runtime indices and missing or incorrect `SelectedMemoryAccess` rows for that `psi_operation`; footprint check passes if `structural_fields::retained` already returned true.

**Verdict REACHABLE (read):** Non-indexed operations skip memory-footprint verification at this helper.

## A0854

Digest item 24 (`area_02`). Opened `dedicated_descriptor_owner` at `4717-4730` and `DescriptorReplay::new` at `4456-4457` (read).

When `dedicated` is false and `operation` is in `installations`, `dedicated_descriptor_owner` returns `Ok(false)` and hardware collection `continue`s (`4724-4725`, `4456-4457`, read). `installed_interrupts::validate` runs at `18597-18604` in the same `source::admit` before the per-operation admit loop (`18598+`, read); definition at `8898` (read header and install loop start only).

Search `dedicated_descriptor_owner` in `omega-rust`: **2** hits (definition + use in `DescriptorReplay::new`) (read). Search `installed_interrupts::validate` in `function_fragments.rs`: definition `8898`, call `18597` (read).

**Verdict CLOSED (read):** Installation-owned descriptor hardware is excluded from `DescriptorReplay` hardware list because `installed_interrupts::validate` is required in the same admit pass; skip is ownership routing, not omission of validation.

## What I did not read

- Full body of `installed_interrupts::validate` beyond `8898-8960`.
- `resolved_procedure_linkage::candidates::validate_candidate` seal macro expansion (inferred via `dynamic_executable.rs:365-384` and `seal_stage!` at `2206-2214`).
- Upstream producers of `FinalImage` and `StagedOptimizedRelocationFreeObjectContainer` beyond cited call sites.
- `native-artifact` consumption paths for artifacts without `fragment_replay`.
- Attribution replay beyond `attribution::validate` (`294-457`) for constant/atomic operations without instruction spans.

SLICE COMPLETE
