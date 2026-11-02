# Linux full-device filesystem acceptance

`pass/filesystem/write_all_full_device_exit` preserves its authored exit 0:
an acquired descriptor can fail its write, and `/dev/full` must produce the
portable `ErrorKind::Other` branch. The fixture now explicitly declares the
filesystem service reach its existing body uses. Its program operation and
numeric oracle are unchanged.

On Linux x86_64, [run 41377130411](https://github.com/CathedralOS/Omega/actions/runs/41377130411)
completed normal package update/review/resume and native exit 0; Terminal
declined the selected foreign binding. This established the missing runtime
supply after repairing the fixture's service declaration. An earlier corpus
pre-check timed out at 120s without reaching either runtime; it established no
compiler rejection.

[Run 41379169868](https://github.com/CathedralOS/Omega/actions/runs/41379169868)
qualified clean source `491f385b3cf9013d4b82bafd4ee3b6964b006495`:

- Optimized CLI SHA256 `867f66a1bf2b52a9bb6fddaa22724850859224777726719b0d453abdd551f8f0`, opt level 1.
- All 192 tracked project/library inputs frozen and verified before and after each phase.
- Ordinary update 170.348s / ReviewRequired 3; 244 pending rows accepted normally;
  ordinary resume 169.835s / exit 0 with its accepted lock.
- `run --both --keep` 51.696s: native exit 0, Terminal exit 0, wrapper exit 0.
- Produced native image SHA256 `7a348324552a75b65a109508a63a0e914aa26df9cf564dc6efd0dd3ea7930da0`;
  Terminal artifact SHA256 `a2cfd8949762d59d5d9cd4d39b883f8388a8a04f214bcee0287a401ac511fe71`.

Artifact `13112742235` retains raw phase streams, accepted review/lock, original
producer/profile, input manifest, and native/Terminal products. Small reports
and the 9.9MB products were independently retained outside scratch with original
SHA256 and ZIP CRC checks. No compiler binary or full artifact ZIP was downloaded.
The independent source-premise review of the same 491f candidate found no blocker.

The trusted adapter admits exact matching Linux target/syscall/declaration/ABI
and the existing explicit host filesystem namespace and service identities.
Openat captures the shared Linux implementation's complete terminated 4096-byte
array. Writes reuse the descriptor owner's captured bytes; standard streams
retain their buffered route. Actual Linux syscall results preserve negative
errno and partial writes. Guest numbers only address this instance's owned
descriptor table; failed close quarantines a number and cannot close a later
reused host descriptor. Remaining owned handles retire with the namespace.

The existing pilot's opt-in `cli_fixtures` lane checks ordinary packages once,
through their real update/review/resume path, and builds only the CLI. It retains
strict explicit native/Terminal exit markers even when a declined Terminal leg
leaves the wrapper at zero. Its 600s review and 240s run allowances are local to
this lane; existing corpus behavior and limits are unchanged. Twenty-nine scoped
Python controls passed. This closes the named Linux full-device witness only;
Windows platform work and bounded occurrence-specific release proofs remain open.
