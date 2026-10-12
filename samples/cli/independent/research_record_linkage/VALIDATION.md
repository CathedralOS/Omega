# NEW064 validation evidence

The complete reconstructed application passed on Linux x86-64. Native stdout
matched every frozen oracle byte-for-byte, with tolerance zero; the public
Terminal comparison explicitly agreed on every exit. The complete
intended-invalid application was rejected at its pinned slot precondition.
No compiler repair was needed.

## Provenance

Reconstruction, not recovery: the recorded original checkpoints
`5a3cd26be912e035ec63144ca7d89ee8b7abe290` and
`4629179e3c34e5673258eafab84bef862881712c` are unreachable (`upload-pack: not
our ref` on origin, absent locally; no remote branch carries the path). The
stated behavior — bounded synthetic record matching with uniqueness,
ambiguity and audits — was re-authored against current sample conventions
with an independent list-scan oracle. No original bytes, original review
decisions, or original acceptance frames are claimed.

## Revision and executor

- Main base: `f93ff527bfa414ed930c77de1ffa5dc0d86fb665`.
- Host: Linux 6.8.0-1061-aws, x86-64, glibc 2.35.
- Rust: pinned `nightly-2026-09-04`, release profile, rebuilt on the rebased candidate (1m28s).
- Target: `linux_x86_64`; one Omega execution at a time.
- Compiler SHA256: `872d123f0bedd93db2b44e36ba2f50f2e88a34614789b9fb6e214ee7d6a4676a`.
- Compiler: `target/release/omega` built from this worktree's unchanged
  Rust/library source.
- Oracle manifest SHA256: `e2aeab6a4fdcd5b75a390e7649df1e15a63f2b579fc423015d06efad3bb6d36a`.

## Commands and results

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
./target/release/omega update --project samples/cli/independent/research_record_linkage --target linux_x86_64
# Reviewed each of the 288 rendered decision rows individually on the
# first proposal: parsed every row's owning package, kind, service path,
# requirement path and permission tags, and resolved every declared
# qualified name against the pinned bundled std source — all resolve
# (3 application terminal_permission rows for the only capabilities this
# program invokes, read_byte/write_byte/exit_process on std Console;
# 1 application dangerous_capability process->Console row for
# exit_process; 141 std callable and 141 std external_supply provider
# declarations for the bundled std package's own Linux/Darwin/Windows/
# console/time/filesystem provider surfaces; 2 std dangerous_capability
# ceiling rows process->Console and filesystem->FilesystemHost). A
# rejected row makes the proposal unpublishable, so all 288 rows were
# accepted; this admits the bundled dependency's declared surface, not a
# filesystem or process grant exercised by this application.
# After rebasing onto current main (f93ff52) and rebuilding the
# producer, `omega update` re-rendered the review: the application source
# changed only by removal of this project's task-plan file, std source is
# identical, and the fresh review contains zero pending capability rows —
# no new or changed capability surface to re-admit.
./target/release/omega update --resume --project samples/cli/independent/research_record_linkage
python3 samples/cli/independent/research_record_linkage/validate.py --omega ./target/release/omega --timeout 600
```

The validator exited 0 with no failures. It ran `--check`, twelve
`run --both` fixture executions (native stdout equality plus explicit
Terminal exit agreement), and the intended-invalid `--check`. Full record:
`build/validation/record.json`.
