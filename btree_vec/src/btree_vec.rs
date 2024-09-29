use std::{
    cmp::Ordering,
    collections::{vec_deque, VecDeque},
};

use crate::nonempty_vec::NonEmptyUnorderVec;

pub struct BTreeVec<K, V> {
    itms: VecDeque<(K, NonEmptyUnorderVec<V>)>,
}

impl<K, V> Default for BTreeVec<K, V> {
    fn default() -> Self {
        Self {
            itms: Default::default(),
        }
    }
}

impl<K: Ord + Clone, V> BTreeVec<K, V> {
    pub fn push(&mut self, key: K, value: V) {
        if self.itms.back().is_some_and(|x| key > x.0) {
            self.itms.push_back((key, NonEmptyUnorderVec::new(value)));
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
        self.itms.len()
    }

    pub fn with_capacity(len: usize) -> Self {
        Self {
            itms: VecDeque::with_capacity(len),
        }
    }

    pub fn new() -> Self {
        Self {
            itms: VecDeque::new(),
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
}

pub struct Iter<'a, K, V> {
    inner: vec_deque::Iter<'a, (K, NonEmptyUnorderVec<V>)>,
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
    inner: vec_deque::IntoIter<(K, NonEmptyUnorderVec<V>)>,
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
