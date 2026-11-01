# boundary_req_inherit

An acyclic satisfier inherits the requirement's published guarantee without repeating `terminates;` or a private witness — and a boundary-trait requirement may publish the same interface (`terminates;` + `reaches;`) with its satisfier declared BEFORE it: inheritance does not depend on declaration order.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
