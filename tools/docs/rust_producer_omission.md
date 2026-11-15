# Rust producer omission audit

`mbx xtask rust-producer-omission` checks that a closure's declared
dependencies do not include the Rust producer (`omega-rust/`), as defined in [omega-rust/README.md](../../omega-rust/README.md#readme):

> a toolchain, release, or bootstrap input set omits the Rust producer when
> its closed dependency set contains no `omega-rust/` artifact, build step,
> or checkout-derived path (including the bundled `source/library/`
> location, which a self-hosted product must resolve without this
> checkout).

The tool checks only the *declared* set; it does not find dependencies the
declaration forgot. If the manifest is incomplete, the declaration is wrong,
not the audit.

## Inputs

- `--manifest PATH` (repeatable) — one member spelling per line. A file
  whose first line is a `*SourceClosureV1` header is read as a
  `*.sources` closure manifest and contributes its `member` row spellings
  (the fifth field). Other files give one relative path per line,
  skipping blank and `#` lines. Members are relative to the manifest's own
  directory, as `mbx xtask source-closure` reads them.
- `--steps PATH` (repeatable) — the set's declared build and run commands,
  one per line (env scripts, run sheets, wrapper contents).
  Each shell token is checked.
- `--root DIR` — the declared checkout root; members that resolve outside it
  count as checkout-derived paths. Defaults to this checkout.
- `--record OUT.json` — write the `ReleaseRecordSubstrateV1` audit record
  (subject, member/step counts, checks, findings, verdict) for the
  release-record family.
- `--require omitted|present` — also fail when the verdict differs from this;
  use it to fix the expected state in a gate.

## Findings

| Check | A member or step fails it when it |
| --- | --- |
| `producer-artifact` | has an `omega-rust` path component |
| `producer-build-step` | invokes `cargo`, `rustc`, `rustup`, `mbx`, `nextest`, or `cargo-nextest`, or references `omega-rust/` |
| `checkout-derived-path` | spells `..` traversal, an absolute path, `CARGO_MANIFEST_DIR`/`OUT_DIR`, or reaches `source/library` through checkout-relative traversal |
| `canonical-spelling` | is not a canonical relative POSIX path (same byte rules as `mbx xtask source-closure`) |

Exit status: `0` = expected verdict matched (omitted by default), `1` = verdict
mismatch, `2` = malformed input. With `--require present`, findings produce
status zero. Record subjects use canonical absolute input paths, so the inputs can be
found from any working directory.

## Example

The bootstrap compiler closure and its env scripts pass today:

```bash
mbx xtask rust-producer-omission \
  --manifest bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources \
  --steps tools/bootstrap/paths.sh \
  --steps tools/bootstrap/omega/compiler_env.sh \
  --require omitted
```

```powershell
mbx xtask rust-producer-omission `
  --manifest bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources `
  --steps tools/bootstrap/paths.sh `
  --steps tools/bootstrap/omega/compiler_env.sh `
  --require omitted
```

## Gate

[rust_producer_omission.sh](../rust_producer_omission.sh) audits the interpreted
variant's numbered rungs and proof manifests, plus the shell scripts under
`tools/bootstrap/`. Independent peer roots declare their own seed and what
they audit. The chain-hygiene gate also audits shell steps under
`tools/bootstrap/` and keeps its fixed manifest, member and entry identity
checks. Both reject hidden Rust producer dependencies; the chain from source
to executable runs neither.

This is **developer-only verification**. Build its Rust executable once with
`mbx build -p xtask`, then run the shell gate. `OMEGA_XTASK_BIN` can name an
already-built verifier; the archive regression fixtures reuse it and pass
their own `--root`. Rust builds this auditor and the source-closure,
alpha-forge, and gamma-invoke commands. Those commands pack and run bytes;
they do not compile a bootstrap stage. The ordinary build still uses the
documented Alpha seed, accepted Beta tape and source inputs.

The verifier rejects invocations without inputs, malformed manifests, non-regular input
files, and forbidden member spellings. The source-closure materializer separately
checks member existence, symlinks, inventory, and byte identities before packing.
Step inspection tokenizes each physical shell line, including quoted and
escaped names, and ignores comments. Incomplete shell fragments are split
on whitespace. The audit reads declared text; it does not run the shell or
find indirect dependencies.

## Boundary

The audit covers input sets only. A product *built by* `omega-rust` can
still carry a checkout-derived `source/library` dependency at runtime
(the development root is recorded by
`omega-rust/omega/packages/package-compilation/build.rs` and selected by
that package's `src/source/toolchain_library.rs`);
that leftover dependency belongs to the self-hosted product's own input set,
not to what this checker reads.
