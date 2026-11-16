#!/usr/bin/env python3
"""Trace clang caller MIR (stop-after=finalize-isel, -O0) into byte placements.

For caller c<k>, each byte of each argument global g<k>_<i> and of the result
global r<k> is mapped to the register byte / outgoing stack byte / indirect copy
it travels through. Unknown instructions that touch tracked values make the
signature a HARNESS failure rather than a guess.
"""
import re

X86 = {}
for full, names in {
    "rax": "rax eax ax al", "rbx": "rbx ebx bx bl", "rcx": "rcx ecx cx cl",
    "rdx": "rdx edx dx dl", "rsi": "rsi esi si sil", "rdi": "rdi edi di dil",
}.items():
    for n in names.split():
        X86[n] = full
for i in range(8, 16):
    for suf in ["", "d", "w", "b"]:
        X86[f"r{i}{suf}"] = f"r{i}"


def canon(reg):
    if reg in X86:
        return X86[reg]
    if reg.startswith("xmm"):
        return reg
    m = re.fullmatch(r"([wx])(\d+)", reg)
    if m:
        return f"x{m.group(2)}"
    m = re.fullmatch(r"([bhsdq])(\d+)", reg)
    if m:
        return f"v{m.group(2)}"
    return None


X86_SIZE = [(r"^MOV8", 1), (r"^MOV16", 2), (r"^MOV32", 4), (r"^MOV64", 8),
            (r"^MOVSS", 4), (r"^MOVSD", 8), (r"^MOVLPD", 8), (r"^MOV(UPS|APS|UPD|APD)", 16),
            (r"^MOV[SZ]X(32|64)rm8", 1), (r"^MOV[SZ]X(32|64)rm16", 2), (r"^MOVSX64rm32", 4)]
A64_SIZE = [(r"^(LDR|STR|LDUR|STUR)(B|SB)", 1), (r"^(LDR|STR|LDUR|STUR)(H|SH)", 2),
            (r"^(LDR|STR|LDUR|STUR)(W|S)", 4), (r"^(LDR|STR|LDUR|STUR)(X|D)", 8),
            (r"^(LDR|STR|LDUR|STUR)Q", 16)]


class Harness(Exception):
    pass


def access_size(op):
    for pat, size in X86_SIZE + A64_SIZE:
        if re.match(pat, op):
            return size
    raise Harness(f"size of {op}")


def split_ops(s):
    return [t.strip() for t in s.split(",")]


def functions(text):
    for m in re.finditer(r"^name:\s+c(\d+)\n(.*?)(?=^---|^\.\.\.)", text, re.M | re.S):
        yield int(m.group(1)), m.group(2)


def trace_function(body):
    vals = {}        # vreg -> {"bytes": {i: src}} or {"ptr": base}
    stackobj = {}    # (objname) -> {off: src}
    out_stack = {}   # outgoing offset -> src or ("ptr", base)
    arg_regs = {}    # canonical reg -> {byte: src} or ("ptr", base)
    globals_written = {}  # (global, off) -> src
    after_call = False
    lines = body.split("\n")
    start = next(i for i, l in enumerate(lines) if l.startswith("body:"))
    for raw in lines[start + 1:]:
        line = raw.strip()
        if not line or line.startswith("bb.") or line.startswith("successors") or line.startswith("liveins"):
            continue
        line = line.split(" :: ")[0]
        line = re.sub(r"\b(killed|undef|dead|renamable|implicit-def|implicit) ", lambda m: m.group(0) if m.group(1) in ("implicit-def", "implicit") else "", line)
        m = re.match(r"^(?:(\S+?)(?::[\w]+)? = )?(\S+)\s*(.*)$", line)
        dst, op, rest = m.group(1), m.group(2), m.group(3)
        rest = re.sub(r",?\s*implicit(-def)? \S+", "", rest)
        ops = split_ops(rest) if rest else []

        def base_of(tok):
            if tok in vals and "ptr" in vals[tok]:
                return vals[tok]["ptr"]
            mm = re.match(r"%stack\.(\d+)", tok)
            if mm:
                return ("stack", int(mm.group(1)), 0)
            if tok in ("$rsp", "$sp"):
                return ("out", None, 0)
            return None

        def sym(tok):
            mm = re.match(r"(?:target-flags\([^)]*\) )?@(\w+)(?: \+ (\d+))?$", tok)
            return (mm.group(1), int(mm.group(2) or 0)) if mm else None

        def read(base, off, size):
            kind, name, b = base
            res = {}
            for j in range(size):
                o = b + off + j
                if kind == "global":
                    res[j] = ("g", name, o)
                elif kind == "stack":
                    res[j] = stackobj.get(name, {}).get(o, ("mem", name, o))
                elif kind == "out":
                    res[j] = None
            return {"bytes": res}

        def write(base, off, size, val):
            kind, name, b = base
            for j in range(size):
                o = b + off + j
                src = val.get("ptr") and ("ptr", val["ptr"]) if "ptr" in val else val["bytes"].get(j)
                if kind == "stack":
                    stackobj.setdefault(name, {})[o] = src
                elif kind == "out":
                    out_stack[o] = src
                elif kind == "global":
                    globals_written[(name, o)] = src

        if op in ("ADJCALLSTACKDOWN64", "ADJCALLSTACKDOWN", "ADJCALLSTACKUP64", "ADJCALLSTACKUP",
                  "RET64", "RET_ReallyLR", "IMPLICIT_DEF"):
            if op == "IMPLICIT_DEF" and dst:
                vals[dst] = {"bytes": {}}
            continue
        if op in ("CALL64pcrel32", "BL"):
            if "memcpy" in ops[0]:
                regsets = [("x0", "x1", "x2"), ("rdi", "rsi", "rdx"), ("rcx", "rdx", "r8")]
                for d_r, s_r, n_r in regsets:
                    dv, sv, nv = arg_regs.get(d_r), arg_regs.get(s_r), arg_regs.get(n_r)
                    if dv and sv and nv and "ptr" in dv and "ptr" in sv and "const" in nv:
                        write(dv["ptr"], 0, nv["const"], read(sv["ptr"], 0, nv["const"]))
                        break
                else:
                    raise Harness("memcpy operands")
                arg_regs.clear()
                continue
            if after_call:
                raise Harness("second real call")
            after_call = True
            frozen = (dict(arg_regs), dict(out_stack))
            continue
        if op in ("MOVi64imm", "MOVi32imm", "MOV32ri64", "MOV32ri", "MOV64ri"):
            vals[dst] = {"const": int(ops[0]), "bytes": {}}
            continue
        if op in ("DUPi64", "DUPi32"):
            w = 8 if op == "DUPi64" else 4
            lane = int(ops[1])
            vals[dst] = {"bytes": {j - lane * w: s for j, s in vals[ops[0]]["bytes"].items() if 0 <= j - lane * w < w}}
            continue
        if op in ("INSvi64gpr", "INSvi64lane", "INSvi32lane"):
            base_v = dict(vals[ops[0]]["bytes"])
            lane = int(ops[1])
            w = 4 if "32" in op else 8
            src_v = vals[ops[2]]["bytes"]
            src_lane = int(ops[3]) if len(ops) > 3 else 0
            for j in range(w):
                base_v[lane * w + j] = src_v.get(src_lane * w + j)
            vals[dst] = {"bytes": base_v}
            continue
        if op == "COPY":
            src = ops[0]
            if dst.startswith("$"):
                r = canon(dst[1:])
                if r is None:
                    raise Harness(f"copy to {dst}")
                v = vals.get(src)
                if v is None:
                    raise Harness(f"untracked copy {line}")
                arg_regs[r] = v
            elif src in ("$rsp", "$sp"):
                vals[dst] = {"ptr": ("out", None, 0)}
            elif src.startswith("$"):
                r = canon(src[1:])
                vals[dst] = {"bytes": {j: ("reg", r, j) for j in range(16)}}
            else:
                vals[dst] = vals.get(src.split(".")[0], {"bytes": {}})
                if "." in src:
                    sub = src.split(".", 1)[1]
                    width = {"dsub": 8, "ssub": 4, "hsub": 2, "bsub": 1, "sub_32": 4, "sub_32bit": 4,
                             "sub_16bit": 2, "sub_8bit": 1, "sub_xmm": 16}.get(sub)
                    if width is None:
                        raise Harness(f"subreg {sub}")
                    if "bytes" in vals[dst]:
                        vals[dst] = {"bytes": {j: b for j, b in vals[dst]["bytes"].items() if j < width}}
                if dst.startswith("%") and src.startswith("%") and ".sub" in src and "ptr" in vals[dst]:
                    raise Harness("subreg of pointer")
            continue
        if op == "LOADgot":
            vals[dst] = {"ptr": ("global",) + sym(ops[0])[:1] + (sym(ops[0])[1],)}
            continue
        if op == "SUBREG_TO_REG":
            vals[dst] = vals[ops[1]]
            continue
        if op == "INSERT_SUBREG":
            if vals.get(ops[0], {"bytes": {}}).get("bytes"):
                raise Harness("insert into live value")
            vals[dst] = vals[ops[1]]
            continue
        if re.match(r"MOV[SZ]X\d+rr", op):
            vals[dst] = vals[ops[0]]
            continue
        if op in ("SHL64ri", "SHL32ri", "SHR64ri", "SHR32ri") or (op == "UBFMXri" and ops[2] == "63") or (op == "UBFMWri" and ops[2] == "31"):
            sh = int(ops[1]) // 8 if int(ops[1]) % 8 == 0 else None
            if sh is None:
                raise Harness("bit shift")
            width = 8 if "64" in op or op.endswith("Xri") else 4
            sh = -sh if op.startswith(("SHR", "UBFM")) else sh
            vals[dst] = {"bytes": {j + sh: s for j, s in vals[ops[0]]["bytes"].items() if 0 <= j + sh < width}}
            continue
        if op in ("ADD64rr_DB", "OR64rr", "ADD32rr_DB", "OR32rr", "ORRXrr", "ORRWrr"):
            merged = dict(vals[ops[0]]["bytes"])
            merged.update(vals[ops[1]]["bytes"])
            vals[dst] = {"bytes": merged}
            continue
        if op == "LEA64r":
            base = base_of(ops[0])
            if ops[0] == "$rip":
                g = sym(ops[3])
                vals[dst] = {"ptr": ("global", g[0], g[1])}
            else:
                vals[dst] = {"ptr": (base[0], base[1], base[2] + int(ops[3]))}
            continue
        if op == "ADDXri":
            base = base_of(ops[0])
            if base is None or int(ops[2]) != 0:
                raise Harness(f"addxri {line}")
            vals[dst] = {"ptr": (base[0], base[1], base[2] + int(ops[1]))}
            continue
        # memory ops
        if re.match(r"^(MOV|LDR|LDUR)", op) and dst and (op.endswith("rm") or "rm" in op[-8:] or op.startswith(("LDR", "LDUR"))):
            if op.startswith("MOV"):
                if ops[0] == "$rip":
                    g = sym(ops[3])
                    if g is None:
                        raise Harness(f"rip load {line}")
                    if "target-flags(x86-gotpcrel)" in ops[3]:
                        vals[dst] = {"ptr": ("global", g[0], g[1])}
                        continue
                    base, off = ("global", g[0], g[1]), 0
                else:
                    base, off = base_of(ops[0]), int(ops[3])
            else:
                base = base_of(ops[0])
                imm = int(ops[1])
                off = imm if op.startswith("LDUR") else imm * access_size(op)
            if base is None:
                raise Harness(f"load base {line}")
            vals[dst] = read(base, off, access_size(op))
            continue
        if re.match(r"^(MOV|STR|STUR)", op) and not dst:
            if op.startswith("MOV"):
                val = vals.get(ops[5])
                if ops[0] == "$rip":
                    g = sym(ops[3])
                    base, off = ("global", g[0], g[1]), 0
                else:
                    base, off = base_of(ops[0]), int(ops[3])
            else:
                val = vals.get(ops[0])
                base = base_of(ops[1])
                imm = int(ops[2])
                off = imm if op.startswith("STUR") else imm * access_size(op)
            if base is None or val is None:
                raise Harness(f"store {line}")
            write(base, off, access_size(op), val)
            continue
        raise Harness(f"unknown op {op}: {line}")
    if not after_call:
        raise Harness("no call")
    return frozen[0], frozen[1], stackobj, globals_written


def placements(k, body, sig):
    """Return (param_maps, result_map): list of {value_byte: location}."""
    arg_regs, out_stack, stackobj, gw = trace_function(body)
    pmaps = [dict() for _ in sig["params"]]
    pointers = {}  # stack obj -> location string of pointer
    for r, v in arg_regs.items():
        if "ptr" in v:
            pointers[v["ptr"][1]] = f"reg:{r}"
            continue
        for j, src in v["bytes"].items():
            if src and src[0] == "g" and src[1].startswith(f"g{k}_"):
                i = int(src[1].split("_")[1])
                pmaps[i][src[2]] = f"reg:{r}@{j}"
    for o, src in out_stack.items():
        if src is None:
            continue
        if src[0] == "ptr":
            if src[1][0] == "stack":
                pointers.setdefault(src[1][1], f"sp+{o}")
            continue
        if src[0] == "g" and src[1].startswith(f"g{k}_"):
            i = int(src[1].split("_")[1])
            pmaps[i][src[2]] = f"sp+{o}"
    # indirect copies
    for obj, loc in pointers.items():
        for o, src in stackobj.get(obj, {}).items():
            if src and src[0] == "g" and src[1].startswith(f"g{k}_"):
                i = int(src[1].split("_")[1])
                pmaps[i].setdefault(src[2], f"ind({loc})@{o}")
    rmap = {}
    for (g, o), src in gw.items():
        if g != f"r{k}" or src is None:
            continue
        if src[0] == "reg":
            rmap[o] = f"reg:{src[1]}@{src[2]}"
        elif src[0] == "mem" and src[1] in pointers:
            rmap[o] = f"ind({pointers[src[1]]})@{src[2]}"
        else:
            rmap[o] = f"?{src}"
    return pmaps, rmap
