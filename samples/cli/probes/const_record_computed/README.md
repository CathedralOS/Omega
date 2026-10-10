# const_record_computed

Computed nominal consts: `const CONFIG: Config = Config{size: SIZE*2, enabled: SIZE==7}` — record literal + bool member in const init, field projection `CONFIG.size`/`CONFIG.enabled` as machine bodies; anonymous `7/2*2` warns exact-fraction landing.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
