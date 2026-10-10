# wrong_era_decode

Decode accepts only the schema's CURRENT era: a hand-built buffer whose era
byte is 5 against a schema era of 0 must yield the sticky `Invalid` verdict
through `CounterMessage::decode(&mut decoded, &buffer, &mut read, &mut
verdict)` — the decoder keeps scanning bounds-checked but nothing clears
the failure.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
