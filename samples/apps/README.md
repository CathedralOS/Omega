# Application customers

These are independently versioned applications, pinned as **Git submodules**.
Omega tracks their exact commits, not copies of their source. They exercise the
real package manager, nested `build.omg` files, native compiler and runtime.

| Application | Scope | Repository |
| --- | --- | --- |
| `squalr` | Essentially 1:1 Rust Squalr port; CLI/headless first | [CathedralOS/Squalr-Omega](https://github.com/CathedralOS/Squalr-Omega) |

Squalr is private during the in-house phase. Initializing it requires repository
access. Ordinary Omega checkout, builds and standard sample tests do not require
that access; do not initialize every submodule merely to build the compiler.

From the Omega root (PowerShell or macOS/Linux shell):

```text
git submodule update --init -- samples/apps/squalr
cargo build -p omega
python samples/apps/squalr/tools/verify.py layout
python samples/apps/squalr/tools/verify.py check --omega target/debug/omega
python samples/apps/squalr/tools/verify.py native --omega target/debug/omega
```

Use `mbx build -p omega` when available, `target/debug/omega.exe` on Windows,
and `python3` on macOS where needed. Python 3.11+ is required. The application
harness retains failed commands and diagnostics; setup checks are not compilation
or native-execution passes. Consult its README and TASKS before port work.

The initial app is a build graph and geometry seed, not a working scanner.
Preserve the actual Rust package layout and algorithms as the port grows; fix
general compiler gaps rather than rewriting the application around them. Small
compiler regression cases still belong in Omega's own tests.

`samples_compile` discovers only maintained `cli`, `gui` and `uefi` examples.
Application submodules run the explicit commands above, so initialized/private
checkouts cannot silently change the standard test inventory. This is an
ownership boundary, not a passing exemption for application failures.

## Updating the pin

Commit and publish application changes in its own repository first. Then stage
`samples/apps/squalr` in Omega and land the exact tested gitlink through Omega's
normal reservation protocol. A parent pin must never name an unpublished commit.
Do not vendor the application, use `submodule update --remote` as reproducible
setup, or put application-specific behavior in the shipped compiler.
