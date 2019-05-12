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

use stockton_bsp::BSPFile;
use stockton_bsp::lumps::*;
use stockton_bsp::lumps::entities::Entity as BSPEntity;
use stockton_bsp::directory::{DirEntry, Header};

use stockton_types::{World, Entity, Vector3};


macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
    };
);


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

	let file = Box::pin(BSPFile {
        directory: Header {
        	version: 1,
        	dir_entries: [DirEntry { offset: 0, length: 0 }; 17]
        },
        entities: EntitiesLump {
        	string: "dummy",
        	entities: vec![
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
        	]
        },
        textures: TexturesLump {
        	textures: vec![].into_boxed_slice()
        },
        planes: PlanesLump {
        	planes: vec![].into_boxed_slice()
        },
        lightvols: LightVolsLump {
        	vols: vec![].into_boxed_slice()
        },
        lightmaps: LightmapsLump {
        	maps: vec![].into_boxed_slice()
        },
        meshverts: MeshVertsLump {
        	meshverts: vec![].into_boxed_slice()
        },
        vertices: VerticesLump {
        	vertices: vec![].into_boxed_slice()
        },
        effects: EffectsLump::empty(),
        brushes: BrushesLump::empty(),
        faces: FaceLump::empty(),
        tree: BSPTree::empty(),
        visdata: VisDataLump {
        	vecs: vec![].into_boxed_slice()
        },
        models: ModelsLump::empty()
	});

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