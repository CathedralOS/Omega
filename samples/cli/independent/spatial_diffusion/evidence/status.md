# SCI04 qualification

Integrated source base: `3097c1001d0b622bbcd35437bc5ef6c02401b010`.
Native numerical producer: `772e7a55f308108c571504be4c01e81702f0347d`,
based on that source base; compiler SHA-256:
`a7720d42717748e7a107d0351d4ef2d2bb4e847f08f688b399d940c7154251e1`.
Terminal numerical producer: `65c0cd4a36602c1d51dab986767b8e5de2ec3053`,
based on `6f44b62a2d4bdc7357d24c750a2532fd0bdf23bc`; compiler SHA-256:
`60b8eb9533607ae821643cb73fb5453479381aefa22e985d011d3da5ceafceb7`.
Its harness SHA-256 is
`3b001c168b80e2689c1ad1f64cd07acde2e4899f29e7dde46dcab27ffdd0d733`.
`observations.json` records accurate producer identities separately per engine.

All 20 native and 20 separately captured Terminal inputs passed the independent
frozen oracle. Every cell, reconstructed true residual, threshold, status,
iteration bound, exit and empty runtime stderr was checked. Measured quadratic
errors are 0.015625000001, 0.003906250159 and 0.001736111311; observed refinement
ratios are 3.999999837 and 2.249999832. Mixed linear and reaction cases converge.
Zero and one iteration limits produce partial fields and explicit failure.
Nine invalid grid controls and four malformed inputs reject as authored.
`observations.json` preserves the actual outputs and five Omega source hashes.
The frozen oracle SHA-256 remains
`08ddc23da792f5ec341e102575488b948b216cfa111d312f3dd30a1e91e89511`.
Dense/analytic checks cover eight meshes and an exact Fraction coarse check in
`oracle-audit.json`. Quadratic Neumann branches and some unused control bounds
have source review but no native preset coverage.

The incoming composed float-requirement changes through `6f44` affect the
sample's guarded output conversion and common contract machinery. A matched
compiler/harness rebuild passed in 8m36s; both full numerical matrices were
repeated with that producer. Scoped Rust checks passed in 2m01s. The subsequent
native conditional-fallthrough repair required another CLI rebuild (3m33s),
fresh native image and full 20-case native requalification. Scoped checks passed
in 57.42s. Psi checking, Terminal production and interpreter/runtime did not
change in this delta, so the independently reviewed 20-case Terminal evidence
retains its actual `6f44` producer identity. Dose-response and stochastic sample
additions do not change SCI04 computation. Both spectral and spatial workspace
registrations remain present. Final independent review covers all captures and
the integrated source; the exact commit receives scoped landing checks.

No compiler/library semantic changes were authored for SCI04. Real authored
program failures exposed exact literal division, module qualification, numeric
conversion and left-to-right mutable-loan ordering errors; repairs follow those
contracts. No checker was weakened. A resumed Rust link failed on a missing
hidden comparison symbol and reproduced after a narrow package clean. Cleaning
the relevant artifacts and disabling learned incremental reuse for that build
resolved it: matched CLI/harness rebuild passed in 13m16s, then the integrated
pair passed in 52.32s. Scoped Rust checks passed in 2m47s; formatting passed.

The exact root terminal permissions remain Console byte input, byte output
and process termination. Build-host inputs/requests and root filesystem
services are absent. A per-row proof verified that all 288 reviewed row changes
were solely baseline schema 12 to 13, including every exact permission value.
The std filesystem/process source-risk classifications grant no host authority.
Automatic review initially refused an insufficiently evidenced admission
command; the exact row proof supported an allowed guarded retry through normal
admission. No denied action was executed or routed around.

The sandbox route maps `/` to uid65534 and rejects cache custody. The documented
per-command approved host route exposes root uid0 and passes unchanged checks.
No ownership or permissions were changed. Doctor passed eight checks.
The original source, outputs, oracle and Git bundle remain in a checksum-verified
recovery archive. Claim coordination was unavailable, with no registration
success asserted. Publication uses the normal landing queue.
