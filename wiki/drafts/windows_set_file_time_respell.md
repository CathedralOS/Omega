# WINDOWS-SET-FILE-TIME-RESPELL — re-verification ledger

Board row: `TASKS.md` `**WINDOWS-SET-FILE-TIME-RESPELL.**` (:16152) — resolved;
the repair landed at `ff782bdf21`, last re-verified at `8f58b6676b`.
Re-verified on this host (linux x86-64) at `891eb5c584` by Zergling-126
(claim `5544df56`, draft path only).

## Repair stands — confirmed

- `tests/omega/pass/filesystem/windows_set_file_time_exit/main.omg:73-81`
  still assembles `st_mtime` byte-by-byte through `widen_u8_to_u64` with the
  eight shifted lanes (`<< 8..56`) and a single `narrow_u64_to_i64_wrapping`
  at the landing — the `255 << 56` Exact-representability comment is in
  place; the overflowing `widen_u8_to_i64(byte) << 56` idiom does not recur.
- The fixture stays registered in `CHECKED_ONLY_PASS_CANARIES` at
  `omega-rust/omega/compiler/compiler/tests/canary_suite.rs:1067`
  (`"filesystem/windows_set_file_time_exit"`), with the checked-run leg
  `windows_set_file_time_exit_canary_runs` at
  `tests/canary_suite/host_text_filesystem_and_abi.rs:965` and the roster
  constants in `tests/fixture_rosters/`.

## Host gating — unchanged

The fixture's native-execution leg is Windows-gated (entry `Main::fs` wants a
selected fused `FilesystemHost` provider); unmeasurable on linux x86-64 and
attributed there per `wiki/drafts/known_baseline_failures.md` +
BASELINE-CANARY-PASS-CLUSTER's disposition (cluster complete since
d74f2145b9). No slice remains on this host; the correct in-fence artifact is
this ledger. Scoped `canary_suite` run of the checked-only leg in flight.

## Scoped run — linux x86-64

`cargo nextest run -p compiler --test canary_suite -E 'test(~checked_only)'`:
`checked_only_canaries_are_not_backend_umbrella_members` PASS — the fixture's
roster registration is verified green on this host.
`checked_only_capability_canaries_compile_in_isolation` FAILS on an unrelated
member — `capabilities/uses_caller_folder` ("cannot make a boundary or service
call at statement 0 while `self.folder` is absent"), a borrowed-storage-restore
frontier in a different canary; pre-existing on this base and outside the
respell surface. `windows_set_file_time_exit_canary_runs` itself is
`#[cfg(windows)]`-gated and correctly absent from the linux binary (0/1455
match under `-E 'test(~windows_set_file_time_exit)'`).
