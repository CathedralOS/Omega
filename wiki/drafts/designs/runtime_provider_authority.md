# Runtime providers and pre-call authority

Ratified 2026-11-12 as `sealed-runtime-providers`. Task runtimes and interrupt
platform providers are ordinary packages using exact compiler-backed activation
mechanisms, contracted target instructions and admitted native boundaries.
Caller authority must be established before the action, including at the
lowest callable dangerous operation; a task claim or mask guard returned later
cannot authorize it retroactively.

The rules live in [task activation](../../spec/build/task_runtime.md#runtime-providers-and-call-authority),
[interrupt obligations](../../spec/build/interrupt_obligations.md#mask-providers-and-initial-authority),
[initial-call authority](../../spec/resources/authority.md#initial-call-authority)
and [sealed bindings](../../spec/build/foreign_bindings.md#sealed-compiler-intrinsics).
They complete the existing [owned-activation design](owned_activations.md),
not a second runtime architecture. No implementation, hardware result or fresh
compiler run is claimed here.

## Responsibilities and evidence

| Responsibility | Owner |
| --- | --- |
| Activation lowering, preservation, plan checking and control transfer | Exact compiler-supported primitives. |
| Runtime queues, executor policy, pools/capacity and task lifecycle | Ordinary selected runtime packages. |
| Interrupt mask transition, exact saved state and restoration | Ordinary platform providers using checked/admitted target operations. |
| Implementation selection and admitted external assumptions | Existing Build/provider and package-boundary machinery. |
| This caller's authority over this runtime/controller | Qualified explicit inputs, established provenance and existing access rules. |

"The compiler provides" states an implementation responsibility, not a result
already achieved. The sealed catalog recognizes core task startup requirements;
this neither supplies a scheduler nor makes any ordinary authored provider
compiler-backed. A generated activation descriptor is not execution custody,
allocation or proof that a runtime will run and settle the work.

Runtime packages use the admitted activation protocol, not private compiler
frame layouts or arbitrary continuation addresses. Necessary missing primitive
support is added for a specific operation/obligation while preserving the
surrounding package implementation. There is no blanket compiler-handled escape.

## Protecting initial calls

Startup takes an explicit runtime-authority value or borrow. Mask saving takes
an explicit controller-authority value or borrow. Their protected qualification,
provenance and instance/context relationship are checked before the operation.
Concrete carrier/domain names are library interface choices, not new keywords.
The existing runtime/controller handle should carry that role where appropriate.

Restricting which operation may issue a guard does not restrict who may call
the issuer. The dangerous entry therefore needs its own input/access obligation;
a publicly callable zero-input masking operation cannot be secured only by
limiting issuance of its output. Use `AccessPlan`, exact `Issuance::Only`
routes and checked preconditions. An unrestricted domain named Authorized,
equal fields or a linear property alone grants no protected provenance.

Every callable lower-level route needs the corresponding authority, or must be
inaccessible outside a genuinely checked restricted path. A guarded wrapper
with an unrestricted core/native primitive underneath fails this requirement.
Admitted entries may establish initial authority and checked helpers may forward
it under existing scoped delegation; there is no endless token-minting chain.

Provider selection, default/shipped status and `reaches` grant no caller
authority. Ordinary foreign/boundary admission binds actual behavior and
incoming resources to an exact contract; `Build::assume_binding_contract` is
not a routine workaround for missing proofs, bad contracts or compiler gaps.

Authority and capacity are independent. `start` establishes reservation/backing
and takes genuine custody before execution; `try_start` can reject and returns
the arguments/resources not retained. The runtime-authority argument does not
manufacture storage. Startup and lifecycle selection must agree on runtime,
activation, storage custody and settlement. A fabricated `Task` bootstrap
record or equal provider/activation numbers is not a real lifecycle claim.

## Masking and restoration

The provider must realize the mask transition under the admitted target's
instruction/native contract and connect the guard to the actual prior and
masked state. Active qualification cannot stand in for the performed operation.
Save and restore agree on representation and provenance. Restore consumes the
matching guard, honors nesting and restores the exact earlier state, including
an initially masked state; unconditional enable is insufficient.

Mask scope and CPU/activation preservation remain part of the provider contract.
Moving a guard cannot authorize restoration in an unrelated context. Existing
carry, opaque-representation, linearity and entry/exit obligations still apply.
The old core source's retired issuance spelling must migrate to the current
access-policy mechanism without treating the existing zero-input bootstrap
signature as a complete authority contract.

## Two independent delivery obligations

1. Refuse authored false compiler backing and retain exact sealed primitive
   custody. Migrate ordinary numeric/worker operations to real checked bodies
   where their contracts permit; device/runtime leaves need real providers or
   explicit unsupported status. An invented foreign symbol is no replacement.
2. Deliver executable package providers with authorized operations and coherent
   lifecycle. Rejection pins and signature/activation-plan tests alone do not
   complete this work. Do not retain false executable claims while waiting for
   the runtime to be finished.

Pure compiler tests may check declaration matching or activation plans without
running a scheduler, but may not present an unrealized provider as supported
source implementation. Root profiles can default-select ordinary packages;
there need not be a universal provider. Unsupported execution demand rejects,
never a no-op, fabricated guard or successful dummy task.

## Acceptance

- A real selected runtime package starts work and obtains its result, with
  startup and `TaskLifecycle` agreeing on exact instance/activation and custody.
- A suspension/resumption execution preserves backing, carried values, loans
  and unique execution custody. Inline completion alone does not demonstrate
  suspension support. A rejected `try_start` returns every unretained argument
  and resource without fabricating a task claim.
- A valid selected provider with no caller authority rejects. Fabricated or
  unrestricted-label authority, wrong runtime/controller/context, duplicated
  execution claims and forged task/guard fields reject. Authorized use works.
- Direct dangerous core/native/control-transfer calls cannot bypass the checked
  wrapper's authority requirement. Test each exposed low-level route rather
  than only the high-level entry.
- A valid authority handle with insufficient reservation/backing cannot make
  `start` succeed. The selected storage plan and resource behavior remain
  explicit; no ambient allocation or inferred authority supplies capacity.
- Interrupt execution on an appropriate privileged target or emulator verifies
  actual mask change, exact restoration, initially masked state and nested
  save/restore. Missing authority rejects before the initial mask; wrong guard
  representation/provenance, repeated restore and invalid context migration
  retain their rejection controls.
- Model-only tests verify bookkeeping separately from hardware emission and
  execution. A planning/typecheck receipt is never reported as a hardware pass.
- An ordinary package cannot acquire compiler backing by copying an intrinsic
  name, signature, source file or retired `CompilerIntrinsic` spelling. Exact
  core primitive recognition remains valid under updated signatures/custody.
- Audit existing `task_runtime_machine_selection_compile`,
  `task_runtime_provider_contract_narrowing`, `interrupt_table_canary`,
  `intrinsic_machine_param`, `rootless_numeric_provider_*` and
  `device_extent_access` witnesses by their actual purpose. Preserve useful
  positive behavior, add honest rejection controls and deliver real providers;
  neither wholesale rejection nor another unsupported stub finishes the runtime.
- Required source checking, Psi interpretation, native realization and foreign
  admission agree on the primitive's real meaning and exact authority/custody.
  No new keyword, special permission subsystem or assumed-binding exemption is
  introduced. This ratification changes no executable fixture or ledger.
