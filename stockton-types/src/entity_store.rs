//! Stores entities in a world.

use std::collections::HashMap;
use std::boxed::Box;
use std::ops::Index;
use crate::Vector3;

/// An entity, capable of recieving events.
pub trait Entity {

	/// Should return the position of this entity in 3d space.
	///
	/// This will likely just be `self.pos`
	fn get_position(&self) -> Vector3;

}

/// Stores all the entities in a live world. The BSPFile only specifies the starting entities,
/// whereas this is mutable and thus is used to represent the current state of the world's entities.
///
/// Internally, this uses a vector to store the entities, and a hashmap to map names to indicies in that vector.
///
/// An entity's index may change, so if you want to target an entity throughout frames you should store its name.
pub struct EntityStore {
	entities: Vec<Box<dyn Entity>>,
	name_to_index: HashMap<String, usize>
}

/// Returned when an entity's name conflicts with an existing entity.
pub struct NameConflict;

impl EntityStore {
	/// Try to add the given entity with the given name.
	///
	/// # Returns
	/// The name & index of the added entity if successful. 
	/// If an entity already exists with the given name, NameConflict is returned.
	pub fn add(&mut self, entity: Box<Entity>, name: String) -> Result<usize, NameConflict> {
		if self.name_to_index.contains_key(&name) {
			return Err(NameConflict)
		}
		self.name_to_index.insert(name, self.entities.len());
		self.entities.push(entity);

		Ok(self.entities.len() - 1)
	}

	/// Remove the entity with the given index, returning it.
	///
	/// Takes O(2n - i) time.
	pub fn remove_by_index(&mut self, index: usize) -> Option<Box<Entity>> {
		if index >= self.entities.len() {
			return None;
		}
		self.name_to_index.retain(|_,v| *v != index);
		Some(self.entities.remove(index))
	}

	/// Removes the entity with the given name, returning it.
	///
	/// Takes O(2n - i) time.
	pub fn remove_by_name(&mut self, name: &str) -> Option<Box<Entity>> {
		let mut index: usize = self.entities.len();
		
		self.name_to_index.retain(|k,v| {
			if k == name {
				index = *v;
				return false;
			}
			true
		});
		
		if index >= self.entities.len() {
			return None;
		}

		Some(self.entities.remove(index))
	}

	/// Make a new EntityStore from a list of entities & names.
	///
	/// Returns None in case of name conflicts in list.
	pub fn from_entities(entities: Vec<(Box<Entity>, String)>) -> Option<EntityStore> {
		let mut store = EntityStore {
			entities: Vec::with_capacity(entities.len()),
			name_to_index: HashMap::with_capacity(entities.len())
		};

		for (entity, name) in entities {
			if store.add(entity, name).is_err() {
				return None;
			}
		}

		Some(store)
	}
}

/// Indexes the EntityStore for a specific index.
/// If you want to target an entity for longer than one tick, store its name, not an index.
impl Index<usize> for EntityStore {
	type Output = Entity;
	fn index(&self, index: usize) -> &Self::Output {
		self.entities[index].as_ref()
	}
}

/// Indexes the EntityStore for a specific name.
/// This is what you should use if you plan to target an entity for more than one tick.
impl Index<&str> for EntityStore {
	type Output = Entity;
	fn index(&self, index: &str) -> &Self::Output {
		self.entities[self.name_to_index[index]].as_ref()
	}
}