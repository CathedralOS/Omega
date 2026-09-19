# Generic counters

Two instances of `Counter<T>` retain independent fields and method selections:
the integer counter records twice, and the Boolean counter records once. The
program exits **16** without printing. Its Console field must be an established
`Service<Console>` supplied by the selected entry. Service validity is intrinsic;
no separate `Bound` qualification is required.

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
guessing its filename. Expect empty stdout and `native exit: 16` on stderr.
Do not add `--target` to this execution command: an explicit target makes
`run` compile-only. These commands describe each host's route, not a claim that
all hosted targets have passed; remaining coverage is in
[SAMPLE-CORPUS](../../../../TASKS.md).

## Compiler-library regression

The compiler-library regression supplies explicit test-owned acceptance and
checks publication and native exit independently of the CLI's project review:

```sh
OMEGA_SAMPLE_RUNTIME_FILTER=generic_counters mbx nextest run -p compiler --test samples_compile --no-fail-fast -E 'test(=samples_with_documented_exit_run_correctly)'
```

In PowerShell, set `$env:OMEGA_SAMPLE_RUNTIME_FILTER='generic_counters'` before
the same test command and remove it afterward with
`Remove-Item Env:OMEGA_SAMPLE_RUNTIME_FILTER`. Use Cargo if `mbx` is unavailable.
