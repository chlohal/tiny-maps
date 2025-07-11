use std::{cmp::Ordering, fmt::Debug, ops::AddAssign, vec};

use crate::nonempty_vec::{self, NonEmptyUnorderVec};

const MAX_ITEMS_FOR_INSERT: usize = 32;

#[derive(Clone, Debug)]
pub struct BTreeVec<K, V> {
    pub(crate) itms: BTreeVecNode<K, V>,
    pub(crate) len: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum BTreeVecNodeValue<K, V> {
    Leaf(NonEmptyUnorderVec<V>),
    ChildList(BTreeVecNode<K, V>),
}

impl<K, V> BTreeVecNodeValue<K, V> {
    fn is_leaf(&self) -> bool {
        match self {
            BTreeVecNodeValue::Leaf(_) => true,
            _ => false,
        }
    }

    fn as_child_list(&self) -> Option<&BTreeVecNode<K, V>> {
        match self {
            BTreeVecNodeValue::Leaf(_) => None,
            BTreeVecNodeValue::ChildList(c) => Some(c),
        }
    }
    fn as_child_list_mut(&mut self) -> Option<&mut BTreeVecNode<K, V>> {
        match self {
            BTreeVecNodeValue::Leaf(_) => None,
            BTreeVecNodeValue::ChildList(c) => Some(c),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BTreeVecNode<K, V> {
    pub(crate) keys: Vec<(K, K)>,
    pub(crate) values: Vec<BTreeVecNodeValue<K, V>>,
}

impl<K, V> Default for BTreeVec<K, V> {
    fn default() -> Self {
        Self {
            itms: BTreeVecNode {
                keys: Vec::default(),
                values: Vec::default(),
            },
            len: 0,
        }
    }
}

impl<K: Ord + Copy + Debug, V: Debug> BTreeVecNode<K, V> {
    fn make_leaf_to_list_with_added(&mut self, index: usize, key: K, new_value: V) {
        let old_key = &mut self.keys[index];
        let value = &mut self.values[index];
        assert!(value.is_leaf());

        let old_value = std::mem::replace(
            value,
            BTreeVecNodeValue::ChildList(BTreeVecNode {
                keys: Vec::with_capacity(2),
                values: Vec::with_capacity(2),
            }),
        );
        let old_value = match old_value {
            BTreeVecNodeValue::Leaf(l) => l,
            BTreeVecNodeValue::ChildList(_) => unreachable!(),
        };

        let (min, max) = if key > old_key.0 {
            let v = value.as_child_list_mut().unwrap();
            v.keys.push((old_key.0, old_key.0));
            v.keys.push((key, key));

            v.values.push(BTreeVecNodeValue::Leaf(old_value));
            v.values
                .push(BTreeVecNodeValue::Leaf(NonEmptyUnorderVec::new(new_value)));
            (old_key.0, key)
        } else {
            let v = value.as_child_list_mut().unwrap();

            v.keys.push((key, key));
            v.keys.push((old_key.0, old_key.0));

            v.values
                .push(BTreeVecNodeValue::Leaf(NonEmptyUnorderVec::new(new_value)));
            v.values.push(BTreeVecNodeValue::Leaf(old_value));

            (key, old_key.0)
        };

        *old_key = (min, max);
    }
    pub fn push(&mut self, key: K, value: V) {
        if self
            .keys
            .last()
            .is_none_or(|(min, max)| key > *max)
        {
            self.keys.push((key, key));
            return self.values
                .push(BTreeVecNodeValue::Leaf(NonEmptyUnorderVec::new(value)));
        }

        let index = self.btree_search(&key);

        match index {
            Ok(i) => match &mut self.values[i] {
                BTreeVecNodeValue::Leaf(v) => return v.push(value),
                BTreeVecNodeValue::ChildList(list) => return list.push(key, value),
            },
            Err(i) => {
                if self.keys.len() < MAX_ITEMS_FOR_INSERT {
                    self.keys.insert(i, (key, key));
                    return self
                        .values
                        .insert(i, BTreeVecNodeValue::Leaf(NonEmptyUnorderVec::new(value)));
                }
                match &mut self.values[i] {
                    BTreeVecNodeValue::Leaf(_) => self.make_leaf_to_list_with_added(i, key, value),
                    BTreeVecNodeValue::ChildList(list) => {
                        //update limits if we're stretching this child list downwards
                        //it's impossible to be stretching it up, since adding onto the end is
                        //special-cased (at the top of this function)
                        self.keys[i].0 = std::cmp::min(self.keys[i].0, key);

                        return list.push(key, value);
                    }
                }
            }
        }
    }

    fn get(&self, key: &K) -> Option<&NonEmptyUnorderVec<V>> {
        let index = self.btree_search(&key).ok()?;

        match &self.values[index] {
            BTreeVecNodeValue::Leaf(l) => Some(l),
            BTreeVecNodeValue::ChildList(c) => c.get(key),
        }
    }

    fn binary_search(&self, key: &K) -> Result<usize, usize> {
        return self.keys.binary_search_by(|(min, max)| {
            if key < min {
                return Ordering::Greater;
            }
            if key == min || key <= max {
                return Ordering::Equal;
            }
            return Ordering::Less;
        });
    }

    fn btree_search(&self, key: &K) -> Result<usize, usize> {
        let keys = &self.keys;

        if self.keys.len() > MAX_ITEMS_FOR_INSERT * 2 {
            return self.binary_search(key);
        }

        for (i, (min, max)) in keys.iter().enumerate() {
            if key < min {
                return Err(i);
            }
            if key == min || key <= max {
                return Ok(i);
            }
        }

        return Err(self.keys.len());
    }
}

impl<K: Ord + Copy + Debug, V: Debug> BTreeVec<K, V> {
    pub fn push(&mut self, key: K, value: V) {
        self.len += 1;

        self.itms.push(key, value)
    }

    pub fn get<'a, 'b>(&'a self, key: &'b K) -> Option<&'a NonEmptyUnorderVec<V>> {
        self.itms.get(key)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn with_capacity(len: usize) -> Self {
        Self {
            itms: BTreeVecNode {
                keys: Vec::with_capacity(len),
                values: Vec::with_capacity(len),
            },
            len: 0,
        }
    }

    pub fn new() -> Self {
        Default::default()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        let keys = self.itms.keys.iter();
        let values = self.itms.values.iter();

        let mut outer_stack = vec![(keys, values)];

        std::iter::from_fn(move || loop {
            let (keys, values) = outer_stack.last_mut()?;
            let Some(k) = keys.next() else {
                outer_stack.pop();
                continue;
            };
            let v = values.next().unwrap();

            match v {
                BTreeVecNodeValue::Leaf(leaf) => return Some((&k.0, leaf)),
                BTreeVecNodeValue::ChildList(BTreeVecNode { keys, values }) => {
                    outer_stack.push((keys.iter(), values.iter()));
                }
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

impl<K: Ord + Copy + Debug, V: Debug> FromIterator<(K, V)> for BTreeVec<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut l = BTreeVec::new();
        for (k, v) in iter.into_iter() {
            l.push(k, v);
        }
        l
    }
}

fn deduplicate<K: Ord + Copy, V, E>(
    mut iter: impl Iterator<Item = Result<(K, V), E>>,
) -> Result<BTreeVecNode<K, V>, E> {
    let mut keys = Vec::with_capacity(iter.size_hint().1.unwrap_or_default());
    let mut values = Vec::with_capacity(iter.size_hint().1.unwrap_or_default());

    let Some(v) = iter.next() else {
        return Ok(BTreeVecNode { keys, values });
    };
    let (k, v) = v?;
    let mut old_key_value = (k, NonEmptyUnorderVec::new(v));

    loop {
        match iter.next() {
            None => {
                keys.push((old_key_value.0, old_key_value.0));
                values.push(BTreeVecNodeValue::Leaf(old_key_value.1));

                return Ok(BTreeVecNode { keys, values });
            }
            Some(trier) => {
                let (new_key, new_value) = trier?;

                debug_assert!(new_key >= old_key_value.0);

                if new_key == old_key_value.0 {
                    old_key_value.1.push(new_value);
                } else {
                    let mut new_key_value = (new_key, NonEmptyUnorderVec::new(new_value));
                    std::mem::swap(&mut old_key_value, &mut new_key_value);

                    keys.push((new_key_value.0, new_key_value.0));
                    values.push(BTreeVecNodeValue::Leaf(new_key_value.1));
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
    type State = Vec<usize>;

    type Item<'s> = (&'s K, &'s NonEmptyUnorderVec<V>);

    fn begin_iteration(&self) -> Self::State {
        vec![0]
    }

    fn stateless_next<'s>(&'s self, indexes: &mut Self::State) -> Option<Self::Item<'s>> {
        //start with [0] to represent the index into the root
        //recursively iterate through each child node.
        //if a leaf is found, then increment the index and yield its value
        //if a inner node is found, then push a 0 onto the stack
        //if the end of the children is found, then pop the index from the stack.

        loop {
            let mut vecs = (&self.itms.keys, &self.itms.values);
            let mut indexes_len = indexes.len();
            let mut i = 0;

            while i < indexes_len {
                let index = indexes[i];

                //if we've gotten to the end of this list, then it MUST be the last index, since indexes are only incremented:
                // - when a leaf is found (which obviously is the last index)
                // - right here, when something is popped.
                //in order to traverse up, we need to recalculate the path, so break.
                if index >= vecs.1.len() {
                    indexes.pop();
                    indexes.last_mut()?.add_assign(1);
                    break;
                }

                match &vecs.1[index] {
                    BTreeVecNodeValue::Leaf(v) => {
                        indexes[i] += 1;
                        return Some((&vecs.0[index].0, v));
                    }
                    BTreeVecNodeValue::ChildList(itms) => {
                        //if a 'leaf' of the indexes chain is found which is actually a
                        //child_list, then push `0` and let the loop take care of descending
                        //into that child
                        if i + 1 == indexes_len {
                            indexes.push(0);
                            indexes_len += 1;
                        }
                        vecs = (&itms.keys, &itms.values);
                    }
                }

                i += 1;
            }
        }
    }
}

pub struct IntoIter<K: Copy, V> {
    kv: Option<(K, nonempty_vec::IntoIter<V>)>,
    outer_stack: Vec<(
        vec::IntoIter<(K, K)>,
        vec::IntoIter<BTreeVecNodeValue<K, V>>,
    )>,
}

impl<K: Copy, V> IntoIterator for BTreeVec<K, V> {
    type Item = (K, V);

    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        let keys = self.itms.keys.into_iter();
        let values = self.itms.values.into_iter();

        let iterations = vec![(keys, values)];

        IntoIter {
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

            let (keys, values) = self.outer_stack.last_mut()?;
            let Some(k) = keys.next() else {
                self.outer_stack.pop();
                continue;
            };
            let v = values.next().unwrap();

            match v {
                BTreeVecNodeValue::Leaf(leaf) => self.kv = Some((k.0, leaf.into_iter())),
                BTreeVecNodeValue::ChildList(BTreeVecNode { keys, values }) => {
                    self.outer_stack
                        .push((keys.into_iter(), values.into_iter()));
                }
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
