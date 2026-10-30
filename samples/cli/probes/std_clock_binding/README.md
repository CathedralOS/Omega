# std_clock_binding

Canonical std `Clock` provider through static machine with `reaches Console + Clock` multi-service ceiling + `Clock::sleep(200)`; `build.omg` uses `Build::depend(Source::Path{std})` and binds `ProgramEntry` for hosted targets (`windows_x86_64`, `linux_x86_64`, `linux_arm64`, `macos_arm64`).

Expected exit: 70.
