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

The port includes geometry and scan sources; source presence is not evidence of
working native scanning. Use the application harness to establish that result.
Preserve the actual Rust package layout and algorithms as the port grows; fix
general compiler gaps rather than rewriting the application around them. Small
compiler regression cases still belong in Omega's own tests.

`samples_compile` discovers only maintained `cli`, `gui` and `uefi` examples.
Application submodules run the explicit commands above, so initialized/private
checkouts cannot silently change the standard test inventory. This is an
ownership boundary, not a passing exemption for application failures.

## Updating the pin

Squalr has one branch: `main`. Do not create feature branches or alternate
publication lineages. Work from its latest `main`, preserve earlier port work
when integrating, and publish application changes there before updating Omega.
Separate workspaces may use detached checkouts; they do not create another
application branch.

The exact tested commit must be reachable from the application's remote `main`;
an older ancestor is valid, an unpublished or divergent commit is not. Check
before staging the gitlink (PowerShell or macOS/Linux shell):

```text
git -C samples/apps/squalr fetch origin main
git -C samples/apps/squalr merge-base --is-ancestor <tested-commit> origin/main
```

The ancestry check must exit zero. Then stage `samples/apps/squalr` in Omega and
land that tested gitlink through Omega's normal reservation protocol. Passing
one small fixture on a divergent commit is not grounds to replace cumulative
application progress. Do not force-push away another contributor's work.

Do not vendor the application, use `submodule update --remote` as reproducible
setup, or put application-specific behavior in the shipped compiler.
