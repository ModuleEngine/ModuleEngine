//! GPU regression: a plane cannot occlude itself; nearby geometry must still occlude.
use module_engine_core::{
    Camera, Frame, Material, Mesh, ObjectsWorld, RenderItem, RenderOverrides, SceneSnapshot,
    Transform, WorldObject, WorldProvider,
};
use module_engine_vulkan::{RenderPath, VulkanRenderer, default_graph};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path};

fn save(path: &Path, frame: &Frame) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let mut png = png::Encoder::new(file, frame.width as u32, frame.height as u32);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&frame.rgba)
        .map_err(|e| e.to_string())
}

fn plane(cube_y: Option<f32>) -> SceneSnapshot {
    let mut items = vec![RenderItem {
        id: 1,
        mesh: Mesh {
            vertices: vec![
                [-20., -0.5, -20.],
                [-20., -0.5, 20.],
                [20., -0.5, 20.],
                [20., -0.5, -20.],
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            ..Mesh::default()
        },
        model: Transform::default().matrix(),
        color: [100, 110, 120, 255],
        material: Material::default(),
    }];
    if let Some(y) = cube_y {
        items.push(RenderItem {
            id: 2,
            mesh: Mesh::cube(),
            model: Transform {
                translation: [0., y, 0.],
                scale: [0.5; 3],
                ..Default::default()
            }
            .matrix(),
            color: [200, 180, 150, 255],
            material: Material::default(),
        });
    }
    SceneSnapshot {
        lights: Vec::new(),
        items,
        time: 0.,
    }
}

fn reference() -> Result<SceneSnapshot, String> {
    let mut objects = (0..6)
        .map(|i| WorldObject {
            id: i + 1,
            parent: None,
            transform: Transform {
                translation: [-0.35, -0.65 + i as f32 * 0.32, 0.35],
                rotation: [0., 0., 0.12 + i as f32 * 0.075],
                scale: [1.45, 0.025, 0.035],
            },
            color: [255; 4],
        })
        .collect::<Vec<_>>();
    objects.push(WorldObject {
        id: 20,
        parent: None,
        transform: Transform {
            translation: [0., -1.05, 0.],
            scale: [3.3, 0.05, 2.5],
            ..Default::default()
        },
        color: [10, 12, 16, 255],
    });
    Ok(ObjectsWorld::from_objects(objects)?.snapshot())
}

fn difference(a: &Frame, b: &Frame) -> (usize, usize, u8) {
    let mut changed = 0;
    let mut lightened = 0;
    let mut maximum = 0;
    for (a, b) in a
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.rgba.as_chunks::<4>().0.iter())
    {
        changed += usize::from(a[..3] != b[..3]);
        lightened += usize::from(a[..3].iter().zip(b).any(|(a, b)| a > b));
        maximum = maximum.max(
            a[..3]
                .iter()
                .zip(b)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap(),
        );
    }
    (changed, lightened, maximum)
}

fn main() -> Result<(), String> {
    let output = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Provide a new output directory")?,
    );
    if output.exists() {
        return Err("Use a new output directory".into());
    }
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut renderer = VulkanRenderer::new().map_err(|error| error.to_string())?;
    let mut reports = vec![];
    let mut failures = vec![];
    for path in [RenderPath::Forward, RenderPath::Deferred] {
        let graph = default_graph(path);
        for (name, scene, camera, width, height, expect_equal) in [
            (
                "plane-oblique",
                plane(None),
                Camera::default(),
                640,
                480,
                Some(true),
            ),
            (
                "plane-close",
                plane(None),
                Camera {
                    distance: 3.,
                    pitch: 0.18,
                    ..Default::default()
                },
                960,
                720,
                Some(true),
            ),
            (
                "plane-portrait",
                plane(None),
                Camera {
                    distance: 5.,
                    pitch: 0.7,
                    yaw: -0.8,
                    ..Default::default()
                },
                480,
                720,
                Some(true),
            ),
            (
                "contact",
                plane(Some(0.)),
                Camera {
                    distance: 4.,
                    pitch: 0.55,
                    ..Default::default()
                },
                640,
                480,
                Some(false),
            ),
            (
                "distant-silhouette",
                plane(Some(2.5)),
                Camera {
                    distance: 6.,
                    target: [0., 1., 0.],
                    ..Default::default()
                },
                640,
                480,
                Some(true),
            ),
            (
                "reference",
                reference()?,
                Camera {
                    distance: 4.5,
                    target: [0., -0.4, 0.],
                    ..Default::default()
                },
                960,
                720,
                None,
            ),
        ] {
            let mut overrides = RenderOverrides::default();
            overrides.enabled.insert("aa".into(), false);
            let on = renderer
                .render(
                    &scene,
                    camera,
                    width,
                    height,
                    &graph,
                    &BTreeMap::new(),
                    &overrides,
                )
                .map_err(|error| error.to_string())?;
            overrides.enabled.insert("ao".into(), false);
            let off = renderer
                .render(
                    &scene,
                    camera,
                    width,
                    height,
                    &graph,
                    &BTreeMap::new(),
                    &overrides,
                )
                .map_err(|error| error.to_string())?;
            let stem = format!("{path:?}-{name}").to_lowercase();
            save(&output.join(format!("{stem}-ao.png")), &on)?;
            save(&output.join(format!("{stem}-bypass.png")), &off)?;
            let (changed, lightened, maximum) = difference(&on, &off);
            let passed = lightened == 0 && expect_equal.is_none_or(|equal| (changed == 0) == equal);
            if !passed {
                failures.push(stem.clone());
            }
            reports.push(json!({"case":stem,"pixels":[width,height],"fxaa":false,"changed_pixels":changed,"lightened_pixels":lightened,"maximum_channel_delta":maximum,"expect_identical_to_bypass":expect_equal,"passed":passed}));
        }
    }
    fs::write(output.join("report.json"), serde_json::to_vec_pretty(&json!({"adapter":renderer.device_name(),"scope":"Actual Forward/Deferred Vulkan pixels, FXAA disabled, same scene/camera/input with AO and bypass. Plane/distant equality and contact difference are behavioral expectations, not a duplicate AO implementation.","cases":reports,"failures":failures})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if !failures.is_empty() {
        return Err(format!("SSAO regressions: {}", failures.join(", ")));
    }
    println!("SSAO checks passed: {}", output.display());
    Ok(())
}
