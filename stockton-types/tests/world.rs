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

extern crate stockton_types;
extern crate stockton_bsp;

#[macro_use]
mod helpers;
use crate::helpers::*;

use stockton_bsp::lumps::entities::Entity as BSPEntity;

use stockton_types::{World, Entity, Vector3};

#[derive(Debug, PartialEq)]
struct DummyEntity;

impl Entity for DummyEntity {
	fn get_position(&self) -> Vector3 {
		Vector3::new(0.0, 0.0, 0.0)
	}
}

/// Test creating a world from a dummy BSPFile with a simple mapper.
#[test]
fn world_creation() {

	let file = dummy_bspfile(vec![
		BSPEntity {
			attributes: map!(
				"name" => "1"
			)
		},
		BSPEntity {
			attributes: map!(
				"name" => "2"
			)
		},
		BSPEntity {
			attributes: map!(
				"name" => "3"
			)
		}
	]);

	let mut called_times = 0;

	let world = World::new(file, |ent: &BSPEntity| {
		called_times += 1;
		(Box::new(DummyEntity), ent.attributes.get("name").unwrap().clone().into())
	}).unwrap();


	assert_eq!(called_times, 3);

	world.live_entities["1"].downcast_ref::<DummyEntity>().unwrap();
	world.live_entities["2"].downcast_ref::<DummyEntity>().unwrap();
	world.live_entities["3"].downcast_ref::<DummyEntity>().unwrap();
}