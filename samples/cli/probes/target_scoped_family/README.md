# target_scoped_family

Per-target bodies of one machine name: `windows_x86_64|macos_arm64|linux_x86_64|linux_arm64 machine Helper::pick` — Psi checks every scoped body in one compilation; a second family `Helper::delta` on a subset of targets coexists; selection is the build binding.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
