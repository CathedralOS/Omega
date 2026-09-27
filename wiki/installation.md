# Install and upgrade Omega

Omega's compiler is experimental. A working installation is not a claim that
every language feature or every program in the repository is implemented.

Use the [GitHub releases page](https://github.com/CathedralOS/Omega/releases)
for **published** prereleases. Drafts are not public downloads. If no prerelease
is published yet, [build from source](../README.md#building-from-source).
Homebrew and WinGet channels are being validated; no public channel is advertised
until its install and upgrade paths are verified and published.

## Install an archive

Download the archive and its matching `.sha256` file from the same release.
Choose your host, not the target you eventually want to compile for:

| Host | Archive suffix | Native build target |
| --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc.zip` | `windows_x86_64` |
| Apple Silicon macOS | `aarch64-apple-darwin.tar.gz` | `macos_arm64` |
| Linux x64 | `x86_64-unknown-linux-gnu.tar.gz` | `linux_x86_64` |
| Linux ARM64 | `aarch64-unknown-linux-gnu.tar.gz` | `linux_arm64` |

The release notes name the tested operating systems. Do not assume an unlisted
older OS or architecture is supported. Initial bundles are unsigned: checksums
detect changed bytes, but are not an independent publisher signature. Obtain both
files from the official release, not an unrelated mirror.

On macOS, replace `VERSION` with the downloaded version:

```sh
shasum -a 256 -c omega-VERSION-aarch64-apple-darwin.tar.gz.sha256
tar -xzf omega-VERSION-aarch64-apple-darwin.tar.gz
export PATH="$PWD/omega-VERSION-aarch64-apple-darwin/bin:$PATH"
omega --version
```

On Linux, use `sha256sum -c` and the Linux archive name; extraction and PATH
setup are otherwise the same. Stop if checksum verification fails.

In Windows PowerShell:

```powershell
$archive = 'omega-VERSION-x86_64-pc-windows-msvc.zip'
$expected = ((Get-Content "$archive.sha256").Trim() -split '\s+')[0]
if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $expected) {
    throw 'Omega archive checksum mismatch'
}
Expand-Archive -LiteralPath $archive -DestinationPath .
$env:PATH = "$PWD\omega-VERSION-x86_64-pc-windows-msvc\bin;$env:PATH"
omega --version
```

Keep the **whole extracted directory**. The executable locates its matching
library relative to `bin`; copying just `omega` or `omega.exe` is not an
installation. The commands above change PATH for that shell only. Place the
directory somewhere permanent and add its `bin` to your usual PATH configuration
if you want it available in new shells. Rust and a native linker are not required
to use the bundle. Install Git if your projects acquire Git dependencies.

## Check source

Save this as `main.omg` in an empty directory:

```omega
machine identity(value: u32) -> u32 { value }
```

```sh
omega --check main.omg
```

Success means source checking completed; it does not create an executable.

## Build a native program

A native application needs a selected program entry. The following minimal
project exits with status 42; it deliberately does not depend on terminal output.
Replace `RELEASE_COMMIT` with the full source commit printed by `omega --version`.
The standard library is an ordinary pinned package, not an ambient global import.

`build.omg`:

```omega
machine build(builder: &mut Build) {
    builder.application("first_program");
    builder.depend(Source::Git {
        repository: "https://github.com/CathedralOS/Omega.git",
        revision: "RELEASE_COMMIT",
        selection: PackageSelection::Named { package: "omega_language_std" }
    });
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
```

`main.omg`:

```omega
use omega_language_std::console;
use omega::language::core::binding;

data Main { console: Binding<Console>; }

machine Main::main(&mut self)
reaches Console
{
    self.console.exit_process(42);
}
```

Acquire and review the dependencies, using your target from the table above:

```sh
omega update --project . --target windows_x86_64
```

Exit status 3 means review is required. Read the reported audit and the generated
`build/package-manager/review-<target>.txt`; decide its pending entries rather
than automatically accepting unfamiliar dependencies. Then resume:

```sh
omega update --resume --project . --offline
omega --offline main.omg
```

Run `./build/omega-program` on macOS/Linux, or `./build/omega-program.exe` on
Windows. Inspect `$?` immediately afterward in a POSIX shell, or `$LASTEXITCODE`
in PowerShell: this example returns **42**, not the conventional success code 0.
The release checks exercise this project shape from a fresh Git cache.

## Upgrade the compiler, not the project's dependencies

For an archive installation, download and verify the new **whole bundle** into
a new directory. Point PATH at its `bin`, verify `omega --version`, and rebuild
your project. Keep the old directory until you have verified the new one; rolling
back is selecting the old installation again. Do not overwrite only the executable
or mix library sources from different compiler versions.

`omega update` reviews and updates **project dependencies**. It does not upgrade
the Omega executable. A compiler upgrade does not itself authorize rewriting a
project's `omega.lock`. If new compiler behavior or a requested dependency change
requires review, inspect it through the normal package workflow.

For problems, include `omega --version`, host OS/architecture, the exact command,
and its diagnostic. The [CLI reference](../omega-rust/omega/omega/README.md#command-surface)
documents the implemented command surface; the [language guide](language_guide/language_guide.md)
describes the language, including features still ahead of the compiler.
