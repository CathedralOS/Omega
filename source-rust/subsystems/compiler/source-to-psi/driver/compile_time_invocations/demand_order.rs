//! Which compile-time value waits for which, and the rounds that follow.
//!
//! A value is held by declarations and reads declarations. `[u8; width()]`
//! written in `main` is held by `main` and reads `width` and everything
//! `width` reaches. A constant is held by its own declaration. Value A waits
//! for value B when A reads a declaration that holds B: A's computation would
//! be refused there until B has its result.
//!
//! The values are numbered by position, and every table here is a flat row
//! indexed by that position.

use symbols::SymbolHandle;

fn key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// What the next round holds.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Round {
    /// These values wait for nothing unsettled. None of them waits for
    /// another, so one program serves them all.
    Evaluate(Vec<usize>),
    /// Every value is settled.
    Finished,
    /// Values remain and each waits for another. The positions are one such
    /// circle, each waiting for the next and the last for the first.
    NoFirstStep(Vec<usize>),
}

pub(super) struct DemandOrder {
    /// How many unsettled values each value waits for.
    waiting_for: Vec<u32>,
    /// `needs[need_starts[v]..need_starts[v + 1]]` are the values `v` waits for.
    need_starts: Vec<u32>,
    needs: Vec<u32>,
    /// `readers[reader_starts[v]..reader_starts[v + 1]]` are the values that wait for `v`.
    reader_starts: Vec<u32>,
    readers: Vec<u32>,
    settled: Vec<bool>,
    /// Unsettled values that wait for nothing, in position order.
    ready: Vec<u32>,
    unsettled: usize,
}

impl DemandOrder {
    /// `holders[v]` are the declarations that hold value `v`. `reads[v]` are
    /// the declarations its computation reaches, sorted by arena index and
    /// generation. A value that reads its own holder waits for itself and is
    /// never ready, unless `waits_for_itself[v]` is false: such a value is
    /// judged with its own result absent, and each use inside its computation
    /// that needs that result is refused there.
    pub(super) fn new(
        holders: &[&[SymbolHandle]],
        reads: &[Vec<SymbolHandle>],
        waits_for_itself: &[bool],
    ) -> Self {
        let count = holders.len();
        assert_eq!(count, reads.len(), "every value has holders and reads");
        assert_eq!(
            count,
            waits_for_itself.len(),
            "every value says whether it waits for itself"
        );
        let mut held = holders
            .iter()
            .enumerate()
            .flat_map(|(value, holders)| {
                holders
                    .iter()
                    .map(move |holder| (key(*holder), value as u32))
            })
            .collect::<Vec<_>>();
        held.sort_unstable();
        held.dedup();
        let mut need_starts = Vec::with_capacity(count + 1);
        let mut needs = Vec::new();
        let mut reader_counts = vec![0u32; count];
        for (reader, read) in reads.iter().enumerate() {
            need_starts.push(needs.len() as u32);
            let first = needs.len();
            for declarations in read.chunk_by(|a, b| a == b) {
                let holder = key(declarations[0]);
                let start = held.partition_point(|&(candidate, _)| candidate < holder);
                for &(_, value) in held[start..]
                    .iter()
                    .take_while(|&&(candidate, _)| candidate == holder)
                {
                    if value as usize != reader || waits_for_itself[reader] {
                        needs.push(value);
                    }
                }
            }
            needs[first..].sort_unstable();
            let mut kept = first;
            for index in first..needs.len() {
                if index == first || needs[index] != needs[kept - 1] {
                    needs[kept] = needs[index];
                    kept += 1;
                }
            }
            needs.truncate(kept);
            for &needed in &needs[first..] {
                reader_counts[needed as usize] += 1;
            }
        }
        need_starts.push(needs.len() as u32);
        let mut reader_starts = Vec::with_capacity(count + 1);
        let mut total = 0u32;
        for &readers in &reader_counts {
            reader_starts.push(total);
            total += readers;
        }
        reader_starts.push(total);
        let mut readers = vec![0u32; total as usize];
        let mut next = reader_starts[..count].to_vec();
        let mut waiting_for = Vec::with_capacity(count);
        for value in 0..count {
            let row = &needs[need_starts[value] as usize..need_starts[value + 1] as usize];
            waiting_for.push(row.len() as u32);
            for &needed in row {
                readers[next[needed as usize] as usize] = value as u32;
                next[needed as usize] += 1;
            }
        }
        let ready = (0..count as u32)
            .filter(|&value| waiting_for[value as usize] == 0)
            .collect();
        Self {
            waiting_for,
            need_starts,
            needs,
            reader_starts,
            readers,
            settled: vec![false; count],
            ready,
            unsettled: count,
        }
    }

    /// The values `value` waits for, settled or not.
    pub(super) fn needs(&self, value: usize) -> impl Iterator<Item = usize> + '_ {
        self.needs[self.need_starts[value] as usize..self.need_starts[value + 1] as usize]
            .iter()
            .map(|&needed| needed as usize)
    }

    /// Takes the next round. The caller settles every value of an
    /// `Evaluate` round before asking again.
    pub(super) fn next_round(&mut self) -> Round {
        if self.unsettled == 0 {
            return Round::Finished;
        }
        if self.ready.is_empty() {
            return Round::NoFirstStep(self.circle());
        }
        let mut round = std::mem::take(&mut self.ready)
            .into_iter()
            .map(|value| value as usize)
            .collect::<Vec<_>>();
        round.sort_unstable();
        Round::Evaluate(round)
    }

    /// Records that `value` has its result. Each value that waited only for
    /// it joins the next round.
    pub(super) fn settle(&mut self, value: usize) {
        assert!(
            !std::mem::replace(&mut self.settled[value], true) && self.waiting_for[value] == 0,
            "a value settles once, after everything it waits for"
        );
        self.unsettled -= 1;
        for index in self.reader_starts[value]..self.reader_starts[value + 1] {
            let reader = self.readers[index as usize];
            self.waiting_for[reader as usize] -= 1;
            if self.waiting_for[reader as usize] == 0 {
                self.ready.push(reader);
            }
        }
    }

    /// One circle among the unsettled values. Every unsettled value waits for
    /// an unsettled value here, so following those waits must repeat.
    ///
    /// Reads are transitive, so a value in a longer circle often reaches its
    /// own holder too. The walk follows another value when there is one, so
    /// the refusal names the values that wait on each other rather than only
    /// the first.
    fn circle(&self) -> Vec<usize> {
        let unsettled_need = |value: usize| {
            let mut unsettled = self.needs(value).filter(|&needed| !self.settled[needed]);
            let first = unsettled.next()?;
            Some(if first == value {
                unsettled.next().unwrap_or(first)
            } else {
                first
            })
        };
        let Some(mut value) = self.settled.iter().position(|settled| !settled) else {
            return Vec::new();
        };
        let mut path = Vec::new();
        let mut step = vec![usize::MAX; self.settled.len()];
        while step[value] == usize::MAX {
            step[value] = path.len();
            path.push(value);
            let Some(needed) = unsettled_need(value) else {
                // Unreachable while `ready` is empty; name what is known.
                return path;
            };
            value = needed;
        }
        let mut circle = path.split_off(step[value]);
        let first = circle
            .iter()
            .enumerate()
            .min_by_key(|&(_, value)| *value)
            .map_or(0, |(index, _)| index);
        circle.rotate_left(first);
        circle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declarations(count: u32) -> Vec<SymbolHandle> {
        (1..=count)
            .map(|index| SymbolHandle::from_parts(index, 1))
            .collect()
    }

    /// Runs the rounds to the end, settling each value in the order given.
    fn rounds(order: &mut DemandOrder) -> (Vec<Vec<usize>>, Round) {
        let mut seen = Vec::new();
        loop {
            match order.next_round() {
                Round::Evaluate(values) => {
                    for &value in &values {
                        order.settle(value);
                    }
                    seen.push(values);
                }
                end => return (seen, end),
            }
        }
    }

    #[test]
    fn a_value_is_evaluated_after_every_value_it_reads() {
        // Value 0 reads the holder of value 1, which reads the holder of
        // value 2. Value 3 reads nothing. Positions alone would run 0 first.
        let d = declarations(4);
        let holders: [&[SymbolHandle]; 4] = [&d[0..1], &d[1..2], &d[2..3], &d[3..4]];
        let reads = vec![vec![d[1]], vec![d[2]], Vec::new(), Vec::new()];
        let mut order = DemandOrder::new(&holders, &reads, &vec![true; reads.len()]);
        let (seen, end) = rounds(&mut order);
        assert_eq!(seen, [vec![2, 3], vec![1], vec![0]]);
        assert_eq!(end, Round::Finished);
    }

    #[test]
    fn a_shared_holder_waits_for_each_value_of_its_generation_once() {
        let holder = SymbolHandle::from_parts(3, 1);
        let other_generation = SymbolHandle::from_parts(3, 2);
        let holders: [&[SymbolHandle]; 4] = [&[holder], &[other_generation], &[holder], &[]];
        let reads = [vec![], vec![], vec![], vec![holder, holder]];
        let order = DemandOrder::new(&holders, &reads, &[true; 4]);
        assert_eq!(order.needs(3).collect::<Vec<_>>(), [0, 2]);
    }

    #[test]
    fn every_value_appears_in_exactly_one_round() {
        // Values 0 and 1 both read value 3 twice over, through two holders.
        // Value 2 reads both of them.
        let d = declarations(6);
        let holders: [&[SymbolHandle]; 4] = [&d[0..1], &d[1..2], &d[2..3], &d[3..5]];
        let reads = vec![
            vec![d[3], d[4]],
            vec![d[3], d[4]],
            vec![d[0], d[1], d[5]],
            Vec::new(),
        ];
        let mut order = DemandOrder::new(&holders, &reads, &vec![true; reads.len()]);
        let (seen, end) = rounds(&mut order);
        assert_eq!(seen, [vec![3], vec![0, 1], vec![2]]);
        assert_eq!(end, Round::Finished);
    }

    #[test]
    fn values_that_wait_on_each_other_have_no_first_step() {
        // Value 1 and value 2 read each other; value 0 waits behind them and
        // value 3 is free. The circle names 1 and 2 only.
        let d = declarations(4);
        let holders: [&[SymbolHandle]; 4] = [&d[0..1], &d[1..2], &d[2..3], &d[3..4]];
        let reads = vec![vec![d[1]], vec![d[2]], vec![d[1]], Vec::new()];
        let mut order = DemandOrder::new(&holders, &reads, &vec![true; reads.len()]);
        let (seen, end) = rounds(&mut order);
        assert_eq!(seen, [vec![3]]);
        assert_eq!(end, Round::NoFirstStep(vec![1, 2]));
    }

    #[test]
    fn a_value_that_reads_its_own_holder_has_no_first_step() {
        let d = declarations(2);
        let holders: [&[SymbolHandle]; 2] = [&d[0..1], &d[1..2]];
        let reads = vec![vec![d[0]], Vec::new()];
        let mut order = DemandOrder::new(&holders, &reads, &vec![true; reads.len()]);
        let (seen, end) = rounds(&mut order);
        assert_eq!(seen, [vec![1]]);
        assert_eq!(end, Round::NoFirstStep(vec![0]));
    }

    #[test]
    fn a_value_judged_without_its_own_result_reads_its_holder_freely() {
        // An access plan's policy can name its own domain. Value 0 reads its
        // holder and value 1's; value 1 reads nothing.
        let d = declarations(2);
        let holders: [&[SymbolHandle]; 2] = [&d[0..1], &d[1..2]];
        let reads = vec![vec![d[0], d[1]], Vec::new()];
        let mut order = DemandOrder::new(&holders, &reads, &[false, true]);
        let (seen, end) = rounds(&mut order);
        assert_eq!(seen, [vec![1], vec![0]]);
        assert_eq!(end, Round::Finished);
    }

    #[test]
    fn a_circle_names_the_values_that_wait_on_each_other() {
        // Both values reach their own holder as well as the other's, as
        // transitive reads do. The refusal names both.
        let d = declarations(2);
        let holders: [&[SymbolHandle]; 2] = [&d[0..1], &d[1..2]];
        let reads = vec![vec![d[0], d[1]], vec![d[0], d[1]]];
        let mut order = DemandOrder::new(&holders, &reads, &[true, true]);
        let (seen, end) = rounds(&mut order);
        assert!(seen.is_empty());
        assert_eq!(end, Round::NoFirstStep(vec![0, 1]));
    }
}
