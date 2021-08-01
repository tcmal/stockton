//! Utility structs & functions

use anyhow::Result;

/// Keeps a given resource for each swapchain image
pub struct TargetSpecificResources<T> {
    elements: Vec<T>,
    next_idx: usize,
}

impl<T> TargetSpecificResources<T> {
    /// Create a new set of resources, given a function to generate them and the count
    /// In most cases, count should be swapchain_properties.image_count
    pub fn new<F>(generator: F, count: usize) -> Result<Self>
    where
        F: FnMut() -> Result<T>,
    {
        let mut elements = Vec::with_capacity(count);
        for _ in 0..count {
            elements.push(generator()?);
        }

        Ok(TargetSpecificResources {
            elements,
            next_idx: 0,
        })
    }

    /// Get the next resource, wrapping around if necessary.
    pub fn get_next<'a>(&'a mut self) -> &'a T {
        let el = &self.elements[self.next_idx];
        self.next_idx = (self.next_idx + 1) % self.elements.len();
        el
    }

    /// Dissolve the resource set, returning an iterator over each item.
    /// In most cases, each item will need deactivated.
    pub fn dissolve(self) -> impl Iterator<Item = T> {
        self.elements.into_iter()
    }
}
