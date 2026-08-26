#!/usr/bin/env python3
"""Independent focused source-meaning/CKIR5 relation for OMGRFN7 R4.

This is deliberately a small parser and evaluator for the frozen pure-sum
tranche.  It does not import or execute the resolver, lowerer, or backend.
"""

from __future__ import annotations

import copy
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
COMPILER = ROOT / "bootstrap/omega-bootstrap/compiler"
GATES = ROOT / "bootstrap/omega-bootstrap/gates"
sys.path[:0] = [str(COMPILER), str(GATES)]

import checked_ir_v5_reference as ir5  # noqa: E402
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


class Reject(ValueError):
    pass


TOKEN = re.compile(r"//[^\n]*|/\*.*?\*/|->|::|[A-Za-z_][A-Za-z0-9_]*|[0-9]+|[^\s]", re.S)


def tokens(text: str) -> list[str]:
    return [m.group() for m in TOKEN.finditer(text) if not m.group().startswith(("//", "/*"))]


@dataclass
class Data:
    name: str
    copyable: bool
    fields: list[tuple[str, str]]
    cases: list[tuple[str, list[tuple[str, str]]]]


@dataclass
class Machine:
    owner: str
    name: str
    receiver_mut: bool
    params: list[tuple[str, str]]
    result: str | None
    entry: list
    states: dict[str, tuple[list[tuple[str, str]], list]]


class Parser:
    def __init__(self, ts: list[str]): self.t, self.i = ts, 0
    def peek(self, value=None):
        if self.i >= len(self.t): return False if value is not None else None
        return self.t[self.i] == value if value is not None else self.t[self.i]
    def take(self, value=None):
        if self.i >= len(self.t): raise Reject("unexpected EOF")
        out = self.t[self.i]
        if value is not None and out != value: raise Reject(f"expected {value}, got {out}")
        self.i += 1
        return out
    def ident(self):
        out = self.take()
        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", out): raise Reject("identifier")
        return out
    def type(self): return self.ident()
    def params(self, receiver=False):
        self.take("("); out=[]; mut=False
        if receiver:
            self.take("&")
            if self.peek("mut"): self.take(); mut=True
            self.take("self")
        while not self.peek(")"):
            self.take(",")
            name=self.ident(); self.take(":"); out.append((name,self.type()))
        self.take(")"); return mut,out
    def program(self):
        data={}; machines={}
        while self.peek() is not None:
            if self.peek("data"):
                d=self.data();
                if d.name in data: raise Reject("duplicate data")
                data[d.name]=d
            elif self.peek("machine"):
                m=self.machine(); key=(m.owner,m.name)
                if key in machines: raise Reject("duplicate machine")
                machines[key]=m
            else: raise Reject("top-level item")
        return data,machines
    def data(self):
        self.take("data"); name=self.ident(); cp=False
        if self.peek("["): self.take(); self.take("copy"); self.take("]"); cp=True
        self.take("{"); fields=[]; cases=[]
        while not self.peek("}"):
            if self.peek("case"):
                self.take(); cn=self.ident(); payload=[]
                if self.peek("("):
                    self.take()
                    while not self.peek(")"):
                        if payload: self.take(",")
                        pn=self.ident(); self.take(":"); payload.append((pn,self.type()))
                    self.take(")")
                self.take(";"); cases.append((cn,payload))
            else:
                fn=self.ident(); self.take(":"); fields.append((fn,self.type())); self.take(";")
        self.take("}")
        if fields and cases: raise Reject("mixed data")
        return Data(name,cp,fields,cases)
    def machine(self):
        self.take("machine"); owner=self.ident(); self.take("::"); name=self.ident()
        mut,params=self.params(True); result=None
        if self.peek("->"): self.take(); result=self.type()
        self.take("{"); entry=self.statements(stop_state=True); states={}
        while self.peek("state"):
            self.take(); sn=self.ident(); _,sp=self.params(True); self.take("{")
            body=self.statements(stop_state=False); self.take("}")
            if sn in states: raise Reject("duplicate state")
            states[sn]=(sp,body)
        self.take("}"); return Machine(owner,name,mut,params,result,entry,states)
    def statements(self, stop_state):
        out=[]
        while not self.peek("}") and not (stop_state and self.peek("state")):
            if self.peek("transition"):
                self.take(); subject=self.expr(False); self.take("{"); arms=[]
                while not self.peek("}"):
                    owner=self.ident(); self.take("::"); case=self.ident(); binds=[]
                    if self.peek("{"):
                        self.take()
                        while not self.peek("}"):
                            if binds: self.take(",")
                            binds.append(self.ident())
                        self.take("}")
                    self.take("->"); target=self.ident(); self.take("("); args=[]
                    while not self.peek(")"):
                        if args: self.take(",")
                        args.append(self.expr())
                    self.take(")"); arms.append((owner,case,binds,target,args))
                self.take("}"); out.append(("transition",subject,arms)); continue
            left=self.expr()
            if self.peek("="):
                self.take(); right=self.expr(); self.take(";"); out.append(("assign",left,right))
            else:
                if self.peek(";"): self.take()
                out.append(("return",left))
        return out
    def expr(self, allow_record=True):
        if self.peek("true") or self.peek("false"):
            node=("scalar",self.take()=="true")
        elif self.peek() and self.peek().isdigit(): node=("scalar",int(self.take()))
        else:
            name=self.ident()
            if self.peek("::"):
                self.take(); case=self.ident(); entries=[]
                if self.peek("{"):
                    self.take()
                    while not self.peek("}"):
                        if entries: self.take(",")
                        field=self.ident(); self.take(":"); entries.append((field,self.expr()))
                    self.take("}")
                node=("case",name,case,entries)
            elif allow_record and self.peek("{"):
                self.take(); entries=[]
                while not self.peek("}"):
                    if entries: self.take(",")
                    field=self.ident(); self.take(":"); entries.append((field,self.expr()))
                self.take("}"); node=("record",name,entries)
            else: node=("name",name)
        while self.peek("."):
            self.take(); member=self.ident()
            if self.peek("("):
                self.take(); args=[]
                while not self.peek(")"):
                    if args: self.take(",")
                    args.append(self.expr())
                self.take(")"); node=("call",node,member,args)
            else: node=("field",node,member)
        return node


class Eval:
    def __init__(self,data,machines): self.data,self.machines,self.obj=data,machines,{}
    def typed(self, typ, value):
        if typ=="bool":
            if type(value) is not bool: raise Reject("bool type")
        elif typ=="u8":
            if type(value) is not int or not 0<=value<=255: raise Reject("u8 type")
        elif not isinstance(value,tuple) or value[0]!=typ: raise Reject("nominal type")
        return copy.deepcopy(value)
    def expression(self,node,env,owner):
        k=node[0]
        if k=="scalar": return node[1]
        if k=="name":
            if node[1] not in env: raise Reject("unknown value")
            if node[1]=="self": return env[node[1]]
            return copy.deepcopy(env[node[1]])
        if k=="field":
            base=self.expression(node[1],env,owner)
            if base is self.obj: return copy.deepcopy(self.obj[node[2]])
            if not isinstance(base,tuple): raise Reject("field base")
            return copy.deepcopy(base[1][node[2]])
        if k in ("record","case"):
            typ=node[1]; decl=self.data.get(typ)
            if decl is None: raise Reject("constructor type")
            if k=="record": formal=decl.fields; case=None; entries=node[2]
            else:
                case=node[2]; matches=[p for n,p in decl.cases if n==case]
                if len(matches)!=1: raise Reject("case identity")
                formal=matches[0]; entries=node[3]
            if len(entries)!=len(formal) or {n for n,_ in entries}!={n for n,_ in formal}: raise Reject("constructor fields")
            supplied={n:self.expression(x,env,owner) for n,x in entries}
            payload={n:self.typed(t,supplied[n]) for n,t in formal}
            return (typ,payload) if case is None else (typ,case,payload)
        if k=="call":
            receiver=self.expression(node[1],env,owner)
            if receiver is not self.obj: raise Reject("receiver")
            args=[self.expression(x,env,owner) for x in node[3]]
            return self.machine(owner,node[2],args)
        raise Reject("expression")
    def block(self,m,body,env):
        for st in body:
            if st[0]=="assign":
                lhs=st[1]
                if lhs[0]!="field" or lhs[1]!=("name","self"): raise Reject("assignment target")
                field=next((t for n,t in self.data[m.owner].fields if n==lhs[2]),None)
                if field is None: raise Reject("field")
                self.obj[lhs[2]]=self.typed(field,self.expression(st[2],env,m.owner))
            elif st[0]=="return": return self.expression(st[1],env,m.owner)
            else:
                subject=self.expression(st[1],env,m.owner)
                if not isinstance(subject,tuple) or len(subject)!=3: raise Reject("dispatch subject")
                typ,tag,payload=subject; decl=self.data[typ]
                if len(st[2])!=len(decl.cases): raise Reject("nonexhaustive")
                seen=set(); selected=None
                for ao,ac,binds,target,args in st[2]:
                    if ao!=typ or ac in seen: raise Reject("arm owner/duplicate")
                    seen.add(ac); formal=next((p for n,p in decl.cases if n==ac),None)
                    if formal is None or binds!=[n for n,_ in formal]: raise Reject("arm payload")
                    if ac==tag: selected=(binds,target,args,payload)
                if selected is None: raise Reject("selected arm")
                binds,target,args,payload=selected; edge=dict(env); edge.update({n:copy.deepcopy(payload[n]) for n in binds})
                values=[self.expression(x,edge,m.owner) for x in args]
                if target not in m.states: raise Reject("target")
                params,target_body=m.states[target]
                if len(params)!=len(values): raise Reject("target arity")
                target_env={n:self.typed(t,v) for (n,t),v in zip(params,values)}
                target_env["self"]=self.obj
                return self.block(m,target_body,target_env)
        raise Reject("missing result")
    def machine(self,owner,name,args):
        m=self.machines.get((owner,name))
        if m is None or len(args)!=len(m.params): raise Reject("call target")
        env={n:self.typed(t,v) for (n,t),v in zip(m.params,args)}; env["self"]=self.obj
        return self.block(m,m.entry,env)
    def run(self,owner,name):
        decl=self.data[owner]
        for field,typ in decl.fields:
            self.obj[field]=False if typ=="bool" else 0 if typ in ("u8","u32") else None
        return self.machine(owner,name,[])


def decode_frame(raw: bytes):
    if len(raw)<40 or raw[:8]!=b"OMGRFN7\0": raise Reject("OMGRFN7")
    version,flags,oc,ow,ck,el,result,reserved=struct.unpack_from("<8I",raw,8)
    if version!=7 or flags>1 or reserved!=result or len(raw)!=40+oc+ow+ck+el: raise Reject("frame")
    c0=40; w0=c0+oc; k0=w0+ow; e0=k0+ck
    return raw[c0:w0],raw[w0:k0],raw[k0:e0],raw[e0:],result


def sources(comp: bytes) -> tuple[list[str],str,str]:
    manifest=compilation.decode(comp)
    texts=[e.content.decode("ascii") for e in manifest.bundle_entries]
    return (texts,manifest.strings[manifest.root_owner_string_id],
            manifest.strings[manifest.root_machine_string_id])


def check(path: Path) -> None:
    comp,_witness,ckir,_elf,claimed=decode_frame(path.read_bytes())
    texts,owner,machine=sources(comp)
    data,machines=Parser(tokens("\n".join(texts))).program()
    value=Eval(data,machines).run(owner,machine)
    if value!=70 or claimed!=70: raise Reject("source/claimed result")
    module=ir5.decode(ckir)
    if ir5.interpret(module)!=value: raise Reject("CKIR/source result")
    ops=module.tables["operations"]; opcodes={r[3] for r in ops}
    if not {7,10,14}<=opcodes: raise Reject("construction/Copy/Call")
    cases=module.tables["cases"]
    if {cases[r[10]][4] for r in ops if r[3]==14} < {0,1,2,3,4}: raise Reject("constructor arities")
    terms=module.tables["terminators"]
    if {r[4] for r in terms if r[3]==5} < {1,2}: raise Reject("dispatch subjects")
    if not any(r[1]==2 for r in module.tables["case_arm_args"]): raise Reject("payload binding")
    if not any(x>0 for x in module.field_offsets): raise Reject("nonzero self field")


def main():
    if len(sys.argv)!=3 or sys.argv[1]!="check": raise Reject("usage: ... check FRAME")
    check(Path(sys.argv[2]))


if __name__=="__main__":
    try: main()
    except (Reject,ValueError,KeyError,IndexError,struct.error,compilation.CompilationError,bundle.BundleError,ir5.Ckir5Error) as exc:
        print(f"OMGRFN7 R4: {exc}",file=sys.stderr); raise SystemExit(251)
