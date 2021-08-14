//! Used for requesting appropriate queue families, and sharing/allocating queues as necessary.
//! This is created by `RenderingContext`, and should mostly be accessed from [`crate::draw_passes::IntoDrawPass`].
//!
//! For example, to use a `TexLoadQueue` in your drawpass:
//! ```
//! # use crate::{types::*, texture::TexLoadQueue};
//! fn find_aux_queues<'c>(
//!    adapter: &'c Adapter,
//!    queue_negotiator: &mut QueueNegotiator,
//! ) -> Result<Vec<(&'c QueueFamilyT, Vec<f32>)>> {
//!     queue_negotiator.find(adapter, &TexLoadQueue)?;
//!
//!     Ok(vec![queue_negotiator
//!         .family_spec::<TexLoadQueue>(&adapter.queue_families, 1)
//!         .ok_or(EnvironmentError::NoSuitableFamilies)?])
//! }
//! ```
//!
//! Then get your queue in [`crate::draw_passes::IntoDrawPass::init`]
//! ```
//! # use crate::{types::*, context::RenderingContext, texture::TexLoadQueue};
//! # use stockton_types::Session;
//! fn init(
//!     self,
//!     session: &mut Session,
//!     context: &mut RenderingContext,
//! ) -> Result<LevelDrawPass<'a, M>> {
//!     let queue = context.queue_negotiator_mut().get_queue::<TexLoadQueue>().unwrap();
//! }
//! ```

use crate::{
    error::{EnvironmentError, UsageError},
    types::*,
};

use anyhow::{bail, Error, Result};
use hal::queue::family::QueueFamilyId;
use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// A queue, possibly shared between threads.
pub type SharedQueue = Arc<RwLock<QueueT>>;

/// Used to find appropriate queue families and share queues from them as needed.
pub struct QueueNegotiator {
    family_ids: HashMap<TypeId, QueueFamilyId>,
    already_allocated: HashMap<TypeId, (Vec<SharedQueue>, usize)>,
    all: Vec<QueueGroup>,
}

/// Can be used to select an appropriate queue family
pub trait QueueFamilySelector: 'static {
    /// Return true if the given family is suitable
    fn is_suitable(&self, family: &QueueFamilyT) -> bool;
}

impl QueueNegotiator {
    /// Attempt to find an appropriate queue family using the given selector.
    /// Returns early if the *type* of the selector has already been allocated a family.
    /// This should usually be called by [`crate::draw_passes::IntoDrawPass::find_aux_queues`].
    pub fn find<T: QueueFamilySelector>(&mut self, adapter: &Adapter, filter: &T) -> Result<()> {
        if self.family_ids.contains_key(&TypeId::of::<T>()) {
            return Ok(());
        }

        let candidates: Vec<&QueueFamilyT> = adapter
            .queue_families
            .iter()
            .filter(|x| filter.is_suitable(*x))
            .collect();

        if candidates.is_empty() {
            return Err(Error::new(EnvironmentError::NoSuitableFamilies));
        }

        // Prefer using unique families
        let family = match candidates
            .iter()
            .find(|x| !self.family_ids.values().any(|y| *y == x.id()))
        {
            Some(x) => *x,
            None => candidates[0],
        };

        self.family_ids.insert(TypeId::of::<T>(), family.id());

        Ok(())
    }

    /// Get a (possibly shared) queue. You should prefer to call this once and store the result.
    /// You should already have called [`self::QueueNegotiator::find`] and [`self::QueueNegotiator::family_spec`],
    /// otherwise this will return an error.
    ///
    /// Round-robin allocation is used to try to fairly distribute work between each queue.
    /// The family of the queue returned is guaranteed to meet the spec of the `QueueFamilySelector` originally used by `find`.
    pub fn get_queue<T: QueueFamilySelector>(&mut self) -> Result<Arc<RwLock<QueueT>>> {
        let tid = TypeId::of::<T>();
        let family_id = self
            .family_ids
            .get(&tid)
            .ok_or(UsageError::QueueNegotiatorMisuse)?;

        match self
            .all
            .iter()
            .position(|x| !x.queues.is_empty() && x.family == *family_id)
        {
            Some(idx) => {
                // At least one remaining unused queue
                let queue = self.all[idx].queues.pop().unwrap();
                let queue = Arc::new(RwLock::new(queue));

                self.add_to_allocated::<T>(queue.clone());

                Ok(queue)
            }
            None => match self.already_allocated.get_mut(&tid) {
                Some((queues, next_share)) => {
                    let queue = (&queues[*next_share]).clone();

                    *next_share = (*next_share + 1) % queues.len();

                    Ok(queue)
                }
                None => bail!(EnvironmentError::NoQueues),
            },
        }
    }

    /// Convenience function to get a queue spec for the given selector.
    /// You should probably call this from [`crate::draw_passes::IntoDrawPass::find_aux_queues`].
    /// `count` is the maximum number of individual queues to request. You may get less than this, in which case they will be shared.
    /// This will return an error if you haven't called [`self::QueueNegotiator::find`] beforehand, or if there were no suitable queue families.
    pub fn family_spec<'a, T: QueueFamilySelector>(
        &self,
        queue_families: &'a [QueueFamilyT],
        count: usize,
    ) -> Result<(&'a QueueFamilyT, Vec<f32>)> {
        let qf_id = self
            .family::<T>()
            .ok_or(UsageError::QueueNegotiatorMisuse)?;

        let qf = queue_families
            .iter()
            .find(|x| x.id() == qf_id)
            .ok_or(EnvironmentError::NoSuitableFamilies)?;
        let v = vec![1.0; count];

        Ok((qf, v))
    }

    /// Get the queue family ID being used by the given selector
    pub fn family<T: QueueFamilySelector>(&self) -> Option<QueueFamilyId> {
        self.family_ids.get(&TypeId::of::<T>()).cloned()
    }

    /// Used internally to get the queue groups from the adapter.
    pub(crate) fn set_queue_groups(&mut self, queue_groups: Vec<QueueGroup>) {
        self.all = queue_groups
    }

    /// Used internally to mark that we've started sharing a queue
    fn add_to_allocated<T: QueueFamilySelector>(&mut self, queue: Arc<RwLock<QueueT>>) {
        let tid = TypeId::of::<T>();
        match self.already_allocated.get_mut(&tid) {
            None => {
                self.already_allocated.insert(tid, (vec![queue], 0));
            }
            Some(x) => {
                x.0.push(queue);
            }
        }
    }
}

impl Default for QueueNegotiator {
    fn default() -> Self {
        QueueNegotiator {
            family_ids: HashMap::new(),
            already_allocated: HashMap::new(),
            all: vec![],
        }
    }
}

/// A queue suitable for drawing to a given surface with.
pub struct DrawQueue {
    pub surface: SurfaceT,
}
impl QueueFamilySelector for DrawQueue {
    fn is_suitable(&self, family: &QueueFamilyT) -> bool {
        self.surface.supports_queue_family(family) && family.queue_type().supports_graphics()
    }
}
