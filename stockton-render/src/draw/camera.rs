//! Things related to converting 3D world space to 2D screen space

use legion::maybe_changed;

use nalgebra_glm::look_at_lh;
use nalgebra_glm::perspective_lh_zo;
use stockton_levels::prelude::MinRenderFeatures;

use crate::Renderer;
use stockton_types::components::{CameraSettings, Transform};
use stockton_types::Vector3;

fn euler_to_direction(euler: &Vector3) -> Vector3 {
    let pitch = euler.x;
    let yaw = euler.y;
    let _roll = euler.z; // TODO: Support camera roll

    Vector3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

#[system(for_each)]
#[filter(maybe_changed::<Transform>() | maybe_changed::<CameraSettings>())]
pub fn calc_vp_matrix<M: 'static + MinRenderFeatures>(
    transform: &Transform,
    settings: &CameraSettings,
    #[resource] renderer: &mut Renderer<M>,
) {
    let ratio = renderer.context.target_chain.properties.extent.width as f32
        / renderer.context.target_chain.properties.extent.height as f32;
    // Get look direction from euler angles
    let direction = euler_to_direction(&transform.rotation);

    // Converts world space to camera space
    let view_matrix = look_at_lh(
        &transform.position,
        &(transform.position + direction),
        &Vector3::new(0.0, 1.0, 0.0),
    );

    // Converts camera space to screen space
    let projection_matrix = {
        let mut temp = perspective_lh_zo(ratio, settings.fov, settings.near, settings.far);

        // Vulkan's co-ord system is different from OpenGLs
        temp[(1, 1)] *= -1.0;

        temp
    };

    // Chain them together into a single matrix
    renderer.context.vp_matrix = projection_matrix * view_matrix
}
