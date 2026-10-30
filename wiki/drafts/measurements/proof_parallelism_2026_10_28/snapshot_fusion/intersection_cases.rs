use std::sync::Arc;
use std::borrow::Borrow;
mod semantic_vocabulary {
    #[derive(Clone,Debug,Eq,PartialEq,Ord,PartialOrd)]
    pub struct Proposition { pub root:u8,pub version:u8 }
}
fn original_intersection(
    retained: &mut Arc<Vec<Arc<semantic_vocabulary::Proposition>>>,
    incoming: &[Arc<semantic_vocabulary::Proposition>],
) -> bool {
    let mut remaining = incoming.iter().peekable();
    let unchanged = retained.iter().all(|premise| {
        while remaining
            .peek()
            .is_some_and(|candidate| *candidate < premise)
        {
            remaining.next();
        }
        remaining
            .peek()
            .is_some_and(|candidate| *candidate == premise)
    });
    if unchanged {
        return false;
    }
    retain_common_premises(Arc::make_mut(retained), incoming)
}

fn retain_common_snapshot(
    retained: &mut Arc<Vec<Arc<semantic_vocabulary::Proposition>>>,
    incoming: &[Arc<semantic_vocabulary::Proposition>],
) -> bool {
    let mut remaining = incoming.iter().peekable();
    let mut is_common = |premise: &Arc<semantic_vocabulary::Proposition>| {
        while remaining
            .peek()
            .is_some_and(|candidate| *candidate < premise)
        {
            remaining.next();
        }
        remaining
            .peek()
            .is_some_and(|candidate| *candidate == premise)
    };
    let Some(first_missing) = retained.iter().position(|premise| !is_common(premise)) else {
        return false;
    };
    // The prefix and first loss are already classified; keep the arrival cursor.
    let retained = Arc::make_mut(retained);
    let mut write = first_missing;
    for read in first_missing + 1..retained.len() {
        if is_common(&retained[read]) {
            retained.swap(write, read);
            write += 1;
        }
    }
    retained.truncate(write);
    true
}

fn retain_common_premises<
    Retained: Borrow<semantic_vocabulary::Proposition>,
    Incoming: Borrow<semantic_vocabulary::Proposition>,
>(
    retained: &mut Vec<Retained>,
    incoming: &[Incoming],
) -> bool {
    let previous = retained.len();
    let mut remaining = incoming
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    retained.retain(|premise| {
        let premise: &semantic_vocabulary::Proposition = premise.borrow();
        while remaining
            .peek()
            .is_some_and(|candidate| *candidate < premise)
        {
            remaining.next();
        }
        remaining
            .peek()
            .is_some_and(|candidate| *candidate == premise)
    });
    retained.len() != previous
}


fn rows(mask:u16)->Vec<Arc<semantic_vocabulary::Proposition>> {
    (0..8).filter(|i|mask&(1<<i)!=0).map(|i|Arc::new(semantic_vocabulary::Proposition{root:i/2,version:i%2})).collect()
}
fn main(){
    let mut cases=0;
    for left in 0..256 {
        for right in 0..256 {
            for mode in 0..3 {
                let input=rows(left);let incoming=rows(right);
                let expected:Vec<_>=input.iter().filter(|row|incoming.contains(row)).cloned().collect();
                let mut actual=Arc::new(input.clone());let mut original=Arc::new(input.clone());
                let snapshot=(mode==1).then(||actual.clone());
                let original_snapshot=(mode==1).then(||original.clone());
                let weak=(mode==2).then(||Arc::downgrade(&actual));
                let original_weak=(mode==2).then(||Arc::downgrade(&original));
                let address=Arc::as_ptr(&actual);let original_address=Arc::as_ptr(&original);
                let changed=retain_common_snapshot(&mut actual,&incoming);
                let baseline_changed=original_intersection(&mut original,&incoming);
                assert_eq!(changed,baseline_changed);assert_eq!(changed,expected.len()!=input.len());
                assert_eq!(*actual,expected);assert_eq!(*actual,*original);
                assert!(actual.iter().zip(&expected).all(|(a,b)|Arc::ptr_eq(a,b)));
                assert_eq!(Arc::as_ptr(&actual)==address,Arc::as_ptr(&original)==original_address);
                if let Some(snapshot)=snapshot {
                    assert_eq!(*snapshot,input);assert_eq!(*original_snapshot.unwrap(),input);
                    assert_eq!(Arc::ptr_eq(&actual,&snapshot),!changed);
                }
                if let Some(weak)=weak {
                    assert_eq!(weak.upgrade().is_some(),!changed);
                    assert_eq!(original_weak.unwrap().upgrade().is_some(),!changed);
                }
                cases+=1;
            }
        }
        let mut actual=Arc::new(rows(left));let incoming=actual.clone();
        assert!(!retain_common_snapshot(&mut actual,&incoming));assert!(Arc::ptr_eq(&actual,&incoming));cases+=1;
    }
    assert_eq!(cases,196864);
    println!("{{\"cases\":{},\"ordered_universe\":8,\"unique_shared_weak\":true,\"incoming_alias\":true}}",cases);
}
