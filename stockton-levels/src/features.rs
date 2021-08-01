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
//! Marker traits for different feature sets

use crate::parts::*;

pub trait MinRenderFeatures<'a>: HasFaces + HasTextures + HasVisData<'a> + Send + Sync {}
impl<'a, T> MinRenderFeatures<'a> for T where
    T: HasFaces + HasTextures + HasVisData<'a> + Send + Sync
{
}
