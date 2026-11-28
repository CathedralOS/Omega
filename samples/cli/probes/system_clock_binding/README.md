# System clock binding

The program waits for the system clock with `Clock::sleep(200)` and exits
with status 70. The initial Build supplies the bundled system package to
`configure.omg`. That stage binds the program entry and grants the application
access to `Clock` and `ProcessExit` on Linux x86-64 and ARM64, macOS ARM64,
and Windows x86-64.
