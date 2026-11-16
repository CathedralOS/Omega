def omega_maps(path):
    res, cur = {}, None
    for line in open(path):
        t = line.split()
        if t[0] == "SIG":
            cur = int(t[1])
            res[cur] = {"status": t[2], "why": " ".join(t[3:]), "params": [], "result": None, "classes": []}
            continue
        parts = [p.strip() for p in line.strip().split(" ; ")]
        head, locs = parts[0].split(), parts[1:]
        bmap = {}
        for loc in locs:
            f = loc.split()
            if f[0] == "REG":
                vo, sz = int(f[2]), int(f[3])
                for b in range(vo, vo + sz):
                    bmap[b] = f"reg:{f[1]}@{b - vo}"
            elif f[0] == "STACK":
                off, vo, sz = map(int, f[1:4])
                for b in range(vo, vo + sz):
                    bmap[b] = f"sp+{off + b - vo}"
            elif f[0] == "IND":
                for b in range(int(f[2])):
                    bmap[b] = f"ind({f[1]})@{b}"
        if head[0] == "P":
            res[cur]["params"].append(bmap)
            res[cur]["classes"].append(head[2])
        else:
            res[cur]["result"] = bmap
    return res


