# Derivation request admission controls

Run `sh tests/gamma/derivation-admission/run.sh` on macOS arm64 with Python 3
and `codesign`, or Windows x64 using Git Bash with Python 3. Other hosts fail
explicitly. Windows execution remains
unvalidated in this checkpoint.

The gate materializes the complete manifested ordinary-Gamma admission source
and a separate explicit diagnostic entrance — the bound `main.gamma` prefix,
1,270 bytes, SHA-256
`d657d412c92123bdc6dc7c95c38eed50a158c5c986507c5eb3f5ae4420a96e88`, recorded
in `tools/bootstrap/proofs/sources_env.sh` and checked by
`require_derivation_admission_entry_identity` before it packs on the bound
member bytes. It frames inputs and compares the entire result, status, and
empty stderr. No host parser or decoder decides an
admission result, and no production function is extracted or replaced.

The vectors cover all short-header extents; every identity/reserved byte;
individual and combined high length bits; missing sections; maximum legal
length claims; trailing bytes; empty and opaque sections; validation priority;
and exact/adjacent 130 MiB request provision. Small controls run twice; large
boundary vectors run once. Every complete Gamma request, including diagnostic
source and the evaluator's four-byte prefix, must fit its selected provision.
The 600-second host watchdog produces no admission or proof judgment.

The diagnostic entry always returns process status zero after reporting its
owned admission observation. A framed result is byte 0 followed by three
little-endian `u32` section ends. A rejection or incompleteness is its one-byte
tag followed by code, coordinate, limit, and requested as four little-endian
`u32` values. These are test observations, not a canonical checker boundary.

The intentionally opaque-section case proves that framing does not inspect the
theory, proposition, or certificate. It establishes no proof acceptance. Inner
decoding, theory formation, proof rules, root comparison, full checker profile,
and the complete Beta encoding certificate remain separate unfinished work.
