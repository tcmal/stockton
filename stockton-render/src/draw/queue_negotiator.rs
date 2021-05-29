use crate::{error::EnvironmentError, types::*};
use anyhow::Result;
use hal::queue::family::QueueFamilyId;
use std::sync::{Arc, RwLock};

pub struct QueueNegotiator {
    family_id: QueueFamilyId,
    already_allocated: Vec<Arc<RwLock<QueueT>>>,
    next_share: usize,
}

impl QueueNegotiator {
    pub fn find<F: FnMut(&&QueueFamilyT) -> bool>(adapter: &Adapter, filter: F) -> Result<Self> {
        let family = adapter
            .queue_families
            .iter()
            .find(filter)
            .ok_or(EnvironmentError::NoSuitableFamilies)?;

        Ok(QueueNegotiator {
            family_id: family.id(),
            already_allocated: Vec::with_capacity(family.max_queues()),
            next_share: 0,
        })
    }

    pub fn family<'a>(&self, adapter: &'a Adapter) -> &'a QueueFamilyT {
        adapter
            .queue_families
            .iter()
            .find(|x| x.id() == self.family_id)
            .unwrap()
    }

    pub fn family_id(&self) -> QueueFamilyId {
        self.family_id
    }

    pub fn get_queue(&mut self, groups: &mut Vec<QueueGroup>) -> Option<Arc<RwLock<QueueT>>> {
        match groups
            .iter()
            .position(|x| x.queues.len() > 0 && x.family == self.family_id)
        {
            Some(idx) => {
                // At least one remaining queue
                let queue = groups[idx].queues.pop().unwrap();
                let queue = Arc::new(RwLock::new(queue));

                self.already_allocated.push(queue.clone());

                Some(queue)
            }
            None => {
                if self.already_allocated.len() == 0 {
                    return None;
                }

                let queue = self.already_allocated[self.next_share].clone();
                self.next_share = (self.next_share + 1) % self.already_allocated.len();

                Some(queue)
            }
        }
    }
}
