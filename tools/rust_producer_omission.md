# Rust producer omission audit

`mbx xtask rust-producer-omission` decides whether a produced closure's
declared dependency set omits the Rust producer (`omega-rust/`), per the
contract in [omega-rust/README.md](../omega-rust/README.md#readme):

> a toolchain, release, or bootstrap input set omits the Rust producer when
> its closed dependency set contains no `omega-rust/` artifact, build step,
> or checkout-derived path (including the bundled `source/library/`
> location, which a self-hosted product must resolve without this
> checkout).

The tool audits a *declared* set; it does not discover dependencies the
declaration forgot to name. Feeding it an incomplete manifest is a
declaration defect, not an audit false-negative.

## Inputs

- `--manifest PATH` (repeatable) — one member spelling per line. A file
  whose first line is a `*SourceClosureV1` header is read as a
  `*.sources` closure manifest and contributes its `member` row spellings
  (the fifth field). Other files contribute one relative path per
  non-blank, non-`#` line. Member spellings are relative to the manifest's
  own directory, exactly as `mbx xtask source-closure` reads
  them.
- `--steps PATH` (repeatable) — the set's declared build/invocation
  surface: one command per line (env scripts, run sheets, wrapper
  contents). Each shell token is checked.
- `--root DIR` — the declared checkout root. Members resolving outside it
  are checkout-derived paths. Defaults to this repository checkout.
- `--record OUT.json` — write the `ReleaseRecordSubstrateV1` audit record
  (subject, member/step counts, checks, findings, verdict) for the
  release-record family.
- `--require omitted|present` — additionally fail when the verdict does
  not match the expectation; use this to pin the current state in a gate.

## Findings

| Check | A member or step fails it when it |
| --- | --- |
| `producer-artifact` | has an `omega-rust` path component |
| `producer-build-step` | invokes `cargo`, `rustc`, `rustup`, `mbx`, `nextest`, or `cargo-nextest`, or references `omega-rust/` |
| `checkout-derived-path` | spells `..` traversal, an absolute path, `CARGO_MANIFEST_DIR`/`OUT_DIR`, or reaches `source/library` through checkout-relative traversal |
| `canonical-spelling` | is not a canonical relative POSIX path (same byte rules as `mbx xtask source-closure`) |

Exit status: `0` = expected verdict matched (omitted by default), `1` = verdict
mismatch, `2` = malformed input. With `--require present`, findings produce
status zero. Record subjects contain canonical absolute input paths so they
remain independently locatable when invoked from a different directory.

## Example

The bootstrap compiler closure and its env surface audit clean today:

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

[rust_producer_omission.sh](rust_producer_omission.sh) audits the interpreted
variant's numbered rungs and proof manifests, plus shell orchestration under
`tools/bootstrap/`. Independent peer roots declare their own seed and audit
boundary. The chain-hygiene gate also audits shell steps under
`tools/bootstrap/` and retains its closed manifest/member/entry identity
checks. Both reject forged Rust producer dependencies; neither is invoked by
the source-to-executable chain.

This is **developer-only verification**. Build its Rust executable once with
`mbx build -p xtask`, then run the shell gate. `OMEGA_XTASK_BIN` can name an
already-built verifier; archive regression fixtures reuse that executable and
pass their own `--root`. Rust builds this auditor and the source-closure,
alpha-forge, and gamma-invoke commands. Those commands pack and invoke bytes;
they do not compile a bootstrap stage. The ordinary construction still uses
the documented Alpha seed, admitted Beta tape, and source inputs.

The verifier rejects invocations without inputs, malformed manifests, non-regular input
files, and forbidden member spellings. The source-closure materializer separately
checks member existence, symlinks, inventory, and byte identities before packing.
Step inspection tokenizes each physical shell line, including quoted/escaped
names; comments are ignored. Incomplete shell fragments use whitespace tokens,
so this remains a lexical audit of a declared surface, not shell interpretation
or a claim to discover indirect dependencies.

## Boundary

The audit covers input sets only. A product *built by* `omega-rust` can
still carry a checkout-derived `source/library` dependency at runtime
(the development root is recorded by
`omega-rust/omega/packages/package-compilation/build.rs` and selected by
that package's `src/toolchain_library.rs`);
that producer-side residual belongs to the self-hosted product's own
input set, not to this checker's declaration surface.
