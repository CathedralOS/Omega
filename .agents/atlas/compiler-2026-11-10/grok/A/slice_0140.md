# slice_0140

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1073 | omega-rust/psi/foundation/filesystem-authority/src/filesystem_sponsor.rs:240-247 | REACHABLE | CloseWriter drops the writer but never subtracts `logical_bytes`, so compile-time staging can refuse further writes after close cycles despite the comment that closing releases extent. |
| A1074 | omega-rust/psi/foundation/flow-effects/src/lib.rs:270-272 | REACHABLE | `visit_calls` returns false when the machine has no operational summary, and proof reachability then skips that machine’s call edges instead of rejecting the program. |
| A1075 | omega-rust/psi/foundation/language-core/src/inline_assembly.rs:1847 | CLOSED | All three production callers of `asm_catalog_entry` either emit a source error on `None` or return `None`/`panic` on paths that only run after a listed mnemonic was already accepted. |
| A1076 | omega-rust/psi/foundation/language-semantics/src/const_value.rs:194-227 | REACHABLE | `new`/`from_atom` store mismatched type name and encoding without checking; `validate_exact_const_value_encoding` decodes from encoding with an empty outer type name. |
| A1077 | omega-rust/psi/foundation/language-semantics/src/content.rs:294-304 | REACHABLE | `separate` flattens only one `Separate` level, and projection fingerprints omit `domain` and `projection_machine`, so distinct terms can hash equal. |
| A1078 | omega-rust/psi/foundation/language-semantics/src/declaration_selection.rs:375-376 | REACHABLE | A slot with covering candidates but no `build_keyed` row gets neither a choice nor an error, and native provider selection returns `Ok` with an empty choice list. |
| A1079 | omega-rust/psi/foundation/language-semantics/src/lib.rs:2121-2131 | REACHABLE | An out-of-range or corrupt reach row id reads as no services, and bad parent updates are silent no-ops instead of diagnostics. |
| A1080 | omega-rust/psi/foundation/language-semantics/src/service_surface.rs:54-60 | REACHABLE | Any invalid `ProviderIdentityId` maps to empty text, so unrelated invalid ids compare equal in service receiver equality. |

## A1073

**Cited digest:** area_15 (F/filesystem-authority). Preamble read: `digests/area_15.md` lines 1-54 (read).

**Lines:** `omega-rust/psi/foundation/filesystem-authority/src/filesystem_sponsor.rs:240-247`, `lib.rs:84-87`, `filesystem.rs:44-60`, `filesystem.rs:670-741`, `filesystem.rs:598-636` (read). Cited line numbers match this checkout.

**Enclosing function:** `PreparedAccountTransaction::commit` (`filesystem_sponsor.rs:209-247`, read).

**Caller search:** `CloseWriter` staging path — `grep StagingChange::CloseWriter omega-rust/psi/foundation/filesystem-authority` → 2 matches in `filesystem_sponsor.rs` (read).

**Trace:** `CreateWriter` adds `writer.extent` to `account.logical_bytes` (`filesystem_sponsor.rs:230-234`, read). `CloseWriter` only `Option::take`s the writer slot (`240-246`, read) and does not decrement `logical_bytes`, while `FilesystemAccount` comment at `73-74` says closing releases path and extent (read). Repeated open/close under a byte ceiling can leave `logical_bytes` inflated so `admit_growth` (`86-104`, read) rejects later legitimate staging that should be allowed after closes.

**`wants_write`:** `open_flags::wants_write` (`lib.rs:84-87`, read) treats access as write only when `(flags & 0x3) == 1` or `== 2`, not when both bits are set (mode 3). Checked-interpreter real FS uses this (`filesystem_real.rs:395-396`, read). Build filesystem simulation additionally treats any non-zero low access bits as write (`runtime.rs:279-280`, read: `wants_write(flags) || flags & 3 != 0`). Same flag word can be classified differently across compile-time eval paths.

**Host layout / UTF-8 paths:** `FilesystemMetadataLayout::host` selects field layout via `cfg!` on the compiler host (`filesystem.rs:670-741`, read), consistent with digest `area_15.md:53` (read). `canonical_filesystem_metadata_path_is_canonical` takes `&[u8]` (`filesystem.rs:44-60`, read); non-UTF-8 relative paths can pass when byte rules hold (read).

**Layout `new` checks (606-636):** overlap and extent validation in `FilesystemMetadataLayout::new` (`598-656`, read) runs at layout construction; no single transactional coupling with later host stat encoding was traced to program acceptance (inferred).

**Plain input:** A compile-time evaluation session that opens staging writers, commits, closes them, and repeats until logical-byte accounting blocks a later open that should be allowed after closes.

## A1074

**Cited digest:** area_15 (F/flow-effects). Preamble read (same as above).

**Lines:** `omega-rust/psi/foundation/flow-effects/src/lib.rs:270-272` (read). Cited lines match.

**Enclosing function:** `impl ProofApplicationCalls<SymbolHandle> for OperationalPlan` — `visit_calls` (`265-289`, read).

**Caller search:** `rg '\.visit_calls\(' omega-rust --glob '**/*.rs'` → 2 production call sites in `proof_applications.rs` (`483`, `921`; read). Trait definition at `flow-effects/src/lib.rs:265` (read).

**Trace:** When `machine_by_symbol(machine)` is `None`, `visit_calls` returns `false` (`270-271`, read) without invoking the visitor. In `ProofApplicationPlan::walk_applications`, `has_summary = self.operational.visit_calls(...)` (`proof_applications.rs:483`, read); when `!has_summary`, the loop `continue`s (`539-540`, read), skipping call and cleanup expansion for that machine while other reachability work may already have queued it (`445-453`, read). A machine present in the proof-application plan but missing from the operational plan’s machine index can be analyzed with incomplete invocation edges rather than a hard failure (read).

**Plain input:** A proof reachability query rooted at a machine that has calls in the checked program but no `MachineOperational` row in the published operational plan, so callee proof obligations along those calls are not walked.

## A1075

**Cited digest:** area_15 (F/language-core). Preamble read.

**Lines:** `omega-rust/psi/foundation/language-core/src/inline_assembly.rs:1847` (read; unlisted mnemonic arm `return None` inside `asm_catalog_entry`, function `1435-1850`).

**Caller search:** `rg 'asm_catalog_entry\(' omega-rust --glob '**/*.rs'` — count mode: 37 in `inline_assembly.rs` (mostly tests), 2 in `statements_and_assembly.rs`, 1 in `calls.rs`, 1 in `machine_calls_core.rs` (read). Production non-test: 3 call sites.

**Trace:**

1. `parse_asm_instruction_statement_handle` (`statements_and_assembly.rs:1654-1670`, read): `let Some(entry) = asm_catalog_entry(...) else { return Err(... unknown asm instruction ...) }`.
2. `user_asm_contract` (`calls.rs:4276-4281`, read): missing catalog entry `panic!`s — only after an intrinsic name was already accepted elsewhere.
3. `machine_calls_core` asm authority helper (`5707-5711`, read): `asm_catalog_entry(instruction)?` propagates `None`, skipping that authority edge rather than accepting bad user asm at parse time.

**Plain input:** User-authored assembly with an unlisted mnemonic is rejected at parse with a diagnostic before acceptance.

## A1076

**Cited digest:** area_15 (F/language-semantics / const_value). Preamble read.

**Lines:** `const_value.rs:194-227`, `406-416` (read). Cited lines match.

**Enclosing functions:** `CanonicalConstValue::new`, `from_atom` (`194-227`, read); `take_length_delimited` (`406-416`, read) vs `take_canonical_piece` (`387-403`, read).

**Caller search:** `from_atom` in `omega-rust/psi/compilation` — many validation and pipeline sites (grep count > 30 files, read samples `type_references_and_wire.rs:2186-2214`, `2379-2425`).

**Trace:** `new` and `from_atom` copy type name, encoding, and display with no cross-check that encoding decodes as the named type (`194-227`, read). Main const-parameter path compares `value.type_name` to the required carrier (`type_references_and_wire.rs:2210-2214`, read) and `validate_exact_const_identity_in_carrier` checks name then `decode_encoding` (`2416-2425`, read). `validate_exact_const_value_encoding_in_carrier` builds `CanonicalConstValue::new("", encoding, "")` and decodes encoding only (`2445-2447`, read), never comparing a forged outer type name to the encoding’s inner type tag.

**Length parsing:** `take_length_delimited` accepts any `usize` length prefix without the empty/leading-zero rules enforced in `take_canonical_piece` (`406-415` vs `390-395`, read) — `from_atom` can accept atoms the canonical decoder would reject (inferred from code comparison, not executed).

**Plain input:** A named canonical const spelling whose atom outer type name says one carrier while the encoding payload describes another, routed through an encoding-only validation path or stored and round-tripped via `from_atom` before a name check runs.

## A1077

**Cited digest:** area_15 (F/language-semantics / content). Preamble read.

**Lines:** `content.rs:294-304`, `379-396` (read). Cited lines match.

**Enclosing functions:** `ContentConservationTerm::separate` (`294-304`, read); `encode_content_conservation_term` for `Projection` (`377-415`, read).

**Trace:** `separate` matches `Self::Separate(children)` and `extend`s children once; nested `Separate` nodes are pushed verbatim, not re-flattened (`297-300`, read). `encode_content_conservation_term` for `Projection` hashes `semantic_domain`, `projection_report_fingerprint`, place version/root/segments only — fields `domain`, `projection_machine`, and parameter symbol handles are bound with `..` and omitted (`379-384`, read). `conservation_report_fingerprint` hashes the encoded equation (`344-355`, read).

**Plain input:** Two content conservation projection terms that differ only in omitted domain or projection machine (or a nested `Separate` wrapper) yield the same fingerprint and sort key, so downstream plan identity may treat them as the same obligation.

## A1078

**Cited digest:** area_15 (F/language-semantics / declaration_selection). Preamble read.

**Lines:** `declaration_selection.rs:375-376`, `158-160` (read). Cited lines match.

**Enclosing functions:** `provider_choices::choose` slot loop (`169-403`, read); `selected_application_slots` (`146-163`, read).

**Caller search:** `choose` — `provider-selection/src/choice/selection.rs:190` (read). `selected_application_slots` — `build-time-evaluation/product.rs:1304`, `provider-selection/.../selection.rs:31` (read).

**Trace:** After defaults/overrides and inheritance, `keyed` retains only `build_keyed()` covering rows (`370-374`, read). `match keyed.as_slice() { [] => {} ...}` (`375-376`, read) emits neither `Choice` nor `Error` when candidates exist but none are build-keyed. `provider_selection::choose` maps errors to diagnostics and otherwise returns `Ok(outcome.choices)` (`selection.rs:265-289`, read); empty choices with empty errors is an accepted outcome (test `partial_maps_never_union_into_a_whole_choice`, `602-607`, read). `selected_application_slots` returns `vec![subject.clone()]` when no dependency slot matches subject keys (`158-159`, read), widening slot resolution to the bare subject.

**Plain input:** A native build target with multiple partial provider maps for a slot but no build-keyed covering row completes provider selection without error and without selecting a provider for that slot.

## A1079

**Cited digest:** area_15 (F/language-semantics). Preamble read.

**Lines:** `lib.rs:2121-2131`, `2208-2216`, `1143-1145` (read). Cited lines match.

**Enclosing functions:** `ServiceReachRowTable::services` (`2121-2131`, read); `ServiceReachTable::set_parents` (`2208-2217`, read); `DirectCallDeclarations::new` (`1137-1153`, read).

**Caller search:** `services(` — widespread (grep ≥200 hits, read `flow-effects/lib.rs:546-547`, `language-semantics/tests.rs:263-267` documenting empty slice for unknown/null row).

**Trace:** `services` uses `checked_sub(1)` and `rows.get`; on missing index returns `&[]` (`2127-2129`, read) instead of error. Abstract rows delegate to bound (`2128`, read). `set_parents` updates only when `definitions.get_mut` succeeds (`2211-2216`, read); unknown ids are ignored (read). `DirectCallDeclarations::new` uses `positions.entry(declaration).or_insert(position)` (`1144-1145`, read), keeping the first position for duplicate declaration keys (read).

**Plain input:** Corrupt or stale reach row id or service id in a persisted table is interpreted as “no services” or leaves parents unchanged, so effect or reach summaries compile without surfacing bad ids.

## A1080

**Cited digest:** area_15 (F/language-semantics / service_surface). Preamble read.

**Lines:** `service_surface.rs:54-60` (read). Cited lines match.

**Enclosing function:** `ProviderIdentityTable::text` (`54-60`, read).

**Caller search:** `ProviderIdentityTable` / `.text(identity` — used in validation and terminal paths (grep sample: `service_surface.rs:95-97`, `engine_calls.rs:2155`, read).

**Trace:** `text` uses `identity.slot().and_then(|slot| self.texts.get(slot))` and `unwrap_or_default()` (`55-59`, read), so any id without a valid slot maps to `""`. `ServiceReceiverApplication::eq_in` compares `left.text(self.application) == right.text(other.application)` and the same for trait and argument identity lists (`95-113`, read).

**Plain input:** Two distinct invalid provider identity ids both yield empty display text and compare equal in receiver-application equality even though neither names a real provider.

## What I did not read

- Full bodies of every `from_atom` call site in typed-trees and pipeline crates (only representative validation paths read).
- Terminal-interpreter and checked-interpreter call chains for service reach rows beyond samples.
- Host stat encode/decode routines paired with `FilesystemMetadataLayout::new` overlap checks (606-636) for runtime atomicity claims.
- Specification chapters in `wiki/` for whether metadata paths must be UTF-8 or whether reach row ids are always validated before use.

SLICE COMPLETE
