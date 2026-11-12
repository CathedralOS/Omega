# indexed_domain_fact

An indexed domain application in proof-fact position: `ensures result in Granted & Resident<SlotPlacement, Slot>` is carried by argument, and `old(&storage)` names the call-site pre-image in an equality fact. `view`/`retire_view` consume the restated instance; `as Extent` strips the qualifiers explicitly.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
