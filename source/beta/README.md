# `source/beta/` — the Beta compiler, written in Beta (SELF-HOSTING)

This directory owns the complete Beta-language surface and its compiler written
*in Beta itself* ([`compiler/bc.beta`](compiler/bc.beta)), not hand-written in assembly. **It self-hosts** —
`bc` compiles its own source to a
compiler that reproduces that compilation byte-for-byte
([`compiler/validation/selfhost.sh`](compiler/validation/selfhost.sh)). This
closes the steady-state execution dependency on Rust. It does not by itself prove
that the cold-started artifact corresponds to `bc.beta`; the adjacent validator
reconstructs that claim, while its non-circular checker root remains open.

The lower-rooted replacement is complete under
[`compiler/cold-start/`](compiler/cold-start/README.md). The Alpha-written compiler covers the
exact pinned surface, builds `bc.beta`, reaches a byte-identical self-hosted fixed
point, and reconstructs the persisted platform-independent
[`compiler/artifacts/bc.tape`](compiler/artifacts/README.md). That artifact passes the whole Beta
corpus and is consumed by the downstream checker, Gamma, Delta, and compiler
validation gates. The former Rust producer has been retired.

```
compiler/     compiler source, admitted artifact, Alpha cold start, and validation
test.sh       language gate: bc compiles and runs the retained Beta corpus
```

## How it bootstraps

```
  bc-alpha.alpha ──(Alpha seed + Alpha assembler)──▶ cold-start compiler
  bc.beta ──(cold-start compiler)──▶ asm ──(Alpha assembler)──▶ initial bc
  bc.beta ──(initial bc, then one self-build)──▶ persisted fixed-point bc.tape
  program.beta ──(persisted bc.tape)──▶ asm ──(Alpha assembler)──▶ tape ──▶ run
```

The default gates stamp the persisted, platform-independent tape into the host's
audited Alpha seed. No parallel Beta producer remains. The
fixed-point equality establishes deterministic self-reproduction, not compiler
correctness. Source-to-artifact authority requires the maximal-observation
reconstruction under `source/beta/compiler/validation/` to terminate in a
checker rooted below `bc`; the current `check.beta` construction does not yet
satisfy that boundary.

Run the gate:

```sh
sh test.sh        # builds bc, compiles arithmetic programs with it, checks results
```

## Status — SELF-HOSTING (slices 1–6 done)

bc implements the whole Beta language and compiles its own source to a byte-for-byte
fixed point. It was built slice by slice and is now the sole compiler path:

| Slice | Adds | Notes |
| --- | --- | --- |
| 1  | arithmetic (`+ - * / %`, parens, precedence) | direct stack-machine code generation |
| 2a | `let` locals, assignment, variable refs | tokenizer + per-proc symbol table; a pre-scan (`count_lets`) sizes the frame |
| 2b | the six comparisons; CFG control flow — `state` blocks + guarded `to … when …` transitions (Beta has no if/while) | generated labels; 0/1 materialization; loops are self-transitioning states |
| 3  | procedures, parameters, calls, recursion | the calling convention (args r0..r3, frames via fp) |
| 4  | `byte[]`/`word[]` memory | `loadb`/`load`/`storeb`/`store` |
| 5  | char literals `'x'`, `read_byte`/`write_byte`, call statements | + prescan skips char/string literals |
| 6  | string literals via `emit("...")` | inline `db` data jumped over + a `__write_str` loop |

`sh compiler/validation/selfhost.sh` is the fixed-point gate; `sh test.sh` is
the per-feature gate. bc's self-tape
is ~52 KB — well within both committed 256 KB tape holes (see below).

The complete cold-start correspondence target is specified in
[`BOOTSTRAP_OBSERVABLE.md`](BOOTSTRAP_OBSERVABLE.md). It includes the complete
output byte stream and distinct halt, trap, checked-exhaustion, and divergence
outcomes; low-byte exit agreement or a finite corpus is not that claim. The
compiler now reserves its full 1 MiB source arena explicitly and returns the
checked source-exhaustion projection before emitting a truncated artifact.

### The hole (resolved)

`emit` lowers each output byte to ~12 tape bytes, and a compiler is mostly fixed
output, so bc's tape grew fast. Two things were done: the assembler gained a `db`
data section (1 byte/char) — but that only trimmed ~12%, because **bc's tape is
dominated by its own compiled *logic*, not emit strings**. Both platform tape
holes were then grown 32 KiB → **256 KiB** (`.space 0x40000` on arm64; an
audited PE extent change plus zero extension on x64). The current self-hosting
`bc` tape therefore fits both committed seed images.

<details><summary>historical: the compact-asm + db steps</summary>

bc emits compact asm (no indent/alignment — the assembler treats commas/indent as
whitespace; slice 1 was ~12.4 KB vs ~15.5 verbose), and `emit("...")` lowers to a
`db` data section + `__write_str` loop. Both preserve both-seeds lockstep (the
assembler's own source uses no `db`, so its tape — and the self-host fixed point —
is unchanged). `imm rD, label` already worked (the assembler's `eo_label` path
resolves a label as an 8-byte immediate), so the only assembler change needed was
the `db` directive itself.

</details>

## Role in the lattice

The Beta compiler has a Rust-free cold-start and steady-state execution path,
and extensive lower-rooted reconstruction of its `B_bc1` source correspondence. It builds
Gamma's canonical interpreter and type checker; Gamma in turn supplies Delta's
meaning substrate. The Alpha-owned derivation checker has independent Beta and
Gamma implementations, but it remains a trust-floor service rather than a
later language rung. No external Beta producer participates in the lineage.
