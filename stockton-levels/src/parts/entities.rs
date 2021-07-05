use std::iter::Iterator;

pub type EntityRef = u32;

/// A game entity
pub trait IsEntity<C: HasEntities + ?Sized> {
    fn get_attr(&self, container: &C) -> Option<&str>;
}

pub trait HasEntities {
    type Entity: IsEntity<Self>;

    fn get_entity(&self, idx: EntityRef) -> Option<&Self::Entity>;
    fn iter_entities(&self) -> Entities<Self> {
        Entities {
            next: 0,
            container: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Entities<'a, T: HasEntities + ?Sized> {
    next: EntityRef,
    container: &'a T,
}

impl<'a, T: HasEntities> Iterator for Entities<'a, T> {
    type Item = &'a T::Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.container.get_entity(self.next);
        self.next += 1;
        res
    }
}
