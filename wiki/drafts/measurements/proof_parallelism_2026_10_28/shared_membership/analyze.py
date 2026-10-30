from pathlib import Path
import hashlib,importlib.util,json,statistics
ROOT=Path(__file__).resolve().parent
REPO=next(p for p in ROOT.parents if (p/"AGENTS.md").is_file())
def load(path):return json.loads(path.read_text())
def main():
    manifest=load(ROOT/"manifest.json")
    for name,digest in manifest.items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,name
    source=load(ROOT/"source-identity.json")
    for patch,key in [("candidate.patch","candidate_patch_sha256"),("timing.patch","timing_patch_sha256"),("counts.patch","counts_patch_sha256")]:
        assert hashlib.sha256((ROOT/patch).read_bytes()).hexdigest()==source[key]
    checks=load(ROOT/"patch-check.json");assert len(checks)==3
    assert all(c["base"]==source["base"] and c["exit_code"]==0 and c["isolated_index"] for c in checks)
    identities={r["producer"]:r for r in load(ROOT/"identities.json")+load(ROOT/"counts-identity.json")}
    assert all(int(r["build_profile"]["opt_level"])>0 for r in identities.values())
    cases=sorted((ROOT/"raw").iterdir());assert len(cases)==15
    counts=[];programs=set()
    spec=importlib.util.spec_from_file_location("scan_harness",REPO/"wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/scans_run.py")
    h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    for folder in cases:
        r=load(folder/"result.json");assert r["exit_code"]==0
        programs.add(r["program_sha256"])
        ident=identities[r["producer"]]
        assert r["compiler_sha256"]==ident["compiler_sha256"]
        assert r["accepted_lock_sha256"]==ident["accepted_lock_sha256"]
        assert not r["phase_profile"]
        assert h.parse((folder/"stderr.txt").read_text())==r["scans"]
        if folder.name=="shared-verified":
            assert r["terminal_stdout_agrees"] and r["retained_image_sha256"]==h.IMAGE
            output=(folder/"stdout.txt").read_bytes()
            assert output==(folder/"terminal.stdout").read_bytes() and len(output)==463
            assert hashlib.sha256(output).hexdigest()==r["guest_stdout_sha256"]
        else:
            assert r["cache_mode"]=="fresh private cache" and r["image_sha256"]==h.IMAGE
            assert load(folder/"runtime/result.json")["exit_code"]==0
            assert b"A bottomless dark room near the dungeon heart." in (folder/"runtime/stdout.txt").read_bytes()
        if r["scan_mode"]=="counts":counts.append(r["scans"])
        else:assert not r["scans"]
    assert len(programs)==1 and len(counts)==2 and counts[0]==counts[1]
    baseline=load(REPO/"wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/raw/scans-counts-a/result.json")["scans"]
    assert len(baseline)==len(counts[0])
    for old,new in zip(baseline,counts[0]):
        assert (old["machines"],old["summary"],old["queries"])==(new["machines"],new["summary"],new["queries"])
        assert {k:v for k,v in old["entries"].items() if k!="predicate_retention"}=={k:v for k,v in new["entries"].items() if k!="predicate_retention"}
    a=max(baseline,key=lambda b:b["machines"])["entries"]["predicate_retention"]
    b=max(counts[0],key=lambda b:b["machines"])["entries"]["predicate_retention"]
    assert a["compared"]==22041404 and a["rosters"]==b["rosters"]==140303
    assert 0<b["compared"]<a["compared"] and b["alias"]>0
    summary=load(ROOT/"summary.json");assert len(summary["rounds"])==6
    for trial in summary["rounds"]:
        n=trial["round"];assert trial["order"]==(["original","hoisted"] if n%2 else ["hoisted","original"])
        for kind in ["original","hoisted"]:
            r=load(ROOT/f"raw/shared-{kind}-{n}/result.json")
            outer=max((x for x in r["reconciliation"] if x["operation"]=="memberships"),key=lambda x:x["machines"])
            assert trial[kind]["wall_seconds"]==r["wall_seconds"]
            assert trial[kind]["cpu_seconds"]==r["cpu_seconds"]
            assert trial[kind]["source_replay_ms"]==outer["metrics"]["source_contract_queries_ns"]/1e6
        for k,value in trial["change_percent"].items():assert abs(value-(trial["hoisted"][k]/trial["original"][k]-1)*100)<1e-10
    print(json.dumps(dict(observations=len(cases),manifest_entries=len(manifest),original_comparisons=a["compared"],hoisted_counters=b,comparison_reduction_percent=(1-b["compared"]/a["compared"])*100,medians=summary["medians"],wins={k:sum(t["hoisted"][k]<t["original"][k] for t in summary["rounds"]) for k in ["wall_seconds","cpu_seconds","source_replay_ms"]}),indent=2))
if __name__=="__main__":main()
