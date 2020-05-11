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
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
/// A game entity
pub struct Entity {
    pub attributes: HashMap<String, String>,
}

pub trait HasEntities<'a> {
	type EntitiesIter: Iterator<Item = &'a Entity>;

	fn entities_iter(&'a self) -> Self::EntitiesIter;
}