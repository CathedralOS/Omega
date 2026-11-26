# system_clock_binding

Canonical system `Clock` provider through static machine with `reaches ProcessExit + Clock` multi-service ceiling + `Clock::sleep(200)`; `build.omg` uses `Build::dependency(builder, dependency::from_source(Source::Path{system}))` and binds `ProgramEntry` for hosted targets (`windows_x86_64`, `linux_x86_64`, `linux_arm64`, `macos_arm64`).

Expected exit: 70.
