use std::{cmp, collections::{BTreeMap, BTreeSet, VecDeque}, fmt::Debug, ops::{Bound::*, RangeBounds, RangeFrom}};
pub struct TopNHeap<const SIZE: usize, T: PartialEq + Ord + Clone, K> {
    priorities: VecDeque<(T, K)>,
}

impl<const SIZE: usize, T: PartialEq + Ord + Clone, K: Debug> Default for TopNHeap<SIZE, T, K> {
    fn default() -> Self {
        Self::new()
    }
}


impl<const SIZE: usize, T: PartialEq + Ord + Clone, K: Debug> TopNHeap<SIZE, T, K> {


    pub fn new() -> Self {
        Self {
            priorities: VecDeque::with_capacity(SIZE),
        }
    }
    pub fn get(&self, item: &T) -> Option<&K> {

        self.priorities.iter().find(|x| x.0 == *item).map(|x| &x.1)
    }
    pub fn insert_and_increase(&mut self, item: T, key: K) {
        let TopNHeap {
            priorities,
        } = self;

        let idx = (priorities).iter().enumerate().find(|x| x.1.0 == item).map(|x| x.0);

        //if it's already in the deque, then move it to the front
        if let Some(i) = idx {
            let itm = priorities.remove(i).unwrap();

            priorities.push_front(itm);
        } else {
            //remove the last item and place this one in the front!
            if priorities.len() >= SIZE {
                priorities.pop_back();
            }
            priorities.push_front((item, key))
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
