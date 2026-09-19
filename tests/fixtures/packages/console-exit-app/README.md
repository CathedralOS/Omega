# console-exit-app

Application fixture for the ordinary CLI package review of a program that
writes one console line and then terminates through `Console::exit_process`.
It depends on the bundled `source/library/std` as a plain path dependency,
selects `ConsoleNativeProvider`, binds the macOS ARM64 program entry, writes
`console-exit-app` through the checked `write_line` adapter over the
`write_byte` intrinsic, and exits with status 70 from `Main::main`.

Expected package evidence on `macos_arm64`:

- the application's review proposes one blocking `terminal_permission` row per
  compiler-intrinsic leaf of the selected std Console provider: the exact
  `Console::exit_process` requirement permitting process termination,
  `Console::write_byte` permitting process output, and `Console::read_byte`
  permitting process input, beside the std `callable`, `external_supply`, and
  `FilesystemHost` `dangerous_capability` rows;
- each row is a decision, never a grant: leaving the exit row pending while
  accepting every other row keeps the lock unpublished;
- an accepted lock retains the three rows for replay of package acceptance.
  Ordinary compilation requires no ecosystem receiving policy and must not
  construct one from those accepted rows. An artifact emitted without a
  receiving policy carries no receiver-admission claim. Explicit receiver
  admission separately checks an independently supplied policy.

After package acceptance, native production currently reaches physical
legalization and rejects with `Selection(Legalization(SourceCustodyMismatch))`.
The CLI regression pins that exact failure. A successful compile must instead
report a nonempty executable in the requested output directory and preserve the
accepted project files.

From the repository root, inspect the fixture without accepting its review:

```sh
omega audit packages --project tests/fixtures/packages/console-exit-app --target macos_arm64 --offline
```

This command also works in PowerShell. The package CLI test verifies the original
relative std location, then copies the fixture and rewrites that location to the
repository checkout.
