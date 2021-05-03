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

//! Helper macros, mostly for the graphics pipeline definitions

/// Macro for easily defining buffer attribute descriptions
/// Usage:
/// ```
/// // 0 is the binding value
/// let attributes: Vec<AttributeDesc> = pipeline_vb_attributes!(0,
/// size_of::<f32>() * 3; Rgb32Sfloat
///     size_of::<f32>() * 2; Rg32Sfloat,
///     size_of::<u32>(); R32Sint
/// );
/// ```
/// See the hal::pso::Format enum for possible types
#[allow(clippy::vec_init_then_push)]
macro_rules! pipeline_vb_attributes {
	// Special case for single item
	( $binding:expr, $firstSize:expr; $firstType:ident ) => ({
		#![allow(clippy::vec_init_then_push)]
		vec![
			AttributeDesc {
				location: 0,
				binding: $binding,
				element: Element {
					format: Format::$firstType,
					offset: $firstSize as u32
				}
			}
		]
	});

	// Start of recursion
	( $binding:expr,
		$firstSize:expr; $firstType:ident,
		$( $size:expr; $type:ident ),*
	) => ({
		use hal::pso::{AttributeDesc, Element};

		let mut vec = Vec::new();

		vec.push(AttributeDesc {
			location: 0,
			binding: $binding,
			element: Element {
				format: Format::$firstType,
				offset: 0
			}
		});

		pipeline_vb_attributes!(
			vec; $binding; 1; $firstSize,
			$($size; $type),*
		);

		vec
	});

	// Middle of recursion
	( $vec:ident; $binding:expr; $location:expr; $prevSize:expr,
		$firstSize:expr; $firstType:ident,
		$($size:expr; $type:ident),* ) => ({

		$vec.push(AttributeDesc {
			location: $location,
			binding: $binding,
			element: Element {
				format: Format::$firstType,
				offset: $prevSize as u32
			}
		});

		pipeline_vb_attributes!(
			$vec; $binding; ($location + 1); ($prevSize + $firstSize),
			$($size; $type),*
		);
	});

	// End of recursion
	( $vec:ident; $binding:expr; $location:expr; $prevSize:expr,
		$firstSize:expr; $firstType:ident ) => ({
			$vec.push(AttributeDesc {
				location: $location,
				binding: $binding,
				element: Element {
					format: Format::$firstType,
					offset: $prevSize as u32
				}
			});
		});
}
