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

//! Parsing data from Q3 and similar BSPs

mod brushes;
mod effects;
mod entities;
mod faces;
pub mod file;
mod header;
mod light_maps;
mod light_vols;
mod models;
mod planes;
mod textures;
mod tree;
mod vertices;
mod visdata;

pub use self::file::Q3BSPFile;
