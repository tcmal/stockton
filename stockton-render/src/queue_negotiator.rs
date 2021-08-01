use crate::{error::EnvironmentError, types::*};

use anyhow::{Error, Result};
use hal::queue::family::QueueFamilyId;
use std::{
    any::TypeId,
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// Used to find appropriate queue families and share queues from them as needed.
pub struct QueueNegotiator {
    family_ids: HashMap<TypeId, QueueFamilyId>,
    already_allocated: HashMap<TypeId, (Vec<Arc<RwLock<QueueT>>>, usize)>,
    all: Vec<QueueGroup>,
}

/// Can be used to select a specific queue family
pub trait QueueFamilySelector: 'static {
    /// Check if the given family is suitable
    fn is_suitable(&self, family: &QueueFamilyT) -> bool;
}

impl QueueNegotiator {
    pub fn new() -> Self {
        QueueNegotiator {
            family_ids: HashMap::new(),
            already_allocated: HashMap::new(),
            all: vec![],
        }
    }

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

    pub fn set_queue_groups(&mut self, queue_groups: Vec<QueueGroup>) {
        self.all = queue_groups
    }

    pub fn get_queue<T: QueueFamilySelector>(&mut self) -> Option<Arc<RwLock<QueueT>>> {
        let tid = TypeId::of::<T>();
        let family_id = self.family_ids.get(&tid)?;
        log::debug!("{:?}", self.all);
        log::debug!("{:?}", self.already_allocated);
        match self
            .all
            .iter()
            .position(|x| !x.queues.is_empty() && x.family == *family_id)
        {
            Some(idx) => {
                // At least one remaining queue
                let queue = self.all[idx].queues.pop().unwrap();
                let queue = Arc::new(RwLock::new(queue));

                self.add_to_allocated::<T>(queue.clone());

                Some(queue)
            }
            None => match self.already_allocated.get_mut(&tid) {
                Some((queues, next_share)) => {
                    let queue = (&queues[*next_share]).clone();

                    *next_share += 1;

                    Some(queue)
                }
                None => None,
            },
        }
    }

    pub fn family<T: QueueFamilySelector>(&self) -> Option<QueueFamilyId> {
        self.family_ids.get(&TypeId::of::<T>()).cloned()
    }

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

    pub fn family_spec<'a, T: QueueFamilySelector>(
        &self,
        queue_families: &'a Vec<QueueFamilyT>,
        count: usize,
    ) -> Option<(&'a QueueFamilyT, Vec<f32>)> {
        let qf_id = self.family::<T>()?;

        let qf = queue_families.iter().find(|x| x.id() == qf_id)?;
        let v = vec![1.0; count];

        Some((qf, v))
    }
}

pub struct DrawQueue {
    pub surface: SurfaceT,
}
impl QueueFamilySelector for DrawQueue {
    fn is_suitable(&self, family: &QueueFamilyT) -> bool {
        self.surface.supports_queue_family(family) && family.queue_type().supports_graphics()
    }
}
