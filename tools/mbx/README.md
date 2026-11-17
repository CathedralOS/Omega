# Checksum-aware mbx

Cargo's content fingerprints pass `-Zchecksum-hash-algorithm=blake3` to rustc.
Upstream mbx 1.29.2 rejects this unmodeled option and bypasses its cache. The
retained patch models the option, keeps its exact value in the action key and
passes it to dependency discovery. Existing storage and restoration retain the
complete dependency file, including checksum records; there is no second writer.

`install.sh` builds upstream commit
`7fa83b28d239bd39d1079127ffd2f8b0f92df00a` (v1.29.2), after checking the archive's
SHA-256 and applying `checksum-cache.patch`. The MIT license is retained in
`LICENSE.upstream`. This is a local build, identified as `1.29.2+omega.1` in
`tools/mbx-version`, rather than an upstream release. Build metadata preserves the
stable version's ordering. The installed `mbx.source` records the upstream commit,
source archive hash, patch hash, host, installed toolchain and binary hash. Change
the local build identity when changing the patch. Doctor retains its numeric
minimum-version check; the installer checks this complete local identity.

Both `tools/agent_setup.sh` and `tools/xtask/bootstrap-mbx.sh` call this installer.
It needs Bash, curl, tar, patch, an installed Rust toolchain, and unzip on Windows.
It downloads a checksum-verified upstream mbx to build the patched executable;
there is no direct Cargo fallback. The build uses private Cargo and mbx caches,
disables whole-program LTO, and preserves the caller's CPU limits. A failed build
leaves the destination executable intact. A matching source and binary receipt
skips downloading and rebuilding. No cloud environment configuration is written.

For an isolated build without replacing the tool on PATH:

```sh
MBX_INSTALL_WORK=/absolute/owned/scratch bash tools/mbx/install.sh /absolute/owned/bin
```

`MBX_INSTALL_WORK` must be an exclusively owned directory. It retains the source
and build outputs for diagnosis and reuse. Without it the installer removes its
temporary build directory after completion. Each build extracts into a newly
created `source.XXXXXX/` directory; older source directories are neither compiled
nor deleted. Keep development checkouts elsewhere. Build caches and targets
remain outside these sources.

The patch contains three focused adapter tests, selected with
`mbx test --release --locked -p mbx-cache-rustc checksum_algorithm --lib` from the
upstream source. Check the existing parallel-frontend option/key control nearby,
and run scoped Clippy on that crate. Use the same release profile overrides as
the builder when retaining its compiled outputs.

After building xtask, the installer controls run without compiling mbx:

```sh
mbx xtask mbx-cache check-installer /absolute/owned/scratch/source.tar.gz /absolute/new/control-directory
```

The real cache control builds a tiny Rust library and caller, removes only its
private build output to force cache replay, compares the restored checksum
dependency files byte for byte, then changes the answer without changing source
length or modification time. The executable must change from exit 7 to exit 9,
and that compilation must miss the cache:

```sh
mbx xtask mbx-cache probe /absolute/owned/bin/mbx /absolute/new/probe-directory
```

The probe retains exact tool identities, per-build cache statistics, diagnostics
and executable results in its directory. Keep checksum fingerprints enabled.
