from pathlib import Path
import hashlib, importlib.util, json
ROOT=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location("scan_harness",ROOT/"scans_run.py")
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
def load(p):return json.loads(p.read_text())
def main():
    manifest=load(ROOT/"manifest.json")
    for name,expected in manifest.items():
        assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==expected,name
    identities={r["producer"]:r for r in load(ROOT/"identities.json")}
    assert all(int(r["build_profile"]["opt_level"])>0 for r in identities.values())
    cases=sorted((ROOT/"raw").iterdir());assert len(cases)==14
    counts=[];rankings={};programs=set()
    for folder in cases:
        r=load(folder/"result.json");assert r["exit_code"]==0
        programs.add(r["program_sha256"])
        identity=identities[r["producer"]]
        assert r["compiler_sha256"]==identity["compiler_sha256"]
        assert r["accepted_lock_sha256"]==identity["accepted_lock_sha256"]
        assert not r["phase_profile"]
        stderr=(folder/"stderr.txt").read_text()
        assert h.parse(stderr)==r["scans"]
        assert "DECLINED" not in stderr
        if r["scan_mode"]=="off":assert not r["scans"]
        else:
            b=max(r["scans"],key=lambda x:x["machines"])
            assert (b["machines"],b["summary"]["roots"],b["summary"]["replays"])==(202,418,173)
            assert b["entries"]["position"]["calls"]==15219
            assert b["entries"]["edge"]["calls"]==17444
            if r["scan_mode"]=="counts":
                counts.append(r["scans"])
                assert all(v["inclusive_ns"]==v["exclusive_ns"]==0 for batch in r["scans"] for v in batch["entries"].values())
            else:
                rankings[folder.name]={k:round(v["exclusive_ns"]/1e6,4) for k,v in sorted(b["entries"].items(),key=lambda kv:-kv[1]["exclusive_ns"])}
        if folder.name=="scans-verified":
            assert r["terminal_stdout_agrees"] and r["retained_image_sha256"]==h.IMAGE
            output=(folder/"stdout.txt").read_bytes()
            assert output==(folder/"terminal.stdout").read_bytes() and len(output)==463
            assert hashlib.sha256(output).hexdigest()==r["guest_stdout_sha256"]
        else:
            assert r["cache_mode"]=="fresh private cache"
            assert r["image_sha256"]==h.IMAGE
            assert load(folder/"runtime/result.json")["exit_code"]==0
            assert b"A bottomless dark room near the dungeon heart." in (folder/"runtime/stdout.txt").read_bytes()
    assert len(programs)==1 and len(counts)==2 and counts[0]==counts[1]
    e=max(counts[0],key=lambda b:b["machines"])["entries"]
    assert e["predicate_retention"]["compared"]==22041404
    assert e["publish"]["advanced"]==20206680
    assert e["retain_snapshot"]["cloned"]==5076178
    assert e["meet"]["same"]==16
    print(json.dumps(dict(observations=len(cases),manifest_entries=len(manifest),exclusive_ms=rankings),indent=2))
if __name__=="__main__":main()
