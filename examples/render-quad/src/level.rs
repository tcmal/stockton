use stockton_levels::parts::{
    data::{FaceRef, Geometry, TextureRef},
    HasFaces, HasTextures, HasVisData, IsFace, IsTexture,
};

pub struct DemoLevel {
    pub faces: Box<[Face]>,
    pub textures: Box<[Texture]>,
}

impl DemoLevel {
    fn face_idx(&self, search: &Face) -> FaceRef {
        for (idx, face) in self.faces.iter().enumerate() {
            if face == search {
                return idx as u32;
            }
        }
        panic!("face not in level")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub geometry: Geometry,
    pub texture_idx: TextureRef,
}

impl HasFaces for DemoLevel {
    type Face = Face;

    fn get_face(&self, index: FaceRef) -> Option<&Self::Face> {
        self.faces.get(index as usize)
    }
}

impl IsFace<DemoLevel> for Face {
    fn index(&self, container: &DemoLevel) -> stockton_levels::parts::data::FaceRef {
        container.face_idx(self)
    }

    fn geometry(&self, _container: &DemoLevel) -> Geometry {
        self.geometry.clone()
    }

    fn texture_idx(&self, _container: &DemoLevel) -> TextureRef {
        self.texture_idx
    }
}

pub struct Texture {
    pub name: String,
}

impl HasTextures for DemoLevel {
    type Texture = Texture;

    fn get_texture(&self, idx: TextureRef) -> Option<&Self::Texture> {
        self.textures.get(idx as usize)
    }
}

impl IsTexture for Texture {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<'a> HasVisData<'a> for DemoLevel {
    type Faces = std::ops::Range<FaceRef>;

    fn get_visible(
        &'a self,
        _transform: &stockton_types::components::Transform,
        _settings: &stockton_types::components::CameraSettings,
    ) -> Self::Faces {
        0..self.faces.len() as u32
    }
}
