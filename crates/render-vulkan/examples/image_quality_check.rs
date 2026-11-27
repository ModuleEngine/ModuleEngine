//! Deterministic GPU image comparisons; not a native Player or scanout test.
use module_engine_core::{
    Camera, Frame, ObjectsWorld, RenderOverrides, Transform, WorldObject, WorldProvider,
};
use module_engine_vulkan::{
    RenderPath, VulkanRenderer, builtin_hlsl_source, builtin_manifest, default_graph,
};
use serde_json::json;
use std::{collections::BTreeMap, fs, path::Path, time::Instant};

fn png(path: &Path, frame: &Frame) -> Result<(), String> {
    let file = fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, frame.width as u32, frame.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(&frame.rgba)
        .map_err(|e| e.to_string())
}

fn decode_rgba8(path: &Path) -> Result<([u32; 2], Vec<u8>), String> {
    let read = || -> Result<_, Box<dyn std::error::Error>> {
        let file = std::io::BufReader::new(fs::File::open(path)?);
        let mut reader = png::Decoder::new(file).read_info()?;
        let size = reader
            .output_buffer_size()
            .ok_or("Invalid PNG buffer size")?;
        let mut data = vec![0; size];
        let frame = reader.next_frame(&mut data)?;
        if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
            return Err("Expected an RGBA8 frame".into());
        }
        data.truncate(frame.buffer_size());
        Ok(([frame.width, frame.height], data))
    };
    read().map_err(|error| format!("Decode {}: {error}", path.display()))
}

fn compare(reference: &Path, candidate: &Path, report: &Path) -> Result<(), String> {
    let (extent, expected) = decode_rgba8(reference)?;
    let (actual_extent, actual) = decode_rgba8(candidate)?;
    if extent != actual_extent {
        return Err(format!(
            "Reference extent {extent:?} differs from candidate {actual_extent:?}"
        ));
    }
    let mut maximum = 0;
    let mut different = 0;
    for (a, b) in expected.chunks_exact(4).zip(actual.chunks_exact(4)) {
        let delta = a.iter().zip(b).map(|(&x, &y)| x.abs_diff(y)).max().unwrap();
        maximum = maximum.max(delta);
        different += usize::from(delta != 0);
    }
    let value = json!({
        "reference": reference,
        "candidate": candidate,
        "render_pixels": extent,
        "max_channel_delta": maximum,
        "changed_pixels": different,
        "channel_tolerance": 0,
        "passed": different == 0,
    });
    let file = fs::File::options()
        .write(true)
        .create_new(true)
        .open(report)
        .map_err(|error| format!("Create {}: {error}", report.display()))?;
    serde_json::to_writer_pretty(file, &value).map_err(|error| error.to_string())?;
    if different != 0 {
        return Err(format!(
            "{different} different pixels; maximum channel delta {maximum}"
        ));
    }
    println!("{}", report.display());
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let [flag, reference, candidate, report] = args.as_slice()
        && flag == "--compare"
    {
        return compare(
            Path::new(reference),
            Path::new(candidate),
            Path::new(report),
        );
    }
    let [folder] = args.as_slice() else {
        return Err("Usage: image_quality_check <new-directory> or --compare <reference.png> <candidate.png> <new-report.json>".into());
    };
    let output = Path::new(folder);
    if output.exists() {
        return Err("Image diagnostic requires a new directory".into());
    }
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let mut objects = (0..6)
        .map(|index| WorldObject {
            id: index + 1,
            parent: None,
            transform: Transform {
                translation: [-0.35, -0.65 + index as f32 * 0.32, 0.35],
                rotation: [0., 0., 0.12 + index as f32 * 0.075],
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
    let scene = ObjectsWorld::from_objects(objects)?.snapshot();
    let root = module_engine_vulkan::resource_root();
    let mut renderer = VulkanRenderer::new_at(root).map_err(|error| error.to_string())?;
    renderer.set_detailed_stats(true);
    let source = builtin_hlsl_source("gpu.fxaa").ok_or("FXAA source missing")?;
    // Control only: the same FXAA footprint operates in sRGB, then decodes
    // back to linear so the existing color-encode pass remains unchanged.
    let control = source.replace("float4 fetch(", "float3 encodeSrgb(float3 c){return float3(c.x<=0.0031308?12.92*c.x:1.055*pow(max(c.x,0.0),1.0/2.4)-0.055,c.y<=0.0031308?12.92*c.y:1.055*pow(max(c.y,0.0),1.0/2.4)-0.055,c.z<=0.0031308?12.92*c.z:1.055*pow(max(c.z,0.0),1.0/2.4)-0.055);}\nfloat3 decodeSrgb(float3 c){return float3(c.x<=0.04045?c.x/12.92:pow((c.x+0.055)/1.055,2.4),c.y<=0.04045?c.y/12.92:pow((c.y+0.055)/1.055,2.4),c.z<=0.04045?c.z/12.92:pow((c.z+0.055)/1.055,2.4));}\nfloat4 fetch(")
        .replace("return color.Load(int3(clamp(p,int2(0,0),int2(size)-1),0));", "float4 c=color.Load(int3(clamp(p,int2(0,0),int2(size)-1),0));return float4(encodeSrgb(c.rgb),c.a);")
        .replace("outputColor[p]=center", "outputColor[p]=float4(decodeSrgb(center.rgb),center.a)")
        .replace("float4(lb<low||lb>high?a:b,center.a)", "float4(decodeSrgb(lb<low||lb>high?a:b),center.a)");
    fs::write(output.join("fxaa-srgb-control.hlsl"), &control).map_err(|e| e.to_string())?;
    let mut reports = vec![];
    for mode in ["off", "fxaa-linear", "fxaa-srgb-control"] {
        let folder = output.join(mode);
        fs::create_dir(&folder).map_err(|e| e.to_string())?;
        let mut graph = default_graph(RenderPath::Forward);
        let aa = graph
            .nodes
            .iter_mut()
            .find(|node| node.module == "gpu.fxaa")
            .ok_or("AA node missing")?;
        let id = aa.id.clone();
        let mut sources = BTreeMap::new();
        let mut overrides = RenderOverrides::default();
        if mode == "off" {
            overrides.enabled.insert(id, false);
        }
        if mode == "fxaa-srgb-control" {
            aa.module = "gpu.custom".into();
            aa.params = json!({"source_id":"srgb-control","manifest":builtin_manifest("gpu.fxaa").unwrap()});
            sources.insert("srgb-control".into(), control.clone());
        }
        let mut sizes = vec![];
        for (width, height) in [
            (960, 540),
            (1280, 720),
            (1920, 1080),
            (900, 900),
            (3840, 2160),
        ] {
            // Warm shaders/resources separately from the measured identical work.
            renderer
                .render(
                    &scene,
                    Camera::default(),
                    width,
                    height,
                    &graph,
                    &sources,
                    &overrides,
                )
                .map_err(|error| error.to_string())?;
            let mut times = vec![];
            for _ in 0..12 {
                let started = Instant::now();
                let frame = renderer
                    .render(
                        &scene,
                        Camera::default(),
                        width,
                        height,
                        &graph,
                        &sources,
                        &overrides,
                    )
                    .map_err(|error| error.to_string())?;
                times.push(started.elapsed().as_secs_f64() * 1000.);
                if times.len() == 1 {
                    png(&folder.join(format!("static-{width}x{height}.png")), &frame)?;
                }
            }
            sizes.push(json!({"pixels":[width,height],"warm_render_cpu_readback_ms":times}));
        }
        let mut stopped = None;
        let mut stop_equal = true;
        for index in 0..72 {
            let mut camera = Camera::default();
            camera.yaw += 0.4 * (index.min(59) as f32 / 59.);
            let frame = renderer
                .render(&scene, camera, 1280, 720, &graph, &sources, &overrides)
                .map_err(|error| error.to_string())?;
            if index >= 59 {
                if let Some(previous) = &stopped {
                    stop_equal &= previous == &frame.rgba;
                }
                stopped = Some(frame.rgba.clone());
            }
            png(&folder.join(format!("motion-{index:03}.png")), &frame)?;
        }
        if !stop_equal {
            return Err(format!("{mode}: pixels changed after camera stopped"));
        }
        reports.push(json!({"mode":mode,"sizes":sizes,"movement_frames":60,"stopped_frames":12,"stopped_frames_byte_equal":stop_equal}));
    }
    let report = json!({"scope":"Deterministic renderer-only GPU frames with CPU readback, fixed scene, identical camera path; not ordinary Player motion/physical scanout or GPU timestamps. sRGB is a diagnostic control, not a shipped filter change.","adapter":renderer.device_name(),"results":reports});
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{}", output.display());
    Ok(())
}
