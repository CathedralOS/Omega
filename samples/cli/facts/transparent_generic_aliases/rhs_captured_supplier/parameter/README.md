# Captured supplier through parameter transport

Fact: A default trait member keeps an alias's captured const supplier separate from its same-named fresh invocation binder while forwarding the same array value.

This place carries the same array through another callable’s incoming `Padded<3>` parameter before forwarding.
The imported `Selected<3>::forward<17>` operation keeps declaration `Bias=1`
separate from caller `Bias=9` and retains the 17/42 observations. Its sole twin
changes `Selected<3>` to `Selected<4>` with the same four-byte input and rejects
required extent 5 versus actual 4. Sources and expected fragments were frozen
before the first probe and remained unchanged through the repair.

[Run 41364458750](https://github.com/CathedralOS/Omega/actions/runs/41364458750)
at source `7ecc47f1c723e982d9b57870cd7e77691a8eba7b` passed this sample pair
and its canonical mirrors: source checked, native exit 0, Terminal exit 0,
agreement, and the intended twin rejected. Execution was hosted Linux x86-64;
checking records keep their actual default check target. The optimized CLI
SHA-256 is `7feb7b610b86587ebc13be614bfa16c89ad993562fd7a514ef8e81c65daa34c1`,
runner `a88aa5d599c42d77673652803203c8dad243f6f269e31f41cd34cfbdd941230d`,
opt1 under pinned Rust `a69a63265`. Shared preparation took 314.773s,
authentication 1.777s, checking 13.499s, canonical execution 9.201s and
samples 27.015s. This run excluded the previously qualified local place.

The first run 41363064951 remains false: the field stopped at generated-origin
replay before native execution; the parameter checked. The repair rejoins a
source alias through its normalized expansion and expression-backed array
extent through the existing exact child comparison. Alias-own formation,
source tuple, symbols, arity and lifetime checks remain independent.
Raw logs, reports and frozen sources are retained outside scratch in
`/Users/zcanann/Omega-build/alias-supplier-transports-20261101/`.
