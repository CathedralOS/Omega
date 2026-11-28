# note_vault

The vault exercises the portable filesystem and host clock. It creates a note, appends two entries, checks the file's size and modification time, reads back its contents, truncates and copies the file, enumerates its directory, and removes the tree. Completing the workflow returns 14. The program uses `omega_vault/` under its working directory. Run from a directory where that name is free.

```
omega run samples/cli/systems/note_vault/build.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/systems/note_vault` from the repository root.

The first Build admits `system` into the next Build's source scope with an empty service ceiling. The second Build can then import the clock service that `system_time_now` reaches. It grants that service, the six filesystem authorities and process exit to the application's `system` dependency, and binds the application's entry on each supported target.

The POSIX provider serves macOS ARM64 and both Linux targets. Windows selects its portable provider over the Windows host services. [Filesystem provider closure](../../../../wiki/spec/build/platform_services.md#provider-closure-transport) · [Earlier Windows execution results](../../../../wiki/drafts/measurements/windows_note_vault_2026_11_02/README.md)
