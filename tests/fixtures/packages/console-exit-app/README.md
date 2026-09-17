# console-exit-app

Application fixture for the ordinary CLI package review of a program that
terminates through `Console::exit_process`. It depends on the bundled
`source/library/std` as a plain path dependency, selects
`ConsoleNativeProvider`, binds the macOS ARM64 program entry, and exits with
status 70 from `Main::main`.

Expected package evidence on `macos_arm64`:

- the application's review proposes one blocking `terminal_permission` row for
  the exact `Console::exit_process` requirement permitting process termination,
  beside the std `callable`, `external_supply`, and `FilesystemHost`
  `dangerous_capability` rows;
- the row is a decision, never a grant: leaving it pending or rejecting it keeps
  the lock unpublished;
- an accepted lock retains the row, and the accepted package permission policy
  projected for native realization contains exactly that row.

The package CLI test copies this fixture and rewrites the std location to the
repository checkout; the relative location above resolves from this directory.
