use std::{
    cmp::{self, min},
    fmt::Debug,
    ops::Index,
};

pub trait IterIsFinal: Iterator {
    fn is_final<'a>(&'a mut self) -> IsFinal<'a, Self>;
    fn pair_chunk<'a>(&'a mut self) -> PairChunk<'a, Self>;
    fn map_until_finished<'a, R, F: FnMut(Self::Item) -> Finished<R>>(
        &'a mut self,
        func: F,
    ) -> MapUntil<'a, R, Self, F>;
}

pub trait IterTryFlatten<I: IntoIterator, E>: Iterator<Item = Result<I, E>> {
    fn try_flatten<'a>(&'a mut self) -> TryFlatten<'a, Self, I, E>;
}

pub trait IterChunksPadded {
    type Item: Sized + Copy + Default;

    fn chunks_padded<'a>(&'a self, num: usize) -> ChunksPadded<'a, Self::Item>;
}

pub struct ChunksPadded<'a, T: Copy + Default> {
    slice: &'a [T],
    num: usize,
}

pub struct IsFinal<'a, T: Iterator + ?Sized> {
    iter: &'a mut T,
    next: Option<T::Item>,
}

pub struct PairChunk<'a, T: Iterator + ?Sized> {
    iter: &'a mut T,
}

pub struct MapUntil<'a, R, T: Iterator + ?Sized, F: FnMut(T::Item) -> Finished<R>> {
    iter: &'a mut T,
    func: F,
    done: bool,
}

impl<T: Default + Copy> IterChunksPadded for [T] {
    type Item = T;

    fn chunks_padded<'a>(&'a self, num: usize) -> ChunksPadded<'a, T> {
        ChunksPadded { slice: self, num }
    }
}

#[derive(Debug)]
pub struct PaddedChunk<'a, T> {
    slice: &'a [T],
    len: usize,
    def: T,
}

impl<'a, T> PaddedChunk<'a, T> {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn iter<'b: 'a>(&'b self) -> impl Iterator<Item = &'a T> + 'b {
        self.slice
            .iter()
            .chain(std::iter::repeat(&self.def).take(self.len - self.slice.len()))
    }
}

impl<'a, T: PartialEq + Debug> PartialEq for PaddedChunk<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len() {
            return false;
        }

        std::iter::zip(self.iter(), other.iter()).all(|(a, b)| a == b)
    }
}

impl<'a, T: PartialEq + Debug, S: AsRef<[T]>> PartialEq<S> for PaddedChunk<'a, T> {
    fn eq(&self, other: &S) -> bool {
        let other = other.as_ref();

        if self.len != other.len() {
            return false;
        }

        for (i, itm) in other.iter().enumerate() {
            if *itm != self[i] {
                return false;
            }
        }
        return true;
    }
}

impl<'a, T> Index<usize> for PaddedChunk<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index < self.slice.len() {
            return &self.slice[index];
        } else {
            return &self.def;
        }
    }
}

impl<'a, T: Default + Copy + Debug> Iterator for ChunksPadded<'a, T> {
    type Item = PaddedChunk<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.slice.is_empty() {
            return None;
        }

        let (head, tail) = self.slice.split_at(cmp::min(self.slice.len(), self.num));

        self.slice = tail;

        return Some(PaddedChunk {
            slice: head,
            len: self.num,
            def: T::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_padded() {
        let values = vec![1, 2, 3, 4, 5, 6, 7];

        let mut chunks = values.chunks_padded(3);

        assert_eq!(chunks.next().unwrap(), &[1, 2, 3][..]);

        assert_eq!(chunks.next().unwrap(), &[4, 5, 6][..]);

        assert_eq!(chunks.next().unwrap(), &[7, 0, 0][..]);

        assert_eq!(chunks.next(), None);
    }
}

pub struct TryFlatten<'a, T: Iterator<Item = Result<I, E>> + ?Sized, I: IntoIterator, E> {
    current_item: Option<Result<I::IntoIter, E>>,
    iter: &'a mut T,
}

impl<'a, T: Iterator<Item = Result<I, E>> + ?Sized, I: IntoIterator, E> Iterator
    for TryFlatten<'a, T, I, E>
{
    type Item = Result<I::Item, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.current_item.take() {
                Some(Err(e)) => {
                    self.current_item = self.iter.next().map(|x| x.map(IntoIterator::into_iter));
                    return Some(Err(e));
                }
                Some(Ok(ref mut i)) => match i.next() {
                    Some(t) => return Some(Ok(t)),
                    None => {
                        self.current_item =
                            self.iter.next().map(|x| x.map(IntoIterator::into_iter));
                    }
                },
                None => return None,
            }
        }
    }
}

impl<T: Iterator<Item = Result<I, E>>, I: IntoIterator, E> IterTryFlatten<I, E> for T {
    fn try_flatten<'a>(&'a mut self) -> TryFlatten<'a, Self, I, E> {
        TryFlatten {
            current_item: self.next().map(|x| x.map(IntoIterator::into_iter)),
            iter: self,
        }
    }
}

impl<T: Iterator> IterIsFinal for T {
    fn is_final<'a>(&'a mut self) -> IsFinal<'a, Self> {
        IsFinal {
            next: self.next(),
            iter: self,
        }
    }

    fn pair_chunk<'a>(&'a mut self) -> PairChunk<'a, Self> {
        PairChunk { iter: self }
    }

    fn map_until_finished<'a, R, F: FnMut(T::Item) -> Finished<R>>(
        &'a mut self,
        func: F,
    ) -> MapUntil<'a, R, Self, F> {
        MapUntil {
            iter: self,
            func,
            done: false,
        }
    }
}

impl<'a, T: Iterator> Iterator for IsFinal<'a, T> {
    type Item = (bool, T::Item);

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(t) => self.next.replace(t).map(|x| (false, x)),
            None => self.next.take().map(|x| (true, x)),
        }
    }
}

impl<'a, T: Iterator> Iterator for PairChunk<'a, T> {
    type Item = (T::Item, T::Item);

    fn next(&mut self) -> Option<Self::Item> {
        Some((self.iter.next()?, self.iter.next()?))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (a, b) = self.iter.size_hint();
        return (a / 2, b.map(|x| x / 2));
    }
}

pub enum Finished<R> {
    NonFinal(R),
    Final(R),
}

impl<'a, R, T: Iterator, F: FnMut(T::Item) -> Finished<R>> Iterator for MapUntil<'a, R, T, F> {
    type Item = R;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let new_val = (self.func)(self.iter.next()?);

        match new_val {
            Finished::NonFinal(v) => Some(v),
            Finished::Final(v) => {
                self.done = true;

                Some(v)
            }
        }
    }
}
