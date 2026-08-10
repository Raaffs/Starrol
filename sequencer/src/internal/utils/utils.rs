use std::collections::HashSet;
use std::hash::Hash;

pub fn unique_elements<T>(elements: &[T]) -> Vec<T> where T: Eq + Hash + Clone {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for element in elements {
        if seen.insert(element) {
            result.push(element.clone());
        }
    }
    result
}