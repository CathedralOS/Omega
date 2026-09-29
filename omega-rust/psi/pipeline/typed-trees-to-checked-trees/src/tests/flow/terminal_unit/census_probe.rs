//! Scratch probe: mimic squalr-tests fixture call shapes to find which
//! statement the `outer calls` census declines at (state 16, statement 16).
use crate::tests::flow::terminal_unit::{checked, machine_named};

fn dump_omissions(name: &str, source: &str) {
    let checked = checked(source);
    for machine in checked.machines() {
        if let Some(omission) = checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine.symbol)
        {
            eprintln!("{}: {} -> {:?}", name, machine.name.as_str(), omission.stage);
        }
    }
}

// The fixture_base call shape: static constructor calls, bare receiver
// statements, receiver calls nested as constructor args, ref-arg calls.
const FIXTURE: &str = r#"
    data Dtr { tag: u64 }
    data Win { n: u64 }
    data Reg { base: u64 }
    data Snap { n: u64 }
    data Cn { v: u64 }
    data Fin { c: u64 }
    data Plan { p: u64 }
    data Tl { n: u64 }
    data Ep { n: u64 }
    data Sc { n: u64 }
    data Rep { n: u64 }
    data Out { n: u64 }
    data Al [copy] { case A1; case A2; }

    machine Dtr::new(v: u64) -> Dtr { Dtr { tag: v } }
    machine Dtr::get_al(&self) -> Al { Al::A1 }
    machine Win::of_length(n: u64) -> Win { Win { n: n } }
    machine Win::set_element(&mut self, i: u64, v: u64) { }
    machine Win::read(&self) -> u64 { 0 }
    machine Reg::new(b: u64, s: u64) -> Reg { Reg { base: b } }
    machine Reg::clone_from(&mut self, src: &Reg) { }
    machine Reg::from_elements(b: u64, w: Win, s: u64) -> Reg { Reg { base: b } }
    machine Reg::wrap(inner: Reg) -> Reg { Reg { base: inner.base } }
    machine Reg::from_elements_ref(b: u64, w: &Win, s: u64) -> Reg { Reg { base: b } }
    machine Reg::from_elements_mut(b: u64, w: &mut Win, s: u64) -> Reg {
        let n: u64 = w.read();
        let m: u64 = Reg::take_ref(w);
        Reg { base: ((n as u64 in Wrapping) + (m as u64 in Wrapping)) as u64 }
    }
    machine Reg::take_ref(w: &Win) -> u64 { 0 }
    machine Reg::install_elements(&mut self, b: u64, w: Win, s: u64) { }
    machine Snap::new() -> Snap { Snap { n: 0 } }
    machine Snap::set_regions(&mut self, r: &[Reg; 1], n: u64) { }
    machine Snap::set_regions2(&mut self) { }
    machine Cn::new(a: Dtr, v: u64) -> Cn { Cn { v: v } }
    machine Fin::new(c: Cn, u: u64) -> Fin { Fin { c: 0 } }
    machine Fin::get_a(&self) -> Dtr { Dtr { tag: 0 } }
    machine Fin::get_b(&self) -> u64 { 0 }
    machine Fin::get_c(&self) -> u64 { 0 }
    machine Fin::has_d(&self) -> bool { false }
    machine Fin::get_e(&self) -> Sc { Sc { n: 0 } }
    machine Fin::get_al(&self) -> Al { Al::A1 }
    machine Fin::get_p(&self) -> u64 { 0 }
    machine Plan::new(a: Dtr, b: u64, c: u64, d: bool, e: Sc, f: Al) -> Plan {
        Plan { p: 0 }
    }
    machine Plan::set_profile(&mut self, p: u64, a: bool, b: bool, c: bool, d: Al) { }
    machine Tl::empty() -> Tl { Tl { n: 0 } }
    machine Tl::empty_of(d: Dtr, a: Al, n: u64) -> Tl { Tl { n: 0 } }
    machine Tl::push(&mut self, p: &Plan) -> bool { false }
    machine Ep::new(a: Al, m: u64, s: bool) -> Ep { Ep { n: 0 } }
    machine Ep::push_plans(&mut self, d: &Dtr, l: &Tl) { }
    machine Ep::push_one(&mut self, d: &Dtr) { }
    machine Ep::push_l(&mut self, l: &Tl) { }
    machine Sc::new(n: u64) -> Sc { Sc { n: 0 } }
    machine Rep::get_a(&self) -> u64 { 0 }
    machine Rep::get_b(&self) -> u64 { 0 }
    machine Rep::get_c(&self) -> u64 { 0 }
    machine Snap::collect(&self, d: &Dtr) -> Out { Out { n: 0 } }
    machine Snap::collect_no(&self) -> Out { Out { n: 0 } }
    machine Snap::count(&self) -> u64 { 0 }
    machine Out::get_n(&self) -> u64 { 0 }
    machine Out::get(&self, i: u64) -> u64 { 0 }
    data Enc { n: u64 }
    machine Enc::new(n: u64) -> Enc { Enc { n: n } }
    data Iter {
        enc: Enc
        idx: u64
        ec: u64
        st: u64
        lp: u64
        ct: Al
        ov: u64
        hf: bool
        sf: Sc
        ft: bool
    }
    data Ss {
        kind_field: u64
        scan_field: Iter
        pending: Reg
    }
    data Scn { n: u64 }
    machine Scn::scan2(s: &mut Snap, e: &Ep) -> Rep { Rep { n: 0 } }
    machine Scn::scan3(s: &mut Snap, e: &Ep, c: &mut Sc) -> Rep { Rep { n: 0 } }
    machine Scn::scan4(s: &mut Snap, sf: &mut [Reg; 1]) -> Rep { Rep { n: 0 } }
    machine Scn::scan5(ce: &mut [u64; 8]) -> Rep { Rep { n: 0 } }
    machine Scn::scan6(sf: &mut [Reg]) -> Rep { Rep { n: 0 } }
    machine Snap::read_regions(&self, n: u64) -> u64 { 0 }
    data RA {
        snap: Snap
        ep: Ep
        ctl: Sc
    }
    machine RA::new() -> RA {
        let fsnap: Snap = Snap::new();
        let eal: Al = Al::A1;
        let fep: Ep = Ep::new(eal, 0, true);
        let fctl: Sc = Sc::new(0);
        let out: RA = RA {
            snap: fsnap,
            ep: fep,
            ctl: fctl
        };
        out
    }
    machine Scn::scan(
        s: &mut Snap,
        e: &Ep,
        c: &mut Sc,
        sf: &mut [Reg; 1],
        sc1: &mut [Tl; 1],
        sc2: &mut [Tl; 1],
        fo: &mut [Reg; 8],
        ce: &mut [u64; 8],
        pe: &mut [u64; 8],
        dc: &mut [Ss; 1],
        ra: &mut [Reg; 8],
        rb: &mut [Reg; 8]
    ) -> Rep { Rep { n: 0 } }

    data Main {
        tag: u64
        f_snap: Snap
        f_ctl: Sc
        f_io: RA
        f_sf: [Reg; 1]
        f_cur: [u64; 8]
        f_regions: [Reg; 1]
        f_report: Rep
        f_r3: Rep
        f_r4: Rep
        f_r5: Rep
        f_win: Win
    }

    machine Main::run(&mut self) {
        let id: u64 = 7;
        let a: Dtr = Dtr::new(id);
        let c5: Dtr = Dtr::new(id);
        let seed: Dtr = Dtr::new(id);
        let res: Dtr = Dtr::new(id);
        let emit: Dtr = Dtr::new(id);
        let mut w: Win = Win::of_length(4);
        w.set_element(0, 0);
        w.set_element(1, 127);
        w.set_element(2, 127);
        w.set_element(3, 1);
        let regions: [Reg; 1] = [Reg::from_elements(4096, w, 1)];
        let mut snapshot: Snap = Snap::new();
        snapshot.set_regions(&regions, 1);
        let cn: Cn = Cn::new(c5, 127);
        let fin: Fin = Fin::new(cn, 1);
        let pa: Dtr = fin.get_a();
        let pe: Sc = fin.get_e();
        let al: Al = Al::A1;
        let mut plan: Plan = Plan::new(pa, fin.get_b(), fin.get_c(), fin.has_d(), pe, al);
        let al2: Al = a.get_al();
        plan.set_profile(fin.get_p(), true, false, false, al2);
        let pushed_plan: Plan = plan;
        let mut list: Tl = Tl::empty();
        _ = list.push(&pushed_plan);
        let ep_al: Al = Al::A1;
        let mut ep: Ep = Ep::new(ep_al, 0, true);
        let pushed_list: Tl = list;
        ep.push_plans(&a, &pushed_list);
        let mut ctl: Sc = Sc::new(0);
        let seed_f: Reg = Reg::new(0, 0);
        let mut seed_filters: [Reg; 1] = [seed_f];
        let col_al_s: Al = Al::A1;
        let coll_s: Tl = Tl::empty_of(seed, col_al_s, 1);
        let mut seed_cols: [Tl; 1] = [coll_s];
        let col_al_r: Al = Al::A1;
        let coll_r: Tl = Tl::empty_of(res, col_al_r, 1);
        let mut res_cols: [Tl; 1] = [coll_r];
        let f0: Reg = Reg::new(0, 0);
        let f1: Reg = Reg::new(0, 0);
        let f2: Reg = Reg::new(0, 0);
        let f3: Reg = Reg::new(0, 0);
        let f4: Reg = Reg::new(0, 0);
        let f5: Reg = Reg::new(0, 0);
        let f6: Reg = Reg::new(0, 0);
        let f7: Reg = Reg::new(0, 0);
        let mut filter_out: [Reg; 8] = [f0, f1, f2, f3, f4, f5, f6, f7];
        let cur: [u64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let prev: [u64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let idle: Enc = Enc::new(0);
        let pend: Reg = Reg::new(0, 0);
        let mut driver_cell: [Ss; 1] = [
            Ss {
                kind_field: 0,
                scan_field: Iter {
                    enc: idle,
                    idx: 0,
                    ec: 0,
                    st: 1,
                    lp: 0,
                    ct: Al::A1,
                    ov: 0,
                    hf: false,
                    sf: Sc::new(0),
                    ft: false
                },
                pending: pend
            }
        ];
        let ra0: Reg = Reg::new(0, 0);
        let ra1: Reg = Reg::new(0, 0);
        let ra2: Reg = Reg::new(0, 0);
        let ra3: Reg = Reg::new(0, 0);
        let ra4: Reg = Reg::new(0, 0);
        let ra5: Reg = Reg::new(0, 0);
        let ra6: Reg = Reg::new(0, 0);
        let ra7: Reg = Reg::new(0, 0);
        let mut round_a: [Reg; 8] = [ra0, ra1, ra2, ra3, ra4, ra5, ra6, ra7];
        let rb0: Reg = Reg::new(0, 0);
        let rb1: Reg = Reg::new(0, 0);
        let rb2: Reg = Reg::new(0, 0);
        let rb3: Reg = Reg::new(0, 0);
        let rb4: Reg = Reg::new(0, 0);
        let rb5: Reg = Reg::new(0, 0);
        let rb6: Reg = Reg::new(0, 0);
        let rb7: Reg = Reg::new(0, 0);
        let mut round_b: [Reg; 8] = [rb0, rb1, rb2, rb3, rb4, rb5, rb6, rb7];
        let scan_plan: Ep = ep;
        let report: Rep = Scn::scan(
            &mut snapshot,
            &scan_plan,
            &mut ctl,
            &mut seed_filters,
            &mut seed_cols,
            &mut res_cols,
            &mut filter_out,
            &mut cur,
            &mut prev,
            &mut driver_cell,
            &mut round_a,
            &mut round_b);
        let snap_done: Snap = snapshot;
        let out: Out = snap_done.collect(&emit);
        let rc: u64 = snap_done.count();
        let sb: u64 = report.get_a();
        let pr: u64 = report.get_b();
        let tr: u64 = report.get_c();
        let e0: u64 = out.get_n();
        let e1: u64 = out.get(0);
        let e2: u64 = out.get(1);
        transition rc == 1 && sb == 4 && pr == 1 && tr == 2 && e0 == 2 && e1 == 4097 && e2 == 4098 {
            true -> done()
            false -> failed()
        }
        state done(&mut self) { }
        state failed(&mut self) { }
    }

    machine Main::scan_probe(&mut self) {
        let srf: Reg = Reg::new(0, 0);
        self.f_sf[0].clone_from(&srf);
        self.f_cur[0] = 0;
        self.f_snap = Snap::new();
        self.f_win = Win::of_length(4);
        self.f_win.set_element(0, 0);
        let regions: [Reg; 1] = [Reg::from_elements_mut(4096, &mut self.f_win, 1)];
        self.f_snap.set_regions(&regions, 1);
        let outer: Reg = Reg::wrap(Reg::new(0, 0));
        self.f_regions[0].clone_from(&outer);

        let rr: u64 = self.f_snap.read_regions(1);
        self.f_io = RA::new();
        let rep: Rep = Scn::scan2(&mut self.f_io.snap, &self.f_io.ep);
        _ = Scn::scan4(&mut self.f_io.snap, &mut self.f_sf);
        _ = Scn::scan5(&mut self.f_cur);
        let sb: u64 = rep.get_a();
        transition sb == 0 {
            true -> done()
            false -> failed()
        }
        state failed(&mut self) { }
        state done(&mut self) { }
    }
"#;

const TWINS: &str = r#"
    machine Main::twin(&mut self) {
        let id: u64 = 7;
        let a: Dtr = Dtr::new(id);
        let fin: Fin = Fin::new(Cn::new(a, 1), 1);
        let pa: Dtr = fin.get_a();
        let pb: u64 = fin.get_b();
        let pc: u64 = fin.get_c();
        let pd: bool = fin.has_d();
        let pe: Sc = Sc::new(0);
        let al: Al = Al::A1;
        let plan: Plan = Plan::new(pa, pb, pc, pd, pe, al);
        let done_at: u64 = plan.p;
        transition done_at == 0 {
            true -> done()
            false -> failed()
        }
        state done(&mut self) { }
        state failed(&mut self) { }
    }

    machine Main::single(&mut self) {
        let id: u64 = 7;
        let a: Dtr = Dtr::new(id);
        let fin: Fin = Fin::new(Cn::new(a, 1), 1);
        let plan: Plan = Plan::new(fin.get_a(), fin.get_b(), fin.get_c(), fin.has_d(), fin.get_e(), Al::A1);
        let done_at: u64 = plan.p;
        transition done_at == 0 {
            true -> done()
            false -> failed()
        }
        state done(&mut self) { }
        state failed(&mut self) { }
    }

    machine Main::partial(&mut self) {
        let id: u64 = 7;
        let a: Dtr = Dtr::new(id);
        let fin: Fin = Fin::new(Cn::new(a, 1), 1);
        let pa: Dtr = fin.get_a();
        let pe: Sc = fin.get_e();
        let plan: Plan = Plan::new(pa, fin.get_b(), fin.get_c(), fin.has_d(), pe, Al::A1);
        let done_at: u64 = plan.p;
        transition done_at == 0 {
            true -> done()
            false -> failed()
        }
        state done(&mut self) { }
        state failed(&mut self) { }
    }
"#;

#[test]
fn probe_twins_call_shape() {
    let defs = FIXTURE.split("    data Main").next().unwrap();
    dump_omissions("twins", &format!("{defs}\n    data Main {{ tag: u64 }}\n{TWINS}"));
}

#[test]
fn probe_fixture_call_shape() {
    dump_omissions("fixture", FIXTURE);
    let checked = checked(FIXTURE);
    let row = checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(machine_named(&checked, "Main::run"));
    eprintln!("Main::run omission: {:?}", row.map(|row| row.stage));
}
