use std::collections::VecDeque;
#[derive(Debug)]
pub struct TopNHeap<const SIZE: usize, T: PartialEq + Ord + Clone, K> {
    priorities: VecDeque<(T, K)>,
}

impl<const SIZE: usize, T: PartialEq + Ord + Clone, K> Default for TopNHeap<SIZE, T, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, T: PartialEq + Ord + Clone, K> TopNHeap<SIZE, T, K> {
    pub fn new() -> Self {
        Self {
            priorities: VecDeque::with_capacity(SIZE),
        }
    }
    pub fn len(&self) -> usize {
        self.priorities.len()
    }
    
    pub fn get(&self, item: &T) -> Option<&K> {
        self.priorities.iter().find(|x| x.0 == *item).map(|x| &x.1)
    }

    pub fn get_mut(&mut self, item: &T) -> Option<&mut K> {
        self.priorities.iter_mut().find(|x| x.0 == *item).map(|x| &mut x.1)
    }

    pub fn get_index(&self, item: &T) -> Option<usize> {
        self.priorities.iter().position(|x| x.0 == *item)
    }
    pub fn index(&self, index: usize) -> Option<&K> {
        self.priorities.get(index).map(|x| &x.1)
    }
    pub fn index_mut(&mut self, index: usize) -> Option<&mut K> {
        self.priorities.get_mut(index).map(|x| &mut x.1)
    }

    pub fn contains(&self, item: &T) -> bool {
        self.get(item).is_some()
    }
    pub fn insert_and_increase(&mut self, item: T, key: K) -> Option<K> {
        let TopNHeap { priorities } = self;

        let idx = (priorities)
            .iter()
            .enumerate()
            .find(|x| x.1 .0 == item)
            .map(|x| x.0);

        //if it's already in the deque, then move it to the front
        if let Some(i) = idx {
            let itm = priorities.remove(i).unwrap();

            priorities.push_front(itm);

            None
        } else {
            //remove the last item and place this one in the front!
            let itm = if priorities.len() >= SIZE {
                priorities.pop_back().map(|x| x.1)
            } else {
                None
            };

            priorities.push_front((item, key));

            itm
        }
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a (T, K)> + 'a {
        self.priorities.iter()
    }
    
    pub fn drain(self) -> impl Iterator<Item = (T, K)> {
        self.priorities.into_iter()
    }
}