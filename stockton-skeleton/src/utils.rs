use crate::types::*;
use hal::{format::Format, memory::Properties as MemProperties, MemoryTypeId};

pub fn find_memory_type_id(
    adapter: &Adapter,
    type_mask: u32,
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

pub fn get_pixel_size(f: Format) -> u32 {
    f.surface_desc().bits as u32 / 8
}
