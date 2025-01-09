use std::{cmp::Ordering, vec};

use crate::nonempty_vec::NonEmptyUnorderVec;

#[derive(Clone)]
pub struct BTreeVec<K, V> {
    itms: Vec<(K, NonEmptyUnorderVec<V>)>,
    len: usize,
}

impl<K, V> Default for BTreeVec<K, V> {
    fn default() -> Self {
        Self {
            itms: Default::default(),
            len: 0,
        }
    }
}

impl<K: Ord + Clone, V> BTreeVec<K, V> {
    pub fn push(&mut self, key: K, value: V) {
        self.len += 1;

        if self.itms.last().is_some_and(|x| key > x.0) {
            self.itms.push((key, NonEmptyUnorderVec::new(value)));
            return;
        }

        match self.btr_search_by(|x| x.cmp(&key)) {
            Ok(insert_to) => self.itms[insert_to].1.push(value),
            Err(insert_at) => self
                .itms
                .insert(insert_at, (key, NonEmptyUnorderVec::new(value))),
        }
    }

    pub fn get<'a, 'b>(&'a self, key: &'b K) -> Option<&'a NonEmptyUnorderVec<V>> {
        let f = self.itms.binary_search_by_key(&key, |x| &x.0).ok()?;

        self.itms.get(f).map(|x| &x.1)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn with_capacity(len: usize) -> Self {
        Self {
            itms: Vec::with_capacity(len),
            len: 0,
        }
    }

    pub fn new() -> Self {
        Self {
            itms: Vec::new(),
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.itms.is_empty()
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, K, V> {
        Iter {
            inner: self.itms.iter(),
            current_tail_iter: None,
        }
    }

    pub fn btr_search_by(&self, f: impl Fn(&K) -> Ordering) -> Result<usize, usize> {
        let f = |x: &(K, NonEmptyUnorderVec<V>)| f(&x.0);

        return self.itms.binary_search_by(f);
    }
    pub unsafe fn from_sorted_iter_failable<E>(
        len: usize,
        iter: impl Iterator<Item = Result<(K, V), E>>,
    ) -> Result<Self, E> {
        let itms = deduplicate(iter)?;

        Ok(Self { len, itms })
    }

    pub unsafe fn from_raw_parts(len: usize, itms: Vec<(K, NonEmptyUnorderVec<V>)>) -> Self {
        Self { len, itms }
    }
}

fn deduplicate<K: Eq, V, E>(
    mut iter: impl Iterator<Item = Result<(K, V), E>>,
) -> Result<Vec<(K, NonEmptyUnorderVec<V>)>, E> {
    let mut vec = Vec::with_capacity(iter.size_hint().1.unwrap_or_default());

    let Some(v) = iter.next() else {
        return Ok(vec);
    };
    let (k, v) = v?;
    let mut old_key_value = (k, NonEmptyUnorderVec::new(v));

    loop {
        match iter.next() {
            None => {
                vec.push(old_key_value);
                return Ok(vec);
            }
            Some(trier) => {
                let (new_key, new_value) = trier?;

                if new_key == old_key_value.0 {
                    old_key_value.1.push(new_value);
                } else {
                    let mut new_key_value = (new_key, NonEmptyUnorderVec::new(new_value));
                    std::mem::swap(&mut old_key_value, &mut new_key_value);

                    vec.push(new_key_value);
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
    fn stateless_next<'s>(&'s self, state: Self::State) -> Option<(Self::State, Self::Item<'s>)>;
}

impl<K: Ord + 'static, V: 'static> BTreeVec<K, V> {
    pub fn begin_range(
        &self,
        start: K
    ) -> <Self as SeparateStateIteratable>::State {
        let start_col = match self.itms.binary_search_by_key(&&start, |x| &x.0) {
            Ok(i) => i,
            Err(i) => i,
        };

        (start_col, 0)
    }
}

impl<K: 'static, V: 'static> SeparateStateIteratable for BTreeVec<K, V> {
    type State = (usize, usize);

    type Item<'s> = (&'s K, &'s V);

    fn begin_iteration(&self) -> Self::State {
        (0, 0)
    }

    fn stateless_next<'s>(&'s self, state: Self::State) -> Option<(Self::State, Self::Item<'s>)> {
        let (mut r, mut c) = state;
        loop {
            let (key, column) = self.itms.get(r)?;

            let Some(value) = column.get(c) else {
                r += 1;
                c = 0;
                continue;
            };

            c += 1;

            return Some(((r, c), (key, value)));
        }
    }
}

pub struct Iter<'a, K, V> {
    inner: std::slice::Iter<'a, (K, NonEmptyUnorderVec<V>)>,
    current_tail_iter: Option<(&'a K, crate::nonempty_vec::Iter<'a, V>)>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let current_tail_iter = match &mut self.current_tail_iter {
                Some(t) => t,
                None => {
                    let (k, vs) = self.inner.next()?;
                    self.current_tail_iter.insert((k, vs.iter()))
                }
            };

            match current_tail_iter.1.next() {
                Some(t) => return Some((current_tail_iter.0, t)),
                None => {
                    self.current_tail_iter = None;
                }
            }
        }
    }
}

pub struct IntoIter<K: Clone, V> {
    inner: vec::IntoIter<(K, NonEmptyUnorderVec<V>)>,
    current: Option<(K, std::vec::IntoIter<V>)>,
}

impl<K: Clone, V> IntoIterator for BTreeVec<K, V> {
    type Item = (K, V);

    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.itms.into_iter(),
            current: None,
        }
    }
}

impl<K: Clone, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (key, values) = match &mut self.current {
                Some(t) => t,
                None => {
                    let (k, vs) = self.inner.next()?;

                    let (front, iter) = vs.into_iter_with_front();
                    self.current = Some((k.clone(), iter));

                    return Some((k, front));
                }
            };

            match values.next() {
                Some(t) => return Some((key.clone(), t)),
                None => {
                    self.current.take();
                    continue;
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

        let mut v_vec = v.into_iter().collect::<Vec<_>>();
        v_vec.sort();

        assert_eq!(
            vec![(1, 10), (2, 1), (2, 3), (2, 10), (8, 1), (8, 3)],
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
}
