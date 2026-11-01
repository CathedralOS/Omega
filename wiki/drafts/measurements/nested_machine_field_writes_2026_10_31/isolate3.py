"""Generate nested-machine transition-path scaling probes.

Usage: python isolate3.py N mode out.omg
  N     nested machines in a call chain inside `machine Main::main`
  mode  `const`   = two constant field writes per machine
        `bounded` = (x+1)%256, (y*2)%256 field writes per machine
        anything else = raw x+1, y*2 writes (rejects at m62 overflow)
"""
import sys

n = int(sys.argv[1])
mode = sys.argv[2]
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
for k in range(n):
    nxt = "done(main)" if k == n - 1 else "m%d(main)" % (k + 1)
    out.append("    machine m%d(main: &mut Main) {" % k)
    if mode == "const":
        out += ["        main.x = 1;", "        main.y = 1;"]
    elif mode == "bounded":
        out += ["        main.x = (main.x + 1) % 256;", "        main.y = (main.y * 2) % 256;"]
    else:
        out += ["        main.x = main.x + 1;", "        main.y = main.y * 2;"]
    out.append("        transition { _ -> %s }" % nxt)
    out.append("    }")
out += [
    "    machine done(main: &mut Main) {",
    "    }",
    "    machine bad(main: &mut Main) { crash Trap; }",
    "}",
]
open(sys.argv[3], "w").write("\n".join(out) + "\n")
