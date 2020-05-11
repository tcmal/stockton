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

//! A complete BSP file

// Trait implementations are stored in their own files.

use bitvec::prelude::*;

use self::header::Header;
use crate::types::Result;

use super::*;
use crate::traits::textures::Texture;
use crate::traits::entities::Entity;
use crate::traits::planes::Plane;
use crate::traits::vertices::{Vertex, MeshVert};
use crate::traits::light_maps::LightMap;
use crate::traits::light_vols::LightVol;
use crate::traits::brushes::Brush;
use crate::traits::effects::Effect;
use crate::traits::faces::Face;
use crate::traits::tree::BSPNode;
use crate::traits::models::Model;

/// A parsed Quake 3 BSP File.
pub struct Q3BSPFile {
	pub(crate) visdata: Box<[BitBox<Local, u8>]>,
	pub(crate) textures: Box<[Texture]>,
	pub(crate) entities: Box<[Entity]>,
	pub(crate) planes: Box<[Plane]>,
	pub(crate) vertices: Box<[Vertex]>,
	pub(crate) meshverts: Box<[MeshVert]>,
	pub(crate) light_maps: Box<[LightMap]>,
	pub(crate) light_vols: Box<[LightVol]>,
	pub(crate) brushes: Box<[Brush]>,
	pub(crate) effects: Box<[Effect]>,
	pub(crate) faces: Box<[Face]>,
	pub(crate) models: Box<[Model]>,
	pub(crate) tree_root: BSPNode,
}

impl Q3BSPFile {
	/// Parse `data` as a quake 3 bsp file.
	pub fn new(data: &[u8]) -> Result<Q3BSPFile> {
		let header = Header::from(data)?;

		let entities = entities::from_data(header.get_lump(&data, 0))?;
		let textures = textures::from_data(header.get_lump(&data, 1))?;
		let planes = planes::from_data(header.get_lump(&data, 2))?;
		let vertices = vertices::verts_from_data(header.get_lump(&data, 10))?;
		let meshverts = vertices::meshverts_from_data(header.get_lump(&data, 11))?;
		let light_maps = light_maps::from_data(header.get_lump(&data, 14))?;
		let light_vols = light_vols::from_data(header.get_lump(&data, 15))?;
		let visdata = visdata::from_data(header.get_lump(&data, 16))?;
		let brushes = brushes::from_data(
			header.get_lump(&data, 8),
			header.get_lump(&data, 9),
			textures.len() as u32,
			planes.len() as u32
		)?;
		let effects = effects::from_data(header.get_lump(&data, 12), brushes.len() as u32)?;
		let faces = faces::from_data(
			header.get_lump(&data, 13),
			textures.len() as u32,
			effects.len() as u32,
			vertices.len() as u32,
			meshverts.len() as u32,
			light_maps.len() as u32
		)?;

		let tree_root = tree::from_data(
			header.get_lump(&data, 3),
			header.get_lump(&data, 4),
			header.get_lump(&data, 5),
			header.get_lump(&data, 6),
			faces.len() as u32,
			brushes.len() as u32
		)?;

		let models = models::from_data(header.get_lump(&data, 7), faces.len() as u32, brushes.len() as u32)?;

		Ok(Q3BSPFile {
			visdata, textures, entities, planes, vertices, meshverts, light_maps,
			light_vols, brushes, effects, faces, tree_root, models
		})
	}
}
