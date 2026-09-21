# CHAIN-OCREQ-ENTRY-BINDING — record

Stub re-mines the OCREQ request-entry binding already resolved on
siblings CHAIN-MANIFEST-OCREQ-ENTRY-BINDING and
OCREQ-REQUEST-ENTRY-BINDING. Re-verified at `94e764a6da` on linux x86-64:

## Bound surface on main

`tools/bootstrap/omega/compiler_env.sh` carries the pinned identities:
`OMEGA_REQUEST_ENTRY_SIZE=4115` /
`OMEGA_REQUEST_ENTRY_SHA256=0d612813…` (the canonical OCREQ request
entry), the sealed-request fixture pair, and
`OMEGA_EXECUTABLE_OCREQ_ENTRY_SIZE=19253` /
`SHA256=9573d734…` for the executable chain leg. The omega-request gate
(`tests/bootstrap/omega-request/`) drives the sealed request through the
Gamma→Delta→Epsilon→D chain; `omega-identity.sh` refuses the bound file.

## Witness at tip

`sh tests/bootstrap/omega-request/run.sh --identity` — PASS: all bound
identities verified and both byte streams assembled (622,933-byte
receipt request, 565,909-byte customer, 45-byte expected observation).
The customer size drifted from the 563,268 bytes recorded at
`163670cf6d` because the stream assembles from the moving source
closure; the pinned entry identities are unchanged. The executing half
stays seed-host-gated by `require_seed_execution_host` — Linux x86-64 is
an admitted audited host (d3776b9890), so that leg is duration-bounded
full-chain interpretation, not a code change.

Sibling stubs on the same bound surface: OCREQ-ENTRY-BINDING,
OCREQ-REQUEST-BINDING, CHAIN-MANIFEST-OCREQ-BINDING,
D-OCREQ-ENTRY-BINDING.

Field note (review ad738b905c49..771d0469a1c4): this record duplicates
the TASKS.md row verbatim — ledger-only. Fold the four sibling stubs
above into OCREQ-REQUEST-ENTRY-BINDING in one edit and delete this file
rather than stamping each stub with the same `--identity` witness.
