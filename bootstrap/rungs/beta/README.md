# `bootstrap/rungs/beta/` — the Beta compiler, written in Beta (SELF-HOSTING)

This directory owns the complete Beta-language surface and its compiler written
*in Beta itself* (`bc.beta`), not in the throwaway Rust producer
(`../../onramps/beta-rust/`) and
not hand-written in assembly. **It self-hosts** — `bc` compiles its own source to a
compiler that reproduces that compilation byte-for-byte (`selfhost.sh`). This
closes the steady-state execution dependency on Rust. It does not by itself prove
that the cold-started artifact corresponds to `bc.beta`; complete lower-rooted
source-to-artifact validation remains open.

The lower-rooted replacement is complete under
[`cold-start/`](cold-start/README.md). The Alpha-written compiler covers the
exact pinned surface, builds `bc.beta`, reaches a byte-identical self-hosted fixed
point, and reconstructs the persisted platform-independent
[`artifacts/bc.tape`](artifacts/README.md). That artifact passes the whole Beta
corpus and is consumed by the downstream proof-kernel, Gamma, Delta, bridge, and
refinement gates. The Rust producer remains diagnostic/on-ramp history.

```
bc.beta       the Beta compiler, in Beta:  reads .beta on stdin, emits Alpha asm
selfhost.sh   THE gate: bc compiles bc.beta -> bc1; assert bc1(bc.beta) == bc(bc.beta)
test.sh       language gate: bc compiles and runs the retained Beta corpus
source-exhaustion.sh  exact B_bc1 compiler-resource boundaries and checked failures
```

## How it bootstraps

```
  bc-alpha.alpha ──(Alpha seed + Alpha assembler)──▶ cold-start compiler
  bc.beta ──(cold-start compiler)──▶ asm ──(Alpha assembler)──▶ initial bc
  bc.beta ──(initial bc, then one self-build)──▶ persisted fixed-point bc.tape
  program.beta ──(persisted bc.tape)──▶ asm ──(Alpha assembler)──▶ tape ──▶ run
```

The default gates stamp the persisted, platform-independent tape into the host's
audited Alpha seed. `bootstrap/onramps/beta-rust/` remains available only as a
diagnostic/reference producer; it is not in this construction lineage. The
fixed-point equality establishes deterministic self-reproduction, not compiler
correctness. Complete source-to-artifact refinement against canonical Beta
meaning remains a separate open assurance edge.

Run the gate:

```sh
sh test.sh        # builds bc, compiles arithmetic programs with it, checks results
```

## Status — SELF-HOSTING (slices 1–6 done)

bc implements the whole Beta language and compiles its own source to a byte-for-byte
fixed point. Built slice by slice, mirroring the `beta-rust` producer:

| Slice | Adds | Notes |
| --- | --- | --- |
| 1  | arithmetic (`+ - * / %`, parens, precedence) | stack-machine codegen, same shape as the on-ramp |
| 2a | `let` locals, assignment, variable refs | tokenizer + per-proc symbol table; a pre-scan (`count_lets`) sizes the frame |
| 2b | the six comparisons; CFG control flow — `state` blocks + guarded `to … when …` transitions (Beta has no if/while) | generated labels; 0/1 materialization; loops are self-transitioning states |
| 3  | procedures, parameters, calls, recursion | the calling convention (args r0..r3, frames via fp) |
| 4  | `byte[]`/`word[]` memory | `loadb`/`load`/`storeb`/`store` |
| 5  | char literals `'x'`, `read_byte`/`write_byte`, call statements | + prescan skips char/string literals |
| 6  | string literals via `emit("...")` | inline `db` data jumped over + a `__write_str` loop |

`sh selfhost.sh` is the fixed-point gate; `sh test.sh` is the per-feature gate. bc's self-tape
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

The Beta compiler has a Rust-free cold-start and steady-state execution path;
complete lower-rooted validation of its source correspondence remains open. It builds
Gamma's canonical interpreter and type checker; Gamma in turn supplies Delta's meaning substrate. The proof kernel
is a cross-cutting service with independent Beta and Gamma implementations, not
a later language rung. The Rust producer (`../../onramps/beta-rust/`) is outside
the lineage and remains only as a documented diagnostic/reference producer.
`compiler/beta-lang-rs` is a compatibility path.
