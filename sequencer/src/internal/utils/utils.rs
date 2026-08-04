use std::collections::HashMap;
use std::hash::Hash;

pub fn unique_elements<T>(elements: &[T]) -> HashMap<T, Vec<usize>> where T: Eq + Hash + Clone {
    let mut map = HashMap::new();
    for (i, element) in elements.iter().enumerate() {
        map.entry(element.clone()).or_insert_with(Vec::new).push(i);
    }
    map
}