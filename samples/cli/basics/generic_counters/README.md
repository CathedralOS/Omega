# Generic counters

Two invocation-local instances of `Counter<T>` retain independent fields. Static
machines take explicit borrowed counters: the integer counter records 30 then 12,
yielding value 12 and count 2; the Boolean counter records true, yielding true
and count 1. The checksum is `12 + 2 + 1 + 1`: process exit **16**, without
printing. Only the observed scalars pass explicitly to the nested final machine;
the entry and final machines declare `reaches Console`.

## Review the project

From the repository root, request review for the host where you will run it:

```text
mbx run -p omega -- update --project samples/cli/basics/generic_counters --target macos_arm64
```

Use `windows_x86_64`, `linux_x86_64`, or `linux_arm64` for those hosts. Exit 3
means decisions are pending, not that acceptance was published. Inspect the
reported `build/package-manager/review-<target>.txt`, change each decision from
`pending` to `accept` or `reject`, then resume:

```text
mbx run -p omega -- update --resume --project samples/cli/basics/generic_counters
```

Resume must succeed before running. Review in the checkout you intend to use:
this sample's local-path source identities bind that checkout, so another
worktree's lock is not reusable approval. Package acceptance does not bypass
proof checking or supply a receiving policy; see
[package acceptance](../../../../wiki/spec/packages/acceptance.md).

## Execute on the host

On macOS ARM64 or Linux:

```sh
counter_exit=0
mbx run -p omega -- run samples/cli/basics/generic_counters/main.omg || counter_exit=$?
test "$counter_exit" -eq 16
```

On Windows PowerShell:

```powershell
mbx run -p omega -- run samples/cli/basics/generic_counters/main.omg
if ($LASTEXITCODE -ne 16) { throw 'Expected exit 16' }
```

`run` compiles, publishes, and executes the receipt-bound artifact without
guessing its filename. Expect empty stdout and `native exit 16` on stderr.
Do not add `--target` to this execution command: an explicit target makes
`run` compile-only. These commands describe each host's route, not a claim that
all hosted targets have passed. The static/local version exits 16 in both
native and checked interpretation on macOS ARM64:

```sh
omega run --both samples/cli/basics/generic_counters/main.omg
```

Windows x86-64 is verified at `b80e3d12bdd` with an optimized dev compiler:
after local package review, EOF and finite Enter each produce empty stdout,
native exit 16 and Terminal exit 16. The CLI comparison checks exits.
On Linux x86-64, candidate 29 at `9da02bdb200017e97206ed4986b7c68dff221806`
completes checking after repairing nested/root boundary classification. Project
update stops with exit 3 at the generated package review; its pending decisions
have not been accepted, so current execution remains unqualified. Linux ARM64
also remains untested for this migration.
Remaining coverage is in [STATIC-MACHINE-MIGRATION](../../../../TASKS.md).

## Native regression

With a built CLI, the maintained gate checks the sample's authored exit oracle:

```sh
python3 tools/sample_native_gate.py --cli target/debug/omega --target macos_arm64 --filter basics/generic_counters --jobs 1
```

On Windows use `python`, `target/debug/omega.exe`, and `windows_x86_64`.
The gate accepts pending package-review decisions only in this checkout to
measure execution. Its result is not a dependency audit or approval for another
checkout. The ordinary manual review route above remains available.
