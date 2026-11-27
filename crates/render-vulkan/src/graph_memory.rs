//! Per-slot lifetimes and backing allocations for graph-owned images.
use super::*;

pub(super) type Lifetimes = BTreeMap<String, (usize, usize)>;

/// Lifetimes are closed intervals. Inputs, outputs and explicit commands in
/// the same pass cannot alias even if one happens earlier in the command buffer.
pub(super) fn lifetimes(
    plan: &Plan,
    operations: &BTreeMap<usize, Vec<RenderResourceCommand>>,
) -> Lifetimes {
    let mut result = BTreeMap::new();
    for (index, pass) in plan.passes.iter().enumerate() {
        for spec in &pass.manifest.outputs {
            result.insert(pass.resource(&spec.name), (index, index));
        }
        if pass.geometry() {
            result.insert(format!("{}.__hardware_depth", pass.id), (index, index));
        }
        let mut read = |resource: &str| {
            result
                .get_mut(resource)
                .expect("validated graph resource")
                .1 = index;
        };
        // A disabled pass copies one input. Keeping the declared reads is
        // conservative and keeps the allocation reusable across enable edits.
        for (_, resource) in pass
            .inputs
            .iter()
            .filter(|(spec, _)| spec.format != "scene")
        {
            read(resource);
        }
        for operation in operations.get(&index).into_iter().flatten() {
            match operation {
                RenderResourceCommand::ClearResource { name, .. } => read(name),
                RenderResourceCommand::CopyResource { from, to } => {
                    read(from);
                    read(to);
                }
            }
        }
    }
    result
}

struct Backing {
    requirements: vk::MemoryRequirements,
    intervals: Vec<(usize, usize)>,
    images: Vec<(String, PendingImage)>,
}

fn union(a: vk::MemoryRequirements, b: vk::MemoryRequirements) -> vk::MemoryRequirements {
    vk::MemoryRequirements {
        size: a.size.max(b.size),
        alignment: a.alignment.max(b.alignment),
        memory_type_bits: a.memory_type_bits & b.memory_type_bits,
    }
}

impl VulkanRenderer {
    pub(super) fn populate_graph_images(
        &self,
        key: &FrameResourceKey,
        images: &mut BTreeMap<String, Image>,
    ) -> Result<(), VulkanError> {
        let [width, height] = key.size.map(|n| n as u32);
        let graphics = self.family;
        let family = self.compute.as_ref().map_or(graphics, |queue| queue.family);
        let families = [graphics, family];
        let concurrent = (family != graphics).then_some(families.as_slice());
        let mut pending = Vec::new();
        for (name, format_name, depth) in &key.images {
            if images.contains_key(name) {
                continue;
            }
            let interval = key.lifetimes.get(name).copied().filter(|_| {
                self.capabilities.synchronization2 && key.leased_output.as_ref() != Some(name)
            });
            let format = if *depth {
                vk::Format::D32_SFLOAT
            } else {
                format(format_name)?
            };
            let image = self.create_viewport_image(
                width,
                height,
                format,
                *depth,
                interval.is_some(),
                concurrent,
            )?;
            pending.push((name.clone(), image, interval));
        }
        // Place large images first. A small, short-lived attachment can use a
        // compatible range already required by a larger image.
        pending.sort_by_key(|(_, image, _)| std::cmp::Reverse(image.requirements.size));
        let mut backings: Vec<Backing> = Vec::new();
        for (name, mut pending, interval) in pending {
            let Some((first, last)) = interval.filter(|_| !pending.dedicated) else {
                let memory = self.allocate_image_memory(&pending)?;
                self.bind_graph_image(name, pending, Arc::new(memory), false, images)?;
                continue;
            };
            // Ignore nonlocal types before testing the common type intersection.
            pending.requirements.memory_type_bits &= self.device_local_memory_types();
            let slot = backings
                .iter_mut()
                .filter(|backing| {
                    backing.requirements.memory_type_bits & pending.requirements.memory_type_bits
                        != 0
                        && backing
                            .intervals
                            .iter()
                            .all(|&(a, b)| last < a || b < first)
                })
                .min_by_key(|backing| backing.requirements.size);
            if let Some(slot) = slot {
                slot.requirements = union(slot.requirements, pending.requirements);
                slot.intervals.push((first, last));
                slot.images.push((name, pending));
            } else {
                backings.push(Backing {
                    requirements: pending.requirements,
                    intervals: vec![(first, last)],
                    images: vec![(name, pending)],
                });
            }
        }
        for backing in backings {
            let requirements = backing.requirements;
            let memory_type = self.memory_type(
                requirements.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;
            let memory =
                Arc::new(self.allocate("graph images", requirements, memory_type, false, None)?);
            let aliased = backing.images.len() > 1;
            for (name, pending) in backing.images {
                self.bind_graph_image(name, pending, memory.clone(), aliased, images)?;
            }
        }
        Ok(())
    }

    fn device_local_memory_types(&self) -> u32 {
        self.memory.memory_types[..self.memory.memory_type_count as usize]
            .iter()
            .enumerate()
            .filter(|(_, ty)| {
                ty.property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .fold(0, |bits, (i, _)| bits | (1 << i))
    }

    fn bind_graph_image(
        &self,
        name: String,
        pending: PendingImage,
        memory: Arc<ResourceMemory>,
        aliased: bool,
        images: &mut BTreeMap<String, Image>,
    ) -> Result<(), VulkanError> {
        let mut image = self.bind_image(pending, memory)?;
        image.aliased = aliased;
        self.context.name(image.image, || name.clone());
        self.context.name(image.view, || format!("{name} view"));
        images.insert(name, image);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RenderPath, default_graph};

    #[test]
    fn diagnostics_can_copy_at_last_use_and_late_copy_extends_source_lifetime() {
        let plan = build_metadata_plan(&default_graph(RenderPath::Deferred)).unwrap();
        let base = lifetimes(&plan, &BTreeMap::new());
        assert_eq!(base["geometry.__hardware_depth"], (0, 0));
        assert_eq!(base["geometry.albedo"], (0, 1));
        assert_eq!(base["geometry.normals"], (0, 2));
        assert_eq!(base["ao.color"], (2, 3));
        assert_eq!(base["encode.frame"], (5, 5));

        // A compatible Custom pass after SSAO can receive a copy of the
        // lighting result even though its declared input comes from SSAO.
        let mut graph = default_graph(RenderPath::Deferred);
        graph.nodes.push(module_engine_core::make_node(
            "post",
            "gpu.custom",
            "Late HDR pass",
            [0., 0.],
            serde_json::json!({
                "source_id": "post",
                "manifest": crate::custom_compute_manifest(),
            }),
        ));
        graph
            .edges
            .iter_mut()
            .find(|edge| edge.to_node == "tonemap")
            .unwrap()
            .from_node = "post".into();
        graph.edges.push(module_engine_core::make_edge(
            "ao", "color", "post", "color",
        ));
        let plan = build_metadata_plan(&graph).unwrap();
        let overrides = RenderOverrides {
            resource_commands: vec![RenderResourceCommand::CopyResource {
                from: "lighting.color".into(),
                to: "post.color".into(),
            }],
            ..Default::default()
        };
        let operations = schedule_overrides(&plan, &overrides).unwrap();
        let extended = lifetimes(&plan, &operations);
        assert_eq!(extended["lighting.color"], (1, 3));
        assert_eq!(extended["ao.color"], (2, 3));
    }
}
