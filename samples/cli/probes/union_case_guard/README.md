# union_case_guard

Union of implicit case domains as a transition subject: `main.cmd in Command::Quit | Command::Move` classifies the held tag — a payload-bearing union arm (`Move{dx}`) still tests tag-only and must not leak the constructed payload into the compare. Exercised twice: held value matches second arm, then first.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
