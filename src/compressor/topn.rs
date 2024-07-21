use std::{cmp, collections::{BTreeMap, BTreeSet}, ops::{Bound::*, RangeBounds, RangeFrom}};
pub struct TopNHeap<T: PartialEq + Ord + Clone, K> {
    priorities: BTreeMap<usize, BTreeSet<T>>,
    values: BTreeMap<T, (usize, K)>,
    zero_point: usize,
    max_length: usize,
}

impl<T: PartialEq + Ord + Clone, K> Default for TopNHeap<T, K> {
    fn default() -> Self {
        Self::new(300)
    }
}


impl<T: PartialEq + Ord + Clone, K> TopNHeap<T, K> {


    pub fn new(max_length: usize) -> Self {
        Self {
            values: BTreeMap::new(),
            priorities: BTreeMap::new(),
            zero_point: 0,
            max_length,
        }
    }
    pub fn get(&self, item: &T) -> Option<&K> {

        self.values.get(item).map(|x|&x.1)
    }
    pub fn insert_and_increase(&mut self, item: T, key: K) {
        let TopNHeap {
            values,
            priorities,
            zero_point,
            max_length,
        } = self;

        //evict any zeros
        let subzeros = priorities.range(0..*zero_point);
        for (_priority, value) in subzeros {
            for item in value {
                values.remove(item);
            }
        }
        *priorities = priorities.split_off(&zero_point);

        //early return if we're full, even after removing the zeros
        if values.len() >= *max_length {
            return ();
        }

        //if we already have the item, then increase its priority
        if let Some(entry) = values.get_mut(&item) {
            let priority = entry.0;

            //take it out of the old priority bucket and put it into the new one
            let itm = self.priorities.get_mut(&priority).unwrap().take(&item).unwrap();
            self.priorities.entry(priority).or_insert_with(|| BTreeSet::new()).insert(itm);

            entry.0 += 1;
        }
        else {
            values.insert(item.clone(), (*zero_point + 1, key));
            priorities.entry(*zero_point + 1).or_insert_with(|| BTreeSet::new()).insert(item);
        }
    }
}

struct CompareByFirst<I: Eq + Ord, T>(I, T);

impl<I: Eq + Ord, T> Eq for CompareByFirst<I, T> {}

impl<I: Eq + Ord, T> Ord for CompareByFirst<I, T> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        Ord::cmp(&self.0, &other.0)
    }
}

impl<I: Eq + Ord, T> PartialEq for CompareByFirst<I, T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<I: Eq + Ord, T> PartialOrd for CompareByFirst<I, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
