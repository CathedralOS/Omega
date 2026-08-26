# Omega-bootstrap normalized provider-resolution witness, schema major 6

[`OMGCOMP2`](OMEGA_BOOTSTRAP_COMPILATION_V2.md) |
[`OMGRSW1`](OMEGA_BOOTSTRAP_RESOLUTION.md) |
[`OMGRSW5`](OMEGA_BOOTSTRAP_RESOLUTION_V5.md)

`OMGRSW6` is the frozen resolution-only witness for the bounded
`Console::exit_process` compatibility-cost carrier. It gives semantic identity
to the exact trait requirement, service-reach edge, external realization,
`satisfies` edge, target applicability, payload-free compiler-intrinsic
candidate, boundary-receiver call, and selected root carried structurally by
`OMGCOMP2`.

This contract stops at resolution. In particular, a realization candidate and
a requirement call are retained as two different identities. Their simultaneous
presence does not select the realization for the call.

`OMGRSW6` is bridge-private cost evidence. It is not an Omega ABI, a stable
product format, a provider plan, an intrinsic-catalog receipt, provider
admission, checked IR, an executable artifact, package authority, or compilation
authority.

## 1. Exact input and canonical identity

The input is one structurally valid `OMGCOMP2` envelope with schema major 2,
schema minor 0, target 1 (`Linux x86-64 System V`), flags zero, and selected
configuration 1 (`native provider substitution`). `OMGCOMP1`, another target,
another configuration, or any other envelope version rejects. The target and
configuration constrain applicability only; neither selects a candidate.

The envelope has exactly the following graph:

- package 0 contains source 0 and source 1;
- package 1 contains source 2;
- source 0 and source 1 have the same resolver-owned logical module `console`;
- source 2 has resolver-owned logical module `app`;
- package 1 has exactly one requester-local direct alias `omega_std` to package
  0; and
- the selected root is source 2, `Main::main`.

Package keys, source labels, source-bundle entry IDs, and string-table IDs are
not fixed by rendered spelling here. They remain the exact canonical IDs from
the input envelope. Source labels remain custody metadata, while the envelope's
source rows alone own logical placement.

After comments and whitespace are removed by ordinary lexical rules, the three
source extents admit exactly these declarations and executable statement:

```omega
// source 0, module console
pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches
        Console;
}

data ConsoleNativeProvider { }
```

```omega
// source 1, module console
linux_x64 machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process
    via Binding::CompilerIntrinsic;
```

```omega
// source 2, module app
use omega_std::console::Console;

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    self.console.exit_process(70);
}
```

The exact declaration import is intentional. This contract does not assign a
general meaning to a module-only `use omega_std::console;` spelling. There is no
authored `module` declaration; resolver-owned placement supplies the module.

The provider owner, requirement, and realization share one logical module, so
this relation needs no private access between distinct modules. The application
names only the public imported trait and its inherited-public requirement. The
private realization is not nameable by the application and is not the target
of its call.

The app field's bare `Console` type is admitted only as the product's existing
boundary-trait-value compatibility fence. In `OMGRSW6` it supplies static
receiver resolution and no runtime carrier, multiplicity, construction,
installation, or authority meaning. This contract does not replace the
destination `Service<Console> in Bound` model.

No additional declaration, requirement, contract clause, reach entry,
realization, import, call, state, field, parameter, provider-default machine,
or provider-selection expression is admitted. In particular,
`ConsoleNativeProvider::provider_defaults` and `select_provider` are outside
this source relation rather than ignored as opaque body bytes.

The output magic is `OMGRSW6\0`, schema major is 6, schema minor is 0, flags are
zero, and header size is 128. No source accepted by this contract has another
canonical OMGRSW identity. Changing only magic, major, envelope version, target,
or configuration never creates a cross-version pair.

The major number follows OMGRSW5, but this profile consumes OMGCOMP2 rather
than widening the OMGCOMP1 source relation. It reuses frozen row shapes where
stated below; it does not inherit subtraction, checked-IR, or artifact meaning
from the numerically preceding compiler-cost slice.

## 2. Header, fixed counts, and table order

All integers are unsigned little-endian. `NO_ID` is `0xffffffff` only where a
row explicitly permits it. Source spans are relative to the exact content
extent of the source named by the row, align to independently lexed token
boundaries, and never cross an extent.

```text
offset  width  field
0       8      magic: ASCII "OMGRSW6\0"
8       u16    schema major: 6
10      u16    schema minor: 0
12      u16    flags: zero
14      u16    header size: 128
16      u32    exact total witness length: 1,064
20      u32    source/unit count: 3
24      u32    import count: 1
28      u32    static-binding count: 6
32      u32    declaration count: 5
36      u32    normalized-type count: 4
40      u32    record count: 2
44      u32    ordinary-field count: 1
48      u32    checked-body machine count: 1
52      u32    checked-body machine-parameter count: 0
56      u32    checked-body block count: 1
60      u32    checked-body block-parameter count: 0
64      u32    pure-sum count: 0
68      u32    case count: 0
72      u32    case-payload-field count: 0
76      u32    boundary-trait count: 1
80      u32    requirement count: 1
84      u32    requirement-parameter count: 1
88      u32    service-reach count: 1
92      u32    external-realization count: 1
96      u32    realization-parameter count: 1
100     u32    requirement-call count: 1
104     u32    selected checked-body machine ID: 0
108     u32    selected target: 1 = Linux x86-64 System V
112     u32    selected configuration: 1 = native provider substitution
116     u32    reserved: zero
120     u32    reserved: zero
124     u32    reserved: zero
```

Tables occur in this exact order:

1. units;
2. imports;
3. static bindings;
4. declarations;
5. normalized types;
6. records;
7. ordinary fields;
8. pure sums;
9. cases;
10. case-payload fields;
11. checked-body machines;
12. checked-body machine parameters;
13. checked-body blocks;
14. checked-body block parameters;
15. boundary traits;
16. requirements;
17. requirement parameters;
18. service reaches;
19. external realizations;
20. realization parameters; and
21. requirement calls.

The first fourteen tables retain the OMGRSW3 row widths and source-custody
rules. Counts fixed to zero have no rows. The exact encoded length is:

```text
128
+ 36 * 3     // units
+ 48 * 1     // imports
+ 28 * 6     // static bindings
+ 28 * 5     // declarations
+ 24 * 4     // normalized types
+ 24 * 2     // records
+ 24 * 1     // ordinary fields
+ 40 * 1     // checked-body machines
+ 40 * 1     // checked-body blocks
+ 24 * 1     // boundary traits
+ 40 * 1     // requirements
+ 24 * 1     // requirement parameters
+ 24 * 1     // service reaches
+ 48 * 1     // external realizations
+ 24 * 1     // realization parameters
+ 40 * 1     // requirement calls
= 1,064 bytes
```

Checked arithmetic precedes every offset. The computed length equals the
header length and exact EOF.

## 3. Inherited rows and exact ID map

Unit rows retain the OMGRSW1 36-byte schema. Their source IDs are 0, 1, and 2;
owners and modules equal their exact OMGCOMP2 source rows. Unit 0 owns
declarations 0 and 1, unit 1 owns declaration 2, and unit 2 owns declarations
3 and 4 plus import 0. Declaration and import spans partition in source order.

Import row 0 retains the 48-byte schema and has requester source 2, origin 1,
the exact requester-local OMGCOMP2 alias row, target package 0, module
`console`, target kind 3 (`boundary trait`), resolved declaration 0, and local
name `Console`. Its complete source span is the exact
`omega_std::console::Console` path. Missing, indirect, another-requester's,
module-only, private, duplicate, or ambiguous resolution rejects.

The inherited 28-byte declaration row adds these V6 kinds:

| Kind | Meaning | kind-table ID |
| ---: | --- | --- |
| 1 | ordinary record data | record ID |
| 2 | checked-body machine | machine ID |
| 3 | inherited pure-sum data | sum ID; absent here |
| 4 | boundary trait | trait ID |
| 5 | bodyless external realization | realization ID |

The exact declaration map is:

| Declaration ID | Source | Kind | Visibility | Identity |
| ---: | ---: | ---: | ---: | --- |
| 0 | 0 | 4 | public | `Console` -> trait 0 |
| 1 | 0 | 1 | private | `ConsoleNativeProvider` -> record 0 |
| 2 | 1 | 5 | private | `ConsoleNativeProvider::exit_process` -> realization 0 |
| 3 | 2 | 1 | private | `Main` -> record 1 |
| 4 | 2 | 2 | private | `Main::main` -> machine 0 |

Names are exact identifier-token spans. The realization and checked-body
machine remain separate declaration kinds even though both are Omega machine
declarations: one has irreducible external supply and no block, while the other
has a checked body and one entry block.

The 24-byte normalized type row retains its existing layout and adds:

| Type ID | Kind | Flags | Payload 0 | Payload 1 | Range low/high |
| ---: | ---: | ---: | ---: | ---: | --- |
| 0 | 4, nominal record | zero | record 0 | zero | zero/zero |
| 1 | 4, nominal record | zero | record 1 | zero | zero/zero |
| 2 | 8, exact unconstrained `i32` | zero | zero | zero | zero/zero |
| 3 | 9, compatibility boundary-trait value | zero | trait 0 | zero | zero/zero |

Kind 8 denotes only the complete built-in `i32` carrier used by this profile;
the zero range words do not assert a singleton range. Kind 9 is not a runtime
layout or service carrier. It exists only so field 0 can retain trait 0 as the
static receiver interface.

Record rows retain their 24-byte OMGRSW schema. Record 0 belongs to declaration
1, has nominal type 0, and has no fields. Record 1 belongs to declaration 3,
has nominal type 1, and owns field 0. Neither record is marked `[copy]`.

Field row 0 retains the 24-byte schema. It belongs to record 1 at ordinal 0,
has exact name `console`, and has normalized type 3. Its source type token has
a role-1 static binding through import 0 to trait declaration 0.

Machine row 0 and block row 0 retain their inherited 40-byte schemas. Machine
0 belongs to declaration 4, has owner record 1, mutable receiver access, Unit
result (`NO_ID`), no explicit parameters, block span `[0,1)`, and entry block
0. Block 0 is its mutable entry block, has no parameters, and retains the exact
body span containing the one admitted call statement. Selected machine 0 must
match the exact OMGCOMP2 root package, source, module, owner, machine, and this
signature.

## 4. Static bindings

The inherited static-binding row remains 28 bytes:

```text
u32  dense binding ID
u32  source ID
u8   role
u8   target kind
u16  reserved: zero
u32  exact authored reference start
u32  exact authored reference length
u32  resolved target ID in the table selected by target kind
u32  import-row ID, or NO_ID for same-package/local resolution
```

V6 target kinds are `1 = data declaration`, `2 = checked-body machine`,
`3 = boundary trait`, and `4 = trait requirement`. V6 roles are `1 = source
type`, `2 = attached owner`, `3 = requirement-call target`, `4 = exact
satisfies target`, and `5 = service-reach target`.

For target kinds 1 and 2 the target word is a declaration ID, as in the
inherited relation. For target kind 3 it is a trait ID, and for target kind 4
it is a requirement ID.

Exactly six rows occur, ordered by `(source ID, authored start, role)`:

1. source 0's `reaches Console` token, role 5, trait 0, `NO_ID` import;
2. source 1's `ConsoleNativeProvider` owner token, role 2, declaration 1,
   `NO_ID` import;
3. source 1's complete `Console::exit_process` path, role 4, requirement 0,
   `NO_ID` import;
4. source 2's field type `Console`, role 1, trait 0, import 0;
5. source 2's `Main` owner token, role 2, declaration 3, `NO_ID` import; and
6. source 2's call-target `exit_process` token, role 3, requirement 0, import 0.

The compiler-owned contextual words `linux_x64`, `Binding`, and
`CompilerIntrinsic` do not invent package declarations or static-binding rows.
Their closed meanings are validated by the rows below.

## 5. Boundary-trait and requirement rows

### Boundary-trait row -- 24 bytes

```text
u32  dense trait ID
u32  declaration ID
u32  compatibility type ID
u32  requirement-row start
u32  requirement-row count
u8   flags: bit 0 boundary; all other bits zero
u8   reserved[3]: zero
```

Trait 0 names declaration 0 and compatibility type 3, owns exact requirement
span `[0,1)`, and has the boundary flag set. Ordinary traits, generic traits,
parents, laws, defaults, associated declarations, explicit trait receivers,
and additional requirements are outside this profile.

### Requirement row -- 40 bytes

```text
u32  dense requirement ID
u32  owner trait ID
u32  ordinal within owner
u32  result type ID, or NO_ID for Unit
u32  requirement-parameter start
u32  requirement-parameter count
u32  service-reach start
u32  service-reach count
u32  requirement-name start
u32  requirement-name length
```

Requirement 0 belongs to trait 0 at ordinal 0, has exact name `exit_process`,
Unit result, parameter span `[0,1)`, and reach span `[0,1)`. It has the implicit
shared boundary-service receiver specified for a boundary requirement with no
authored receiver. That service receiver is semantic requirement metadata and
is distinct from both the app's compatibility field and provider-private state.

### Requirement-parameter row -- 24 bytes

```text
u32  dense parameter ID
u32  owner requirement ID
u32  ordinal within owner
u32  normalized type ID
u32  parameter-name start
u32  parameter-name length
```

Parameter 0 belongs to requirement 0 at ordinal 0, has exact name
`return_code`, and has normalized type 2 (`i32`).

### Service-reach row -- 24 bytes

```text
u32  dense reach ID
u32  owner requirement ID
u32  ordinal within owner
u32  target boundary-trait ID
u32  exact authored target start
u32  exact authored target length
```

Reach 0 belongs to requirement 0 at ordinal 0, targets trait 0, and spans the
exact `Console` token in the `reaches` clause. This row retains the requirement's
published service-reach ceiling; it grants no service carrier or provider
authority.

## 6. External realization and candidate binding

### External-realization row -- 48 bytes

```text
u32  dense realization ID
u32  declaration ID
u32  owner record ID
u32  applicable target: 1 = Linux x86-64 System V
u32  exact satisfied requirement ID
u32  binding kind: 1 = payload-free CompilerIntrinsic candidate
u32  result type ID, or NO_ID for Unit
u32  realization-parameter start
u32  realization-parameter count
u32  realization-name start
u32  realization-name length
u32  target-qualifier start
```

The final field points at the exact `linux_x64` token, whose length is fixed at
9 bytes and therefore is not repeated. Realization 0 belongs to declaration 2,
has owner record 0, applies to target 1, satisfies requirement 0, has Unit
result, parameter span `[0,1)`, exact name `exit_process`, and binding kind 1.
Its target equals the OMGCOMP2 target.

The exact package-qualified realization symbol is determined by package 0,
source 1's resolver-owned module, owner declaration 1, the realization name,
and its normalized signature. Its `satisfies` path resolves requirement 0
explicitly. Name coincidence, visibility, or a unique candidate cannot create
that edge.

### Realization-parameter row -- 24 bytes

```text
u32  dense parameter ID
u32  owner realization ID
u32  ordinal within owner
u32  normalized type ID
u32  parameter-name start
u32  parameter-name length
```

Parameter 0 belongs to realization 0 at ordinal 0, has exact name
`return_code`, and has normalized type 2. Requirement and realization have the
same parameter/result shape. The realization inherits requirement 0's contract,
including reach 0; it may not repeat or widen that contract.

Binding kind 1 requires the exact token sequence
`via Binding::CompilerIntrinsic;`. Parentheses, arguments, strings, payload
bytes, another binding variant, a checked body, or a missing semicolon reject.
The row retains the candidate key ingredients: exact realization symbol,
normalized signature, and applicable target. It does not assert that a sealed
toolchain catalog contains that key, that the catalog implementation refines
the requirement, that the origin package is authorized, or that the candidate
is admitted.

## 7. Requirement-call row

### Requirement-call row -- 40 bytes

```text
u32  dense call ID
u32  source ID
u32  caller checked-body machine ID
u32  owner block ID
u32  compatibility receiver field ID
u32  resolved requirement ID
u32  exact call-target token start
u32  exact call-target token length
u32  explicit argument count
u32  flags: zero
```

Call 0 occurs in source 2, machine 0, block 0. Its receiver is exact field 0,
whose compatibility type names trait 0. Its target token is exact
`exit_process`, its target is requirement 0, and it has one explicit argument.
The exact source must spell the sole argument as decimal `70`; that literal is
pure, nontrapping, and type-compatible with exact `i32` parameter 0. It creates
no separate declaration or witness type.

Computed, indexed, parenthesized, chained, parameter, scalar, array, ordinary
record, or differently typed receivers reject. A missing, overloaded,
inaccessible, differently named, wrong-arity, or wrong-typed requirement
rejects. The receiver field and target requirement are reconstructed from exact
source and compared with the row; neither is selected by the witness.

Most importantly, call 0 targets requirement 0. It does not target realization
0. A witness mutation that replaces the requirement relation with the sole
visible candidate rejects. Candidate uniqueness is not selection.

## 8. Status, publication, and negative boundary

Complete OMGCOMP2 structural validation precedes semantic resolution. Its
declared public resource excess retains status 252. Malformed framing, wrong
version/target/configuration, source-graph mismatch, unsupported source form,
syntax failure, missing/duplicate/ambiguous/private name, wrong signature,
contract mismatch, forged span or ID, wrong table order/count/extent, trailing
bytes, or any other failed V6 relation selects 251. Later inspection may not
downgrade an already selected 252.

The canonical output is always exactly 1,064 bytes, so this fixed profile adds
no independent output-exhaustion case. No byte is published until complete
source, graph, visibility, type, requirement, reach, realization, target,
binding-shape, call, root, length, and canonical-version validation succeeds.
Status 251 or 252 has empty stdout.

Required semantic controls include:

- wrong, module-only, undeclared, transitive-only, private, duplicate, or
  ambiguous trait imports;
- zero or two requirements, bodyful requirements, explicit receivers, wrong
  parameter/result shapes, or missing/extra/wrong `reaches` entries;
- missing or wrong target qualifiers, a target/envelope mismatch, bodyful
  realizations, missing/ambiguous/wrong `satisfies`, and signature drift;
- any non-payload-free or non-`CompilerIntrinsic` `via` form;
- unknown, ambiguous, wrong-arity, wrong-type, or wrong-receiver calls;
- forged declaration, type, requirement, reach, realization, call, selected
  root, source-span, magic, major, count, ordering, length, or EOF rows; and
- any `provider_defaults`, `select_provider`, second candidate, second call,
  unrelated declaration, unsupported trait surface, or unsupported source
  construct.

Source trivia remains valid only with newly reconstructed token spans; stale
span rows reject. A custody-label-only change that leaves source IDs, modules,
and bytes fixed changes no witness identity. Package, source, or declaration
reordering is outside this exact profile and rejects rather than accepting a
permuted fixed-shape witness.

## 9. Explicit non-expansion and unresolved product boundary

`OMGRSW6` proves only that one exact requirement, one applicable external
candidate, and one call to the requirement coexist in one resolved source
graph. It does not:

- select realization 0 for call 0;
- derive or validate a complete `ProviderPlan`;
- evaluate target-package defaults or `Build::select_provider`;
- accept `ConsoleNativeProvider::provider_defaults` as language semantics;
- validate sealed intrinsic-catalog membership, origin authorization,
  operational refinement, trust, or admission;
- establish `Service<Console> in Bound`, runtime service authority, provider
  state, construction, root provisioning, replacement cohorts, or installation;
- lower the call, define an intrinsic opcode or ABI, emit CKIR or ELF, or claim
  executable behavior;
- cover the six-requirement product `Console`, another target, general traits,
  general boundary traits in Delta, general overloads, general target-qualified
  machines, generics, domains, proofs, or private access between distinct
  logical modules; or
- grant a lock, accepted-closure, digest, package, resolver, artifact, or
  compilation-authority claim.

The literal product issue left outside this frozen cost relation is the runtime
carrier: bare boundary-trait values are only a transitional compatibility
fence, while destination Omega requires `Service<Console> in Bound` supplied by
installation. Provider selection and intrinsic-catalog acceptance are also
separate later conjuncts. Until those owners close those relations, OMGRSW6
cannot be extended into an executable provider call by implication.

The currently saved OMGCOMP2 custody fixture is not itself canonical OMGRSW6
input: it still contains the excluded `provider_defaults` declaration and uses
the excluded module-only import. A later gate must build a distinct OMGCOMP2
envelope over the exact V6 source profile above; this contract does not
reinterpret the existing fixture bytes.
