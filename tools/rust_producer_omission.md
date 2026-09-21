# Rust producer omission audit

`tools/rust_producer_omission.py` decides whether a produced closure's
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
  own directory, exactly as `tools/bootstrap/source_closure.py` reads
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
| `canonical-spelling` | is not a canonical relative POSIX path (same byte rules as `source_closure.py`) |

Exit status: `0` = omitted, `1` = producer present (or `--require`
mismatch), `2` = malformed input.

## Example

The Omega compiler D closure and its env surface audit clean today:

```bash
python3 tools/rust_producer_omission.py \
  --manifest bootstrap/5_omega/omega_compiler.epsilon.sources \
  --steps tools/bootstrap/paths.sh \
  --steps tools/bootstrap/omega/compiler_env.sh \
  --require omitted
```

```powershell
python tools/rust_producer_omission.py `
  --manifest bootstrap/5_omega/omega_compiler.epsilon.sources `
  --steps tools/bootstrap/paths.sh `
  --steps tools/bootstrap/omega/compiler_env.sh `
  --require omitted
```

## Gate

[rust_producer_omission.sh](rust_producer_omission.sh) pins the canonical
bootstrap input set with `--require omitted`: every `*.sources` closure
manifest under `bootstrap/` plus every `*.sh` orchestration step under
`tools/bootstrap/`. Both sets are discovered rather than listed, so a new
rung, closure manifest, or env script is audited without an edit here.
It refuses a tree with no manifests or no step surface at all — an empty
set cannot silently pass. Test fixtures under `tests/` are not release or
bootstrap input sets and stay outside the audit. The gate runs on any
host with `python3` and POSIX `find`; it does not execute the seeds.

## Boundary

The audit covers input sets only. A product *built by* `omega-rust` can
still carry a checkout-derived `source/library` dependency at runtime
(the bundled root is baked from `CARGO_MANIFEST_DIR` in
`omega-rust/omega/pipeline/source-files-to-assembled-syntax/src/frontend/mod.rs`);
that producer-side residual belongs to the self-hosted product's own
input set, not to this checker's declaration surface.
