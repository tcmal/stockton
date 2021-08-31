use crate::types::*;

pub use shaderc::ShaderKind;

use anyhow::{Context, Result};
use hal::pso::Specialization;
use shaderc::Compiler;

#[derive(Debug, Clone)]
pub struct ShaderDesc {
    pub source: String,
    pub entry: String,
    pub kind: ShaderKind,
}

impl ShaderDesc {
    pub fn compile(&self, compiler: &mut Compiler, device: &mut DeviceT) -> Result<ShaderModuleT> {
        let artifact = compiler
            .compile_into_spirv(&self.source, self.kind, "shader", &self.entry, None)
            .context("Shader compilation failed")?;

        // Make into shader module
        Ok(unsafe {
            device
                .create_shader_module(artifact.as_binary())
                .context("Shader module creation failed")?
        })
    }

    pub fn as_entry<'a>(&'a self, module: &'a ShaderModuleT) -> EntryPoint<'a> {
        EntryPoint {
            entry: &self.entry,
            module,
            specialization: Specialization::default(),
        }
    }
}
