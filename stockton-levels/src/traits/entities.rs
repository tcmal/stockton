use std::collections::HashMap;
use std::iter::Iterator;

#[derive(Debug, Clone, PartialEq)]
/// A game entity
pub struct Entity {
    pub attributes: HashMap<String, String>,
}

pub trait HasEntities {
    type EntitiesIter<'a>: Iterator<Item = &'a Entity>;

    fn entities_iter(&self) -> Self::EntitiesIter<'_>;
}
