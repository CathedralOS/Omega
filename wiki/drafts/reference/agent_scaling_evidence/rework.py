import csv, re, random, collections, json, sys
csv.field_size_limit(10**9)
B='<tables>/'
rows=list(csv.DictReader(open(B+'commits.tsv'),delimiter='\t'))
bodies={}
for chunk in open(B+'reports/bodies.raw').read().split('\x01')[1:]:
    h,b=chunk.split('\x02',1); bodies[h[:12]]=b
for r in rows: r['body']=bodies.get(r['hash'][:12],'')
KW=re.compile(r"\b(revert\w*|restor\w*|repair\w*|regress\w*|unbreak\w*|follow[- ]?up|again|re-?land\w*|undo\w*|undoes|broke|broken|breaks)\b",re.I)
FIXT=re.compile(r"^\w[\w-]*:\s*(fix|repair|restore|revert)\b",re.I)
CITE=re.compile(r"(#\d{3,5})|\b[0-9a-f]{9,40}\b")
def own_pr(r): return '#'+r['pr'] if r['pr'] else None
def is_cand(r):
    t=r['title']; b=r['body']
    # strip provenance/gate/spec/proof lines so their wording doesn't count
    core='\n'.join(l for l in b.split('\n') if not re.match(r'^(Agent|Gate|Spec|Proof|Co-Authored|Claude-Session):',l))
    cites=[c for c in re.findall(r'#\d{3,5}',core) if c!=own_pr(r)]
    hits=set(m.lower() for m in KW.findall(t+'\n'+core))
    return bool(hits or FIXT.search(t) or cites), sorted(hits), cites
if __name__=='__main__':
    cands=[]
    for r in rows:
        c,h,ci=is_cand(r)
        r['cand']=c; r['hits']=h; r['cites']=ci
        if c: cands.append(r)
    print(len(rows),len(cands))
    random.seed(20261123)
    sample=random.sample(cands,100)
    json.dump([{'hash':r['hash'],'day':r['day'],'lane':r['lane'],'pr':r['pr'],'title':r['title'],'hits':r['hits'],'cites':r['cites']} for r in sample],open(B+'reports/sample.json','w'),indent=0)

# ---------------- part 2: hand classification of the 100-commit sample ----------------
# Codes: R = repairs a regression an earlier landed change introduced; V = revert / re-land;
# C = completes work an earlier change left unfinished; O = repairs an old defect not traced to
# recent work; N = not a repair (feature, refactor, performance, docs, ruling).
# Index i refers to sample.json order (random.seed(20261123), random.sample(cands, 100)).
CLASS = dict(enumerate((
 "C R O O O N O O C N C O N R O N O O N O N N N N N "
 "N N N O N N N O O R O N N N O N N N N R N O N R N "
 "O C O O O N C N N N N N C O O N N O O R N N N O O "
 "O O O N N N N O O O N O N N R R N N N N C N N N N").split()))
# For R: (cause PR named in the body, how it was caught, as the body says it)
CAUSE = {
 1: ('2923', 'post-merge CI job: test target failed to compile (E0063)'),
 13:('5150', 'later agent working on Build files'),
 34:('4837', 'test suite: 26 of 42 tests failed; bisect named the cause'),
 44:('2370', 'corpus ledger run: 41 rows recorded as checking failed on main'),
 48:('3350', 'later agent: 8 tests could not start (retired imports)'),
 69:(None,   'test target no longer compiled after a migration (cause PR not named)'),
 89:('1818', 'corpus fixtures rejected (cause predates the window)'),
 90:('3309', 'later agent program refused ("could no longer"); cause inferred'),
}
import math, datetime
def wilson(k,n,z=1.96):
    if n==0: return (0,0)
    p=k/n; den=1+z*z/n; c=(p+z*z/(2*n))/den; h=z*math.sqrt(p*(1-p)/n+z*z/(4*n*n))/den
    return (max(0,c-h),min(1,c+h))
def run_part2():
    allrows=rows
    cands=[r for r in allrows if is_cand(r)[0]]
    random.seed(20261123); sample=random.sample(cands,100)
    N=len(allrows); K=len(cands); n=len(sample)
    out=[]
    cnt=collections.Counter(CLASS.values())
    out.append(('pop',N,K,n,dict(cnt)))
    for code in 'RVCON':
        k=cnt.get(code,0); lo,hi=wilson(k,n)
        out.append((code,k,n,k/n,lo,hi,K*k/n,K*lo,K*hi,K*k/n/N,K*lo/N,K*hi/N))
    rep=sum(cnt.get(c,0) for c in 'RVC'); lo,hi=wilson(rep,n)
    out.append(('RVC',rep,n,rep/n,lo,hi,K*rep/n,K*lo,K*hi,K*rep/n/N,K*lo/N,K*hi/N))
    # by lane and day: candidates and sample observations
    bylane=collections.defaultdict(lambda:[0,0,0,collections.Counter()])
    byday=collections.defaultdict(lambda:[0,0,0,collections.Counter()])
    for r in allrows:
        bylane[r['lane'] or '(none)'][0]+=1; byday[r['day']][0]+=1
    for r in cands:
        bylane[r['lane'] or '(none)'][1]+=1; byday[r['day']][1]+=1
    for i,r in enumerate(sample):
        for t,key in ((bylane,r['lane'] or '(none)'),(byday,r['day'])):
            t[key][2]+=1; t[key][3][CLASS[i]]+=1
    # lags for R
    bypr={r['pr']:r for r in allrows if r['pr']}
    lags=[]
    for i,(pr,how) in CAUSE.items():
        s=sample[i]; c=bypr.get(pr) if pr else None
        lag=None
        if c:
            a=datetime.datetime.fromisoformat(c['date']); b=datetime.datetime.fromisoformat(s['date'])
            lag=(b-a).total_seconds()/3600
        lags.append((s['hash'],s['pr'],pr,lag,how))
    return out,bylane,byday,lags,sample
if __name__=='__main__':
    out,bylane,byday,lags,sample=run_part2()
    for o in out: print(o)
    print('LANE'); [print(k,v[0],v[1],v[2],dict(v[3])) for k,v in sorted(bylane.items(),key=lambda x:-x[1][0])]
    print('DAY'); [print(k,v[0],v[1],v[2],dict(v[3])) for k,v in sorted(byday.items())]
    print('LAGS'); [print(l) for l in lags]
    for i in sorted(CLASS):
        if CLASS[i] in 'RC': print(CLASS[i], sample[i]['hash'], sample[i]['day'], sample[i]['title'][:90])

# ---------------- part 3: grouped estimates and out-of-sample example lags ----------------
STRONG=re.compile(r'first bad commit|bisect\w*|(?:fails?|failed|broken|red) (?:to compile )?on main|#\d{3,5}\W{0,3} (?:broke|introduced|removed|regressed)|(?:since|after|by|in) #\d{3,5}\W{0,3} (?:broke|regress)|regressed (?:by|in|since) #\d+|broke (?:in|by|with|since) #\d+',re.I)
def run_part3():
    out,bylane,byday,lags,sample=run_part2()
    res={}
    def est(groups,table):
        for name,keys in groups:
            tot=sum(table[k][0] for k in keys); K=sum(table[k][1] for k in keys)
            c=collections.Counter()
            for k in keys: c+=table[k][3]
            n=sum(c.values()); rep=c['R']+c['V']+c['C']; r=c['R']
            lo,hi=wilson(rep,n); rlo,rhi=wilson(r,n)
            res[name]=(tot,K,n,r,rep,K*r/n/tot if n else None,K*rlo/tot,K*rhi/tot,K*rep/n/tot if n else None,K*lo/tot,K*hi/tot)
    other=[k for k in bylane if k not in ('psi','compiler','docs')]
    est([('psi',['psi']),('compiler',['compiler']),('docs',['docs']),('all other lanes',other)],bylane)
    days=sorted(byday)
    est([('Nov 10-16',[d for d in days if d<='2026-11-16']),('Nov 17-23',[d for d in days if d>'2026-11-16'])],byday)
    strong=[r for r in rows if STRONG.search(' '.join(r['body'].split()))]
    bypr={r['pr']:r for r in rows if r['pr']}
    ex=[('a799ad13633a','3000'),('bb3d4d0b02fb','4861'),('947f3bd4748e','5121'),('c4da5c01c8c1','3917'),('cc1c3bd01d72','3524')]
    exl=[]
    for h,pr in ex:
        a=[r for r in rows if r['hash'].startswith(h)][0]; c=bypr.get(pr)
        exl.append((h,a['pr'],pr,(datetime.datetime.fromisoformat(a['date'])-datetime.datetime.fromisoformat(c['date'])).total_seconds()/3600 if c else None))
    return res,len(strong),exl
if __name__=='__main__':
    res,ns,exl=run_part3()
    print('GROUPS');[print(k,v) for k,v in res.items()]
    print('STRONG',ns); print(exl)

# ---------------- part 4: the 25-commit check of the STRONG phrase filter ----------------
# Hand result: 10 of 25 describe a repair of a regression caused by an earlier landed change:
# a799ad13633a bb3d4d0b02fb dfe21346cb04 1a6b9b5f1460 91d86f1ab00e 5a9797fe 947f3bd4748e
# 58977eae90a2 3527cc8e4e8a dbdf852e4fea. The rest mention tests already red on main in passing.
if __name__=='__main__' and 'strong' in sys.argv:
    st=[r for r in rows if STRONG.search(' '.join(r['body'].split()))]
    random.seed(7)
    for r in random.sample(st,25): print(r['hash'],r['title'][:90])
# bodies.raw was produced with:
#   git -C /home/user/Omega log --first-parent origin/main --since=2026-11-09 --format='%x01%H%x02%B' > reports/bodies.raw
