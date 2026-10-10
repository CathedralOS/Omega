# float_match_effects

Effectful calls as match subject AND patterns: left-to-right evaluation — subject once, patterns until first hit; `calls` counts 3 (subject + 2 patterns); unselected arms never evaluate `divide(0)`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
