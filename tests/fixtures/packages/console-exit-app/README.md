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
- an accepted lock retains the three rows, and the accepted package permission
  policy projected for native realization contains exactly those rows, so
  ordinary compilation must pass package acceptance without an ecosystem
  receiving policy. The misplaced gate is removed: the receiving permission
  policy is optional through native realization, and an artifact emitted
  without it binds no admission claim. Explicit receiver admission still
  rejects the emitted artifact under an insufficient independent policy.

The package CLI test copies this fixture and rewrites the std location to the
repository checkout; the relative location above resolves from this directory.
