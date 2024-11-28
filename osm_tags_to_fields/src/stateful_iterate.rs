pub trait StatefulIterate: Iterator {
    fn stateful_filter<S: Default + IntoIterator<Item = T>, T, TIter: IntoIterator<Item = T>>(
        self,
        state: S,
        func: fn(&mut S, Self::Item) -> TIter,
    ) -> StatefulFilterMap<Self, S, T, TIter>
    where
        Self: Sized,
    {
        StatefulFilterMap::<Self, S, T, TIter> {
            inner: self,
            state: StatefulFilterRemainder::State(state),
            func,
            current_subiter: None,
        }
    }
}

impl<I: Iterator> StatefulIterate for I {}

pub struct StatefulFilterMap<I: Iterator, S: Default + IntoIterator<Item = T>, T, TIter: IntoIterator<Item = T>> {
    state: StatefulFilterRemainder<S, S::IntoIter>,
    func: fn(&mut S, I::Item) -> TIter,
    inner: I,
    current_subiter: Option<TIter::IntoIter>
}

enum StatefulFilterRemainder<A, B> {
    State(A),
    Iterator(B),
}

impl<A: Default, B> StatefulFilterRemainder<A, B> {
    pub fn swap_from_state_to_iterator(&mut self, op: fn(A) -> B) {
        let StatefulFilterRemainder::State(a) = self else {
            return;
        };

        let a = std::mem::take(a);

        let b = op(a);

        *self = StatefulFilterRemainder::Iterator(b);
    }
}

impl<I: Iterator, S: Default + IntoIterator<Item = T>, T, TIter: IntoIterator<Item = T>> Iterator for StatefulFilterMap<I, S, T, TIter> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        use StatefulFilterRemainder::*;

        loop {
            if let Some(i) = &mut self.current_subiter {
                match i.next() {
                    Some(t) => return Some(t),
                    None => self.current_subiter = None,
                }
            }

            match &mut self.state {
                State(state) => match self.inner.next() {
                    Some(i) => self.current_subiter = Some((self.func)(state, i).into_iter()),
                    None => {
                        self.state
                            .swap_from_state_to_iterator(IntoIterator::into_iter);
                    }
                },
                Iterator(iter) => return iter.next(),
            }
        }
    }
}
