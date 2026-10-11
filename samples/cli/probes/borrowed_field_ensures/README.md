# borrowed_field_ensures

A guarantee may name a field below a borrowed parameter of another type: `Scope::open(picker: &mut Picker)` resets the caller's record and returns its own — `picker.picks < picker.limit.len` substitutes to the caller's place and discharges the next callee's requires.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
