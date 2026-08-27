#!/usr/bin/env python3
"""Deterministic independent canonical carrier for OMGRFN23 owner tests."""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import struct
import sys
import zlib
from pathlib import Path

from omgrfn23_bundle import pack
from omgrfn23_elf import reconstruct

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[3] / "source/on-ramp/omega-bootstrap/gates"


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


SOURCE_FIXTURE = load("omgrfn23_source_fixture",
                      GATES / "omgrsw12_token_stream_fixture.py")
CKIR_FIXTURE = load("omgrfn23_ckir_fixture",
                    GATES / "delta-checked-ir-v20-fixture.py")

# Frozen canonical OMGRSWC12 wire reconstruction.  This literal is decoded
# locally; neither resolver implementation nor a producer verdict is invoked.
_WITNESS_B85 = b"""
c-n<oNsJU#7_L`UT~*y(Jxfn_4>N4Tz9=p@Y|4&+2s4bJEG~fSAZj#5MTLXLphiqY3_)XJVkB??jfvr4Opt^`g2cpl0Hef%i3ju`
9x!?V>*fF6)Yp%y@g<e&df)%He|i7XYx`ENTfcFc6Fa({bheFloc<Axqmr*i@<hp#bUB6Oc^uzf@|2UO1#dTbT9c<n-?}OM*7QE+
2$Cnv!TYJ-SuDTw@3~GFkRB(i?ZX`VJJUYj$!`j{%E4XWZVqnt>*e4+Jz1?l{K#qn@E`{dfrmMGG|X7tioo;EUjm+Y{xa~q^T%|C
tX9cc`zqdAuiOlRt4VX9YhI3Ab!*dS7-oDs&Uj5*EU#)Fjukqz>=gE}_*$wO@Inq=2VQB~_3--I4!J5Cp~!}ol1{s%#~$8$a3)2(
^GFx31oB&v+<fSY9$ze_ax0J?mqD|+Bz#w-djWW%_r0Jjv<~U5eviVKmmAsoqIF?{lkSaWJb-?eFx|f$XY(u_{($t7#~dYl^hn3s
!_*Ev;_c*6@Q~Kf>n;n0M-i>B`v*fgN0E4|ZKS6p-z7_S^GY&*nRu<|-C2@3W8#PIf2Abf72=2fy|*NLZ=tj4{)3=zCHZTb!=P_N
e*wQwfxeyYaNYki_ESZFi2lpal@9br=&zS$of_$grdeB-?>gN}nr3HN&f1{+Nbliu=u#)iMvwiWEPEJ1a@KqM2kUmB-<)}LEWGbV
e*m7$k7dsz$zSRIfmqHpigZ}xuQr<R+lcJv1mwuNYxH1@K7&eem+s&3$um89PP?+UvUGC|=6S0iZ)NG$J>cOxq3|s4mbqa00;ao2
`s7%)beZ(uvF>9JXUg)ivy{wZ+p}&9mFzXJ_@UbdC431jUg$Qcgy)gP2i;y!={=?_pSwq;_NOeLxeu6;4g1;;akt07BEpU+p*449
k+9<`wKZk=n3F1vqm+~0mAai#>0P3*u4(77UOU|frv0W;8&Fu+v`ZM*0j!m<D>z?`?$n{!z?C`c<d=s0nk&3$0IL!<)s-`J0_z}b
rYpQ3LAGz&BFxzZtWH?JEBx(t(s<RhEv}qlBrpmMogG-u_?{7j?E*F$SQlY$x+1eNz`EJDfG=Z#jU?<4&fWuT6k(rX+&EyP3HuUf
7@rY~UBuZZkS{asUz}kgFdiFta)wD1ADGtZ$$FD11~zSiCvuqLr1optY>b-<jO}PCuxY^9j{32e>A={Iwqo3MCjKDo2~YNYJutST
=RH|}1~9gx*FE{Z0T|oS`&jQr@}EQU!#-~U#&&cZbKXpOkZET<v7MO|BbfH9C%m|Y{MwLhda}1(U~EU8FYC_&#&$Hy7n#il#&$Ht
7aq<5#&$Hvmov--#&)y{<K_WlJKBu(ZUx46v>oGa1IBjrqA#+Z4~*^T4PWH202tfRd%o~&AuzV1kFl3Uz}SwCVZFt`#t`;3#w`Ih
mauPehNZwb`~1llomd8J9ASU>qCd9-8_zii#w`anf%Arf$bAK{i7dZ@j9Uq85@B;O?hX^561E!SRsowr*fzutcLJMA*fRx@!)jpD
2-^*8O-3Ge80+;>%xvO=uL>f+eu|q-oE#wkSj&AFyDkuZ4p8oGVuT%m%y}2(GbTpZABbMBqxsAo^GzW8Tn{WH>^#QZ4J;z;VvxpH
z7_9%fqY>D#Xbf*ft+C@<sSz7J`|hVM0Ev&{S}H`ZKj%n!P+9Rt9vPbF<5UT<L)DV8Ekzd_s#uOM=;pak=zXr&{_uD3v3JJh6X!|
y=-;Tn95*hBeC&qP8w4g>{2B6+k?Pj!iq(i^C4gr!e#(_nDQ5c^%dprdjwc3VNYV*qrln-d%Gy-dkk1RVaJLhqe03m40Z<Nwgc;6
eJQ4QW*_OxsY0qFeH4S8gDt!0H$LTe*p)|hGTk3TA5Hzt%+nL3d0#)xGdoD@I|G!DJPSKAwq){<kAl>P8sBc}C?&G(-LdT9(?m!k
3(d6)6I)lT8ifA+OI6mdBupFhnrm<-UZ{{CH*xjh2JV#PeQucA$$|LV{6^1pG;Xf(8UK7TZmt8Excfq!-aQWK&fjsGcRQ3bcUPM8
Tyy^A6_EqiKy?3>O0&L2=zpmqx>h7v>HY(iw2s9!kw2@~))n3VsVzAf`PTRr*>i95-sCs)iRlhjg%?Mv;yb;ZTr#UhsxorGB^|<(
=aTo2lqb5DFUhKzDq7$8kc~Cr<tH_{D?+ROn^y;oC|^`GE~}Qx^F$_|tIOCGb@2x!Yaa7`q%LDm*TqiDG!N&2Jq^T!4T(cy$}gF2
phM<+s*%=&Sa#)MWKyvFnPc&w)zYa#M$F58TlWvv()%f(d3M#(`zfG#-mRtgQ%F4K+IK|v6U(be>yD|XHMb(I+gq1$B^s~$kJr<0
j{o&t_g%uvL$szM8k2Ls)+y&KS$e|xtg>X)uxzE4VTXp!wdg#n_Gsuli!QKgk%rE<=(vW&bOnp9L^{EA0nyb-j|1Xm!}{;1PI??#
bdg0@A-iC@qD9w2cENNdi>{S;-$M0+vPIWMGU4;a7F|2tJ4{!x=&Ct<Zn5Y(a`;>znFiK9=9ABiaF!T*Xu+A=u-B@UJKFz^W%&p5
-(ElIh3!8bt2SW!QOBwe*#6Y9Y6P}lweKg}zdFdDoQ(e(Dj9ys_P4gbwEeE_e{BD2xBmdpdcB+
"""


def canonical_witness(omgcomp: bytes) -> bytes:
    witness = zlib.decompress(base64.b85decode(b"".join(_WITNESS_B85.split())))
    if len(witness) != 7_520:
        raise RuntimeError("frozen OMGRSWC12 extent")
    if struct.unpack_from("<I", witness, 20)[0] != len(omgcomp):
        raise RuntimeError("frozen OMGRSWC12 OMGCOMP pairing")
    if hashlib.sha256(witness).hexdigest() != \
            "8927bb7c515731a40f022583ae5fa721920f21eb46e93d0afd8b5372d5d967d3":
        raise RuntimeError("frozen OMGRSWC12 identity")
    return witness


def components() -> tuple[bytes, bytes, bytes, bytes]:
    omgcomp = SOURCE_FIXTURE.encode(SOURCE_FIXTURE.CANONICAL)
    witness = canonical_witness(omgcomp)
    ckir = CKIR_FIXTURE.encode(CKIR_FIXTURE.tables())
    return omgcomp, witness, ckir, reconstruct(ckir)


def canonical() -> bytes:
    return pack(*components())
