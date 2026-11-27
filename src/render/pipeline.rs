//! Pixel-perfect GPU pipeline: CPU layers → light map → composite at 482×272 →
//! integer upscale with letterbox. The sub-pixel camera offset is applied only
//! while upscaling.

use super::canvas::Canvas;
use super::scene::{SCENE_H, SCENE_W, VIEW_H, VIEW_W};
use babel_core::game::light::{self, Light};
use babel_core::game::vec::{V2, v2};
use babel_core::world::grid::Grid;
use bevy::asset::{RenderAssetUsages, load_internal_asset, uuid_handle};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, Extent3d,
    PrimitiveTopology, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin};
use bevy::window::PrimaryWindow;
use std::collections::HashMap;

/// The shaders are built into the program.
const LIGHT_SHADER: Handle<Shader> = uuid_handle!("6c1f3b0e-5b7a-4b0e-9a53-1d7c2f4e8a11");
const COMPOSITE_SHADER: Handle<Shader> = uuid_handle!("0b9d8a2c-3e41-4f6a-8c0d-55a7b1e9c322");

const LIGHT_LAYER: usize = 1;
const COMPOSITE_LAYER: usize = 2;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct LightMaterial {
    #[uniform(0)]
    pub unused: Vec4,
}

impl Material2d for LightMaterial {
    fn fragment_shader() -> ShaderRef {
        LIGHT_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        _pipeline: &bevy::sprite_render::Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let additive = BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        if let Some(fragment) = descriptor.fragment.as_mut()
            && let Some(Some(target)) = fragment.targets.first_mut()
        {
            target.blend = Some(BlendState {
                color: additive,
                alpha: additive,
            });
        }
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct CompositeSettings {
    pub ambient: Vec4,
    pub flash: Vec4,
    pub params: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CompositeMaterial {
    #[uniform(0)]
    pub settings: CompositeSettings,
    #[texture(1)]
    #[sampler(2)]
    pub scene: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub lightmap: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    pub emissive: Handle<Image>,
}

impl Material2d for CompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        COMPOSITE_SHADER.into()
    }
}

/// Handles to everything the frame is built from.
#[derive(Resource)]
struct Screen {
    scene: Handle<Image>,
    emissive: Handle<Image>,
    ui: Handle<Image>,
    composite: Handle<CompositeMaterial>,
    light_mesh: Handle<Mesh>,
}

/// Where the view sits inside the window.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Placement {
    /// Integer upscale factor currently in use.
    pub scale: f32,
    /// Window position of the view's top-left corner, in pixels.
    pub origin: Vec2,
}

impl Placement {
    /// Integer scaling with letterbox. `preferred` caps the scale (0 = as
    /// large as fits).
    pub fn fit(width: f32, height: f32, preferred: u32) -> Placement {
        let mut scale = (width / VIEW_W as f32)
            .min(height / VIEW_H as f32)
            .floor()
            .max(1.0);
        if preferred > 0 {
            scale = scale.min(preferred as f32);
        }
        // Keep sprite edges on whole pixels for odd leftover sizes.
        let origin = Vec2::new(
            ((width - VIEW_W as f32 * scale) / 2.0).floor(),
            ((height - VIEW_H as f32 * scale) / 2.0).floor(),
        );
        Placement { scale, origin }
    }
}

#[derive(Component)]
pub struct WorldSprite;

#[derive(Component)]
pub struct UiSprite;

/// What the renderer produced this frame.
#[derive(Resource)]
pub struct Frame {
    pub scene: Canvas,
    pub emissive: Canvas,
    pub ui: Canvas,
    pub fraction: V2,
    pub settings: CompositeSettings,
    pub lights: LightGeometry,
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            scene: Canvas::new(SCENE_W, SCENE_H),
            emissive: Canvas::new(SCENE_W, SCENE_H),
            ui: Canvas::new(VIEW_W, VIEW_H),
            fraction: V2::ZERO,
            settings: CompositeSettings::default(),
            lights: LightGeometry::default(),
        }
    }
}

/// One light's triangle fan: its centre and the outline of what it sees.
pub struct Fan<'a> {
    pub centre: &'a [f32; 3],
    pub ring: &'a [[f32; 3]],
    pub radius: f32,
    pub colour: [f32; 3],
}

#[derive(Default)]
pub struct LightGeometry {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    pub count: usize,
    /// First vertex, outline length and radius of every light.
    outlines: Vec<(usize, usize, f32)>,
    /// Visibility polygons of lights that did not move.
    cache: HashMap<(i32, i32, i32), Vec<V2>>,
}

impl LightGeometry {
    pub fn invalidate(&mut self) {
        self.cache.clear();
    }

    pub fn clear(&mut self) {
        self.positions.clear();
        self.uvs.clear();
        self.colors.clear();
        self.indices.clear();
        self.outlines.clear();
        self.count = 0;
    }

    pub fn fans(&self) -> impl Iterator<Item = Fan<'_>> {
        self.outlines.iter().map(|&(first, length, radius)| {
            let [r, g, b, _] = self.colors[first];
            Fan {
                centre: &self.positions[first],
                ring: &self.positions[first + 1..first + 1 + length],
                radius,
                colour: [r, g, b],
            }
        })
    }

    /// Builds one triangle fan per light, in light-map space.
    pub fn build(&mut self, grid: &Grid, lights: Vec<Light>, origin: (i32, i32), time: f32) {
        self.clear();
        let view_min = v2(origin.0 as f32, origin.1 as f32);
        let view_max = view_min + v2(SCENE_W as f32, SCENE_H as f32);
        let lights = light::cull(lights, view_min, view_max);
        self.count = lights.len();
        if self.cache.len() > 400 {
            self.cache.clear();
        }
        for l in lights {
            let key = (
                l.pos.x.round() as i32,
                l.pos.y.round() as i32,
                l.radius.round() as i32,
            );
            let polygon = self
                .cache
                .entry(key)
                .or_insert_with(|| light::visibility_polygon(grid, l.pos, l.radius));
            // Two incommensurate waves read as candle noise.
            let phase = l.pos.x * 0.37 + l.pos.y * 0.19;
            let flicker = 1.0
                + l.flicker
                    * ((time * 9.1 + phase).sin() * 0.06
                        + (time * 23.7 + phase * 1.7).sin() * 0.035
                        + (time * 3.3 + phase).sin() * 0.04);
            let color = [
                l.color[0] * flicker,
                l.color[1] * flicker,
                l.color[2] * flicker,
                1.0,
            ];
            let to_mesh = |p: V2| {
                [
                    p.x - view_min.x - SCENE_W as f32 / 2.0,
                    SCENE_H as f32 / 2.0 - (p.y - view_min.y),
                    0.0,
                ]
            };
            let center = self.positions.len() as u32;
            self.positions.push(to_mesh(l.pos));
            self.uvs.push([0.0, 0.0]);
            self.colors.push(color);
            for &point in polygon.iter() {
                let offset = (point - l.pos) * (1.0 / l.radius.max(1.0));
                self.positions.push(to_mesh(point));
                self.uvs.push([offset.x, offset.y]);
                self.colors.push(color);
            }
            let n = polygon.len() as u32;
            self.outlines
                .push((center as usize, polygon.len(), l.radius));
            for i in 0..n {
                self.indices
                    .extend([center, center + 1 + i, center + 1 + (i + 1) % n]);
            }
        }
    }

    fn write(&self, mesh: &mut Mesh) {
        // An empty vertex buffer is invalid, so keep one degenerate triangle.
        let empty = self.positions.is_empty();
        let positions = if empty {
            vec![[0.0f32; 3]; 3]
        } else {
            self.positions.clone()
        };
        let uvs = if empty {
            vec![[2.0f32; 2]; 3]
        } else {
            self.uvs.clone()
        };
        let colors = if empty {
            vec![[0.0f32; 4]; 3]
        } else {
            self.colors.clone()
        };
        let indices = if empty {
            vec![0, 1, 2]
        } else {
            self.indices.clone()
        };
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
    }
}

fn cpu_image(w: i32, h: i32) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (w * h * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    image
}

fn camera(order: isize, clear: Color) -> impl Bundle {
    (
        Camera2d,
        Camera {
            order,
            clear_color: ClearColorConfig::Custom(clear),
            ..default()
        },
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
    )
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut light_materials: ResMut<Assets<LightMaterial>>,
    mut composite_materials: ResMut<Assets<CompositeMaterial>>,
) {
    let scene = images.add(cpu_image(SCENE_W, SCENE_H));
    let emissive = images.add(cpu_image(SCENE_W, SCENE_H));
    let ui = images.add(cpu_image(VIEW_W, VIEW_H));

    let mut lightmap = Image::new_target_texture(
        SCENE_W as u32,
        SCENE_H as u32,
        TextureFormat::Rgba16Float,
        None,
    );
    lightmap.sampler = ImageSampler::linear();
    let lightmap = images.add(lightmap);
    let mut canvas = Image::new_target_texture(
        SCENE_W as u32,
        SCENE_H as u32,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    canvas.sampler = ImageSampler::nearest();
    let canvas = images.add(canvas);

    // 1. Light map.
    commands.spawn((
        camera(-3, Color::BLACK),
        Hdr,
        RenderTarget::Image(lightmap.clone().into()),
        RenderLayers::layer(LIGHT_LAYER),
    ));
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    LightGeometry::default().write(&mut mesh);
    let light_mesh = meshes.add(mesh);
    commands.spawn((
        Mesh2d(light_mesh.clone()),
        MeshMaterial2d(light_materials.add(LightMaterial { unused: Vec4::ZERO })),
        Transform::default(),
        NoFrustumCulling,
        RenderLayers::layer(LIGHT_LAYER),
    ));

    // 2. Composite.
    let composite = composite_materials.add(CompositeMaterial {
        settings: CompositeSettings::default(),
        scene: scene.clone(),
        lightmap,
        emissive: emissive.clone(),
    });
    commands.spawn((
        camera(-2, Color::BLACK),
        RenderTarget::Image(canvas.clone().into()),
        RenderLayers::layer(COMPOSITE_LAYER),
    ));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(SCENE_W as f32, SCENE_H as f32))),
        MeshMaterial2d(composite.clone()),
        RenderLayers::layer(COMPOSITE_LAYER),
    ));

    // 3. Window: the upscaled world with the interface on top.
    commands.spawn(camera(0, Color::srgb_u8(16, 9, 26)));
    commands.spawn((
        Sprite {
            image: canvas,
            custom_size: Some(Vec2::new(VIEW_W as f32, VIEW_H as f32)),
            rect: Some(bevy::math::Rect::new(
                0.0,
                0.0,
                VIEW_W as f32,
                VIEW_H as f32,
            )),
            ..default()
        },
        WorldSprite,
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    commands.spawn((
        Sprite {
            image: ui.clone(),
            custom_size: Some(Vec2::new(VIEW_W as f32, VIEW_H as f32)),
            ..default()
        },
        UiSprite,
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));

    commands.insert_resource(Screen {
        scene,
        emissive,
        ui,
        composite,
        light_mesh,
    });
}

fn upload_image(images: &mut Assets<Image>, handle: &Handle<Image>, canvas: &Canvas) {
    if let Some(mut image) = images.get_mut(handle)
        && let Some(data) = image.data.as_mut()
        && data.len() == canvas.px.len()
    {
        data.copy_from_slice(&canvas.px);
    }
}

/// Sends the CPU layers, light geometry and settings to the GPU.
fn upload(
    frame: Res<Frame>,
    screen: Res<Screen>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CompositeMaterial>>,
) {
    upload_image(&mut images, &screen.scene, &frame.scene);
    upload_image(&mut images, &screen.emissive, &frame.emissive);
    upload_image(&mut images, &screen.ui, &frame.ui);
    if let Some(mut mesh) = meshes.get_mut(&screen.light_mesh) {
        frame.lights.write(&mut mesh);
    }
    if let Some(mut material) = materials.get_mut(&screen.composite) {
        material.settings = frame.settings;
    }
}

/// One of the two window sprites, disjoint from the other.
type LayerQuery<'w, 's, Own, Other> =
    Query<'w, 's, (&'static mut Sprite, &'static mut Transform), (With<Own>, Without<Other>)>;

/// Integer scaling with letterbox, plus the sub-pixel scroll of the world.
fn fit(
    window: Query<&Window, With<PrimaryWindow>>,
    mut placement: ResMut<Placement>,
    frame: Res<Frame>,
    preferred: Res<PreferredScale>,
    mut world: LayerQuery<WorldSprite, UiSprite>,
    mut ui: LayerQuery<UiSprite, WorldSprite>,
) {
    let Ok(window) = window.single() else {
        return;
    };
    let (w, h) = (window.width(), window.height());
    *placement = Placement::fit(w, h, preferred.0);
    let Placement { scale, origin } = *placement;
    let size = Vec2::new(VIEW_W as f32 * scale, VIEW_H as f32 * scale);
    let center = Vec3::new(
        origin.x + size.x / 2.0 - w / 2.0,
        h / 2.0 - origin.y - size.y / 2.0,
        0.0,
    );
    let fraction = Vec2::new(
        (frame.fraction.x * scale).floor() / scale,
        (frame.fraction.y * scale).floor() / scale,
    );
    for (mut sprite, mut transform) in &mut world {
        sprite.custom_size = Some(size);
        sprite.rect = Some(bevy::math::Rect::new(
            fraction.x,
            fraction.y,
            fraction.x + VIEW_W as f32,
            fraction.y + VIEW_H as f32,
        ));
        transform.translation = center;
    }
    for (mut sprite, mut transform) in &mut ui {
        sprite.custom_size = Some(size);
        transform.translation = center.with_z(1.0);
    }
}

/// Maximum pixel scale chosen in the options (0 = as large as fits).
#[derive(Resource, Default)]
pub struct PreferredScale(pub u32);

pub struct PipelinePlugin;

impl Plugin for PipelinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            LIGHT_SHADER,
            "../../assets/shaders/light.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            COMPOSITE_SHADER,
            "../../assets/shaders/composite.wgsl",
            Shader::from_wgsl
        );
        app.add_plugins((
            Material2dPlugin::<LightMaterial>::default(),
            Material2dPlugin::<CompositeMaterial>::default(),
        ))
        .init_resource::<Frame>()
        .init_resource::<PreferredScale>()
        .init_resource::<Placement>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, (upload, fit));
    }
}
