from pathlib import Path
import hashlib,importlib.util,json,statistics,subprocess,sys
ROOT=next(p for p in Path(__file__).resolve().parents if (p/"AGENTS.md").exists())
spec=importlib.util.spec_from_file_location("scans",ROOT/"wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/scans_run.py")
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
OUT=ROOT/"build/proof-profile";WORK=OUT/"shared-membership"
PRODUCERS={"original":"payload","hoisted":"shared-membership"}
def identities(names):
    result=[]
    for name in names:
        d=OUT/"producers"/name
        profile=json.loads(subprocess.check_output([str(d/"omega.exe"),"--build-profile"]))
        assert int(profile["opt_level"])>0
        result.append(dict(producer=name,compiler_sha256=hashlib.sha256((d/"omega.exe").read_bytes()).hexdigest(),accepted_lock_sha256=hashlib.sha256((d/"omega.lock").read_bytes()).hexdigest(),build_profile=profile))
    return result
def summarize():
    rounds=[]
    for n in range(6):
        trial={"round":n+1,"order":["original","hoisted"] if n%2==0 else ["hoisted","original"]}
        for kind in PRODUCERS:
            r=json.loads((OUT/f"shared-{kind}-{n+1}"/"result.json").read_text())
            m=max((x for x in r["reconciliation"] if x["operation"]=="memberships"),key=lambda x:x["machines"])
            trial[kind]=dict(wall_seconds=r["wall_seconds"],cpu_seconds=r["cpu_seconds"],source_replay_ms=m["metrics"]["source_contract_queries_ns"]/1e6,membership_ms=m["wall_ns"]/1e6,peak_working_set_bytes=r["peak_working_set_bytes"],peak_commit_bytes=r["peak_commit_bytes"])
        trial["change_percent"]={k:(trial["hoisted"][k]/trial["original"][k]-1)*100 for k in ["wall_seconds","cpu_seconds","source_replay_ms"]}
        rounds.append(trial)
    result=dict(rounds=rounds,medians={kind:{k:statistics.median(row[kind][k] for row in rounds) for k in rounds[0][kind]} for kind in PRODUCERS})
    (WORK/"summary.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8",newline="\n")
    print("SUMMARY",json.dumps(result),flush=True)
    return result
if __name__=="__main__":
    if len(sys.argv)>1 and sys.argv[1]=="counts":
        a=h.run("shared-membership-counts","shared-counts-a","counts")
        b=h.run("shared-membership-counts","shared-counts-b","counts")
        assert a["scans"]==b["scans"]
        (WORK/"counts-identity.json").write_text(json.dumps(identities(["shared-membership-counts"]),indent=2)+"\n",encoding="utf-8",newline="\n")
    else:
        ids=identities(PRODUCERS.values())
        old=json.loads((ROOT/"wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/identities.json").read_text())[0]
        assert ids[0]==old
        (WORK/"identities.json").write_text(json.dumps(ids,indent=2)+"\n",encoding="utf-8",newline="\n")
        for n in range(6):
            for kind in (["original","hoisted"] if n%2==0 else ["hoisted","original"]):h.run(PRODUCERS[kind],f"shared-{kind}-{n+1}")
        h.run(PRODUCERS["hoisted"],"shared-verified",comparison=True)
        summarize()
