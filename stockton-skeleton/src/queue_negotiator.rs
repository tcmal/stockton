//! Used for requesting appropriate queue families, and sharing/allocating queues as necessary.
//! You'll mostly use these from [`crate::draw_passes::IntoDrawPass`].
//!
//! For example, to use a `TexLoadQueue` in your drawpass, first find the family during the init phase:
//! ```
//! # use stockton_skeleton::{types::*, texture::TexLoadQueue, queue_negotiator::*};
//! # use anyhow::Result;
//! fn find_aux_queues<'c>(
//!    adapter: &'c Adapter,
//!    queue_negotiator: &mut QueueFamilyNegotiator,
//! ) -> Result<()> {
//!     queue_negotiator.find(adapter, &TexLoadQueue, 1)?;
//!
//!     Ok(())
//! }
//! ```
//!
//! Then get your queue in [`crate::draw_passes::IntoDrawPass::init`]
//!
//! ```
//! # use stockton_skeleton::{types::*, context::RenderingContext, texture::TexLoadQueue, queue_negotiator::*};
//! # use anyhow::Result;
//! # use stockton_types::Session;
//! # struct YourDrawPass;
//! # fn init(
//! #    context: &mut RenderingContext,
//! # ) -> Result<()> {
//! let queue = context.queue_negotiator_mut().get_queue::<TexLoadQueue>()?;
//!  // ...
//! # Ok(())
//! # }
//! ```

use crate::{
    error::{EnvironmentError, UsageError},
    types::*,
};
use anyhow::{bail, Error, Result};
use hal::queue::family::QueueFamilyId;
use std::{
    any::TypeId,
    collections::hash_map::{Entry, HashMap},
    sync::{Arc, RwLock},
};

/// A queue, possibly shared between threads.
pub type SharedQueue = Arc<RwLock<QueueT>>;

/// Used to find appropriate queue families during init phase.
pub struct QueueFamilyNegotiator {
    /// Family and count being used for each selector
    family_ids: HashMap<TypeId, (usize, QueueFamilyId)>,
}

impl QueueFamilyNegotiator {
    /// Create a new, empty, QueueFamilyNegotiator
    pub fn new() -> Self {
        QueueFamilyNegotiator {
            family_ids: HashMap::new(),
        }
    }

    /// Attempt to find an appropriate queue family using the given selector.
    /// If T has already been used in a different call for find, it will request the sum of the `count` values from both calls.
    /// This should usually be called by [`crate::draw_passes::IntoDrawPass::find_aux_queues`].
    pub fn find<'a, T: QueueFamilySelector>(
        &mut self,
        adapter: &'a Adapter,
        filter: &T,
        mut count: usize,
    ) -> Result<()> {
        if let Entry::Occupied(e) = self.family_ids.entry(TypeId::of::<T>()) {
            count = count + e.get().0;
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
            .find(|x| !self.family_ids.values().any(|y| y.1 == x.id()))
        {
            Some(x) => *x,
            None => candidates[0],
        };

        self.family_ids
            .insert(TypeId::of::<T>(), (count, family.id()));

        Ok(())
    }

    /// Used to get a spec passed to [`hal::adapter::PhysicalDevice::open`]
    pub(crate) fn get_open_spec<'a>(&self, adapter: &'a Adapter) -> AdapterOpenSpec<'a> {
        // Deduplicate families & convert to specific type.
        let mut spec = Vec::with_capacity(self.family_ids.len());
        for (count, qf_id) in self.family_ids.values() {
            if let Some(existing_family_spec) = spec
                .iter()
                .position(|(qf2_id, _): &(&QueueFamilyT, Vec<f32>)| qf2_id.id() == *qf_id)
            {
                for _ in 0..*count {
                    spec[existing_family_spec].1.push(1.0);
                }
            } else {
                let family = adapter
                    .queue_families
                    .iter()
                    .find(|x| x.id() == *qf_id)
                    .unwrap();
                spec.push((family, vec![1.0; *count]))
            }
        }
        AdapterOpenSpec(spec.into_boxed_slice())
    }

    /// Finish selecting our queue families, and turn this into a `QueueNegotiator`
    pub fn finish<'a>(self, queue_groups: Vec<QueueGroup>) -> QueueNegotiator {
        QueueNegotiator {
            family_ids: self.family_ids,
            already_allocated: HashMap::new(),
            all: queue_groups,
        }
    }
}

/// Used internally in calls to [`hal::adapter::PhysicalDevice::open`]
pub(crate) struct AdapterOpenSpec<'a>(Box<[(&'a QueueFamilyT, Vec<f32>)]>);

impl<'a> AdapterOpenSpec<'a> {
    pub fn as_vec(&self) -> Vec<(&'a QueueFamilyT, &[f32])> {
        let mut v = Vec::with_capacity(self.0.len());
        for (qf, cs) in self.0.iter() {
            v.push((*qf, cs.as_slice()));
        }

        v
    }
}

/// Used to share queues from families selected during init phase.
pub struct QueueNegotiator {
    family_ids: HashMap<TypeId, (usize, QueueFamilyId)>,
    already_allocated: HashMap<TypeId, (Vec<SharedQueue>, usize)>,
    all: Vec<QueueGroup>,
}

/// Can be used to select an appropriate queue family
pub trait QueueFamilySelector: 'static {
    /// Return true if the given family is suitable
    fn is_suitable(&self, family: &QueueFamilyT) -> bool;
}

impl QueueNegotiator {
    /// Get a (possibly shared) queue. You should prefer to call this once and store the result.
    /// You should already have called [`self::QueueFamilyNegotiator::find`], otherwise this will return an error.
    ///
    /// The family of the queue returned is guaranteed to meet the spec of the `QueueFamilySelector` originally used by `find`.
    pub fn get_queue<T: QueueFamilySelector>(&mut self) -> Result<Arc<RwLock<QueueT>>> {
        let tid = TypeId::of::<T>();
        let (_, family_id) = self
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

    /// Get the queue family ID being used by the given selector
    pub fn family<T: QueueFamilySelector>(&self) -> Option<QueueFamilyId> {
        self.family_ids.get(&TypeId::of::<T>()).map(|x| x.1)
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

/// A queue suitable for drawing to a given surface with.
pub struct DrawQueue {
    pub surface: SurfaceT,
}
impl QueueFamilySelector for DrawQueue {
    fn is_suitable(&self, family: &QueueFamilyT) -> bool {
        self.surface.supports_queue_family(family) && family.queue_type().supports_graphics()
    }
}
