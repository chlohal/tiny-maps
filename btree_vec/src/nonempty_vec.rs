
#[derive(Clone)]
pub struct NonEmptyUnorderVec<T>(T, Vec<T>);

impl<T> NonEmptyUnorderVec<T> {
    pub fn new(value: T) -> Self {
        Self(value, Vec::with_capacity(0))
    }
    
    pub fn push(&mut self, value: T) {
        self.1.push(value);
    }
    pub fn iter<'a>(&'a self) -> Iter<'a, T> {
        Iter(Some(&self.0), self.1.iter())
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index == 0 {
            return Some(&self.0);
        } else {
            return self.1.get(index - 1);
        }
    }
    
    pub fn into_iter_with_front(self) -> (T, std::vec::IntoIter<T>) {
        (self.0, self.1.into_iter())
    }
    
    pub fn len(&self) -> usize {
        1 + self.1.len()
    }
}

pub struct Iter<'a, T>(Option<&'a T>, std::slice::Iter<'a, T>);

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.take() {
            Some(t) => Some(t),
            None => self.1.next(),
        }
    }
}

pub struct IntoIter<T>(Option<T>, Vec<T>);

impl<T> IntoIterator for NonEmptyUnorderVec<T> {
    type Item = T;

    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(Some(self.0), self.1)
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.1.pop().or_else(|| self.0.take())
    }
}

#[cfg(test)]
mod test {
    use super::NonEmptyUnorderVec;

    #[test]
    pub fn into_iter() {
        let mut v = NonEmptyUnorderVec::new(1);
        v.push(2);
        v.push(3);

        let mut v_vec = v.into_iter().collect::<Vec<_>>();
        v_vec.sort();

        assert_eq!(vec![1, 2, 3], v_vec)
    }

    #[test]
    pub fn iter() {
        let mut v = NonEmptyUnorderVec::new(1);
        v.push(2);
        v.push(3);

        let mut v_vec = v.iter().copied().collect::<Vec<_>>();
        v_vec.sort();

        assert_eq!(vec![1, 2, 3], v_vec)
    }
}