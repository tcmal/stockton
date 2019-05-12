// Copyright (C) Oscar Shrimpton 2019  

// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.

// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.

//! The thing you play on and all the associated state.

use crate::{EntityStore, Entity};
use stockton_bsp::lumps::entities::Entity as BSPEntity;
use stockton_bsp::BSPFile;

use std::pin::Pin;

/// A live and playable world. There are two parts: The map, which has walls and other static objects,
/// and entities, which can move around and do things and are physics simulated.
pub struct World<'a> {
	pub map: Pin<Box<BSPFile<'a>>>,
	pub live_entities: EntityStore,
}

impl<'a> World<'a> {
	/// Create a new world from a BSPFile.
	///
	/// Returns None if any entities in the map have name conflicts.
	///
	/// `mapper` is called for each BSPEntity to map it to a concrete rust type.
	pub fn new<F>(bsp: Pin<Box<BSPFile<'a>>>, mut mapper: F) -> Option<World<'a>>
		where F: FnMut(&BSPEntity) -> (Box<Entity>, String) {

		let mut entities: Vec<(Box<Entity>, String)> = Vec::with_capacity(bsp.entities.entities.len());
		for bsp_ent in bsp.entities.entities.iter() {
			entities.push(mapper(&bsp_ent));
		}

		let store = EntityStore::from_entities(entities);
		if store.is_none() {
			return None;
		}

		Some(World {
			map: bsp,
			live_entities: store.unwrap()
		})
	}
}