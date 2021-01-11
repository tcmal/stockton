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

use crate::types::*;
use hal::{memory::Properties as MemProperties, prelude::*, MemoryTypeId};

pub fn find_memory_type_id(
    adapter: &Adapter,
    type_mask: u64,
    props: MemProperties,
) -> Option<MemoryTypeId> {
    adapter
        .physical_device
        .memory_properties()
        .memory_types
        .iter()
        .enumerate()
        .find(|&(id, memory_type)| {
            type_mask & (1 << id) != 0 && memory_type.properties.contains(props)
        })
        .map(|(id, _)| MemoryTypeId(id))
}
