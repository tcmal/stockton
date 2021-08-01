use crate::types::Rgba;
use na::{Vector2, Vector3};
use serde::de;
use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde::Deserialize;
use std::fmt;

pub type VertexRef = u32;

/// A vertex, used to describe a face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: Vector3<f32>,
    pub tex: Vector2<f32>,
    pub color: Rgba,
}

impl Serialize for Vertex {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Vertex", 5)?;
        state.serialize_field("pos_x", &self.position.x)?;
        state.serialize_field("pos_y", &self.position.y)?;
        state.serialize_field("pos_z", &self.position.z)?;
        state.serialize_field("tex_u", &self.tex.x)?;
        state.serialize_field("tex_v", &self.tex.y)?;
        state.serialize_field("color", &self.color)?;

        state.end()
    }
}

impl<'de> Deserialize<'de> for Vertex {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            PosX,
            PosY,
            PosZ,
            TexU,
            TexV,
            Color,
        }
        const FIELDS: &[&str] =
            &["pos_x", "pos_y", "pos_z", "tex_x", "tex_y", "color"];

        struct VertexVisitor;

        impl<'de> Visitor<'de> for VertexVisitor {
            type Value = Vertex;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Vertex")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Vertex, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let pos_x = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let pos_y = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let pos_z = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let tex_u = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let tex_v = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let color = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(5, &self))?;
                Ok(Vertex {
                    position: Vector3::new(pos_x, pos_y, pos_z),
                    tex: Vector2::new(tex_u, tex_v),
                    color,
                })
            }

            fn visit_map<V>(self, mut map: V) -> Result<Vertex, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut pos_x = None;
                let mut pos_y = None;
                let mut pos_z = None;
                let mut tex_u = None;
                let mut tex_v = None;
                let mut color = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::PosX => {
                            if pos_x.is_some() {
                                return Err(de::Error::duplicate_field("pos_x"));
                            }
                            pos_x = Some(map.next_value()?);
                        }
                        Field::PosY => {
                            if pos_y.is_some() {
                                return Err(de::Error::duplicate_field("pos_y"));
                            }
                            pos_y = Some(map.next_value()?);
                        }
                        Field::PosZ => {
                            if pos_z.is_some() {
                                return Err(de::Error::duplicate_field("pos_z"));
                            }
                            pos_z = Some(map.next_value()?);
                        }
                        Field::TexU => {
                            if tex_u.is_some() {
                                return Err(de::Error::duplicate_field("tex_u"));
                            }
                            tex_u = Some(map.next_value()?);
                        }
                        Field::TexV => {
                            if tex_v.is_some() {
                                return Err(de::Error::duplicate_field("tex_v"));
                            }
                            tex_v = Some(map.next_value()?);
                        }
                        Field::Color => {
                            if color.is_some() {
                                return Err(de::Error::duplicate_field("color"));
                            }
                            color = Some(map.next_value()?);
                        }
                    }
                }
                let position = Vector3::new(
                    pos_x.ok_or_else(|| de::Error::missing_field("pos_x"))?,
                    pos_y.ok_or_else(|| de::Error::missing_field("pos_y"))?,
                    pos_z.ok_or_else(|| de::Error::missing_field("pos_z"))?,
                );
                let tex = Vector2::new(
                    tex_u.ok_or_else(|| de::Error::missing_field("tex_u"))?,
                    tex_v.ok_or_else(|| de::Error::missing_field("tex_v"))?,
                );
                let color = color.ok_or_else(|| de::Error::missing_field("nanos"))?;
                Ok(Vertex {
                    position,
                    tex,
                    color,
                })
            }
        }

        deserializer.deserialize_struct("Vertex", FIELDS, VertexVisitor)
    }
}
