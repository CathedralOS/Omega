# CHAIN-MANIFEST OCREQ entry binding — re-verification ledger

**Status:** resolved on main; re-verified at `b46b34a87f3` (Linux x86-64, this host) under claim `d1a13e6d` (CHAIN-MANIFEST-OCREQ-ENTRY-BINDING, expires 2026-09-21T07:16Z).

## What the item is

Mined-candidate row in TASKS.md (`**CHAIN-MANIFEST-OCREQ-ENTRY-BINDING.**`, ~TASKS.md:7487), resolved as a re-mine of the CHAIN-MANIFEST OCREQ-entry surface already bound on main (sibling CHAIN-MANIFEST-D-OCREQ-REQUEST-BINDING resolution). Sibling stubs folding into the same surface: CHAIN-MANIFEST-OCREQ-BINDING, CHAIN-OCREQ-ENTRY-BINDING, D-OCREQ-ENTRY-BINDING, OCREQ-ENTRY-BINDING, OCREQ-REQUEST-BINDING.

## Binding surface verified at b46b34a87f3

`tools/bootstrap/omega/compiler_env.sh` pins the OCREQ request entry and its sealed fixture pair:

- `OMEGA_REQUEST_ENTRY_SIZE=4115` / `OMEGA_REQUEST_ENTRY_SHA256=0d612813e17cfbe2e755b7398d90bb3572f5ed32da249c8863b37f545d3822c0` (lines 40-41), consumed by `require_omega_request_entry_identity` (lines 133-134).
- `OMEGA_REQUEST_FIXTURE_SIZE=132` / `OMEGA_REQUEST_FIXTURE_SHA256=ab2e980a89d20651b69782446cd8a8333313dce109636fd3e26cc7f52bc98062` (lines 42-43) — the sealed-request fixture pair.
- `OMEGA_EXECUTABLE_OCREQ_ENTRY_SIZE=19253` / `OMEGA_EXECUTABLE_OCREQ_ENTRY_SHA256=9573d73423c2ed3e586b0298ae333733d8828c958a38f1859e1baff3d5ac4a9d` (lines 46-47), bound into the executable manifest check at line 156.

`tests/bootstrap/omega-identity.sh` invokes `require_omega_request_entry_identity` at lines 39, 160, and 171 — a mutated request entry is refused at each stage boundary.

`tests/bootstrap/omega-request/gate.py:130` asserts the canonical request publishes the `coverage_request_semantics` frame through the sealed boundary; `tests/bootstrap/omega-request/README.md` records the gate's contract (the entry is the request boundary itself — it reads one sealed OCREQ V1 stream).

Provisions live in `bootstrap/5_omega/outcome.epsilon` and `wiki/spec/build/compiler_request.md` (`coverage_request_semantics` / `request_staging_bytes` rows), which drive the entry's Incomplete/resource refusals in `main.epsilon`.

## Residuals

None on the bound surface — the row states "no unbound residual" and re-verification found every pin, refusal hook, and spec provision still in place. Executable-chain legs remain host-gated per the frontier (macOS arm64 / Windows x64), as recorded for sibling CHAIN-MANIFEST rows; this host cannot run them.

## Re-verification — `6f91898606` (Zergling-126, claim `4aaa9aad`)

`sh tests/bootstrap/omega-request/run.sh --identity` PASS on this host:
"identity legs green; execution legs need a seed host" — 622,933-byte
receipt request, 565,909-byte customer, 45-byte expected observation.

Fresh drift vs the ledger above: `OMEGA_EXECUTABLE_OCREQ_ENTRY_*` rotated
again — now `SIZE=19249` / `SHA256=5d5d0b8ed0146b055ffdbb6d680bb902a0e350c80b13577bf148879c6c753943`
(compiler_env.sh:46-47), previously 19253/`9573d734…` at the row's last
verification (`e7c0099cb2` via the `28bb320201` OCOUT-table rebase). The
`OMEGA_REQUEST_ENTRY_*` (4115/`0d612813…`) and `OMEGA_REQUEST_FIXTURE_*`
(132/`ab2e980a…`) pins are unchanged, and the identity gate confirms the
rotated executable pin binds the actual bytes. Board text at :9253 and
:11224 still records the superseded 19253/`9573d734…` pair — stale
annotation, not unbound surface.
