# Outputs and drivers triage receipts

Packet complete. Each claim in the [outputs-and-drivers boundary note](../../boundaries/outputs_and_drivers.md) has a receipt below. None shows a compiler accepting a program it should refuse or publishing different bytes than it produced. One claim reproduces from source alone: when the Omega-written typer's type table is full, four callers keep the absent type 0 without rejecting (H17.2c). It stays open because its fixture runs through the `omega` binary, which this review did not build. Several survey statements are stale: publication reads back its bytes (H8), and the compiled-review cache and the historical CLI comparison’s stream gap were removed or fixed. This packet changes no code.

Evidence baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. The source was read at the branch head `a0152d26`. Between the two, only `tools/xtask/src/samples.rs` among the cited files changed: [#5407](https://github.com/CathedralOS/Omega/pull/5407) rewrote the review-decision editor and added the samples ledger. Line numbers in `tools/xtask/src/samples.rs` are therefore at `a0152d26`. The survey read `4f3c255d0559`. Path shorthands follow the boundary note: `app/`, `cmp/`, `build/`, `pm/`, `int/`, `drv/`, `S2P/`, `con/`. A claim is named by handoff, item and bullet: H3.3b is the second bullet of item 3 ("What is checked") of H3. Joined ledgers: [CLI](../01-cli-foundation.md) (cited as 01:row), [Build and packages](../06-15-build-packages.md) (06-15:row), [interpreter](../14-interpreter.md), [tools](../16-tools.md), [samples and sweep](../16-samples-and-sweep.md), [Omega source](../17-omega-source.md).

Outcomes: **Fixed** (a merged change removed the problem), **Intended** (code and spec or comment agree; includes record inaccuracies), **Duplicate** (another ledger row owns it), **Feature gap** (stated missing capability), **Reproduced** (the defect is shown), **Still open**.

## H1. CLI to the package manager

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H1.1, one lease per target, one `open_products` call, `plans.select` per target. | `app/commands/compile.rs:122-170` | Present as described. | Intended |
| H1.1, `omega run` always passes `offline = false`. | `app/execution/compilation.rs:139-145`; `app/arguments/run.rs:69`, `:171-172` | The run parser refuses `--offline` with "omega run does not support --offline", so `false` is the only reachable value. | Intended |
| H1.2a, `.expect("an acquired target lease opens a Product")`. | `app/commands/compile.rs:157-170` | `active` holds exactly the profiles whose lease succeeded, and `open_products` runs whenever it is nonempty. Each row applies `lease.as_ref().map_err(output_error)?` before the `expect`, so the `expect` runs only when `plans` is `Some`. `plans.select(profile)?` returns an error for a profile without a plan. | Intended |
| H1.2b, target names resolved three ways (digest 01 §3). | — | 01:104–115 maps the flag and target questions to numbered rows. | Duplicate (01:104–115) |
| H1.3, `--verbose` stripped anywhere in `argv`; value readers differ from their header comment. | `app/arguments/mod.rs` | 01:243 repaired option-value consumption; 01:104–115 says the source comment now describes each reader. | Duplicate (01:243, 01:104–115) |
| H1.4, host lock through `OMEGA_HOST_PACKAGE_LOCK` and its SHA-256 variable. | `app/saved_program.rs:17-31` | 01:189: `ReviewedHostPackages::open` requires a 64-digit SHA-256 before using the lock. | Duplicate (01:189) |

## H2. Package manager to Build

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H2.1, recovery, baseline, per-target lock sections, root capture, Git pins, `open_products` sequence, `SourceFileSystem` handoff. | `pm/dependencies/prepare_project.rs:298-501`; `pm/project/mod.rs:487-578` | Present as described. | Intended |
| H2.2a, `SourceFileSystem::route` falls back to the live disk; whether the compiler refuses earlier "was not traced". | `pm/source-acquisition/capture/filesystem.rs:175-215`; `build/inputs/package.rs:205-206`; `build/inputs/source/source_imports/files.rs:239-244`; `build/inputs/source/source_imports/frontier.rs:550-560` | Traced on the project route. Prepared Build inputs refuse a package with no capture ("prepared Build source lost its immutable capture"), and `package/construction.rs:189-201` mounts every package capture. A logical file import outside every capture is refused ("logical file import requires an admitted capture"), and `captured_files` refuses discovery on a physical path. A closed contract root outside a capture is captured before use. The physical branch therefore serves paths under no package root, such as a no-capture filesystem in tests. This matches [scoped execution](../../../../../wiki/spec/build/scoped_execution.md) ("Source capture happens before authored build execution"). | Intended |
| H2.2b, runtime consent only with an accepted section. | `pm/project/mod.rs:557-573`; `cmp/compiler.rs:313-317` | Without a lock the compiler requires no component assumptions; see [review](../../../../../wiki/spec/packages/review.md#when-review-runs) ("they create no acceptance and supply no accepted bindings"). | Intended |
| H2.2c, one Product shared across profiles without dependency sources or accepted sections. | `pm/project/mod.rs:530-541` | 06-15 row for 15:177/180 traces the sharing condition and later per-deployment selection. | Duplicate (06-15, 15:180) |
| H2.3a–d, file-pair recheck, root-request check, compiler identity check, `require_current_acceptance` comparisons. | `pm/dependencies/prepare_project.rs:389-406`, `:462-471`; `pm/dependencies/accepted_compilation_identity.rs:20-110` | Present as described. That the stored side used the same constructor stays inferred, as the note says; the lock reader recovers through the same subject constructor (06-15 row for 15:181). | Intended |
| H2.3e, execution profile `None == None` passes. | `pm/dependencies/accepted_compilation_identity.rs:36-37`; `pm/project/mod.rs:73`, `:459` | The project entry requires `host_if_supported()` before Build or `accepted_deployment` runs, so `None` cannot reach the comparison on this route. | Duplicate (06-15, 15:177) |
| H2.3f, activation locking only on the preparation path. | `pm/dependencies/source/acquisition.rs:86-106`; `pm/dependencies/prepare_project.rs:497-499` | The [review contract](../../../../../wiki/spec/packages/review.md#when-review-runs) puts full review in explicit review, acceptance and update workflows, which must run unreviewed Builds. Ordinary compilation holds every activation to the lock. | Intended |
| H2.4, editing reads `build.omg` by syntax and by tokens; live-disk path beside captures. | `pm/` edit path (digest 15 §1, §3) | Two readers of one file with a reconciliation step. No differing result is recorded. The live-disk half is H2.2a. | Intended |

## H3. Build program to the interpreter

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H3.1, three call-site tables, entry precondition check, minted Build value, `retain`. | `build/evaluation/activation.rs:226-258`, `:405-430` | Present as described. | Intended |
| H3.2a–c, no trust witness and no proof check on interpreter loading; Build program not authorized. | `int/README.md:28-31`, `:68-76`; `int/embedding/program.rs:3259-3278` | The interpreter README states that byte-loading APIs "do not take that trust witness yet" and links the [claim-admission plan](../../../../../wiki/drafts/designs/psi_claim_admission.md). The Build program comes from the same in-process compiler. | Feature gap (tracked in `psi_claim_admission.md`) |
| H3.3a, host adapters authenticate rows, receivers, tickets and markers. | digest 06 §2 | Descriptive. | Intended |
| H3.3b, `commit_boundary_result` accepts seven pairings. | digest 14 §4 | Descriptive. | Intended |
| H3.3c, default `poll_*` handlers return `Ok(true)`. | digest 14 §10 item 8 | The interpreter ledger: a readiness default alone does not accept an invalid result; any defect needs a failing completion case. | Duplicate (14, section 5 item 1) |
| H3.3d, resume checks the stopped call, rejoins the stack, compares requests. | `build/evaluation/activation.rs:337-356` | Present as described. | Intended |
| H3.3e, the `(Some(a), Some(b)) => a == b` arm applies no span remap to request and filesystem actions. | `build/evaluation/activation/protocol.rs:13-18`, `:20-44`, `:252`; `S2P` `07_checked-program-to-psi/src/build_bindings.rs:224-238`, `:362-372` | That arm compares only `Action::Request(TerminalBuildRequestOperation)` and `Action::Filesystem(TerminalBuildFilesystemOperation)`. Both are field-less enums, so they carry no span to remap. Binding and query actions, which do carry spans, take the remapping arms above it. | Intended (record inaccuracy) |
| H3.3f, `continue_checked_entry` drops the successor's call observations. | — | The interpreter ledger: checked continuation now refuses a call observer or issued layout target on either frame. | Fixed (14, section 5 item 8) |
| H3.4, side tables by artifact custody; resumption keyed by source spans. | `build/evaluation/activation.rs:411-430`; `protocol.rs` `Sources::rejoin` | A span that does not rejoin clears `accounted` and the resumption is refused. | Intended |

## H4. Returned Build value to settings

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H4.1, six named fields and `ComputedBuildConfig`. | `build/settings/settings.rs:30-80`; `build/evaluation/result/configuration.rs:51-76` | Present as described. | Intended |
| H4.2a, fields outside the six are skipped. | `build/settings/settings.rs:51-61`; `build/inputs/declarations/prelude/declarations/builder.omg:38-56` | The other `Build` fields are `seal`, `source`, `inputs`, `output`, `log` and `product`. They are host capability handles that reach Build through host calls, not settings values. | Intended |
| H4.2b, missing `identifier`, `elf_interpreter` and `tests` get defaults. | `build/inputs/build_execution/build_value/configuration.rs:90-131`; `build/settings/settings.rs:67-72` | The settings route refuses a missing declared field ("Build backing has no admitted `{name}` field"), and the prelude declares all three. The defaults apply only to a value that never carried the row, as the comment at `configuration.rs:116-117` says. | Intended |
| H4.2c, `ComputedBuildConfig` equality ignores `accepted_builds`. | `build/evaluation/result/configuration.rs:78-115` | The comment says retaining a judgment changes custody, not the output, and that an `Arc` address must not become an identity. `execution_history` is compared, and it carries the program fingerprints. | Intended |
| H4.2d, `ends_with` accepts a history with no `Resumed` step. | — | 06-15 row for 06:239: expected single-stage behavior, with fixtures. | Duplicate (06-15, 06:239) |
| H4.3a–b, ambiguous, missing and erased fields refused; `complete` downgrades to `CheckedAvailableDeclarations`. | `build/settings/settings.rs:67-72`, `:101-103`; `build/evaluation/build_evaluation.rs:289-295`; `build/evaluation/result/result.rs:22-31` | The variant's doc comment: "The Build returned through a checked arm. Other arms still need an unavailable dependency." `build/tests/planning_stage.rs:386` exercises it. | Intended |
| H4.3c, H4.4, checks are Build code; labels selected by spelling. | `build/settings/settings.rs:64` | Labels are the canonical descriptor identities of the toolchain-declared fields of one structural type, and the type is matched by `receiver_for` first. | Intended |

## H5. Compile-time calls

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H5.1, H5.2a, H5.3a, bind/execute, minted inputs, receipt, shape and layout-target checks. | `build/evaluation/invocations/invocation.rs:126-318` | Present as described. | Intended |
| H5.2b, `scalar_value` has no range check for narrow integers. | — | Landed in [#5273](https://github.com/CathedralOS/Omega/pull/5273). | Fixed (06-15, 06:240) |
| H5.3b, the interpreter recognises the layout target by `"toolchain::LayoutTarget"`. | `int/embedding/program.rs:3570-3597`; `cmp/psi/semantics/src/calls/layout_target.rs:93` | Besides the text, the parameter must be owned, its type opaque, and the input of the same structural type and not repeated. Whether an authored package can declare a type whose canonical identity text is `toolchain::LayoutTarget` was not traced. | Still open |
| H5.3c, per-target constants compared by canonical encoding. | digest 06 §2 | Descriptive. | Intended |
| H5.3d, `validate_constant_evaluations` returns early. | `S2P` `driver/compile_time_invocations/constants.rs:169-246` | It returns early only when no constant has a computation, no receipt names a constant and no constant has an evaluation entry or expression, so nothing remains to validate. Every receipt is checked against its owner and requester before that test. | Intended |
| H5.4, four `BuildTimeValue` decoders. | digest 06 §3 line 40 | Duplicate implementations. Following the CLI ledger rule (01, records rows 130–148), consolidation needs a failing contract, and none is recorded. | Intended |

## H6. Build to the compilers

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H6.1, `product_stage`, `source`, `program`, `native_target`, default `main` binding with `owner::slot`. | `build/planning/product.rs:37-251` | Present as described. | Intended |
| H6.2a, the compiler takes executed Build records as given. | `build/planning/product.rs:135-172` | Build and the compiler are one trust domain; the lowered-form checker judges the Product. | Intended |
| H6.2b, consent `None` requires no component assumptions. | `cmp/compiler.rs:313-317` | Same contract as H2.2b. | Intended |
| H6.2c, host target found by canonical name. | `build/planning/product.rs:49-54` | An unknown name returns an error. | Intended |
| H6.3a, executed events joined to requests by `(source_id, start, end)` in four places. | digest 06 §3 line 31 | Duplicate implementations; no differing join result recorded. Same rule as H5.4. | Intended |
| H6.3b, slot string parsed back with `rsplit_once("::")` in three places. | `cmp/psi-to-native/targets/target/src/target_profile.rs:183`; `cmp/psi-to-native/pipeline/01_.../program_entry/published.rs:149`; `cmp/psi-to-native/representations/target-bound-program/entry_settlement.rs:201` | The producer joins an owner path and a slot identifier, which contains no `::`, so the last-separator split recovers both parts. | Intended |
| H6.3c, nothing independent checks the Build records. | — | Descriptive of the trust domain. | Intended |
| H6.4, cloned parsed source; `ComponentSuppliers::publish` callback with `HostProducerTrust::accept`. | `build/planning/product.rs:139`, `:254-290`; `cmp/compiler.rs:298` | 01:200: the trust value is a process-local capability for a product of the current invocation. | Duplicate (01:200) |

## H7. Compiler entry and lock consent

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H7.1, `compile`, `prepare`, `CompileOutput`. | `cmp/compiler.rs:156-322` | Present as described. | Intended |
| H7.2a–c, unconditional current-invocation trust; validators only on request. | `cmp/compiler.rs:103-109`; `cmp/psi/artifact/authorization/authorization.rs:133-142` | 01:200. Default compilation does not re-verify its own stage output (AGENTS.md, "Verify at trust boundaries"). | Duplicate (01:200) |
| H7.2d, advisories discarded with `advisories: None`. | — | 01:248. | Duplicate (01:248) |
| H7.3a–c, requested validators; `runtime_consent::compare`; stored-versus-fresh comparison. | `cmp/compiler.rs:215-218`; `cmp/runtime_consent.rs:29-99` | 01:201 traces the permission filter. | Duplicate (01:201) |
| H7.3d, permissions with no classes and no binding skipped. | `cmp/runtime_consent.rs:73-75` | 01:201: a nonempty permission without a unique owner is an error. | Duplicate (01:201) |
| H7.3e, `verify_component` against schema V3 only. | `cmp/compiler.rs:55-68` | 01, records rows 153–165: foreign package review checks its supported schema. | Duplicate (01:153–165) |
| H7.4, two closures from the compiler entry. | `cmp/compiler.rs:298-318` | Descriptive. | Intended |

## H8. Products to the CLI and files

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H8.1, H8.2, `publish`, `publish_psi`, `save_shared_psi`, printed lines, variant check, `portable[0]` identity. | `build/publication/publication.rs:28-48`; `build/publication/output.rs:532-594`; `app/commands/compile.rs:283-294`, `:429-466` | Present as described. `save_shared_psi` requires byte equality or a shared `Arc` before one identity is printed. | Intended |
| H8.3a–d, metadata check, second-publication block, extension and 64 MiB cap, receipts after staging. | `build/publication/output.rs:255-257`, `:538-580` | Present as described. | Intended |
| H8.3e, "No code seen here reads the written file back". | `foundation/filesystem/src/file_publication.rs:45`, `:63`; `foundation/filesystem/src/record_file/stage.rs:6-8`, `:53-66`; `foundation/filesystem/src/record_file.rs:157-188` | `publish_psi` stages with `stage_exact_bytes` and installs with `install_staged_files`, which read the staged and installed bytes. The `--emit-psi` route writes through `RecordFileRoot`; its stage replays the written bytes from offset 0, and the installed hard link is checked as the same regular file. A staging-name collision was fixed in [#5270](https://github.com/CathedralOS/Omega/pull/5270). | Fixed (stale; repair queue README; 01:220) |

## H9. Saved `.psi` back into the CLI

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H9.1, H9.2a, H9.3a–b, 64 MiB read, canonical decode, identity pin, trust. | `app/saved_program/input.rs:4-46`; `cmp/psi/artifact/codec/src/canonical_artifact.rs:114-316` | 01:188: the pin covers every canonical section. | Duplicate (01:188) |
| H9.2b, producer exclusions, boundaries and settings replaced with defaults. | `app/saved_program.rs:146-163` | 01:247: Build settings are not serialized as receiving-host authorizations. | Duplicate (01:247) |
| H9.2c, non-UTF-8 lock digest becomes an empty string. | `app/saved_program.rs:23` | 01:189. | Duplicate (01:189) |
| H9.3c, structural checks only; no proof check. | `int/README.md:28-31` | Same gap as H3.2. | Feature gap (tracked in `psi_claim_admission.md`) |
| H9.3d, `omega run` refuses an untrusted `.psi`. | `app/execution/compilation.rs:129-136` | Present ("execution of a saved Psi program requires producer trust"). | Intended |
| H9.4, host packages from the environment-named lock. | `app/saved_program.rs:17-31` | 01:189. | Duplicate (01:189) |

## H10. Published Psi to the interpreter

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H10.1, request, `execute`, `prepare`, `execute_installed`. | `app/execution/compilation.rs:196-237`; `int/host/retained-image/execution.rs:23-48`; `int/host/src/execution.rs:185-225` | Present as described. | Intended |
| H10.2a–b, plain `CanonicalTerminalArtifact`; no proof check. | `int/host/src/execution.rs:233-234` | Same gap as H3.2. | Feature gap (tracked in `psi_claim_admission.md`) |
| H10.2c, host filesystem, services and clock. | `app/execution/compilation.rs:211-234` | 01:190: the surveyed run requested them. | Duplicate (01:190) |
| H10.3a–c, entry, provision, target, provider and layout checks; image identity check. | `int/host/src/execution.rs:235-316`; `int/host/retained-image/execution.rs:67-74` | Present as described. | Intended |
| H10.3d, `execute_installed` skips the entry-machine check without provisions. | `int/host/src/execution/hosted_invocation.rs:31-41` | The check compares the provisions' entry with `program.entry_installed(&installation)`. Without provisions there is no second entry claim; the entry comes from the installation, and `establish_host_entry` then builds the arrival. | Intended |
| H10.4a, code layouts travel from the image into the interpreter. | `int/host/retained-image/execution.rs:75-93` | Descriptive. | Intended |
| H10.4b, `admit_runtime_supplies` runs twice on the image path. | `int/host/retained-image/execution.rs:38-47`; `int/host/src/execution.rs:199-215`; `int/embedding/program.rs:3305-3330` | Confirmed: `execute_sliced` admits supplies with the image layouts, and `execute_with_image_installation` admits again from the host request with those same layouts. The function takes `&self` and returns a fresh value, so the second call repeats work but cannot change the result. | Intended (redundant work) |

## H11. Native and Psi runs back to the CLI

| Claim | Code at triage revision | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H11.1, native output, RAM, frames and exit; Psi observation. | `app/execution/mod.rs` | Present. | Intended |
| H11.2a–b, only status and RAM compared; no stdout or stderr comparison. | `app/execution/mod.rs:75`, `:331-332`, `:558-559`, `:626-627` | [#5188](https://github.com/CathedralOS/Omega/pull/5188) compared stdout and stderr byte for byte once terminations agreed. | Fixed (#5188) |
| H11.3a, Unix low byte; declined when the Psi status did not fit. | `app/execution/process_exit.rs:56-67`; `app/execution/mod.rs:701-716` | 01:194–195. | Duplicate (01:194–195) |
| H11.3b, crash compared only for `CrashCause::Trap`. | `app/execution/process_exit.rs:69-104` | Other causes returned `None`, which declined the comparison. Every hosted target had a trap presentation. The Windows ARM64 value had yet to be observed on a host (comment at `:97-98`), tracked by `TASKS.md` **WINDOWS-ARM64-TARGET**. | Intended |
| H11.3c, entry RAM byte-equal. | `app/execution/mod.rs` | Present. | Intended |
| H11.3d, stdin replay only after a `"runtime-standard-input"` refusal. | `app/execution/mod.rs:50-57`, `:258-264` | The refusal is the typed preflight error `MissingProvision { capability }`, raised before execution. The module comment explains that reading stdin eagerly would hang programs that never read it. | Intended |
| H11.3e, both executions used the same artifact. | — | Descriptive limit of a differential check. | Intended |
| H11.4, the interpreted path returned the `ExternalCompletion` code unclamped. | `app/commands/run.rs` | 01:193. | Duplicate (01:193) |

## H12. Compiler review to the lock

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H12.1, `review_policy`, `compile_occurrence`, `retain_product`, publication. | `build/planning/package_review/mod.rs:28-93`; `pm/review/candidate/fresh_occurrence.rs:9-90` | Present. `retain_product` no longer writes a cache record: it calls `compile_occurrence` and `retain_occurrence`. | Intended |
| H12.2a, the package manager takes Build's policy projection as given. | — | Build and the package manager share the compiler-issued review. The structural validators in `review-evidence` check it (H12.3c). | Intended |
| H12.2b–c, compiled-review cache integrity is a self-hash; cache write errors swallowed. | `_recycling/source-rust/subsystems/package-manager/review/candidate/review_cache.rs` | The review cache and its record codec are archived; no active crate writes or reads it. | Fixed (retired code path; 06-15, 15:181/184/185/237 and 06:243–244) |
| H12.3a, `compile_occurrence` identity agreement. | `pm/review/candidate/fresh_occurrence.rs:46-63` | Present. | Intended |
| H12.3b, cache-hit field checks and `recover_canonical`. | — | Retired with the cache. | Fixed (retired code path) |
| H12.3c, `review-evidence` validators are separate code. | — | Descriptive. | Intended |
| H12.3d, schema digest compared only by `debug_assert_eq!`. | — | 06-15 row for 15:208: production callers establish equality before construction. | Duplicate (06-15, 15:208) |
| H12.3e, an empty plan target passes. | — | Landed in [#5251](https://github.com/CathedralOS/Omega/pull/5251). | Fixed (06-15, 15:205) |
| H12.4, `finish_reviews` copies the root review's service audits. | `pm/review/candidate/passes_occurrences.rs:598-635` | [Critical service audit policy](../../../../../wiki/spec/packages/review.md#critical-service-audit-policy): the consuming root sets the critical audit set, and "a dependency's own build cannot select the consumer's policy". Each copied review is re-encoded with a new commitment. | Intended |

## H13. The review file

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H13.1, review files, `review: <path>`, exit 3, fingerprint per decision. | `app/commands/packages.rs:154-195` | Present. | Intended |
| H13.2a, recovery trusts the authors to record decisions. | — | Spec-defined owner role (digest 15 §8). | Intended |
| H13.2b, `--dangerously-accept-all` rewrites pending decisions for up to four rounds. | `app/commands/packages.rs:40-188` | The flag names its effect. 01:187 fixed its CRLF handling and checks that recovery regenerates the template. | Duplicate (01:187) |
| H13.3, recovery validates structure, not reading. | — | Same as H13.2a. | Intended |
| H13.4, xtask and omega-bench rewrite pending decisions. | `tools/xtask/src/samples.rs:334-352` | Samples ledger row 112 (#5407): intended for positive samples. Root-role and source-replacement decisions stay pending, and the edit preserves every byte outside the row choice. | Duplicate (16-samples, 112) |

## H14–H20. Omega-written compiler

| Claim | Current code | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| Shared pattern: stage records with `accepted`, driver gates, spelling comparisons, same-package checks. | `drv/compiler.omg:82-187` | Descriptive. The span-spelling defect was fixed in #5271 (17 ledger, first row). | Intended |
| H14.1–H14.3, `TokenStream`, gated `parse`, lexer capacity rejections. | `S2P/representations/tokens/tokens.omg:160-215`; `S2P/pipeline/source_files_to_tokens/lexer.omg:389-411` | The note already answers the digest's "not traced". `parse` has one production caller, the gated driver. | Intended |
| H15.2a, `emit_expression` stores `literal: 0` in every node and the typer reads `expression.literal`. | `S2P/pipeline/tokens_to_syntax_trees/expression.omg:255-280`; `S2P/pipeline/symbol_resolved_trees_to_typed_trees/expressions.omg:125-131` | Integer literals call `take_integer_literal` and store its handle (`literal: literal_handle`); other kinds store 0. The typer reads `expression.literal` only in the integer branch. The survey commit has the same code (`expression.omg:308-327` there). | Intended (record inaccuracy) |
| H15.2b, H15.3, forms parsed then refused; `check_declaration_forms`. | `S2P/pipeline/syntax_trees_to_symbol_resolved_trees/resolver.omg:26-35` | Refusal of unsupported forms. | Intended |
| H16.2a, `type_check` does not test the incoming flag. | `drv/compiler.omg:151-163` | One gated production caller. | Intended |
| H16.2b, `*_binding_at` readers return any row below capacity, and `name_resolution_clear` does not zero the arrays, so stale data could be read. | `S2P/representations/symbol_resolved_trees/symbol_resolved_trees.omg:389-400`, `:668-722`; `S2P/pipeline/syntax_trees_to_symbol_resolved_trees/resolution_state.omg:15-24`; `drv/compiler.omg:147-148` | The driver declares a fresh `let mutable resolver` for each compilation. Its storage starts zeroed ([zero initialization](../../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)), and `initialize` clears it once. An unbound row therefore reads the zero binding of this compilation, never data left from an earlier one. The `_at` readers say they serve rows "bound or not yet". | Intended |
| H17.2a, checker gets no source set and starts accepted. | `S2P/pipeline/typed_trees_to_checked_trees/checker.omg:208-254` | Descriptive. | Intended |
| H17.2b, the path after the last transition arm is infeasible because typing proved coverage. | digest 17 §5 | Stated design. | Intended |
| H17.2c, `type_table_intern` returns the absent type 0 when the table is full. | `S2P/representations/typed_trees/typed_trees.omg:741-799`; callers `S2P/pipeline/symbol_resolved_trees_to_typed_trees/arrays.omg:20`, `:35`, `:65`, `joins.omg:113` | Reproduced from source. `typing_state::typer_intern` (`typing_state.omg:108-125`) rejects with `TypeCapacityExceeded` when the result is 0, and `references.omg:326-330` tests it. The four array and join callers call `type_table_intern` directly and do not test the result. `arrays.omg:65` writes the 0 as the expression's type while typing stays accepted. How the checker and lowerer treat type 0 was not traced. **Proposed fix:** route those four interns through `typer_intern` with the expression's span, and pin it with a fixture in `tests/typing.py` that fills the 8,192-row table before an array literal. Not repaired: the fixture runs through an optimized `omega` binary, which this review did not build. | Reproduced; still open |
| H17.2d, `check_holds` runs only for `Call`. | — | Selected binary operators now apply call argument roles and compare overlapping operands. | Fixed (17, line 162) |
| H18.2a, no run-time bounds checks; the checker owes them. | digest 17 §5, §9 | Stated design; no program traced. | Intended |
| H18.2b, `proves` claims a proof when 128 goals are pending. | — | [#5178](https://github.com/CathedralOS/Omega/pull/5178). | Fixed (17, goal-stack row) |
| H18.2c, locals past the 128th get `last_use` 0. | — | The checker now refuses with `CheckCapacityExceeded` (#5283). | Fixed (17, loan last-use row) |
| H18.3a–c, `scalar_lower` requires all four flags; refuses contracts; repeats the driver gate. | `S2P/pipeline/checked_trees_to_scalar_operations/scalar.omg:192-196` | The refusal of contract-bearing machines is a stated limit. | Feature gap |
| H19.2, operation kinds outside the emitter's list `crash Trap`. | `source/subsystems/compiler/alpha/emit.omg:114`, `:194` | A trap stops the compiler before any tape is published. | Intended (refusal) |
| H19.3, `alpha_emit` only for `Complete`; driver maps other statuses to `Unsupported`. | `alpha/emit.omg:382-391`; `drv/compiler.omg:170-180` | 17 ledger, line 176. | Duplicate (17, 176) |
| H20.1, exit statuses and 2 MiB publication limit. | `con/main.omg:340-352`, `:354` onward | [Construction README](../../../../../source/application/construction/README.md) lines 135–139 list the same codes, including "unsupported lowering or entry 246". | Intended |
| H20.2, packet producers not read; Alpha tape consumer not traced. | — | The note marks these as untraced. They are inventory gaps, not claims about behavior. | Still open (untraced inventory) |
| H20.3, version-2 packets set `has_build_sources`; `check_build_input` exits 246 or 253. | `con/build_sources.omg:182-196` | The README (lines 125–128) states this: a valid inventory exits 246 because the application cannot yet run Build files. | Feature gap |
| H20.4, the OCREQ decoder never returns a verdict. | `source/subsystems/compiler/request/protocol.omg:360-366`; `request/outcome.omg:216-221` | The decoder publishes the `CoverageRequestSemantics` provision because the semantic phases are not implemented. | Feature gap |

## H21. Repository tools reading CLI output

| Claim | Code at triage revision | Contract, fixture or trace | Outcome |
| --- | --- | --- | --- |
| H21.1a, sample-gate parsed `native exit ` and `interp exit: `. | `tools/xtask/src/samples.rs:794-810`; `app/commands/run.rs:140-170` | Present. | Intended |
| H21.1b, omega-bench requires one `published native output to ` line. | `tools/benchmark/src/experiment.rs:484`; `tools/benchmark/src/records.rs:272`; `app/commands/compile.rs:463` | Present. | Intended |
| H21.1c, sweep picks the stopping stage from error wording. | — | Samples ledger: stage names group diagnostic text for reporting. | Duplicate (16-samples) |
| H21.1d, producer receipt. | — | Samples ledger row 108 binds changed inputs to the receipts. | Fixed (16-samples, 108) |
| H21.2a, expected output checked with `stdout.contains` against native stdout only. | `tools/xtask/src/samples.rs:834-840` | At the triage revision the CLI required byte-equal Psi and native streams before declaring agreement (H11.2), so native stdout represented both. | Intended |
| H21.2b, guest stderr forwarded first; the last match won. | `app/commands/run.rs:140-152`; `tools/xtask/src/samples.rs:800-828` | On a normal agreement the CLI's own lines came last and won. When the CLI printed no `interp exit:` line, a guest-written pair could be parsed, but the gate then required the CLI status to equal that exit. On a crash the CLI returned the native status (-1 for a signal), so a guest-written `0` could not pass. | Intended |
| H21.2c, a crash agreement failed the sample. | `tools/xtask/src/samples.rs:811-816` | Samples ledger: a crash or missing exit report failed the sample. | Duplicate (16-samples) |
| H21.2d, `no-oracle`, `checked` and `not-host-runnable` counted as passes. | `tools/xtask/src/samples.rs:700-779` | Samples ledger row 111: the gate reports them separately from failures. | Duplicate (16-samples, 111) |
| H21.3a–b, both exit lines, equality and CLI status; negative samples. | `tools/xtask/src/samples.rs:601-657`, `:811-828` | Present. | Intended |
| H21.3c, `records::validate` does not rehash declared inputs. | — | Tools ledger, benchmark row 169. | Duplicate (16-tools, 169) |
| H21.3d, H21.4, tools judge the compiler by text. | — | Descriptive. | Intended |

## Summary section of the note

The closing list repeats H3–H21 and needs no separate receipt. Its H8 entry ("Nothing reads them back") is stale (H8.3e). Its H12 cache entry describes retired code (H12.2b–c). Its H21 entry already records the stream comparison.

## Unresolved claims

- **H17.2c.** The Omega-written typer can keep the absent type 0 for an array literal when the type table is full. A fix is proposed above. It needs a run of `source/subsystems/compiler/source_to_psi/tests/typing.py` with an `omega` binary.
- **H5.3b.** It is not traced whether an authored declaration can have the canonical identity text `toolchain::LayoutTarget`. Next step: find the canonical identity builder for package declarations and test an authored opaque type of that name in a `toolchain` package.
- **H20.2.** The construction packet producers (`con/construct.py`, `tests/bootstrap/compiled.py`) and the Alpha tape consumer were not read. This is an inventory gap from the note, not a behavior claim.

## Validation

No test ran for this packet, and no code changed. Each receipt comes from reading the cited source at `a0152d26` (identical to `24ad2700` for every cited path except `tools/xtask/src/samples.rs`), with the survey revision `4f3c255d` fetched to compare H15.2a. The only command run was `mbx xtask provenance --full-session`, for the audit line.
