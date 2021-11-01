//! A simple GPU compute library based on [`wgpu`](https://github.com/gfx-rs/wgpu).
//! It is meant to be used alongside `wgpu` if desired.
//!
//! Right now `gpgpu` uses some of `wgpu`'s type on its public API.
//! It may be removed in the future.
//!
//! To start using `gpgpu`, just create a [`Framework`](crate::Framework) instance
//! and follow the [examples](https://github.com/UpsettingBoy/gpgpu-rs) in the main repository.
//!
//! # Example
//! Small program that multiplies 2 vectors A and B; and stores the
//! result in another vector C.
//! ## Rust program
//! ```no_run
//!  use gpgpu::*;
//!
//!  fn main() -> GpuResult<()> {
//!     let fw = Framework::default();
//!     
//!     // Original CPU data
//!     let cpu_data = (0..10000).into_iter().collect::<Vec<u32>>();
//!
//!     // GPU buffer creation
//!     let buf_a = GpuBuffer::from_slice(&fw, &cpu_data);       // Input
//!     let buf_b = GpuBuffer::from_slice(&fw, &cpu_data);       // Input
//!     let buf_c = GpuBuffer::<u32>::new(&fw, cpu_data.len());  // Output
//!
//!     // Shader load from SPIR-V binary file
//!     let shader_module = utils::shader::from_spirv_file(&fw, "<SPIR-V shader path>")?;
//!     //  or from a WGSL source file
//!     let shader_module = utils::shader::from_wgsl_file(&fw, "<WGSL shader path>")?;    
//!
//!     // Descriptor set creation
//!     let desc_set = DescriptorSet::default()
//!         .bind_storage_buffer(&buf_a, AccessMode::ReadOnly)
//!         .bind_storage_buffer(&buf_b, AccessMode::ReadOnly)
//!         .bind_storage_buffer(&buf_c, AccessMode::ReadWrite);
//!     
//!     // Kernel creation and enqueuing
//!     fw.create_kernel_builder(&shader_module, "main")   // Entry point
//!         .add_descriptor_set(desc_set)                      
//!         .build()
//!         .enqueue(cpu_data.len() as u32, 1, 1);         // Enqueuing, not very optimus 😅
//!
//!     let output = buf_c.read()?;                        // Read back C from GPU
//!     for (a, b) in cpu_data.into_iter().zip(output) {
//!         assert_eq!(a.pow(2), b);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Shader program
//! The shader is writen in [WGSL](https://gpuweb.github.io/gpuweb/wgsl/)
//! ```ignore
//! // Vector type definition. Used for both input and output
//! [[block]]
//! struct Vector {
//!     data: [[stride(4)]] array<u32>;
//! };
//!
//! // A, B and C vectors
//! [[group(0), binding(0)]] var<storage, read>  a: Vector;
//! [[group(0), binding(1)]] var<storage, read>  b: Vector;
//! [[group(0), binding(2)]] var<storage, read_write> c: Vector;
//!
//! [[stage(compute), workgroup_size(1)]]
//! fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
//!     c.data[global_id.x] = a.data[global_id.x] * b.data[global_id.x];
//! }
//! ```
//!

use std::marker::PhantomData;

use primitives::generic_buffer::GenericBuffer;
pub use wgpu;

pub mod features;
pub mod framework;
pub mod kernel;
pub mod primitives;
pub mod utils;

/// Lazy error handling :)
pub type GpuResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Entry point of `gpgpu-rs`. A [`Framework`] must be created
/// first as all GPU primitives needs it to be created.
#[allow(dead_code)]
pub struct Framework {
    instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    limits: wgpu::Limits,
}

#[derive(PartialEq, Eq)]
pub enum GpuBufferUsage {
    /// Read-only object.
    /// ### Example WGSL syntax:
    /// ```ignore
    /// [[group(0), binding(0)]] var<storage, read> input: Vector;
    /// ```
    ReadOnly,
    /// Read-write object.
    /// ### Example WGSL syntax:
    /// ```ignore
    /// [[group(0), binding(0)]] var<storage, read_write> input: Vector;
    /// ```
    ReadWrite,
}

/// Vector of contiguous homogeneous elements on GPU memory.
/// Its elements must implement [`bytemuck::Pod`].
///
/// Equivalent to OpenCL's Buffer objects.
///
/// More information about its shader representation
/// under the [`DescriptorSet::bind_buffer`](crate::DescriptorSet::bind_buffer) documentation.
pub struct GpuBuffer<'fw, T: bytemuck::Pod>(GenericBuffer<'fw, T>);

/// Uniform vector of contiguous homogeneous elements on GPU memory.
/// Recommended for small, read-only buffers.
/// Its elements must implement [`bytemuck::Pod`].
///
/// Equivalent to OpenCL's Uniform Buffer objects.
///
/// More information about its shader representation
/// under the [`DescriptorSet::bind_uniform_buffer`](crate::DescriptorSet::bind_uniform_buffer) documentation.
pub struct GpuUniformBuffer<'fw, T: bytemuck::Pod>(GenericBuffer<'fw, T>);

#[derive(PartialEq, Eq)]
pub enum ImageUsage {
    // TODO textures can be different of WriteOnly if
    //      wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES enabled
    //
    // /// Read-only object.
    // /// ### Example WGSL syntax:
    // /// ```ignore
    // /// [[group(0), binding(0)]] var<storage, read> input: Vector;
    // /// ```
    // ReadWrite,
    /// Write-only object.
    /// ### Example WGSL syntax:
    /// ```ignore
    /// [[group(0), binding(1)]] var output: texture_storage_2d<rgba8uint, write>;
    /// ```
    WriteOnly,
}

/// 2D-image of homogeneous pixels.
///
/// Equivalent to OpenCL's Image objects.
pub struct GpuImage<'fw, P> {
    fw: &'fw Framework,
    pub texture: wgpu::Texture,
    pub size: wgpu::Extent3d,
    full_view: wgpu::TextureView,
    _pixel: PhantomData<P>,
}

/// Contains a binding group of resources.
#[derive(Default)]
pub struct DescriptorSet<'res> {
    set_layout: Vec<wgpu::BindGroupLayoutEntry>,
    binds: Vec<wgpu::BindGroupEntry<'res>>,
}

/// Creates a [`Kernel`] instance with the bindings
/// used during the configuration of this structure.
pub struct KernelBuilder<'fw, 'res, 'sha> {
    fw: &'fw Framework,
    layouts: Vec<wgpu::BindGroupLayout>,
    descriptors: Vec<DescriptorSet<'res>>,
    sets: Vec<wgpu::BindGroup>,
    shader: &'sha wgpu::ShaderModule,
    entry_point: String,
}

/// Used to enqueue the execution of a shader with the bidings provided.
///
/// Can only be created from [`KernelBuilder`].
/// Equivalent to OpenCL's Kernel.
pub struct Kernel<'fw> {
    fw: &'fw Framework,
    // layouts: Vec<wgpu::BindGroupLayout>,
    // pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::ComputePipeline,
    // descriptors: Vec<DescriptorSet<'res>>,
    sets: Vec<wgpu::BindGroup>,
    // shader: &'sha wgpu::ShaderModule,
    entry_point: String,
}
