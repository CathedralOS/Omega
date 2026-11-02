# Typed visitation as a completion value

Parent: `WIRE-CODEC-LIBRARY` in [TASKS.md](../../../../../TASKS.md).
This is a production of the existing [exclusive visitation fact](../README.md):

> Authorized typed mutation lends each non-erased field exclusively to the selected callback once and restores the caller's access afterward.

Each place retains the three-field subject, exact increment operation, wrapped
maximum and three callback observations. The selected query completes an
ordinary Unit machine without a semicolon. Each `invalid/main.omg` changes only
the callback requirement from `u64` to `u32`; it must reject that actual field
type. Both callback bodies support u64 and u32, so changing that requirement
does not make the twin body independently ill-formed. The field place checks nonzero canaries around the subject. The match
join executes both selection arms and checks the untouched subject after the
first call, then checks both updated subjects after the second.

The nine pairs are [local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg)
and [selected implementation](target_body/main.omg). All four hosted entry
bindings are authored. Byte-identical sources and reject expectations are
pinned as `tests/omega/{pass,fail}/facts/typed_reflection_mutation_completion_*`.

The [controls](controls/) exercise an unselected terminal conditional arm,
a shared query in a generic owner, active payload and nullary cases with distinct
runtime outcomes, and declaration inspection during a const initializer. The
last retains context evaluation count, all three descriptions, erased-member
metadata and exact declaration order. These use the same query elaboration;
no codec format operation or alternate field emitter is introduced. Foreign
structural inspection and local field-loan escape controls must still reject;
the open-generic query also has its own excluded-type twin. The original
value-producing `match` control remains a reject: Unit completes a machine
through an invocation, and cannot supply a scalar value arm. The positive
conditional uses the ordinary terminal `transition` form.

The optimized Linux baseline at `bc2c778d57d90b75587d484a6a882ad2cecac9a4`
rejects all nine positive places and the initial query probes with
`typed field visitation requires a unit statement invocation`. Earlier cache
ownership failures are environment observations, not compiler refusal evidence.
The final field and match witnesses strengthen the original observations with
canaries and both incoming branches; their baseline observations are separate.

- Establishment and soundness: every selected callback must admit its exact
  field type; the excluded-type twin supplies no such proof. Reflection requires
  the original lexical owner's structural authority and creates no new access.
- Identity and lifetime: ordinary arguments evaluate once at the expression's
  evaluation point; each callback receives its own subloan and context reborrow.
  The caller subsequently reads its original storage and canaries.
- Composition: all nine transports complete through an ordinary Unit helper.
  Conditional arms remain conditional; no mutable actual is hoisted over a guard.
- Psi lowering: one existing reflection statement elaborator produces ordinary
  checked callbacks inside that helper. Source dependency collection follows the
  retained exact query and source occurrence, including during const evaluation.
- Omega realization: the numeric values, callback count, both branch selections
  and neighboring storage make missing calls or incorrect projections observable
  through native and Terminal execution. This introduces no new ABI or layout
  mechanism and establishes no independent stack-bound or other-host evidence.

On Linux x86-64 the repaired optimized producer executes the nine positive
places and four focused positive controls with native/Terminal exit-zero
agreement. The nine twins and four soundness controls reject their intended
obligations. Both callback bodies also check with matching u32 fields. An ignored copy
changing only the first expected result from 2 to 3 traps in native execution
(signal 4) and Terminal Psi (Trap), with agreeing host presentation. Exact
producer identities, the separate initial authoring failures and execution
reports are retained with the PR; corpus observations are recorded separately. Full authored schema queries, codec synthesis removal, other hosts and
the parent capability's remaining acceptance stay open.
