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

#[macro_use]
extern crate stockton_types;
extern crate stockton_bsp;

#[macro_use]
mod helpers;

use stockton_bsp::lumps::entities::Entity as BSPEntity;

use stockton_types::{Entity, Vector3};

#[derive(Debug, PartialEq)]
struct A;
impl Entity for A {
	fn get_position(&self) -> Vector3 {
		Vector3::new(0.0, 0.0, 0.0)
	}
}

#[derive(Debug, PartialEq)]
struct B {
	data: String,
}
impl Entity for B {
	fn get_position(&self) -> Vector3 {
		Vector3::new(0.0, 0.0, 0.0)
	}
}

#[derive(Debug, PartialEq)]
struct CustomStruct {
	one: i32,
	two: i32,
	three: i32
}

impl From<&str> for CustomStruct {
	fn from(_: &str) -> CustomStruct {
		CustomStruct { one: 1, two: 2, three: 3 }
	}
}

#[derive(Debug, PartialEq)]
struct C {
	data2: String,
	into: CustomStruct
}
impl Entity for C {
	fn get_position(&self) -> Vector3 {
		Vector3::new(0.0, 0.0, 0.0)
	}
}

#[test]
fn ent_map_macro() {
	let map_func = ent_map![
		"A" => A [],
		"B" => B [
			"data" => data
		],
		"C" => C [
			"data" => data2,
			"into" => into
		]
	];

	assert_eq!(map_func(&BSPEntity {
		attributes: map![
			"classname" => "A"
		]
	}).unwrap().downcast_ref::<A>().unwrap(), &A);

	assert_eq!(map_func(&BSPEntity {
		attributes: map![
			"classname" => "B",
			"data" => "foobar"
		]
	}).unwrap().downcast_ref::<B>().unwrap(), &B { data: "foobar".to_string() });

	assert_eq!(map_func(&BSPEntity {
		attributes: map![
			"classname" => "C",
			"data" => "foobar",
			"into" => "a b c"
		]
	}).unwrap().downcast_ref::<C>().unwrap(), &C { data2: "foobar".to_string(), into: CustomStruct { one: 1, two: 2, three: 3 } });

	assert!(map_func(&BSPEntity {
		attributes: map![
			"classname" => "D"
		]
	}).is_none());

	assert!(map_func(&BSPEntity {
		attributes: map![
			"classname" => "B",
			"ebeb" => "foobar"
		]
	}).is_none());
}