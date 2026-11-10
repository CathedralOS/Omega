# slice_0111

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0855 | omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments.rs:20219 | REACHABLE | `cleanup_actions_match` requires every listed discard to be valid but not one discard per structural read-byte result. |
| A0856 | omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs:3628 | REACHABLE | `_ => true` accepts `AdmittedProvider` execution paired with arbitrary non-hosted realizations before custody replay. |
| A0857 | omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs:1170 | REACHABLE | Most abstract body operations `continue` without the RAM-preserving call check. |
| A0858 | omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs:1673 | REACHABLE | Missing `descriptor_input` on the foreign-call record returns `Some(())` and passes custody derivation. |
| A0859 | omega-rust/omega/compilation/backend/images/image-emission/src/object_artifact.rs:2594 | REACHABLE | Parameter-style argument sources take the whole call instruction span with no per-argument transport proof. |
| A0860 | omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs:328 | REACHABLE | Many `MachineAlternativeFamily` variants hit `_ => {}` and skip extra state accounting beyond decode. |
| A0861 | omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs:3891 | REACHABLE | A free-Unit artifact on a target with no `unit_entry_form` returns `Ok(None)` and emits without an entry shim. |
| A0862 | omega-rust/omega/compilation/backend/images/image-emission/src/entries.rs:5575 | REACHABLE | Duplicate entry identities merge `None` with `Some` for plan, assumption, and scope without conflict. |

## A0855

**Cited site (re-located; digest `ff` = `image-emission/src/function_fragments.rs`).** Read `cleanup_actions_match` at `function_fragments.rs:20219-20277` (`read`). It collects structural `BoundaryCall` results (`:20233-20245`), then returns true only when `!actions.is_empty()` and `actions.iter().all(...)` (`:20247-20248`). Each action must match one unread result place (`:20253-20271`); there is no check that every collected result has a matching discard. Tests assert success for strict subsets and reorderings (`:20644-20649`, `read`).

**Callers.** Grep `cleanup_actions_match|read_result_cleanup_actions_match` in `omega-rust`: **9** matches, all in `function_fragments.rs` (`read`, count mode). One production use: `read_result_cleanup_actions_match` at `:18994`, re-exported from `byte_input::cleanup_actions_match` at `:20965`.

**Upward trace.** `source::admit` (`:18534`) loops abstract operations; `ReturnUnit` is admitted when `cleanup_actions.is_empty() || control_flow::retained(...) || read_result_cleanup_actions_match(...)` (`:18990-18994`, `read`). Failure on a non-matching operation returns `Error::UnaccountedOperation` (`:19026-19031`). `admit` is invoked from `replay::validate` (`:24432`, `read`).

**Input that reaches the branch.** A fragment whose abstract function is `Unit`, has two hosted read-byte structural results, and whose `ReturnUnit` carries a non-empty `cleanup_actions` list with only one `DiscardRoot` for one of those places.

**Wrong program (plain words).** A unit-returning function performs two hosted byte reads that each need affine root discard on return, but the recorded cleanup actions discard only one of the two results; fragment replay still admits the `ReturnUnit`.

## A0856

**Cited site (re-located from digest `EM/object_artifact.rs` to `image-emission/src/object_artifact.rs`).** Read `validate_completion_custody` at `object_artifact.rs:3573-3643` (`read`). `execution_realization_pair_is_exact` matches hosted pairs explicitly (`:3600-3621`); `CompilerBuiltin` vs wrong realization is false (`:3622-3627`); **`_ => true` at `:3628`**.

**Callers.** Grep `validate_completion_custody` in `omega-rust`: **11** matches — one production call in `function_fragments.rs:23027` inside structural settlement replay; remainder are tests in `object_artifact.rs` (`read`, count mode).

**Upward trace.** Replay failure surfaces as `Error::Mismatch("completion receipts")` (`:23027-23028`). `derive_completion_provider_custody` (`machine-code/src/boundary.rs:51-72`, `read`) does not consult `realization`, only `execution` and receipt bindings; stored custody is compared at `object_artifact.rs:3633-3640`.

**Wrong program (plain words).** A structural boundary settlement row records `AdmittedProvider` execution with a hosted-style `realization` variant that is not one of the three whitelisted hosted pairings, while completion provider custody bytes were recorded consistently with the provider execution id; the pairing check passes at `:3628` and custody equality may still succeed.

## A0857

**Cited site.** Read `ram::validate` at `object_artifact.rs:1089-1188` (`read`). For each abstract operation, `EstablishAccessReference` errors (`:1145-1148`); `BoundaryCall` and `ExternalCompletion` may reach the call uniqueness check (`:1150-1185`); **all other operation kinds hit `_ => continue` at `:1170`** and skip call verification.

**Callers.** Grep `ram::validate` in `omega-rust`: **1** match, call at `object_artifact.rs:1250` inside `derive` for installed body fault premises (`:1192-1259`, `read`).

**Upward trace.** `derive` returns `Err` on `ram::validate` failure; it runs only when the artifact has fatal installed entries, empty foreign/private deps, and fragment replay (`:1198-1214`, `read`). `derive` is used from installed-body fault analysis paths (`read` from caller context at `:1250`).

**Wrong program (plain words).** An installed RAM-closure abstract body includes a non-boundary abstract operation (for example an internal `Call`) that lowers to a concrete call targeting a machine outside the admitted closure roster; `ram::validate` never inspects that operation because of `continue` at `:1170`.

## A0868

Wait - I need to fix - row is A0858 not A0868

## A0858

**Cited site.** Read `descriptor_input::validate` at `object_artifact.rs:1668-1729` (`read`). **`let Some(guard) = record.call.descriptor_input else { return Some(()); }` at `:1673-1675`.**

**Callers.** Grep `descriptor_input::validate` in `omega-rust`: **1** match at `object_artifact.rs:1969` (`read`, count mode).

**Upward trace.** `derive_normalized_foreign_call_custody` (`:1916`) iterates normalized foreign calls; `.ok_or_else(|| invalid("descriptor input guard does not match the selected call"))` at `:1969-1971` treats `None` from validate as success path only when validate returns `Some(())`. Selection pipeline emits no guard when `call.descriptor_input` is absent (`selection_scalar_graph.rs:4829-4830`, `read`).

**Wrong program (plain words).** A descriptor-consuming foreign import should carry a count guard, but the normalized foreign-call record omits `descriptor_input`; validation accepts the call and custody derivation proceeds.

## A0859

**Cited site.** Read `foreign_scalar_argument_custody` at `object_artifact.rs:2486-2691` (`read`). For `Parameter`, `BlockParameter`, `BooleanImmediate`, and `IeeeFloatImmediate`, the span is the **entire call instruction** via `fragment_instruction_span` (`:2594-2608`, `read`). `IntegerImmediate` and `Home` use `scalar_immediate_span` with transport checks (`:2610-2667`, `read`).

**Callers.** Grep `foreign_scalar_argument_custody` in `omega-rust`: **2** matches — definition `:2486`, call `:2040` inside `derive_normalized_foreign_call_custody` (`read`, count mode).

**Upward trace.** Derived scalar argument records feed `ObjectForeignCall` custody compared on artifact validation (`:2040-2048`, `read`).

**Wrong program (plain words).** A foreign call’s parameter argument is not actually emitted in the call instruction’s span, but the record uses a parameter source variant; custody records the whole call span as transport evidence without per-argument verification.

## A0860

**Cited site.** Read `body_footprint::collect_from` in `entries.rs:89-337` (`read`). After `footprint.decoded(model, encoded)?` (`:248`), `match row.alternative.family` handles segment, control-register, and MSR families (`:249-327`); **`_ => {}` at `:328`.**

**Families (read).** `MachineAlternativeFamily` in `selected-instructions/src/effects/catalog.rs:757-834` lists many variants (for example `PortWrite`, `SystemRegisterRead`, `AtomicAccess`, `HostedReadByte`) beyond those in the privileged arms (`read`).

**Callers.** Grep `body_footprint::collect` / `collect_from` in `image-emission`: calls from `observe_native_body_cpu_state` at `entries.rs:854-865` (`read`, inferred from earlier grep).

**Wrong program (plain words).** An interrupt-body instruction uses an alternative family that needs explicit control or segment state beyond decoded encoding (for example port I/O), but falls through `:328` so footprint collection never adds the extra machine-state union applied to privileged families.

## A0861

**Cited site.** Read `hosted_unit_entry::prepare` at `entries.rs:3880-3897` (`read`). After physical and hosted-receiver branches, **`if !has_free_unit_entry(artifact)? { return Ok(None); }` (`:3891-3892`)**; **`let Some(form) = unit_entry_form(artifact.target) else { return Ok(None); }` (`:3894-3896`)**. `unit_entry_form` returns `None` for targets outside the five named hosted targets (`:3875-3876`, `read`).

**Callers.** Grep `hosted_unit_entry::prepare` in `omega-rust`: **8** matches, all in `image.rs` (`read`, count mode). `prepared_parts` maps `None` to `entry_shim: None` (`:3790-3807`, `read`).

**Wrong program (plain words).** A free-Unit entry artifact targets a `NativeTarget` with no unit entry adapter form; image emission proceeds with the artifact’s raw text and no entry shim.

## A0862

**Cited site.** Read `EntryOrigins::new` at `entries.rs:5557-5601` (`read`). On duplicate `MachineFunctionIdentity`, conflict is detected for fields 1–2 always (`:5573-5574`); for optional fields 3–5 **only when both sides are `Some` and differ** (`:5575-5577`); otherwise `None` is filled from the later row (`:5584-5595`).

**Callers.** Grep `EntryOrigins::new` in `image-emission`: **2** production calls — `object.rs:900`, `object_artifact.rs:188` (`read`, count mode).

**Wrong program (plain words).** Two composed artifacts contribute duplicate private-entry identities where one row lacks an interrupt entry plan (`None`) and another supplies a concrete plan (`Some`); merge accepts the combined row without rejecting the missing-vs-present disagreement.

## What I did not read

- Full bodies of `completion_receipts_have_exact_custody`, `scalar_immediate_span`, and `owned_stack::prepare`.
- Whether pipeline stages always populate `descriptor_input` when a descriptor import requires it (`pipeline/02` and legalization paths beyond the `emit` early return).
- Exhaustive proof that non-boundary abstract operations can appear in installed RAM closures subject to `ram::validate`.
- All call sites of `derive` beyond the `ram::validate` invocation.
- PE/Mach-O image paths beyond `image.rs` hosted-unit entry preparation.
- Test modules in `object_artifact.rs` for completion custody beyond cited production line.

SLICE COMPLETE
