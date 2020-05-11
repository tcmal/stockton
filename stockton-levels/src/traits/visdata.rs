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

use std::iter::Iterator;

pub type ClusterId = u32;

pub trait HasVisData {
	/// The iterator returned from all_visible_from
	type VisibleIterator: Iterator<Item = ClusterId>; 

	/// Returns an iterator of all clusters visible from the given Cluster ID
	fn all_visible_from<'a>(&'a self, from: ClusterId) -> Self::VisibleIterator;

	/// Returns true if `dest` is visible from `from`.
	fn cluster_visible_from(&self, from: ClusterId, dest: ClusterId) -> bool;
}