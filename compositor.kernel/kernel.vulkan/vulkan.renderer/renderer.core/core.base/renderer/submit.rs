//! `submit_frame`: replay the queued draw ops into one composite pass and
//! submit (SDR or HDR record path; synchronous or native-KMS-IN_FENCE sync).

use ash::vk;
use smithay::backend::renderer::sync::SyncPoint;
use compositor_developer_stats_registry_base::base as stats;

use crate::error::VulkanError;
use crate::frame::DrawOp;
use super::VulkanRenderer;

impl VulkanRenderer {
    /// Replay the queued draw ops into one composite pass and submit. Called by
    /// `VulkanFrame::finish`.
    pub(crate) fn submit_frame(
        &mut self,
        image: vk::Image,
        view: vk::ImageView,
        format: vk::Format,
        extent: (u32, u32),
        clear: [f32; 4],
        ops: Vec<DrawOp>,
    ) -> Result<SyncPoint, VulkanError> {
        self.ensure_pipelines(format)?;
        if self.use_hdr() {
            self.ensure_hdr_pipeline(format)?;
        }
        let use_hdr = self.use_hdr();
        // Build any fullscreen-shader-pass pipelines this frame references (a
        // mutable borrow, done before the immutable borrows used to record).
        for op in &ops {
            if let DrawOp::ShaderPass { sdr, hdr } = op {
                let v = if use_hdr { hdr.as_ref().unwrap_or(sdr) } else { sdr };
                self.ensure_shader_pass(v, format)?;
            }
        }

        // The native-fence path reuses one command buffer + descriptor pool and
        // does NOT device_wait_idle, so before re-recording we must ensure the
        // previous frame's GPU work finished. Pace on the VkFence (created
        // signaled, so the first frame passes through). The synchronous path
        // device_wait_idles after submit, so it needs no pre-wait.
        if self.use_native_fence() {
            unsafe {
                self.dev
                    .device
                    .wait_for_fences(&[self.frame_fence], true, u64::MAX)?;
                self.dev.device.reset_fences(&[self.frame_fence])?;
            }
        }

        self.frame_counter += 1;
        let value = self.frame_counter;
        stats::frame();

        let dev = &self.dev;
        let pipelines = self
            .pipelines
            .get(&format)
            .expect("pipelines ensured above");
        let shader_passes = &self.shader_passes;
        let pool = self.descriptor_pool;
        let cmd = self.cmd;

        if self.use_hdr() {
            // HDR path (M5 1a): composite via the WGSL HDR pipeline.
            let hdr = self
                .hdr_pipelines
                .get(&format)
                .expect("hdr pipeline ensured above");
            let t = compositor_developer_stats_registry_base::base::hdr_tuning();
            hdr.update_tuning(&crate::hdr_composite::HdrTuningUbo {
                enabled: t.enabled,
                sdr_white_nits: t.sdr_white_nits,
                max_nits: t.max_nits,
                brightness: t.brightness,
                contrast: t.contrast,
                saturation: t.saturation,
                gamut: t.gamut,
                tone_map: t.tone_map,
                transfer: t.transfer,
                gamma: t.gamma,
                exposure: t.exposure,
                _pad: 0.0,
            });
            let to_push = |q: &compositor_kernel_vulkan_pipeline_composite_base::composite::PushQuad,
                           surf: [f32; 4]| {
                crate::hdr_composite::HdrPush {
                    dst: q.dst,
                    src: q.src,
                    color: q.color,
                    surf,
                }
            };
            let sdr = [0.0_f32; 4];
            compositor_kernel_vulkan_command_record_base::record::record_composition(
                dev, cmd, image, view, extent, clear, pipelines,
                |cmd| {
                    hdr.begin_frame(dev, cmd);
                    for op in ops.iter() {
                        match op {
                            DrawOp::Solid { quad } => hdr.draw_solid(dev, cmd, to_push(quad, sdr)),
                            DrawOp::ShaderPass { sdr: s, hdr: h } => {
                                let v = if use_hdr { h.as_ref().unwrap_or(s) } else { s };
                                if let Some(fp) = shader_passes.get(&(v.id, format)) {
                                    fp.draw(dev, cmd, &v.push);
                                }
                            }
                            DrawOp::Textured { quad, view: v, surf } => {
                                match hdr.texture_set(dev, *v) {
                                    Ok(set) => hdr.draw_textured(dev, cmd, set, to_push(quad, *surf)),
                                    Err(e) => warn!("hdr texture set: {e}"),
                                }
                            }
                        }
                    }
                },
            )
            .map_err(|e| VulkanError::Vk(format!("hdr record: {e}")))?;
        } else {
            unsafe {
                dev.device
                    .reset_descriptor_pool(pool, vk::DescriptorPoolResetFlags::empty())?;
            }

            // One descriptor set per textured op, in order; None for solids.
            let mut sets: Vec<Option<vk::DescriptorSet>> = Vec::with_capacity(ops.len());
            for op in &ops {
                match op {
                    DrawOp::Solid { .. } => sets.push(None),
                    DrawOp::ShaderPass { .. } => sets.push(None),
                    DrawOp::Textured { view, .. } => {
                        let layouts = [pipelines.descriptor_layout];
                        let info = vk::DescriptorSetAllocateInfo::default()
                            .descriptor_pool(pool)
                            .set_layouts(&layouts);
                        let set = unsafe { dev.device.allocate_descriptor_sets(&info)? }[0];
                        compositor_kernel_vulkan_element_texture_base::texture::bind_texture(
                            dev, pipelines, set, *view,
                        );
                        sets.push(Some(set));
                    }
                }
            }

            compositor_kernel_vulkan_command_record_base::record::record_composition(
                dev,
                cmd,
                image,
                view,
                extent,
                clear,
                pipelines,
                |cmd| {
                    for (op, set) in ops.iter().zip(sets.iter()) {
                        match op {
                            DrawOp::Solid { quad } => {
                                compositor_kernel_vulkan_element_solid_base::solid::draw(
                                    dev, pipelines, cmd, *quad,
                                );
                            }
                            DrawOp::ShaderPass { sdr, hdr } => {
                                let v = if use_hdr { hdr.as_ref().unwrap_or(sdr) } else { sdr };
                                if let Some(fp) = shader_passes.get(&(v.id, format)) {
                                    fp.draw(dev, cmd, &v.push);
                                }
                            }
                            DrawOp::Textured { quad, .. } => {
                                compositor_kernel_vulkan_element_texture_base::texture::draw(
                                    dev,
                                    pipelines,
                                    cmd,
                                    set.expect("textured op has a set"),
                                    *quad,
                                );
                            }
                        }
                    }
                },
            )
            .map_err(|e| VulkanError::Vk(format!("record: {e}")))?;
        }

        if !self.use_native_fence() {
            // Synchronous (the DEFAULT; winit; anything but the infence opt-in):
            // signal the timeline, then device_wait_idle. The returned SyncPoint
            // is already-signaled.
            compositor_kernel_vulkan_device_queue_base::queue::submit_with_timeline(
                dev,
                &self.queue,
                cmd,
                self.timeline,
                value,
            )
            .map_err(VulkanError::Vk)?;
            unsafe {
                dev.device.device_wait_idle()?;
            }
            stats::fence_synchronous();
            // Post-scene capture (native Vulkan path): copy the now-complete
            // composed scene into the registry entry dmabufs. No-op unless the
            // backend set capture targets for this frame. Ends the `dev` borrow
            // first (it needs `&mut self`'s capture fields).
            let _ = dev;
            let targets = std::mem::take(&mut self.capture_targets);
            compositor_kernel_vulkan_capture_blit_base::blit::blit_into_targets(
                &self.dev,
                self.command_pool,
                self.queue.queue,
                &mut self.capture_cache,
                image,
                extent,
                &targets,
            );
            return Ok(SyncPoint::signaled());
        }

        // Native KMS path: submit signalling the binary render semaphore and the
        // VkFence (CPU pacing), WITHOUT device_wait_idle, then export the
        // semaphore's pending signal directly as a `sync_file` fd for the
        // atomic-commit IN_FENCE.
        let cmd_info = vk::CommandBufferSubmitInfo::default().command_buffer(cmd);
        let timeline_sig = vk::SemaphoreSubmitInfo::default()
            .semaphore(self.timeline)
            .value(value)
            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS);
        let render_sig = vk::SemaphoreSubmitInfo::default()
            .semaphore(self.render_semaphore)
            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS);
        let signals = [timeline_sig, render_sig];
        let submit = vk::SubmitInfo2::default()
            .command_buffer_infos(std::slice::from_ref(&cmd_info))
            .signal_semaphore_infos(&signals);
        unsafe {
            dev.device
                .queue_submit2(self.queue.queue, &[submit], self.frame_fence)
                .map_err(|e| VulkanError::Vk(format!("queue_submit2: {e}")))?;
        }
        match compositor_kernel_vulkan_sync_export_base::export::export_sync_file(
            dev,
            self.render_semaphore,
        ) {
            Ok(fd) => {
                stats::fence_kms_infence();
                Ok(SyncPoint::from(crate::sync_fence::SyncFileFence::new(fd)))
            }
            Err(e) => {
                if self
                    .last_fence_warn
                    .is_none_or(|t| t.elapsed().as_secs() >= 60)
                {
                    warn!("native KMS fence export failed ({e}); draining device (throttled: once/min)");
                    self.last_fence_warn = Some(std::time::Instant::now());
                }
                unsafe { dev.device.device_wait_idle()? };
                stats::fence_fallback();
                Ok(SyncPoint::signaled())
            }
        }
    }
}
