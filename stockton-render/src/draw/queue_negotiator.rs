use crate::{error::EnvironmentError, types::*};
use anyhow::{Error, Result};
use hal::queue::family::QueueFamilyId;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct QueueNegotiator {
    family_ids: HashMap<TypeId, QueueFamilyId>,
    already_allocated: HashMap<TypeId, (Vec<Arc<RwLock<QueueT>>>, usize)>,
}

pub trait QueueFamilySelector: 'static {
    fn is_suitable(&self, family: &QueueFamilyT) -> bool;

    fn get_type_id_self(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn get_type_id() -> TypeId
    where
        Self: Sized,
    {
        TypeId::of::<Self>()
    }
}

impl QueueNegotiator {
    pub fn find(adapter: &Adapter, stacks: &[&dyn QueueFamilySelector]) -> Result<Self> {
        let mut families = HashMap::new();
        for filter in stacks {
            let candidates: Vec<&QueueFamilyT> = adapter
                .queue_families
                .iter()
                .filter(|x| filter.is_suitable(*x))
                .collect();

            if candidates.len() == 0 {
                return Err(Error::new(EnvironmentError::NoSuitableFamilies));
            }

            // Prefer using unique families
            let family = match candidates
                .iter()
                .filter(|x| !families.values().any(|y| *y == x.id()))
                .next()
            {
                Some(x) => *x,
                None => candidates[0],
            };

            families.insert(filter.get_type_id_self(), family.id());
        }

        Ok(QueueNegotiator {
            family_ids: families,
            already_allocated: HashMap::new(),
        })
    }

    pub fn get_queue<T: QueueFamilySelector>(
        &mut self,
        groups: &mut Vec<QueueGroup>,
    ) -> Option<Arc<RwLock<QueueT>>> {
        let tid = T::get_type_id();
        let family_id = self.family_ids.get(&tid)?;

        match groups
            .iter()
            .position(|x| x.queues.len() > 0 && x.family == *family_id)
        {
            Some(idx) => {
                // At least one remaining queue
                let queue = groups[idx].queues.pop().unwrap();
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
        let tid = T::get_type_id();
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
