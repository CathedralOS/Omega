# Captured map result

`Selected<7>` and `Selected<8>` select the same authored owned method with
independent const captures. The application checks `7 * 10 + 8 = 78` and
crashes if a selected value is outside the checked range or the result differs.

Run `omega run --both samples/cli/generics/captured_map_result/main.omg`.
Expected exit: 0. Expected stdout: `78` followed by a newline.
Use an empty bounded stdin file. Output uses the ordinary Console provider and
its checked stdout adapter; Source risk disclosure does not grant host authority.
The build binds `Main::main` explicitly, and each Console call borrows the
entry's `Binding<Console>` authority.

The map capture telescope and method invocation telescope remain distinct.
Named implementation selection follows the language conformance contract;
provider selection does not supply missing map operands.

Invalid controls reject missing or surplus map operands, wrong binder categories,
unclosed callbacks, incompatible dependent callback Type/Const shapes, nominal
satisfaction mismatches, and used or unused callbacks exceeding their authored
service ceiling. Method invocation binders shadow captures by declaration scope.
An open generic method schema supplies no finite dynamic dispatch roster.

Native and Terminal runs use ordinary accepted Source bindings and Console
permissions. Source risk acceptance is distinct from host execution authority;
artifact receiving checks structural joins under ordinary producer trust and
does not claim PCC verification.
