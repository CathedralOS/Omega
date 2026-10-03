# Source-produced behavior exclusions

From the repository root:

```sh
cargo nextest run -p package-manager behavior_exclusion_verdicts --no-fail-fast
```

Use `mbx` instead of Cargo when available. The command works on macOS and
Windows; native execution also supports the Linux x86-64 and AArch64 runners.
Each application binds those four hosted targets.

The harness copies the projects and their sibling dependencies into isolated
temporary workspaces, resolves and explicitly reviews the test packages, then
uses ordinary native realization and publication. This test-only review is not
approval for a user's packages. Successful no-op assertion and ordinary silent
logger products execute on the matching host, exit zero and emit no output,
with ControlFlowCleanup disabled and enabled.

Linux-target refusal controls retain public Trap/service allowances: checking
assertions (including `direct-unit.omg`), eager predicate traps under a no-op
provider and actual Sink calls must reject with attributable exclusions.
Additional controls cover executed versus untaken helper selection, a
same-named ordinary machine, and rejection of a copied Build activation.
The helpers change only the copied test sources before closure resolution.

These controls do not close generated-entry/dispatch replay or physical
publication/replacement coverage in TASKS.md. No native result is claimed for a
foreign target merely because it was realized.
