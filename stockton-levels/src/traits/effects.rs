use super::HasBrushes;
use crate::coords::CoordSystem;

/// One effect definition
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    /// The name of the effect - always 64 characters long
    pub name: String,

    /// The brush used for this effect
    pub brush_idx: u32, // todo: unknown: i32
}

pub trait HasEffects<S: CoordSystem>: HasBrushes<S> {
    type EffectsIter<'a>: Iterator<Item = &'a Effect>;

    fn effects_iter(&self) -> Self::EffectsIter<'_>;
    fn get_effect(&self, index: u32) -> &Effect;
}
