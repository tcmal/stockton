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

/// Convenience function for creating mappers for `World::new()`.
#[macro_export]
macro_rules! ent_map {
	( $ ( $name:expr => $type:ident [ $( $key:expr => $target:ident ),* ] ),* ) => {
		{
			use stockton_bsp::lumps::entities::Entity as BSPEntity;	
			use stockton_types::Entity;	
			|ent: &BSPEntity| -> Option<Box<dyn Entity>> {
				$(
					if ent.attributes["classname"] == $name {
						let mut valid = true;
						{
							$(let mut $target = false;);*
							for key in ent.attributes.keys() {
								$(if key == &$key {
									$target = true;
									continue;
								});*
							}
							$(if !$target {
								valid = false;
							});*
						}
						if valid {
							return Some(Box::new($type {
								$( $target : ent.attributes[$key].into() ),*
							}));
						}
					}
				);*
				None
			}
		}
	}
}