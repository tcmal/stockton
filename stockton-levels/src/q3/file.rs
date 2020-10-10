/*
 * Copyright (C) Oscar Shrimpton 2020
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

//! A complete BSP file

// Trait implementations are stored in their own files.

use bitvec::prelude::*;
use std::marker::PhantomData;

use self::header::Header;
use crate::coords::*;
use crate::types::Result;

use super::*;
use crate::traits::brushes::Brush;
use crate::traits::effects::Effect;
use crate::traits::entities::Entity;
use crate::traits::faces::Face;
use crate::traits::light_maps::LightMap;
use crate::traits::light_vols::LightVol;
use crate::traits::models::Model;
use crate::traits::planes::Plane;
use crate::traits::textures::Texture;
use crate::traits::tree::BSPNode;
use crate::traits::vertices::{MeshVert, Vertex};

/// A parsed Quake 3 BSP File.
pub struct Q3BSPFile<T: CoordSystem> {
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
    _phantom: PhantomData<T>,
}

impl Q3BSPFile<Q3System> {
    /// Parse `data` as a quake 3 bsp file.
    pub fn parse_file(data: &[u8]) -> Result<Q3BSPFile<Q3System>> {
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
            planes.len() as u32,
        )?;
        let effects = effects::from_data(header.get_lump(&data, 12), brushes.len() as u32)?;
        let faces = faces::from_data(
            header.get_lump(&data, 13),
            textures.len() as u32,
            effects.len() as u32,
            vertices.len() as u32,
            meshverts.len() as u32,
            light_maps.len() as u32,
        )?;

        let tree_root = tree::from_data(
            header.get_lump(&data, 3),
            header.get_lump(&data, 4),
            header.get_lump(&data, 5),
            header.get_lump(&data, 6),
            faces.len() as u32,
            brushes.len() as u32,
        )?;

        let models = models::from_data(
            header.get_lump(&data, 7),
            faces.len() as u32,
            brushes.len() as u32,
        )?;

        Ok(Q3BSPFile {
            visdata,
            textures,
            entities,
            planes,
            vertices,
            meshverts,
            light_maps,
            light_vols,
            brushes,
            effects,
            faces,
            tree_root,
            models,
            _phantom: PhantomData,
        })
    }
}

impl<T: CoordSystem> Q3BSPFile<T> {
    pub fn swizzle_to<D: CoordSystem>(mut self) -> Q3BSPFile<D>
    where
        Swizzler: SwizzleFromTo<T, D>,
    {
        for vertex in self.vertices.iter_mut() {
            Swizzler::swizzle(&mut vertex.normal);
            Swizzler::swizzle(&mut vertex.position);
        }

        for model in self.models.iter_mut() {
            Swizzler::swizzle(&mut model.mins);
            Swizzler::swizzle(&mut model.maxs);
        }

        for face in self.faces.iter_mut() {
            Swizzler::swizzle(&mut face.normal);
        }

        for plane in self.planes.iter_mut() {
            Swizzler::swizzle(&mut plane.normal);
        }

        // TODO: Possibly don't need to move?
        Q3BSPFile {
            visdata: self.visdata,
            textures: self.textures,
            entities: self.entities,
            planes: self.planes,
            vertices: self.vertices,
            meshverts: self.meshverts,
            light_maps: self.light_maps,
            light_vols: self.light_vols,
            brushes: self.brushes,
            effects: self.effects,
            faces: self.faces,
            tree_root: self.tree_root,
            models: self.models,
            _phantom: PhantomData,
        }
    }
}
