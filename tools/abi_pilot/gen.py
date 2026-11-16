#!/usr/bin/env python3
"""Generate random C signatures plus an Omega-harness spec.

Usage: gen.py SEED COUNT OUTDIR
Writes OUTDIR/sigs.c, OUTDIR/sigs.json (layouts) and OUTDIR/spec.<target>.txt.
"""
import json, random, sys

SCALARS = {  # name: (size, align, is_float)
    "i8": (1, 1, False), "i16": (2, 2, False), "i32": (4, 4, False), "i64": (8, 8, False),
    "f32": (4, 4, True), "f64": (8, 8, True), "i128": (16, 16, False),
}
CNAME = {"i8": "signed char", "i16": "short", "i32": "int", "i64": "long long",
         "f32": "float", "f64": "double", "i128": "__int128"}
TARGETS = ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"]


def layout(fields):
    off, align, leaves = 0, 1, []
    for f in fields:
        s, a, fl = SCALARS[f]
        off = (off + a - 1) // a * a
        leaves.append((off, s, fl))
        off += s
        align = max(align, a)
    size = (off + align - 1) // align * align
    return size, align, leaves


def rand_type(rng):
    r = rng.random()
    if r < 0.35:
        return ("scalar", rng.choice(["i8", "i16", "i32", "i64"]))
    if r < 0.55:
        return ("scalar", rng.choice(["f32", "f64"]))
    if r < 0.75:
        n = rng.randint(1, 4)
        pool = [x for x in SCALARS if x != "i128"]
        if rng.random() < 0.3:
            fields = [rng.choice(pool) for _ in range(n - 1)] + ["i128"]
            rng.shuffle(fields)
            return ("rec", fields[:3])
        return ("rec", [rng.choice(pool) for _ in range(n)])
    if r < 0.90:
        n = rng.randint(1, 4)
        return ("rec", [rng.choice(["f32", "f64"])] * n)
    # integer-only small records (packing / narrow shapes)
    n = rng.randint(1, 3)
    return ("rec", [rng.choice(["i8", "i16", "i32"]) for _ in range(n)])


def describe(t):
    if t[0] == "scalar":
        s, a, fl = SCALARS[t[1]]
        return {"kind": "scalar", "name": t[1], "size": s, "align": a,
                "leaves": [(0, s, fl)]}
    size, align, leaves = layout(t[1])
    return {"kind": "rec", "fields": t[1], "size": size, "align": align, "leaves": leaves}


def spec_line(tag, d):
    if d["kind"] == "scalar":
        kind = "float" if SCALARS[d["name"]][2] else "int"
        return f"{tag} {kind} {d['size']} {d['align']}"
    leaves = ",".join(f"{o}:{s}:{int(f)}" for o, s, f in d["leaves"])
    return f"{tag} rec {d['size']} {d['align']} {leaves}"


def main():
    seed, count, outdir = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
    rng = random.Random(seed)
    sigs, c = [], []
    for k in range(count):
        n = rng.choice([1, 2, 3, 4, 6, 8, 9, 10, 12, 14])
        params = [rand_type(rng) for _ in range(n)]
        result = None if rng.random() < 0.3 else rand_type(rng)
        ctype = {}
        decls = []
        for i, t in enumerate(params + ([result] if result else [])):
            nm = f"T{k}_{i}"
            if t[0] == "scalar":
                decls.append(f"typedef {CNAME[t[1]]} {nm};")
            else:
                body = " ".join(f"{CNAME[f]} m{j};" for j, f in enumerate(t[1]))
                decls.append(f"typedef struct {{ {body} }} {nm};")
            ctype[i] = nm
        c += decls
        for i in range(n):
            c.append(f"extern {ctype[i]} g{k}_{i};")
        rt = ctype[n] if result else "void"
        if result:
            c.append(f"extern {rt} r{k};")
        c.append(f"extern {rt} f{k}({', '.join(ctype[i] for i in range(n))});")
        args = ", ".join(f"g{k}_{i}" for i in range(n))
        c.append(f"void c{k}(void) {{ {'r%d = ' % k if result else ''}f{k}({args}); }}")
        sigs.append({"id": k, "params": [describe(t) for t in params],
                     "result": describe(result) if result else None})
    open(f"{outdir}/sigs.c", "w").write("\n".join(c) + "\n")
    json.dump(sigs, open(f"{outdir}/sigs.json", "w"))
    for target in TARGETS:
        lines = [f"TARGET {target}"]
        for s in sigs:
            lines.append(f"SIG {s['id']}")
            lines += [spec_line("P", p) for p in s["params"]]
            lines.append(spec_line("R", s["result"]) if s["result"] else "R none")
            lines.append("END")
        open(f"{outdir}/spec.{target}.txt", "w").write("\n".join(lines) + "\n")


main()
