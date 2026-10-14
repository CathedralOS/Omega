# Placed scalar access to original RAM

`omega run --both main.omg` exercises initialized ordinary RAM through the explicit `Placed::view_mut` factory. Status is a readonly u32 at byte 0; transmit is a write-only u8 at byte 4. Every access uses the generated sealed accessor.

The frozen output files cover capture retirement, helper calls, whole returned views, direct reads and writes, descriptor capture, one evaluated array selector, stored whole views, forwarding, state transitions and Unit-returning writers. Each final 4109 line means the read observed 41 and the original transmit byte was 9 after loan retirement.

Prepare the package with `omega update` and the normal capability review. Source acceptance and the static placement plan do not grant backing or runtime authority. Native host and callback entrances still need an independently proved lender; this sample captures its own initialized RAM.

For exact stream qualification, run `python3 validate.py --cli /absolute/path/to/omega --evidence /absolute/path/to/evidence` after normal package preparation. The validator compares separate native and captured Terminal stdout, stderr and exit status against the frozen files for this composition sample, the original-backing fixture, and the typed fixture. The typed fixture reads and writes i16, bool, f32 and f64 values; its two `1111` lines verify all four equalities inside the helper and again in the original storage after loan retirement.
