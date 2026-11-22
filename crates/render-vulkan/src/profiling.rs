use crate::{VulkanError, presentation::DeviceLifetime};
use ash::vk;
use module_engine_core::{FrameTimings, GpuPassTiming, GpuTimings};
use std::sync::Arc;

pub(crate) struct TimestampPool {
    context: Arc<DeviceLifetime>,
    raw: vk::QueryPool,
    capacity: u32,
}

impl Drop for TimestampPool {
    fn drop(&mut self) {
        unsafe { self.context.device.destroy_query_pool(self.raw, None) };
    }
}

pub(crate) struct TimestampCapture {
    pool: Arc<TimestampPool>,
    count: u32,
    valid_bits: u32,
    period_ns: f64,
    passes: Vec<(String, bool)>,
}

impl TimestampCapture {
    pub(crate) fn new(
        slot: &mut Option<Arc<TimestampPool>>,
        context: &Arc<DeviceLifetime>,
        valid_bits: u32,
        period_ns: f64,
        passes: Vec<(String, bool)>,
    ) -> Result<Self, VulkanError> {
        if !(1..=64).contains(&valid_bits) || !period_ns.is_finite() || period_ns <= 0. {
            return Err(VulkanError::Profiling(
                "Selected Vulkan queue does not support timestamps",
            ));
        }
        let count = u32::try_from(passes.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(2))
            .ok_or(VulkanError::Profiling(
                "Too many graph passes for Vulkan timestamp queries",
            ))?;
        if slot.as_ref().is_none_or(|pool| pool.capacity < count) {
            let raw = unsafe {
                context.device.create_query_pool(
                    &vk::QueryPoolCreateInfo::default()
                        .query_type(vk::QueryType::TIMESTAMP)
                        .query_count(count),
                    None,
                )
            }
            .map_err(VulkanError::api("create timestamp query pool"))?;
            context.name(raw, || "ModuleEngine GPU frame timestamp queries".into());
            *slot = Some(Arc::new(TimestampPool {
                context: context.clone(),
                raw,
                capacity: count,
            }));
        }
        Ok(Self {
            pool: slot.as_ref().unwrap().clone(),
            count,
            valid_bits,
            period_ns,
            passes,
        })
    }

    pub(crate) fn reset(&self, cmd: vk::CommandBuffer) {
        unsafe {
            self.pool
                .context
                .device
                .cmd_reset_query_pool(cmd, self.pool.raw, 0, self.count)
        };
    }

    pub(crate) fn begin(&self, cmd: vk::CommandBuffer, scope: u32) {
        self.write(cmd, scope * 2, vk::PipelineStageFlags::TOP_OF_PIPE);
    }

    pub(crate) fn end(&self, cmd: vk::CommandBuffer, scope: u32) {
        self.write(cmd, scope * 2 + 1, vk::PipelineStageFlags::BOTTOM_OF_PIPE);
    }

    fn write(&self, cmd: vk::CommandBuffer, index: u32, stage: vk::PipelineStageFlags) {
        unsafe {
            self.pool
                .context
                .device
                .cmd_write_timestamp(cmd, stage, self.pool.raw, index)
        };
    }

    /// The producer already confirmed completion. This diagnostic never waits
    /// for a query or turns a query failure into a frame failure.
    fn read(&self) -> GpuTimings {
        let mut values = vec![0_u64; self.count as usize];
        let result = unsafe {
            self.pool.context.device.get_query_pool_results(
                self.pool.raw,
                0,
                &mut values,
                vk::QueryResultFlags::TYPE_64,
            )
        };
        if let Err(error) = result {
            return GpuTimings::Unavailable {
                reason: format!("Vulkan timestamp results: {error:?}"),
            };
        }
        let mut pairs = values
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[start, end]| duration_ms(start, end, self.valid_bits, self.period_ns));
        let graph_ms = pairs.next().unwrap();
        let passes = self
            .passes
            .iter()
            .zip(pairs)
            .map(|((id, bypassed), ms)| GpuPassTiming {
                id: id.clone(),
                bypassed: *bypassed,
                ms,
            })
            .collect();
        GpuTimings::Available { graph_ms, passes }
    }
}

fn duration_ms(start: u64, end: u64, bits: u32, period_ns: f64) -> f64 {
    let mask = u64::MAX >> (64 - bits);
    (end.wrapping_sub(start) & mask) as f64 * period_ns / 1_000_000.
}

pub(crate) struct PendingTimings {
    pub capture: Result<TimestampCapture, VulkanError>,
    pub record_ms: f64,
    pub submit_ms: Option<f64>,
}

impl PendingTimings {
    pub(crate) fn completed(&self) -> FrameTimings {
        FrameTimings {
            cpu_record_ms: Some(self.record_ms),
            cpu_submit_ms: self.submit_ms,
            gpu: Some(match &self.capture {
                Ok(capture) => capture.read(),
                Err(reason) => GpuTimings::Unavailable {
                    reason: reason.to_string(),
                },
            }),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::duration_ms;

    #[test]
    fn timestamps_use_period_and_wrap_at_the_queue_width() {
        assert_eq!(duration_ms(250, 5, 8, 2_000.), 0.022);
        assert_eq!(duration_ms(u64::MAX, 1, 64, 2_000.), 0.004);
        assert_eq!(duration_ms(101, 101, 36, 2_000.), 0.);
        assert_eq!(duration_ms(10, 11, 36, 2_000.), 0.002);
    }
}
