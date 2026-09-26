# Omega subsystems

Start with the subsystem that owns the operation:

| Directory | Responsibility |
| --- | --- |
| [omega/](omega/README.md) | Application entry, command dispatch and complete command workflows |
| [compiler/](compiler/README.md) | Compilation coordinator and independent supporting packages |
| [package-manager/](package-manager/README.md) | Package operations, source acquisition, resolver execution and review |
| [pipeline/](../pipeline.md) | Transformations from Terminal Psi through native realization |
| [representations/](representations/README.md) | Independent durable native IR and shared compiler data packages |
| `backend/` | Target, ABI, encoding, image and runtime mechanisms |
| [foundation/](foundation/README.md) | Shared host process execution and filesystem custody |

The application is the Cargo package `omega` in `omega/`; run it from the
workspace root with `mbx run -p omega -- --check <root.omg>`.

Supporting crates retain their own manifests. Their placement exposes an owner;
Cargo dependencies enforce the boundaries. No subsystem absorbs IRs or shared
utilities into its `src/` tree. Shared host infrastructure lives in `foundation/`.
Source and semantic infrastructure remains in
[Psi foundation](../psi/foundation), and target-neutral interpretation remains in
[Psi interpreters](../psi/interpreter/README.md).

The [pipeline contract](../pipeline.md) defines the Psi/Omega firewall and
placement rules. Ordinary Rust library implementations live separately in
[`../library/`](../library/README.md).
