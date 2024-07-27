use super::bbox::DeltaBoundingBox;

pub struct OrderByBBox<T>(pub DeltaBoundingBox<u32>, pub T);

impl<T> PartialOrd for OrderByBBox<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.morton_origin_point().partial_cmp(&other.0.morton_origin_point())
    }
}

impl<T> Eq for OrderByBBox<T> {}


impl<T> PartialEq for OrderByBBox<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.morton_origin_point().eq(&other.0.morton_origin_point())
    }
}

impl<T> Ord for OrderByBBox<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.morton_origin_point().cmp(&other.0.morton_origin_point())
    }
}