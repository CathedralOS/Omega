# build_state_machine

`machine build` may carry explicit `state` blocks — the activation is
a normal state machine, and `entry` still holds the required
`builder: &mut Build` first parameter. Named states (`exclusions`
here) declare additional build statements under the same activation.
