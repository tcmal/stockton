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
