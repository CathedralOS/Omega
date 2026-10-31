"""Generate nested-machine field-write scaling probes.

Usage: python isolate.py N writes guard out.omg
  N       nested machines inside `machine Main::main`
  writes  field writes per machine (0 = none, 1 = x only, 2 = x and y)
  guard   `full` (two conjuncts), `one`, or `none`
"""
import sys

n = int(sys.argv[1])
writes = int(sys.argv[2])
guard = sys.argv[3]
out = [
    "data Main",
    "{",
    "    x: u64;",
    "    y: u64;",
    "}",
    "",
    "machine Main::main(main: &mut Main) crashes Trap {",
    "    main.x = 0;",
    "    main.y = 1;",
    "    transition main.x < 5 { true -> m0(main) _ -> done(main) }",
]
body = []
if writes >= 1:
    body.append("        main.x = main.x + 1;")
if writes >= 2:
    body.append("        main.y = main.y * 2;")
g = (
    "main.x < 4000 && main.y > 0"
    if guard == "full"
    else ("main.x < 4000" if guard == "one" else "")
)
for k in range(n):
    nxt = "done(main)" if k == n - 1 else "m%d(main)" % (k + 1)
    out.append("    machine m%d(main: &mut Main) {" % k)
    out += body
    tr = (
        "transition { _ -> %s }" % nxt
        if not g
        else "transition %s { true -> %s _ -> bad(main) }" % (g, nxt)
    )
    out.append("        " + tr)
    out.append("    }")
out += [
    "    machine done(main: &mut Main) {",
    "    }",
    "    machine bad(main: &mut Main) { crash Trap; }",
    "}",
]
open(sys.argv[4], "w").write("\n".join(out) + "\n")
