from pathlib import Path
import hashlib,json,os,subprocess,time
root=Path.cwd();out=root/'build/fs-range-replay'
def rows(name):return [json.loads(s) for s in (out/name).read_text().splitlines() if s.startswith('{')]
normal,=[r for r in rows('cargo.jsonl') if r.get('reason')=='compiler-artifact' and r['target']['name']=='filesystem-range-replay' and r.get('executable') and not r['profile']['test']]
test,=[r for r in rows('tests.jsonl') if r.get('reason')=='compiler-artifact' and r['target']['name']=='filesystem-range-replay' and r.get('executable') and r['profile']['test']]
results=[]
for label,artifact,args in [
 ('sibling-cut',test,['--exact','checking::prove::tests::sibling_disjunctions_in_one_premise_remain_separate_cuts','--nocapture']),
 ('active-transported-cut',test,['--exact','checking::prove::tests::case_search_does_not_materialize_exhausted_or_active_cuts','--nocapture']),
 ('exact-shared-range',normal,[str(out)])]:
 assert artifact['profile']['opt_level']=='1' and artifact['profile']['debuginfo']==0
 binary=Path(artifact['executable']);command=[str(binary),*args];started=time.monotonic()
 with (out/(label+'.stdout')).open('wb') as stdout,(out/(label+'.stderr')).open('wb') as stderr:
  result=subprocess.run(command,stdout=stdout,stderr=stderr,timeout=30)
 record={'command':command,'returncode':result.returncode,'elapsed_seconds':time.monotonic()-started,'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'artifact':artifact}
 results.append(record);(out/(label+'.json')).write_text(json.dumps(record,indent=2)+'\n')
 assert result.returncode==0,record
 if artifact['profile']['test']:assert '1 passed' in (out/(label+'.stdout')).read_text(),label
 else:assert 'INPUT_MATCH values=7988 assumptions=13097 axioms=25' in (out/(label+'.stderr')).read_text()
print(json.dumps(results,indent=2))
