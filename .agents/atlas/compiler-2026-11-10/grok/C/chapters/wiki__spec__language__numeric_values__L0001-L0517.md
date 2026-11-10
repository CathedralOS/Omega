# Numeric values and bounds

Source: `wiki/spec/language/numeric_values.md` lines 1–517 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
NUMERIC_VALUES1 | "A scalar carrier does not accept a separate bracketed range annotation." | Numeric values and bounds | Reject a separate bracketed range annotation on a scalar carrier | not stated | 5
NUMERIC_VALUES2 | "Explicit nominal qualification follows [domains](domains.md#declaration-and-membership); internal interval analysis does not introduce another authored type form." | Numeric values and bounds | Interval analysis must not introduce a new authored type form; nominal qualification must follow the domains rules | not stated | 5
NUMERIC_VALUES3 | "`min`, `max`, `abs`, `clamp` and `sqrt` are ordinary standard-library machines, not language functions or compiler-known source builtins." | Library numeric helpers | These spellings must resolve only as ordinary library machines, not as compiler-known builtins | at a call | 3
NUMERIC_VALUES4 | "A missing library declaration is a name-resolution failure, not permission to recognize its spelling in the compiler." | Library numeric helpers | Without a library declaration, the compiler must fail name resolution and must not recognize the spelling specially | at a call | 3
NUMERIC_VALUES5 | "Checking learns result bounds and mathematical meaning from those contracts and checked bodies, not name-specific proof rules or source-call expansions." | Library numeric helpers | Type and proof checking must use declared contracts and bodies, not name-specific rules or source expansions | at a call | 8
NUMERIC_VALUES6 | "in particular `abs(x)` is not universally `max(x, 0 - x)`." | Library numeric helpers | The compiler must not treat `abs(x)` as universally equivalent to `max(x, 0 - x)` | not stated | 5
NUMERIC_VALUES7 | "An optional optimization may realize a checked operation with a suitable instruction, but does not make any of these five names intrinsic or replace the ordinary call path." | Library numeric helpers | Instruction optimizations must not make the five names intrinsic or bypass the ordinary call path | not stated | 9
NUMERIC_VALUES8 | "No reserved-name, qualified-name or implicit-import compatibility route preserves the rejected builtins." | Library numeric helpers | Reject compatibility routes that preserve rejected builtin recognition | not stated | 3
NUMERIC_VALUES9 | "Library placement does not authorize a dependency from `core` to `std`." | Library numeric helpers | `core` must not depend on `std` because library helpers live in `std` | not stated | 9
NUMERIC_VALUES10 | "The compiler supports the general type/refinement, evaluation, representation and contract machinery; it does not recognize API names, replace calls with unproved facts, or search for implicit providers." | Core integer and float APIs | Must not special-case core API names, substitute unproved facts for calls, or search implicit providers | at a call | 3
NUMERIC_VALUES11 | "Physical layout size is not semantic integer width." | Core integer and float APIs | Semantic integer width must not be equated with physical layout size | not stated | 5
NUMERIC_VALUES12 | "`u32::MAX`, `i32::Maximum`, and similar spellings are not aliases, keywords, or a supported primitive-member lookup route." | Core integer and float APIs | Reject primitive-member or alias spellings for carrier extrema | at the operation | 1
NUMERIC_VALUES13 | "Unsupported types reject." | Core integer and float APIs | Carrier query calls on unsupported types must be rejected | at a call | 5
NUMERIC_VALUES14 | "They do not solve the maximum of an arbitrary domain, mint qualifications, or assign extrema to unbounded mathematical integers." | Core integer and float APIs | Carrier queries must not be used to solve arbitrary-domain maxima, mint qualifications, or assign extrema to unbounded math integers | at a call | 8
NUMERIC_VALUES15 | "Arithmetic policy does not redefine these carrier bounds." | Integer carrier queries | Overflow/arithmetic policy must not change carrier endpoint bounds | not stated | 5
NUMERIC_VALUES16 | "A constrained or routed type does not inherit a constructor merely because its underlying carrier has those endpoints." | Integer carrier queries | A constrained or routed type must not gain endpoint constructors solely from underlying carrier bounds | not stated | 1
NUMERIC_VALUES17 | "`float::min<T>()` and `float::max<T>()` return the least and greatest finite values of `T`, not infinities." | Floating values and format queries | `float::min`/`max` results must be finite endpoints, not infinities | at a call | 5
NUMERIC_VALUES18 | "In particular, `min` is most-negative finite, never a smallest-positive value." | Floating values and format queries | `float::min` must not be interpreted as smallest positive normal | at a call | 5
NUMERIC_VALUES19 | "`float::epsilon<T>()` … Not a universal comparison tolerance." | Floating values and format queries | `epsilon` must not be treated as a universal comparison tolerance in checking | not stated | 5
NUMERIC_VALUES20 | "They preserve exact presented representation bits, including signed zeros, NaN sign, payload, and quiet/signaling state." | Floating representation and classification | Bit wrappers must preserve exact representation bits | at the operation | 5
NUMERIC_VALUES21 | "They perform no numeric conversion, quieting, canonicalization, or qualification establishment." | Floating representation and classification | Bit wrappers must not convert, quiet, canonicalize, or establish qualifications | at the operation | 5
NUMERIC_VALUES22 | "`float::classify<T>(value)` returns ordinary case-bearing library data with `Zero`, `Subnormal`, `Normal`, `Infinity`, and `NaN` alternatives." | Floating representation and classification | Classification must return the stated case set | at a call | 5
NUMERIC_VALUES23 | "Classification and convenience queries … inspect representation without an arithmetic operation that could quiet a signaling NaN." | Floating representation and classification | NaN/sign queries must not use arithmetic that could quiet signaling NaN | at the operation | 5
NUMERIC_VALUES24 | "Only the low 22 bits for `f32` or 51 bits for `f64` may be present; signaling payloads must be nonzero so the result is not infinity." | Floating representation and classification | NaN constructors must enforce payload width and nonzero signaling payload | at a call | 5
NUMERIC_VALUES25 | "Calls prove those requirements or use an ordinary explicitly failure-returning adapter; constructors never silently truncate." | Floating representation and classification | Reject or use explicit failure adapter when payload invalid; no silent truncation | at a call | 3
NUMERIC_VALUES26 | "Exact bit composition and its validity/access obligations remain the foundation." | Floating representation and classification | NaN bit construction must satisfy validity and access obligations | at the operation | 5
NUMERIC_VALUES27 | "Arithmetic may produce any representation allowed by its existing contract; proof-only `FloatMeaning` still erases NaN payload and sign." | Floating representation and classification | Arithmetic checking must not strengthen NaN guarantees beyond existing contracts; FloatMeaning erases payload/sign in proof | not stated | 8
NUMERIC_VALUES28 | "A compile-time observation of computed possibly-NaN bits still requires non-NaN proof, explicit canonicalization, or an exact-bit refinement." | Floating representation and classification | Compile-time raw NaN-bit observation requires non-NaN proof, canonicalization, or exact-bit refinement | not stated | 8
NUMERIC_VALUES29 | "A wrapper cannot launder that dependency into cross-target reproducibility." | Floating representation and classification | Wrappers must not remove the compile-time NaN-bit proof dependency for cross-target reproducibility | not stated | 9
NUMERIC_VALUES30 | "Hardware exceptions remain masked; signaling-state APIs do not promise an ambient hardware trap or replace Omega's result-based `Trapping` policy." | Floating representation and classification | Signaling APIs must not imply unmasked hardware traps or replace result-based Trapping | not stated | 10
NUMERIC_VALUES31 | "An incompatible destination rejects rather than erasing the suffix." | Literal landing and destinations | Literal with suffix landing at incompatible destination must reject, not strip suffix | at the operation | 5
NUMERIC_VALUES32 | "Anonymous numeric arguments land at the selected explicit parameter; an implicit receiver does not shift that pairing." | Literal landing and destinations | Anonymous numeric args pair with explicit parameters only; implicit receiver must not redirect landing | at a call | 3
NUMERIC_VALUES33 | "Landing selects a carrier and never a domain." | Literal landing and destinations | Literal landing must choose a carrier type, not a domain qualification | at the operation | 5
NUMERIC_VALUES34 | "Anonymous integer-literal division is rational division, not truncation: the destination checks or rounds the completed value, never selects its intermediate operations." | Literal landing and destinations | Anonymous `/` is rational division; truncation must not be selected as intermediate; destination checks completed value | at the operation | 5
NUMERIC_VALUES35 | "Float landing rounds once to the selected format; integer landing requires an integral in-range result and explicit `as` when acknowledging fractional intermediates, as specified below." | Literal landing and destinations | Enforce single float round; integer landing requires integral in-range or explicit `as` per fractional rules | at the operation | 5
NUMERIC_VALUES36 | "Mixing an already-typed operand with an anonymous operand first lands the latter for that typed operation." | Literal landing and destinations | When mixing typed and anonymous operands, land anonymous operand for that operation first | at the operation | 5
NUMERIC_VALUES37 | "This introduces no runtime rational arithmetic." | Literal landing and destinations | Rational intermediates must not introduce runtime rational evaluator | not stated | 5
NUMERIC_VALUES38 | "Landed `f32` and `f64` values do not implicitly change format at scalar destinations." | Literal landing and destinations | Reject or avoid implicit f32/f64 format changes at scalar destinations | on every edge | 5
NUMERIC_VALUES39 | "A binary result uses the selected operator's result type, not a guess from operand formats." | Literal landing and destinations | Binary operation result type comes from selected operator, not operand format inference | at the operation | 5
NUMERIC_VALUES40 | "Builtin remainder requires integer operand meaning before folding or destination checking." | Literal landing and destinations | `%` folding and destination checks require integer operand meaning first | before folding; at the operation | 5
NUMERIC_VALUES41 | "A wholly anonymous numeric tree does not acquire that meaning from a destination, including when an intermediate division is fractional." | Literal landing and destinations | Anonymous trees must not gain integer meaning from destination context | not stated | 5
NUMERIC_VALUES42 | "constant folding, and evaluation during compilation do not move the landing boundary." | Literal landing and destinations | Folds and compile-time evaluation must not relocate landing boundary | not stated | 5
NUMERIC_VALUES43 | "No fold may make a landed value anonymous again." | Literal landing and destinations | Constant folding must not erase landed typing back to anonymous | not stated | 5
NUMERIC_VALUES44 | "Anonymous division by zero has no numeric value, including in proofs or comparisons." | Literal landing and destinations | Anonymous rational division by zero must have no defined numeric value in checking or proofs | at the operation | 5
NUMERIC_VALUES45 | "Nor can a Saturating, Wrapping, or Trapping destination truncate an anonymous fraction or repair an out-of-range initial value." | Literal landing and destinations | Policy destinations must not truncate anonymous fractions or repair out-of-range anonymous values | at the operation | 5
NUMERIC_VALUES46 | "Integer landing of a fractional final value rejects with its exact value and destination type." | Fractional-intermediate diagnostics | Reject implicit integer landing when final value is fractional; report exact value and destination | at the operation | 5
NUMERIC_VALUES47 | "An authored anonymous calculation with a fractional intermediate rejects at an implicit integer landing, even when its final value is integral and in range." | Fractional-intermediate diagnostics | Reject when fractional intermediate exists in anonymous tree even if final value is integral in-range | at the operation | 5
NUMERIC_VALUES48 | "A destination annotation alone does not acknowledge the fractional intermediate." | Fractional-intermediate diagnostics | Destination type annotation must not waive fractional-intermediate acknowledgement | not stated | 5
NUMERIC_VALUES49 | "`as T` applied to an anonymous expression, with an integer destination `T`, explicitly lands its complete operand." | Fractional-intermediate diagnostics | Integer `as` on anonymous expression lands entire operand per exact-arithmetic then integrality rules | at the operation | 5
NUMERIC_VALUES50 | "It cannot repair division by zero, a fractional final value or an out-of-range result." | Fractional-intermediate diagnostics | `as` must not fix div-by-zero, fractional final, or out-of-range | at the operation | 5
NUMERIC_VALUES51 | "The cast neither truncates nor retroactively types them." | Fractional-intermediate diagnostics | `as` must not truncate fractional intermediates or retroactively type them as integer division | at the operation | 5
NUMERIC_VALUES52 | "An acknowledgement covers only the cast's operand." | Fractional-intermediate diagnostics | Fractional acknowledgement applies only to the direct `as` operand | at the operation | 5
NUMERIC_VALUES53 | "`as` binds more tightly than multiplication: `(5 / 2) * 2 as u32` casts only `2`, so multiplication requires `5/2` to land as `u32` and rejects." | Fractional-intermediate diagnostics | Enforce `as` precedence vs multiplication for landing boundaries | at the operation | 5
NUMERIC_VALUES54 | "A cast around an enclosing result does not bypass earlier landing boundaries at machine arguments or typed operations, nor make already-typed values anonymous again." | Fractional-intermediate diagnostics | Outer `as` must not bypass earlier landing sites or re-anonymize typed values | at the operation | 5
NUMERIC_VALUES55 | "The trigger is an actual fractional intermediate, not comparison with a hypothetical truncating evaluation." | Fractional-intermediate diagnostics | Diagnostic triggers only on actual fractional intermediate, not hypothetical truncation | at the operation | 5
NUMERIC_VALUES56 | "cancellation cannot erase the acknowledgement requirement." | Fractional-intermediate diagnostics | Algebraic cancellation must not remove fractional-intermediate rejection requirement | not stated | 5
NUMERIC_VALUES57 | "Decimal spelling grants no exemption; `5` and `5.0` denote the same number." | Fractional-intermediate diagnostics | Decimal suffix must not exempt from fractional-intermediate rules | at the operation | 5
NUMERIC_VALUES58 | "This is an admission error, with no warning-suppression escape." | Fractional-intermediate diagnostics | Fractional-intermediate violations are hard errors without suppressible warnings | not stated | 5
NUMERIC_VALUES59 | "Matching a token is insufficient to use builtin arithmetic or order laws." | Arithmetic and comparisons | Operator token match alone must not select builtin arithmetic or order laws | at the operation | 5
NUMERIC_VALUES60 | "A negated guard retains the original operator selection before deriving a consequence; integer order complements are not IEEE float laws." | Arithmetic and comparisons | Negated guards must preserve original operator selection; integer complements must not use float IEEE laws | at the operation | 5
NUMERIC_VALUES61 | "An early const-argument normalizer retains an application whose operator selection is unresolved rather than folding it with the token's builtin meaning." | Arithmetic and comparisons | Unresolved operator selection must block premature fold using token builtin meaning | not stated | 5
NUMERIC_VALUES62 | "Every Exact operation owes its own representability proof in its actual carrier, including unsigned values beyond a signed analysis window." | Arithmetic and comparisons | Each Exact operation requires representability proof in its actual carrier | at the operation | 5
NUMERIC_VALUES63 | "A safe final result, mathematical substitution, or unknown analysis endpoint cannot excuse an unsafe intermediate." | Arithmetic and comparisons | Unsafe intermediates must not be excused by safe final result or math substitution | at the operation | 5
NUMERIC_VALUES64 | "There is no implicit widening: arithmetic in `u8` must fit `u8` under Exact; use an explicit exact conversion to request a wider carrier." | Arithmetic and comparisons | Reject implicit widening; Exact arithmetic must fit operand carrier | at the operation | 5
NUMERIC_VALUES65 | "Each node produces its declared-width policy result before its parent consumes it." | Arithmetic and comparisons | Policy reduction applies at each node before parent use | at the operation | 5
NUMERIC_VALUES66 | "A proved overflow in an already-landed Trapping operation is still a runtime trap, not an Exact-style compile error;" | Arithmetic and comparisons | Proved overflow on landed Trapping op is runtime trap, not compile-time Exact error | at the operation | 5
NUMERIC_VALUES67 | "A bare binding cannot receive a Wrapping value without `as -Wrapping`." | Arithmetic and comparisons | Storing Wrapping-qualified value in bare binding requires `as -Wrapping` | at the operation | 5
NUMERIC_VALUES68 | "After that removal, Exact operations act on the actual stored value, not a mathematical pre-wrap result." | Arithmetic and comparisons | After policy removal, Exact ops use stored value not pre-wrap mathematical value | at the operation | 5
NUMERIC_VALUES69 | "A machine naming no arithmetic policy publishes Exact; a caller-selectable policy must occur in its contract." | Arithmetic and comparisons | Default policy is Exact when unspecified; selectable policy must be in contract | not stated | 3
NUMERIC_VALUES70 | "Omission does not choose a weaker policy, and incompatible policies do not mix implicitly." | Arithmetic and comparisons | Must not default to weaker policy or mix incompatible policies implicitly | not stated | 5
NUMERIC_VALUES71 | "A [policy operator](#policy-operators) selects its policy for that one operation and leaves its operands and result unqualified." | Arithmetic and comparisons | Suffixed operator applies policy only at that node; result and operands stay unqualified | at the operation | 5
NUMERIC_VALUES72 | "They require no common operand carrier, implicit cast, or representability of either value in the other operand's carrier." | Integer equality and order | Integer comparisons must not require common carrier, implicit cast, or cross-carrier representability | at the operation | 5
NUMERIC_VALUES73 | "Register width, bit patterns, padding, sign/zero extension, and host-language promotion rules do not define comparison meaning." | Integer equality and order | Comparison meaning must not follow target register/promotion rules | not stated | 5
NUMERIC_VALUES74 | "In particular, a negative signed value remains negative when compared with an unsigned value." | Integer equality and order | Mixed signed/unsigned compare uses mathematical integer values preserving signed negativity | at the operation | 5
NUMERIC_VALUES75 | "This adds neither implicit arithmetic widening nor conversion at assignments, arguments, or returns." | Integer equality and order | Mixed-integer comparison rules must not add implicit widening at assign/arg/return | at the operation | 5
NUMERIC_VALUES76 | "a qualified operand uses a comparison declared for its exact type or is cast first." | Integer equality and order | Qualified operands must use matching comparison declaration or cast first | at the operation | 5
NUMERIC_VALUES77 | "It does not override a selected domain/user operation or grant mixed float, address, or Boolean comparisons." | Integer equality and order | Builtin integer compare must not override user/domain ops or allow mixed float/address/bool | at the operation | 5
NUMERIC_VALUES78 | "Contracts, constant evaluation, checked and Terminal interpretation, optimization, and native execution must use this same relation." | Integer equality and order | All pipeline stages must implement the same integer comparison relation | not stated | SHARED
NUMERIC_VALUES79 | "Terminal normalization and target realization implement it; neither may substitute an unsigned promotion or reject source operands merely because their integer carriers differ." | Integer equality and order | Terminal/native must not use unsigned promotion shortcut or reject differing carriers | not stated | 9
NUMERIC_VALUES80 | "For a signed index, `index < length` establishes only an upper bound: negative integers also satisfy it when `length` is nonnegative." | Integer equality and order | `index < length` must not be treated as proving nonnegative index | not stated | 6
NUMERIC_VALUES81 | "An overflow policy does not invent a result for undefined operands such as an integer zero divisor." | Overflow policies | Overflow policies must not fabricate results for undefined operands like divide by zero | at the operation | 5
NUMERIC_VALUES82 | "A type may carry any number of value qualifications but at most one arithmetic policy." | Overflow policies | Reject more than one arithmetic policy on a type | not stated | 5
NUMERIC_VALUES83 | "the [observation profile](../terminal-psi/observations.md) records that site in its operation-crash group with the exact primitive denotation, never as a fabricated edge or boundary route." | Overflow policies | Terminal must record Trapping site with exact primitive denotation, not fabricated edge | not stated | 9
NUMERIC_VALUES84 | "a conversion traps when the value is not representable." | Overflow policies | Trapping conversion when value not representable in target carrier | at the operation | 5
NUMERIC_VALUES85 | "Checking, verification, interpretation, and realization preserve the operation's exact denotation, cause, and path-conditioned site." | Overflow policies | All stages must preserve Trapping denotation, cause, and path-conditioned trap site | not stated | SHARED
NUMERIC_VALUES86 | "Both operands have the same fixed-integer carrier under the Exact operator's operand rules; a shift's count keeps its own." | Policy operators | Suffixed arithmetic requires same fixed-integer carrier for value operands; shift count separate | at the operation | 5
NUMERIC_VALUES87 | "At least one operand must already be integer-typed and the other lands to that carrier." | Policy operators | Reject suffixed op when neither operand is integer-typed and other cannot land | at the operation | 5
NUMERIC_VALUES88 | "A wholly anonymous tree rejects, because it has no carrier to wrap, clamp or trap at." | Policy operators | Wholly anonymous operand tree for suffixed operator must reject | at the operation | 5
NUMERIC_VALUES89 | "A suffixed operator neither consults nor strips a domain, so an operand carrying any domain is first cast, as in `(distance as -Km) +% step`." | Policy operators | Domain-carrying operands must cast before suffixed operator; suffixed op ignores domain on operands | at the operation | 5
NUMERIC_VALUES90 | "The result is the bare carrier with no policy qualification, so the next operation is Exact unless it names a policy itself." | Policy operators | Suffixed operator result is unqualified bare carrier; following ops default Exact | at the operation | 5
NUMERIC_VALUES91 | "A `%` or `|` form is total over its overflow case and adds no crash site." | Policy operators | Wrapping/Saturating `%` and `|` forms must not add crash sites beyond overflow totality | at the operation | 5
NUMERIC_VALUES92 | "Like its policy, it does not license a zero divisor, and a `|` shift still owes the Exact count proof." | Policy operators | Suffixed `%`/`|` still require nonzero divisor; `|` shift requires Exact count proof | at the operation | 5
NUMERIC_VALUES93 | "A `!` form is a Trapping operation." | Policy operators | Suffixed `!` operators are Trapping with trap site and crash contract | at the operation | 5
NUMERIC_VALUES94 | "Path facts that disprove the predicate remove the route." | Policy operators | Proved false trap predicate removes crash route | on every edge | 3
NUMERIC_VALUES95 | "On normal return the result equals the exact mathematical result of the unsuffixed operation, and that equality is an established fact about the result." | Policy operators | When trap avoided, result equals unsuffixed exact math result as established fact | at normal return | 5
NUMERIC_VALUES96 | "The suffixed forms are compiler-owned integer primitives." | Policy operators | Suffixed forms are language primitives not user-declarable operators | not stated | 5
NUMERIC_VALUES97 | "A `machine` declaration cannot bind one, floats have no suffixed forms, and no compound-assignment spelling is introduced." | Policy operators | Reject machine binding of suffixed ops; no float suffixed forms; no compound assignment spellings | not stated | 5
NUMERIC_VALUES98 | "An operand that also carries another domain, such as `u32 in Km & Wrapping`, matches none of them." | Policy domains | Operand with multiple domains including policy must not match core policy operator declarations | at the operation | 5
NUMERIC_VALUES99 | "A policy domain is added and removed only by a [domain cast](domains.md#domain-casts), like every other domain." | Policy domains | Policy qualification changes only via domain cast | at the operation | 5
NUMERIC_VALUES100 | "Builtin `%` needs at least one already-integer-typed operand." | Integer quotient and remainder | `%` requires at least one integer-typed operand | at the operation | 5
NUMERIC_VALUES101 | "The other operand lands to the required integer type; a fractional operand rejects." | Integer quotient and remainder | Second operand lands to integer type; fractional rejects | at the operation | 5
NUMERIC_VALUES102 | "A destination annotation, proof context, positive operands, or known zero remainder cannot type an anonymous `%`." | Integer quotient and remainder | Anonymous `%` must not be typed by destination, proof context, or operand signs alone | at the operation | 5
NUMERIC_VALUES103 | "Both operations require a nonzero divisor." | Integer quotient and remainder | Division and remainder require nonzero divisor | at the operation | 5
NUMERIC_VALUES104 | "Under Exact, signed `MIN / -1` and `MIN % -1` both reject because the common quotient is unrepresentable." | Integer quotient and remainder | Exact `MIN / -1` and `MIN % -1` must reject | at the operation | 5
NUMERIC_VALUES105 | "Wrapping returns quotient `MIN`, Saturating returns `MAX`, and both return remainder zero." | Integer quotient and remainder | Wrapping/Saturating `MIN/-1` behavior must match stated quotient/remainder | at the operation | 5
NUMERIC_VALUES106 | "Neither policy licenses a zero divisor." | Integer quotient and remainder | Wrapping/Saturating do not allow zero divisor | at the operation | 5
NUMERIC_VALUES107 | "There is no context-selected second meaning of `%`, or anonymous Euclidean-remainder warning: the untyped operation rejects." | Integer quotient and remainder | Reject untyped `%` and context-selected Euclidean alternate | at the operation | 5
NUMERIC_VALUES108 | "Exact | Prove `0 <= count < value_width` before formation." | Shift counts | Exact shifts require proved count in `0..width` before formation | before formation | 5
NUMERIC_VALUES109 | "Trapping | An out-of-range count is an executable trap condition." | Shift counts | Trapping shifts trap on out-of-range count | at the operation | 5
NUMERIC_VALUES110 | "Exact left-shift result representability is a separate obligation." | Shift counts | Exact left shift needs separate result representability proof | at the operation | 5
NUMERIC_VALUES111 | "Hardware masking cannot discharge the Exact count proof or silently change its meaning." | Shift counts | Hardware shift masking must not satisfy or alter Exact count proof obligations | not stated | 9
NUMERIC_VALUES112 | "A right shift cannot overflow its value, so `>>|` means the same as `>>`; it exists to keep the suffix set uniform." | Shift counts | `>>|` behavior equals `>>` for value overflow purposes | at the operation | 5
NUMERIC_VALUES113 | "Integer bitwise AND, OR, XOR, and complement preserve their signed/unsigned carrier and are total at its width;" | Shift counts | Bitwise ops preserve carrier and are total at carrier width | at the operation | 5
NUMERIC_VALUES114 | "Bitwise operations themselves add no overflow-policy requirement;" | Shift counts | Bitwise ops must not impose overflow-policy requirements | at the operation | 5
NUMERIC_VALUES115 | "Boolean negation likewise does not skip validation of operands." | Shift counts | Boolean negation must validate operands | at the operation | 0
NUMERIC_VALUES116 | "Evaluating interval endpoints alone is not a sound bound for AND, OR, or XOR." | Shift counts | Interval endpoint analysis must not be used as sole bound for bitwise ops | not stated | 8
NUMERIC_VALUES117 | "Fields and case payloads contribute their interval relationships through their owning `requires`;" | Numeric values and bounds | Field and case payload interval facts must come from owning `requires`, not separate scalar annotations | not stated | 1
NUMERIC_VALUES118 | "parameters use `requires`; results use `ensures`; locals use established flow facts." | Numeric values and bounds | Parameter/result/local numeric bounds use the stated contract and flow mechanisms | not stated | 0
NUMERIC_VALUES119 | "For width `w`, unsigned bounds are `0` through `2^w - 1`; signed bounds are `-2^(w-1)` through `2^(w-1) - 1`." | Integer carrier queries | Fixed-integer carrier bounds must match the width formulas for signed and unsigned | not stated | 5
NUMERIC_VALUES120 | "`integer::width_bits<T>()` | `u64` | Semantic fixed integer width, not stored field width or an ABI register size." | Integer carrier queries | `width_bits` must report semantic width, not layout or ABI register size | at a call | 5
NUMERIC_VALUES121 | "Wrapping reduces representable-range overflow modulo the selected carrier;" | Overflow policies | Wrapping overflow must reduce modulo carrier range | at the operation | 5
NUMERIC_VALUES122 | "Saturating clamps it to carrier bounds;" | Overflow policies | Saturating overflow must clamp to carrier bounds | at the operation | 5
NUMERIC_VALUES123 | "Trapping uses the primitive's exact crash predicate." | Overflow policies | Trapping must use the primitive's exact crash predicate | at the operation | 5
NUMERIC_VALUES124 | "Terminal carries it as one `TrappingInteger` operation per primitive (add, subtract, multiply, divide, remainder, shift left, shift right, conversion) that is its own `Trap` site;" | Overflow policies | Each listed Trapping primitive is its own Terminal TrappingInteger Trap site | not stated | 9
NUMERIC_VALUES125 | "the trap predicates are the settled integer-policy catalog rows: result outside the carrier, a zero divisor or signed `MIN / -1` (including `MIN % -1`), and a shift count outside `0..width`;" | Overflow policies | Trap predicates must match catalog: out of range result, zero divisor, MIN/-1, shift count out of 0..width | at the operation | 5
NUMERIC_VALUES126 | "The bare operator stays Exact:" | Policy operators | Unsuffixed integer arithmetic/shift operators remain Exact policy | at the operation | 5
NUMERIC_VALUES127 | "Both operands are unqualified, as for every builtin operator under the [operand rule](domains.md#operands)." | Policy operators | Suffixed operator operands must be unqualified per operand rule | at the operation | 5
NUMERIC_VALUES128 | "It contributes that route to its machine's [crash contract](effects.md#guarded-crashes) like any other Trapping operation." | Policy operators | Trapping suffixed ops must contribute crash route to machine crash contract | at the operation | 3
NUMERIC_VALUES129 | "`Trapping`'s operators publish `crashes Trap`." | Policy domains | Trapping domain operators must publish `crashes Trap` | not stated | 3
NUMERIC_VALUES130 | "`counter + 1` needs no cast." | Policy domains | Core policy-domain operator declarations must allow plain carrier operand without cast where stated | at the operation | 5
NUMERIC_VALUES131 | "`/!` and `%!` trap on a zero divisor and on signed `MIN / -1` or `MIN % -1`." | Integer quotient and remainder | Trapping divide/remainder must trap on zero divisor and signed MIN/-1 cases | at the operation | 5
NUMERIC_VALUES132 | "all four still owe a nonzero-divisor proof." | Integer quotient and remainder | Wrapping/Saturating MIN/-1 forms still require nonzero-divisor proof | at the operation | 5
NUMERIC_VALUES133 | "The suffixed shifts take the matching row: `<<%` and `>>%` reduce the count, `<<|` and `>>|` keep the Exact count proof, and `<<!` and `>>!` trap on an out-of-range count." | Shift counts | Suffixed shifts must apply the policy row matching their suffix for count handling | at the operation | 5
NUMERIC_VALUES134 | "They do not acquire Boolean bounds." | Shift counts | Bitwise integer ops must not acquire Boolean interval bounds | not stated | 5

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Numeric bounds are propositions established by data-level `requires`, callable contracts, intentionally named domains, and ordinary flow reasoning." | The compiler must derive numeric bounds only through those mechanisms, rejecting other bound sources. | The sentence describes how bounds arise in the language model without stating what the compiler must reject when a bound is missing. |
"Fields and case payloads contribute their interval relationships through their owning `requires`;" | The compiler must not accept interval facts on fields/payloads except via owning `requires`. | Fields and payloads may also gain bounds from flow; "contribute through" does not exclude other established facts. |
"Checking learns result bounds and mathematical meaning from those contracts and checked bodies, not name-specific proof rules or source-call expansions." | The compiler is forbidden from implementing special-case proof rules for these library names. | Library bodies might still be partially evaluated by generic constant folding; "source-call expansions" might mean only macro-like spelling expansion, not ordinary body checking. |
"The standard library must make signed-minimum `abs`, inverted clamp bounds, floating NaNs and signed zeros, and square-root domain and rounding behavior explicit in its contracts." | Programs calling these without satisfying explicit library preconditions must fail checking per those contracts. | This is a library authoring requirement; the compiler enforces only what is actually declared in shipped `std` contracts. |
"These zero-value-argument carrier queries are distinct from two-value comparison helpers such as `max(left, right)` above." | Name resolution must distinguish zero-arg `max<T>()` from binary `max`. | Distinctness is semantic only; overload resolution rules elsewhere decide which applies. |
"Unsupported types reject." | Any carrier query with type argument outside admitted carriers/formats is a compile error. | "Unsupported" might mean only types without declarations yet, not all non-admitted types in every compilation mode. |
"Format properties are library queries with these exact values." | Checker and evaluator must treat listed `f32`/`f64` query results as matching the tabulated constants. | Tables are ratified library semantics; compiler checks calls against library contracts that may encode the same values indirectly. |
"They are checked wrappers around [representation views](../layouts/recasts.md) and scalar reads," | Violations of recast/view rules apply equally to these wrappers. | Wrappers might admit additional syntactic sugar beyond raw recast rules without stating equivalence in every case. |
"Future unsigned narrow payload carriers may make them unnecessary; this decision does not admit those carriers or automatically deprecate the APIs." | The compiler must not introduce narrow payload carriers or deprecate APIs based on this chapter alone. | Future language decisions might still admit carriers without contradicting this sentence's scope ("this decision"). |
"Anonymous fixed-integer expressions retain exact rational intermediates until landing requires an integral, in-range value." | Rational form is retained in the checker until an integral landing site forces proof. | Retention might be representation-only in constant folding while surface typing already shows a carrier. |
"A contract comparison obtains its destination from a typed operand or the owning callable's declared result, not a verifier guess." | The compiler must not infer comparison landing from heuristics beyond typed operand or declared result. | "Contract comparison" might name a specific construct; other comparisons might use different destination rules. |
"Retention does not establish support for later const evaluation." | Retaining an unresolved application must not imply the expression is a valid constexpr candidate later. | Retention is a parsing/normalization fact only; const evaluation might still succeed when operator selection later resolves. |
"Wrapping loop updates need an independent no-wrap proof before monotonicity can be inferred; the proposed descent cannot supply its own premise." | Loop proof inference must require separate no-wrap evidence, not the descent relation itself. | Applies only when inferring monotonicity through wrapping updates, not to all loop proofs. |
"The generic spelling for that choice remains unsettled." | Until settled, compiler must not accept a generic caller-selectable arithmetic policy spelling. | Unsettled spelling is a documentation gap; an implementation might still reject unknown spellings without naming the future form. |
"The comparison itself is total for admitted integer operands; forming either operand still owes its own operation and validity obligations." | Comparison is defined for all admitted pairs; operand formation errors are separate from comparison totality. | "Total" might mean the relation is never undefined, while some operand pairs might still be rejected before comparison forms. |
"Arithmetic policies govern computation of the operands, not a reinterpretation of their resulting values when builtin integer comparison is selected." | Policy must not change compared values after operand evaluation for builtin integer compare. | Policies on operands might still affect which values are produced before compare in ways not labeled "reinterpretation." |
"Which operator a qualified operand selects follows [exact operator matching](domains.md#semantic-roles-and-operators)." | Operator resolution for qualified operands is entirely delegated to domains exact matching rules. | This sentence links policy to domains without stating precedence when builtin and domain operators conflict. |
"Use a suffixed operator when one operation needs a policy." | Style guidance only; qualified bindings remain optional for multi-operation policy. | Using suffixed operator might be required when result must stay unqualified while one op uses policy. |
"A `Trapping` qualification in particular says nothing about the value it qualifies: a returned Trapping result is the exact value." | Trapping qualification must not be read as a value bound or partial correctness fact on the qualified value. | Return type might still carry crash obligations separate from the numeric value fact. |
"Compiler and library support is pending **OPERATION-ARITHMETIC-POLICY** in [TASKS.md](../../../TASKS.md)." | Features described may be rejected until implementation lands; behavior in this chapter is normative when implemented. | Pending status might mean only library surface is incomplete while compiler primitives already enforce suffixed forms. |
"This includes builtin proof `Int`, whose quotient truncates toward zero with no width bound." | Proof `Int` division follows truncate-toward-zero rules without fixed-width overflow policy. | "No width bound" might mean no carrier width, not that overflow checks are absent in mixed proof/runtime contexts. |
"The mathematical equation does not itself prove the safety of its authored machine intermediates or replace overflow-policy behavior." |quotient/remainder identities must not discharge intermediate safety or policy obligations | The equation is normative for q,r meaning only when divisor and policy preconditions already hold. |
"A separately named Euclidean modulo operation remains deferred with no selected spelling." | No Euclidean modulo spelling may be accepted until separately specified. | Deferred operation might still exist under another name not listed here. |
"Exact | Prove `0 <= count < value_width` before formation." | Formation of Exact shift must be rejected without proof. | "Before formation" might mean before typing the shift node or before lowering to Terminal. |
"Wrapping | Reduce signed or unsigned count by Euclidean modulo `value_width`." | Negative counts wrap by Euclidean modulo on width. | "Reduce" might apply only at runtime realization while compile-time folds use a different rule unless stated elsewhere. |
"Saturating | Prove `0 <= count < value_width`; saturation defines value overflow, not invalid counts." | Saturating shifts require Exact-style valid count; invalid counts are not saturated to endpoints. | Invalid counts might trap or reject rather than saturate; sentence distinguishes saturation of value overflow only. |

## Terms used without definition in this chapter

- numeric bounds / interval relationships / interval analysis
- scalar carrier / bracketed range annotation
- data-level `requires` / owning `requires` / flow facts
- intentionally named domains
- executable-numeric-builtins / builtin route
- standard-library machines / language functions / compiler-known source builtins
- `std`'s math surface / declaration identity
- signed-minimum `abs` / inverted clamp bounds
- admitted target realizations
- core-integer-float-apis
- carrier queries / carrier-specific values
- authoritative semantic carrier/format metadata
- exact type-equality refinement
- unqualified fixed integer carriers
- permanent `f32`/`f64` formats
- arbitrary-width integer families
- semantic fixed integer width / stored field width / ABI register size
- constrained or routed type
- implicit leading bit
- decimal -> T -> decimal round trips / T -> decimal -> T round trips
- representation views / scalar reads
- unchecked transmute / owned-aggregate cast primitive
- case-bearing library data / Quiet / Signaling kind
- canonical positive quiet NaN
- FloatMeaning
- cross-target reproducibility
- result-based `Trapping` policy (float context)
- suffixed integer or float literal / suffix
- terminal transfers
- implicit receiver
- exact rational / rational intermediates / nested rational carrier
- destination / landing / landing boundary
- typed operand / anonymous operand / wholly anonymous numeric tree
- builtin remainder / integer operand meaning
- contract comparison
- semantic evaluation
- fractional-intermediate diagnostics / acknowledgement
- operator selection / builtin arithmetic / order laws
- const-argument normalizer
- signed analysis window
- no-wrap proof / monotonicity
- mathematical Euclidean modulo (runtime signed remainder contrast)
- policy domain / domain cast / `as -Wrapping`
- caller-selectable policy / generic spelling (unsettled)
- mixed-integer-comparison
- target-independent carrier range
- operand rule / qualified operand
- source-independent verification
- Exact / Wrapping / Saturating / Trapping (operation policies)
- arithmetic policy qualification
- TrappingInteger / operation-crash group / observation profile
- integer-policy catalog rows
- policy operator / suffixed forms
- compound-assignment spelling
- `omega::core` domains
- exact operator matching
- OPERATION-ARITHMETIC-POLICY
- builtin proof `Int` / IntPair
- Euclidean modulo (deferred)
- value_width
- hardware masking
- mathematical proof contracts (cross-reference)
- integer certificates (cross-reference)

CHAPTER COMPLETE
