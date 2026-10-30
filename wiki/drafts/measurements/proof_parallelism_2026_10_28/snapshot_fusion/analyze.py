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
        if folder.name=="fusion-verified":
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
        targets={"retain_snapshot","retain_premises"}
        assert {k:v for k,v in old["entries"].items() if k not in targets}=={k:v for k,v in new["entries"].items() if k not in targets}
        if "retain_snapshot" in old["entries"]:
            a=old["entries"]["retain_snapshot"];b=new["entries"]["retain_snapshot"]
            for k in ["calls","rows_a","rows_b","rows_c","same","alias","detached","cloned"]:assert a[k]==b[k],(k,a[k],b[k])
            assert b["rosters"]==b["calls"] and b["visited"]==b["rows_a"]
    old=max(baseline,key=lambda b:b["machines"])["entries"]
    new=max(counts[0],key=lambda b:b["machines"])["entries"]
    a=old["retain_snapshot"];b=new["retain_snapshot"]
    assert a["cloned"]==b["cloned"]==5076178 and b["detached"]==14456
    assert b["calls"]==28540 and b["same"]==4760 and b["rosters"]==28540
    visited_before=sum(old[k]["visited"] for k in targets)
    visited_after=sum(new[k]["visited"] for k in targets)
    comparisons_before=sum(old[k]["compared"] for k in targets)
    comparisons_after=sum(new[k]["compared"] for k in targets)
    assert 0<visited_after<visited_before and 0<comparisons_after<comparisons_before
    assert b["removed"]+new["retain_premises"]["removed"]==old["retain_premises"]["removed"]
    model=load(ROOT/"algorithm-result.json");assert model["cases"]==196864
    model_source=(ROOT/"intersection_cases.rs").read_text()
    start=model_source.index("fn retain_common_snapshot(");end=model_source.index("fn retain_common_premises<",start)
    assert hashlib.sha256(model_source[start:end].encode()).hexdigest()==source["extracted_candidate_sha256"]
    summary=load(ROOT/"summary.json");assert len(summary["rounds"])==6
    for trial in summary["rounds"]:
        n=trial["round"];assert trial["order"]==(["original","fused"] if n%2 else ["fused","original"])
        for kind in ["original","fused"]:
            r=load(ROOT/f"raw/fusion-{kind}-{n}/result.json")
            outer=max((x for x in r["reconciliation"] if x["operation"]=="memberships"),key=lambda x:x["machines"])
            assert trial[kind]["wall_seconds"]==r["wall_seconds"]
            assert trial[kind]["cpu_seconds"]==r["cpu_seconds"]
            assert trial[kind]["source_replay_ms"]==outer["metrics"]["source_contract_queries_ns"]/1e6
        for k,value in trial["change_percent"].items():assert abs(value-(trial["fused"][k]/trial["original"][k]-1)*100)<1e-10
    print(json.dumps(dict(observations=len(cases),manifest_entries=len(manifest),algorithm_cases=model["cases"],snapshot_counters=b,visits_before=visited_before,visits_after=visited_after,comparisons_before=comparisons_before,comparisons_after=comparisons_after,visit_reduction_percent=(1-visited_after/visited_before)*100,comparison_reduction_percent=(1-comparisons_after/comparisons_before)*100,medians=summary["medians"],wins={k:sum(t["fused"][k]<t["original"][k] for t in summary["rounds"]) for k in ["wall_seconds","cpu_seconds","source_replay_ms"]}),indent=2))
if __name__=="__main__":main()
