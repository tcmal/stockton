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
//! Traits for parts of files that can exist

pub mod visdata;
pub mod entities;
pub mod textures;
pub mod planes;
pub mod vertices;
pub mod light_maps;
pub mod light_vols;
pub mod brushes;
pub mod effects;
pub mod faces;
pub mod tree;
pub mod models;

pub use self::visdata::HasVisData;
pub use self::textures::HasTextures;
pub use self::entities::HasEntities;
pub use self::planes::HasPlanes;
pub use self::vertices::{HasVertices, HasMeshVerts};
pub use self::light_maps::HasLightMaps;
pub use self::light_vols::HasLightVols;
pub use self::brushes::HasBrushes;
pub use self::effects::HasEffects;
pub use self::faces::HasFaces;
pub use self::tree::HasBSPTree;
pub use self::models::HasModels;