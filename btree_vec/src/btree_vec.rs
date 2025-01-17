use std::{
    cmp::{self, Ordering}, collections::BTreeMap, mem::MaybeUninit, vec
};

use crate::nonempty_vec::{self, NonEmptyUnorderVec};

const MAX_ITEMS_FOR_INSERT: usize = 8;

#[derive(Clone)]
pub struct BTreeVec<K, V> {
    itms: BTreeVecRoot<K, V>,
    len: usize,
}

#[derive(Clone)]
enum BTreeVecRoot<K, V> {
    Empty,
    Capacity(Vec<BTreeVecNode<K, V>>),
    Node(BTreeVecNode<K, V>),
}

#[derive(Clone, Debug)]
enum BTreeVecNode<K, V> {
    Leaf((K, NonEmptyUnorderVec<V>)),
    ChildList {
        min: K,
        max: K,
        itms: Vec<BTreeVecNode<K, V>>,
    },
}

impl<K, V> Default for BTreeVec<K, V> {
    fn default() -> Self {
        Self {
            itms: BTreeVecRoot::Empty,
            len: 0,
        }
    }
}

impl<K: Ord + Copy, V> BTreeVecNode<K, V> {
    fn make_leaf_to_list_with_added(&mut self, key: K, value: V) {
        let old = match self {
            BTreeVecNode::Leaf(l) => l,
            BTreeVecNode::ChildList { .. } => return,
        };
        //fine to cheat and zero this because we're going to overwrite `self` at the end of this block.
        //we're not calling any functions or doing ANYTHING other than initiating 2 vecs, which are certified okay.
        let old = unsafe { std::ptr::read(old) };
        let old_key = old.0;

        let (itms, min, max) = if key > old_key {
            (
                vec![
                    BTreeVecNode::Leaf(old),
                    BTreeVecNode::Leaf((key, NonEmptyUnorderVec::new(value))),
                ],
                old_key,
                key,
            )
        } else {
            (
                vec![
                    BTreeVecNode::Leaf((key, NonEmptyUnorderVec::new(value))),
                    BTreeVecNode::Leaf(old),
                ],
                key,
                old_key,
            )
        };

        unsafe { std::ptr::write(self, BTreeVecNode::ChildList { min, max, itms }) }
    }
    pub fn push(&mut self, key: K, value: V) {
        match self {
            BTreeVecNode::Leaf(old) => {
                if key == old.0 {
                    return old.1.push(value);
                }

                self.make_leaf_to_list_with_added(key, value);
            }
            BTreeVecNode::ChildList { min, max, itms } => {
                if key > *max {
                    *max = key;
                    return itms.push(BTreeVecNode::Leaf((key, NonEmptyUnorderVec::new(value))));
                }

                let index = Self::btree_search(itms, &key);

                if key < *min {
                    *min = key;
                }

                match index {
                    Ok(i) => match &mut itms[i] {
                        BTreeVecNode::Leaf(l) => l.1.push(value),
                        child_list => child_list.push(key, value),
                    },
                    //it must be inserted at index i, displacing other items.
                    //if the item that's already there is a leaf, then take it away and put a child list there.
                    //if the item that's already there is a child list already, then if it's less than 8 items then we can just give up and
                    // do an insert(). if it's a ton of items then split it up!
                    Err(i) => {
                        if itms.len() < MAX_ITEMS_FOR_INSERT {
                            return itms.insert(
                                i,
                                BTreeVecNode::Leaf((key, NonEmptyUnorderVec::new(value))),
                            );
                        }

                        match &mut itms[i] {
                            l @ BTreeVecNode::Leaf(_) => l.make_leaf_to_list_with_added(key, value),
                            child_list => child_list.push(key, value),
                        }
                    }
                }
            }
        }
    }

    fn get(&self, key: &K) -> Option<&NonEmptyUnorderVec<V>> {
        let itms = match self {
            BTreeVecNode::Leaf((k, v)) => {
                if k == key {
                    return Some(v);
                } else {
                    return None;
                }
            }
            BTreeVecNode::ChildList { itms, .. } => itms,
        };
        let index = Self::btree_search(itms, &key).ok()?;

        match self {
            BTreeVecNode::Leaf(_) => unreachable!(),
            BTreeVecNode::ChildList { itms, .. } => itms[index].get(key),
        }
    }

    fn btree_search(itms: &Vec<BTreeVecNode<K, V>>, key: &K) -> Result<usize, usize> {
        for (i, t) in itms.iter().enumerate() {
            match t {
                BTreeVecNode::Leaf((l_key, _)) => if key == l_key {
                    return Ok(i)
                } else if key < l_key {
                    return Err(i);
                },
                BTreeVecNode::ChildList { min, max, .. } => if key < min {
                    return Err(i);
                } else if key < max {
                    return Err(i);
                },
            }
        }

        return Err(itms.len());
    }
}

impl<K: Ord + Copy, V> BTreeVec<K, V> {
    pub fn push(&mut self, key: K, value: V) {
        self.len += 1;

        match &mut self.itms {
            BTreeVecRoot::Empty => {
                self.itms =
                    BTreeVecRoot::Node(BTreeVecNode::Leaf((key, NonEmptyUnorderVec::new(value))));
            }
            BTreeVecRoot::Capacity(cvec) => {
                cvec.push(BTreeVecNode::Leaf((key, NonEmptyUnorderVec::new(value))));
                let cvec = std::mem::replace(cvec, Vec::with_capacity(0));
                self.itms = BTreeVecRoot::Node(BTreeVecNode::ChildList {
                    min: key,
                    max: key,
                    itms: cvec,
                });
            }
            BTreeVecRoot::Node(n) => n.push(key, value),
        }
    }

    pub fn get<'a, 'b>(&'a self, key: &'b K) -> Option<&'a NonEmptyUnorderVec<V>> {
        match &self.itms {
            BTreeVecRoot::Capacity(_) | BTreeVecRoot::Empty => None,
            BTreeVecRoot::Node(node) => node.get(key),
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn with_capacity(len: usize) -> Self {
        Self {
            itms: BTreeVecRoot::Capacity(Vec::with_capacity(len)),
            len: 0,
        }
    }

    pub fn new() -> Self {
        Self {
            itms: BTreeVecRoot::Empty,
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        let t = match &self.itms {
            BTreeVecRoot::Capacity(_) | BTreeVecRoot::Empty => [].iter(),
            BTreeVecRoot::Node(n) => match n {
                leaf @ BTreeVecNode::Leaf(_) => std::slice::from_ref(leaf).iter(),
                BTreeVecNode::ChildList { min, max, itms } => itms.iter(),
            },
        };

        let mut node_iter = t;
        let mut iterations = vec![];

        std::iter::from_fn(move || loop {
            let Some(node) = node_iter.next() else {
                node_iter = iterations.pop()?;
                continue;
            };

            match node {
                BTreeVecNode::Leaf(l) => return Some(l),
                BTreeVecNode::ChildList { itms, .. } => iterations.push(itms.iter()),
            }
        })
        .flat_map(|(k, vs)| vs.iter().map(move |v| (k, v)))
    }

    pub unsafe fn from_sorted_iter_failable<E>(
        len: usize,
        iter: impl Iterator<Item = Result<(K, V), E>>,
    ) -> Result<Self, E> {
        let itms = deduplicate(iter)?;

        Ok(Self { len, itms })
    }
}

fn deduplicate<K: Eq + Copy, V, E>(
    mut iter: impl Iterator<Item = Result<(K, V), E>>,
) -> Result<BTreeVecRoot<K, V>, E> {
    let mut vec = Vec::with_capacity(iter.size_hint().1.unwrap_or_default());

    let Some(v) = iter.next() else {
        return Ok(BTreeVecRoot::Capacity(vec));
    };
    let (k, v) = v?;
    let min_key = k;
    let mut old_key_value = (k, NonEmptyUnorderVec::new(v));

    loop {
        match iter.next() {
            None => {
                let max_key = old_key_value.0;
                vec.push(BTreeVecNode::Leaf(old_key_value));

                return Ok(BTreeVecRoot::Node(BTreeVecNode::ChildList {
                    min: min_key,
                    max: max_key,
                    itms: vec,
                }));
            }
            Some(trier) => {
                let (new_key, new_value) = trier?;

                if new_key == old_key_value.0 {
                    old_key_value.1.push(new_value);
                } else {
                    let mut new_key_value = (new_key, NonEmptyUnorderVec::new(new_value));
                    std::mem::swap(&mut old_key_value, &mut new_key_value);

                    vec.push(BTreeVecNode::Leaf(new_key_value));
                }
            }
        }
    }
}

pub trait SeparateStateIteratable {
    type State;
    type Item<'s>
    where
        Self: 's;

    ///
    /// Retrieve or create initial state to begin iteration.
    /// While this State exists, the underlying container should not be modified.
    /// If it is modified, then any type invariants will not be violated from continued iteration,
    /// but items may be skipped or yielded multiple times.
    fn begin_iteration(&self) -> Self::State;
    fn stateless_next<'s>(&'s self, state: &mut Self::State) -> Option<Self::Item<'s>>;
}

impl<K: Ord + 'static, V: 'static> BTreeVec<K, V> {
    pub fn begin_range(&self, start: K) -> <Self as SeparateStateIteratable>::State {
        todo!()
    }
}

impl<K: 'static, V: 'static> SeparateStateIteratable for BTreeVec<K, V> {
    type State = (bool, Vec<usize>);

    type Item<'s> = (&'s K, &'s NonEmptyUnorderVec<V>);

    fn begin_iteration(&self) -> Self::State {
        (false, vec![0])
    }

    fn stateless_next<'s>(&'s self, state: &mut Self::State) -> Option<Self::Item<'s>> {
        let (done, iterations) = state;

        if *done {
            return None;
        }
        let mut vec = match &self.itms {
            BTreeVecRoot::Capacity(_) | BTreeVecRoot::Empty => return None,
            BTreeVecRoot::Node(BTreeVecNode::Leaf((k, v))) => {
                *done = true;
                return Some((k, v));
            }
            BTreeVecRoot::Node(BTreeVecNode::ChildList { min, max, itms }) => itms,
        };

        loop {
            if iterations.is_empty() {
                return None;
            }
            for i in 0..(iterations.len()) {
                let index = iterations[i];
                if index >= vec.len() {
                    iterations.pop()?;
                    break;
                }
                iterations.last_mut().map(|x| *x += 1);
                match &vec[index] {
                    BTreeVecNode::Leaf((k, v)) => return Some((k, v)),
                    BTreeVecNode::ChildList { itms, .. } => {
                        iterations.push(0);
                        vec = itms;
                        break;
                    }
                }
            }
        }
    }
}

pub struct IntoIter<K: Copy, V> {
    kv: Option<(K, nonempty_vec::IntoIter<V>)>,
    inner: vec::IntoIter<BTreeVecNode<K, V>>,
    outer_stack: Vec<vec::IntoIter<BTreeVecNode<K, V>>>,
}

impl<K: Copy, V> IntoIterator for BTreeVec<K, V> {
    type Item = (K, V);

    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        let t = match self.itms {
            BTreeVecRoot::Capacity(_) | BTreeVecRoot::Empty => Vec::with_capacity(0).into_iter(),
            BTreeVecRoot::Node(n) => match n {
                leaf @ BTreeVecNode::Leaf(_) => vec![leaf].into_iter(),
                BTreeVecNode::ChildList { itms, .. } => itms.into_iter(),
            },
        };

        let node_iter = t;
        let iterations = vec![];

        IntoIter {
            inner: node_iter,
            outer_stack: iterations,
            kv: None,
        }
    }
}

impl<K: Copy, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((k, vs)) = &mut self.kv {
                let Some(v) = vs.next() else {
                    self.kv = None;
                    continue;
                };
                return Some((*k, v));
            }

            let Some(node) = self.inner.next() else {
                self.inner = self.outer_stack.pop()?;
                continue;
            };

            match node {
                BTreeVecNode::Leaf((k, vs)) => self.kv = Some((k, vs.into_iter())),
                BTreeVecNode::ChildList { itms, .. } => self.outer_stack.push(itms.into_iter()),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn into_iter() {
        let mut v = BTreeVec::new();
        v.push(2, 1);
        v.push(2, 3);
        v.push(2, 10);
        v.push(1, 10);
        v.push(8, 1);
        v.push(8, 3);
        v.push(0, 3);

        let mut v_vec = v.into_iter().collect::<Vec<_>>();
        v_vec.sort();

        assert_eq!(
            vec![(0, 3), (1, 10), (2, 1), (2, 3), (2, 10), (8, 1), (8, 3)],
            v_vec
        )
    }

    #[test]
    pub fn iter() {
        let mut v = BTreeVec::new();
        v.push(2, 1);
        v.push(2, 3);
        v.push(8, 3);
        v.push(2, 10);
        v.push(1, 10);
        v.push(8, 1);

        let v_vec = v.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>();

        assert_eq!(
            vec![(1, 10), (2, 1), (2, 3), (2, 10), (8, 3), (8, 1)],
            v_vec
        )
    }

    #[test]
    pub fn stateless_iter() {
        let mut v = BTreeVec::new();
        v.push(2, 1);
        v.push(2, 3);
        v.push(8, 3);
        v.push(2, 10);
        v.push(1, 10);
        v.push(8, 1);

        let mut v_vec = Vec::new();
        let mut state = v.begin_iteration();
        while let Some((k, vs)) = v.stateless_next(&mut state) {
            for v in vs.iter() {
                v_vec.push((*k, *v));
            }
        }

        assert_eq!(
            vec![(1, 10), (2, 1), (2, 3), (2, 10), (8, 3), (8, 1)],
            v_vec
        )
    }
}
