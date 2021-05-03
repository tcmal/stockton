use crate::types::Rgb;

#[derive(Debug, Clone, Copy)]
pub struct LightVol {
    pub ambient: Rgb,
    pub directional: Rgb,
    pub dir: [u8; 2],
}

pub trait HasLightVols {
    type LightVolsIter<'a>: Iterator<Item = &'a LightVol>;

    fn lightvols_iter(&self) -> Self::LightVolsIter<'_>;
    fn get_lightvol(&self, index: u32) -> &LightVol;
}
