# Open implementation capture carriers

A generic caller's selected implementation compares its declared parameter
carriers under the exact supplied type/const telescope, even while another
capture operand remains open. The implementation body is specialized only when
its complete tuple closes. Carrier correspondence neither infers new operands
nor grants domain or borrow authority.

The retained telescope also keeps a closed constant's canonical identity;
executable value materialization belongs to the value consumer. The existing
`runtime_captured_map_record_const` witness checks that two record constants
retain their distinct payloads through this same exact-binding admission.

The witness forwards two distinct nominal records and a two-element array
through selected implementations inside `carry<Role>`. The final caller checks
all four payload values. `omega run --both main.omg` must report native and
Terminal Psi exit 0. The three `selected_open_capture_wrong_*` fail fixtures
keep nominal identity, fixed extent and mutable-reference access enforced.

The retained pre-repair producer at `09e378cb4335` rejects all three forwarding calls with
`selected executable carrier differs from its exact public application`:
the raw `Value` binder is compared with `First`/`Second`, and
`[Element; Count]` with `[u64; 2]`. This isolates the same failure as Cathedral's
`NativePlacement<InactivePageTablePlacement, InactivePageTables, Role>::initialize`
without firmware dependencies. Cathedral's fresh-image QEMU boot remains the
customer acceptance; this fixture does not establish that boot by itself.
The integrated base `9be76689b25` changes Omega stage-02 admission, not the
Psi checking or monomorphization inputs responsible for this rejection.
