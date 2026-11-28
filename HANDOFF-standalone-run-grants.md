# Handoff: standalone run grants (PSI-UNTRUSTED-INTERPRETATION)

Remove this file before merging the branch.

## Done (merged)
- **#6593**: bare `omega run app.psi` interprets saved Psi without producer trust (contained route).
  - Runtime admission takes `psi_authorization::AdmittedTerminalArtifact` (authorized or contained).
  - The optimizer (stage 04), image, component emission, foreign adapters and `CompileRequest::Native` refuse a contained artifact.
  - `TerminalProgram::borrow_admitted` runs compile-time evaluation (calling policies, scalar conditions) of a contained artifact as a contained program.
- **#6623**: contained runs mount the working directory read/write at `/`; explicit `--run-directories-*` replace it.
  - The mapped filesystem serves contained execution (`FilesystemContext::confined`).
  - Sockets and program launch have no contained adapter.

## In flight
Branch `claude/project-thread-uuczrp`, head 51ec709e, pushed. **No PR open yet.** The body is drafted below.

What the branch contains:
- On Linux, source and trusted saved runs default to the cwd mount when the interpreter can confine them. That means a Linux target on the matching host with no published native adapters (`application/src/execution/psi.rs`). Other runs keep the host namespace.
- The mapped binding (`interpreter/host/native-worker/src/host_filesystem/mapped.rs`) gains:
  - a confined `fchmodat`;
  - single-component names under issued directory descriptors (`entry_beneath`), needed by `remove_dir_all`.
- Owner question 2 (`#contained-file-write-bound`): how to bound a contained guest's file writes.
- Owner question 3 (`#unconfinable-standalone-filesystem`): whether runs that need native providers or another target's filesystem should be refused instead of keeping the host namespace.
- Updates to TASKS, the README and `wiki/drafts/designs/psi_containment_audit.md`.

## Next steps
1. Delete this file.
2. Open the PR. Title: `psi: give confinable source runs the working-directory default`.
   - Validate with `mbx xtask check-rules --base origin/main --title ... --body-file ...` (it was clean).
   - Assign Smotto.
3. Squash-merge after the `admission` check passes, with the full head SHA.
4. Once Jarod answers owner questions 2 and 3, implement the rulings:
   - Q2 recommendation: a growth budget in `mapped.rs` (1 GiB and 65,536 entries) that returns `EDQUOT`.
   - Q3 recommendation: refuse unconfinable runs before entry, in `psi.rs`.
   - Then remove the questions per the OWNER_QUESTIONS rules.

## Tests that matter
- `mbx test -p omega --test contained_saved_psi`: 5 tests, pass.
- `mbx test -p native-provider-runtime -- mapped`: 5 tests, pass.
- `mbx test -p omega --test portable_filesystem_values --test provider_reference --test product_directory_mappings --test named_psi_inspection`: pass. `portable_filesystem_values` takes about 5 minutes.
- Full `mbx test -p omega --no-fail-fast`: 48 failures, all also on the base (checker "not supported yet" refusals and others), including three in `target_selected_implementations`.

## Gotchas
- Pre-existing failures on main:
  - terminal-runtime `containment_tests` and native-compiler `saved_linux_program`: the fixture `tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi` hits `UnsupportedFormatMarker(302)`.
  - `runtime_boundary::compilation_and_cli_do_not_own_runtime_integration`.
  - `typed-trees-to-prepared-typed-trees` tests don't compile.
- Main is not rustfmt-clean. Running rustfmt on a file also reformats its `mod` children. Format only your hunks, or revert the noise.
- When merging main, check for leftover conflict markers before committing: `git grep -n '^<<<<<<< '`.
- Saved Psi doesn't carry Build `bind_native` choices. Runs need `OMEGA_HOST_PACKAGE_LOCK` and `OMEGA_HOST_PACKAGE_LOCK_SHA256` from `omega packages update ... --dangerously-accept-all` (see the `Saved` helper in `contained_saved_psi.rs`).
- Never force-push. Merge main into the branch. Leave stash@{0} unpushed: it is the abandoned `--untrusted-artifact` flag.

## Draft PR body
See `/mnt/project-files/handoff/standalone-run-grants-pr-body.md`.
