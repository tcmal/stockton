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

use crate::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Position of the object
    pub position: Vector3,

    /// Rotation of the object (euler angles in radians)
    pub rotation: Vector3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSettings {
    /// FOV (radians)
    pub fov: f32,

    /// Near clipping plane (world units)
    pub near: f32,

    /// Far clipping plane (world units)
    pub far: f32,
}
