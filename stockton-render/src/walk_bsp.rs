// Copyright (C) 2019 Oscar Shrimpton

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

//! Walks a compiled BSP tree and renders it

use crate::draw::RenderingContext;
use stockton_bsp::BSPFile;
use std::pin::Pin;
use std::boxed::Box;

fn walk_tree<'a>(ctx: &RenderingContext<'a>, file: &Pin<Box<BSPFile>>) -> (){
	// TODO

}