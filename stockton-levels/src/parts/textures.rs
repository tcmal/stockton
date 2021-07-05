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

pub type TextureRef = u32;

pub trait IsTexture {
    fn name(&self) -> &str;
}

pub trait HasTextures {
    type Texture: IsTexture;

    fn get_texture(&self, idx: TextureRef) -> Option<&Self::Texture>;
    fn iter_textures(&self) -> Textures<Self> {
        Textures {
            next: 0,
            container: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Textures<'a, T: HasTextures + ?Sized> {
    next: TextureRef,
    container: &'a T,
}

impl<'a, T: HasTextures> Iterator for Textures<'a, T> {
    type Item = &'a T::Texture;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.container.get_texture(self.next);
        self.next += 1;
        res
    }
}
