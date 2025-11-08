/*
 * Copyright (C) 2025  Chianti GALLY
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use std::sync::{ Once, OnceLock };

use bytemuck::{ Pod, Zeroable };
use image::{ DynamicImage, GenericImageView, ImageBuffer, Rgba };
use wgpu::util::DeviceExt;

static PRINT_ADAPTER_ONCE: Once = Once::new();
static CTX: OnceLock<Option<GpuCtx>> = OnceLock::new();

struct GpuCtx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_layout: wgpu::BindGroupLayout,
    limits: wgpu::Limits,
}

fn init_ctx() -> Option<GpuCtx> {
    let instance = wgpu::Instance::default();

    // Prefer high-performance adapter.
    let mut adapter = pollster::block_on(
        instance.request_adapter(
            &(wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
        )
    );

    // If not discrete, try to pick one manually (prefer NVIDIA).
    if let Some(a) = adapter.as_ref() {
        let info = a.get_info();
        if info.device_type != wgpu::DeviceType::DiscreteGpu {
            let mut best: Option<wgpu::Adapter> = None;
            for cand in instance.enumerate_adapters(wgpu::Backends::all()) {
                let i = cand.get_info();
                if i.device_type == wgpu::DeviceType::DiscreteGpu {
                    if i.vendor == 0x10de {
                        best = Some(cand);
                        break;
                    }
                    if best.is_none() {
                        best = Some(cand);
                    }
                }
            }
            if best.is_some() {
                adapter = best;
            }
        }
    }

    let Some(adapter) = adapter else {
        return None;
    };

    PRINT_ADAPTER_ONCE.call_once(|| {
        let i = adapter.get_info();
        eprintln!(
            "wgpu: selected adapter: {} ({:?}, vendor=0x{:04X})",
            i.name,
            i.device_type,
            i.vendor
        );
    });

    let (device, queue) = match
        pollster::block_on(
            adapter.request_device(
                &(wgpu::DeviceDescriptor {
                    label: Some("watermark-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                }),
                None
            )
        )
    {
        Ok(x) => x,
        Err(_) => {
            return None;
        }
    };

    // WGSL compute kernel: straight alpha blend.
    let shader_src =
        r#"
struct Params {
  total: u32,
  width: u32,
};

@group(0) @binding(0) var<storage, read>       base_img: array<u32>;
@group(0) @binding(1) var<storage, read>       over_img: array<u32>;
@group(0) @binding(2) var<storage, read_write> out_img:  array<u32>;
@group(0) @binding(3) var<uniform>             params:   Params;

fn unpack(px: u32) -> vec4<u32> {
  let r = px & 0xFFu;
  let g = (px >> 8)  & 0xFFu;
  let b = (px >> 16) & 0xFFu;
  let a = (px >> 24) & 0xFFu;
  return vec4<u32>(r,g,b,a);
}
fn pack(c: vec4<u32>) -> u32 {
  return (c.x & 0xFFu) | ((c.y & 0xFFu) << 8) | ((c.z & 0xFFu) << 16) | ((c.w & 0xFFu) << 24);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= params.total) { return; }

  let b = unpack(base_img[i]);
  let o = unpack(over_img[i]);

  let oa = f32(o.w) / 255.0;
  let inv = 1.0 - oa;

  let r  = u32(clamp(f32(b.x)*inv + f32(o.x)*oa, 0.0, 255.0));
  let g  = u32(clamp(f32(b.y)*inv + f32(o.y)*oa, 0.0, 255.0));
  let bb = u32(clamp(f32(b.z)*inv + f32(o.z)*oa, 0.0, 255.0));

  out_img[i] = pack(vec4<u32>(r,g,bb,255u));
}
"#;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blend-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let bind_layout = device.create_bind_group_layout(
        &(wgpu::BindGroupLayoutDescriptor {
            label: Some("blend-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    );

    let pipeline_layout = device.create_pipeline_layout(
        &(wgpu::PipelineLayoutDescriptor {
            label: Some("blend-pl"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        })
    );

    let pipeline = device.create_compute_pipeline(
        &(wgpu::ComputePipelineDescriptor {
            label: Some("blend-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        })
    );

    let limits = device.limits();

    Some(GpuCtx {
        device,
        queue,
        pipeline,
        bind_layout,
        limits,
    })
}

fn ctx() -> Option<&'static GpuCtx> {
    CTX.get_or_init(|| init_ctx()).as_ref()
}

pub fn try_gpu_blend(
    base: &DynamicImage,
    overlay: &DynamicImage
) -> Result<Option<DynamicImage>, String> {
    let (bw, bh) = base.dimensions();
    if overlay.dimensions() != (bw, bh) {
        return Err("GPU blend requires same dimensions".into());
    }
    let Some(ctx) = ctx() else {
        return Ok(None);
    };

    let device = &ctx.device;
    let queue = &ctx.queue;

    // Limits & tiling
    let max_binding_bytes = (ctx.limits.max_storage_buffer_binding_size as u64).min(
        ctx.limits.max_buffer_size
    );

    const WG_SIZE: u32 = 256;
    const MAX_GROUPS_X: u32 = 65_535;
    let max_pixels_per_dispatch = (WG_SIZE as u64) * (MAX_GROUPS_X as u64);

    let base_rgba = base.to_rgba8();
    let over_rgba = overlay.to_rgba8();
    let base_bytes = base_rgba.as_raw();
    let over_bytes = over_rgba.as_raw();

    let bw_u64 = bw as u64;
    let bh_u64 = bh as u64;
    let row_bytes_u64 = bw_u64 * 4;

    let rows_by_binding = ((max_binding_bytes / row_bytes_u64) as u32).max(1);
    let rows_by_dispatch = ((max_pixels_per_dispatch / bw_u64) as u32).max(1);
    let rows_per_chunk = rows_by_binding.min(rows_by_dispatch).clamp(1, bh);

    let total_bytes = (bw_u64 * bh_u64 * 4) as usize;
    let mut out_pixels = vec![0u8; total_bytes];

    let mut y0: u32 = 0;
    while y0 < bh {
        let rows = rows_per_chunk.min(bh - y0);
        let chunk_pixels_u64 = (bw as u64) * (rows as u64);
        let chunk_bytes_u64 = chunk_pixels_u64 * 4;
        let chunk_pixels_u32 = chunk_pixels_u64 as u32;
        let groups = (chunk_pixels_u32 + WG_SIZE - 1) / WG_SIZE;
        debug_assert!(groups <= MAX_GROUPS_X);

        let start_byte = ((y0 as u64) * row_bytes_u64) as usize;
        let end_byte = start_byte + (chunk_bytes_u64 as usize);

        let base_slice = &base_bytes[start_byte..end_byte];
        let over_slice = &over_bytes[start_byte..end_byte];

        let base_chunk = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("base_chunk"),
                contents: base_slice,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            })
        );
        let over_chunk = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("over_chunk"),
                contents: over_slice,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            })
        );
        let out_chunk = device.create_buffer(
            &(wgpu::BufferDescriptor {
                label: Some("out_chunk"),
                size: chunk_bytes_u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        );

        #[repr(C)]
        #[derive(Clone, Copy, Pod, Zeroable)]
        struct ParamsU {
            total: u32,
            width: u32,
        }
        let params = ParamsU {
            total: chunk_pixels_u32,
            width: bw,
        };
        let params_buf = device.create_buffer_init(
            &(wgpu::util::BufferInitDescriptor {
                label: Some("params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        );

        let bind_group = device.create_bind_group(
            &(wgpu::BindGroupDescriptor {
                label: Some("blend-bg"),
                layout: &ctx.bind_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: base_chunk.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: over_chunk.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: out_chunk.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params_buf.as_entire_binding(),
                    },
                ],
            })
        );

        let mut encoder = device.create_command_encoder(
            &(wgpu::CommandEncoderDescriptor {
                label: Some("blend-encoder"),
            })
        );
        {
            let mut cpass = encoder.begin_compute_pass(
                &(wgpu::ComputePassDescriptor {
                    label: Some("blend-pass"),
                    timestamp_writes: None,
                })
            );
            cpass.set_pipeline(&ctx.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(groups, 1, 1);
        }

        let read_buf = device.create_buffer(
            &(wgpu::BufferDescriptor {
                label: Some("read_buf"),
                size: chunk_bytes_u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        );
        encoder.copy_buffer_to_buffer(&out_chunk, 0, &read_buf, 0, chunk_bytes_u64);
        queue.submit(Some(encoder.finish()));

        let slice = read_buf.slice(..);
        let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
        slice.map_async(wgpu::MapMode::Read, move |v| {
            tx.send(v).ok();
        });
        device.poll(wgpu::Maintain::Wait);
        let _ = pollster::block_on(async { rx.receive().await });

        let data = slice.get_mapped_range();
        out_pixels[start_byte..end_byte].copy_from_slice(&data);
        drop(data);
        read_buf.unmap();

        y0 += rows;
    }

    let result = ImageBuffer::<Rgba<u8>, _>
        ::from_raw(bw, bh, out_pixels)
        .ok_or_else(|| "Failed to reconstruct image buffer".to_string())?;
    Ok(Some(DynamicImage::ImageRgba8(result)))
}