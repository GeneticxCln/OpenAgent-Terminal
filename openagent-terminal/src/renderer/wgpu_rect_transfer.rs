use wgpu::{Buffer, BufferAddress, BufferDescriptor, BufferUsages, CommandEncoder, Device};

/// Helper for batching rect vertices via a mapped staging buffer and a GPU-local vertex buffer.
///
/// The staging buffer is CPU-mappable (MAP_WRITE | COPY_SRC). Each frame we write all batched
/// vertices into the staging buffer and then flush a single copy into the GPU vertex buffer
/// (COPY_DST | VERTEX). This minimizes queue.write_buffer calls and allows us to control
/// synchronization explicitly.
#[derive(Debug)]
pub struct WgpuRectTransfer {
    staging: Buffer,
    vertex: Buffer,
    vertex_stride: usize,
    capacity_vertices: usize,
    used_vertices: usize,
    // CPU-side scratch to aggregate multiple appends into a single map+copy per frame.
    cpu_scratch: Vec<u8>,
    // Growth policy factor (>1.0). Defaults to 2.0 (doubling).
    growth_factor: f32,
}

impl WgpuRectTransfer {
    pub fn new(device: &Device, capacity_vertices: usize, vertex_stride: usize) -> Self {
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("rect-transfer-staging"),
            size: (capacity_vertices * vertex_stride) as BufferAddress,
            usage: BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let vertex = device.create_buffer(&BufferDescriptor {
            label: Some("rect-transfer-vertex"),
            size: (capacity_vertices * vertex_stride) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            staging,
            vertex,
            vertex_stride,
            capacity_vertices,
            used_vertices: 0,
            cpu_scratch: Vec::with_capacity(capacity_vertices * vertex_stride),
            growth_factor: 2.0,
        }
    }

    #[allow(dead_code)]
    pub fn set_growth_factor(&mut self, factor: f32) {
        self.growth_factor = factor.max(1.1);
    }

    pub fn begin_frame(&mut self) {
        self.used_vertices = 0;
        self.cpu_scratch.clear();
    }

    fn ensure_capacity(&mut self, device: &Device, required_vertices: usize) {
        if required_vertices <= self.capacity_vertices {
            return;
        }
        let mut new_cap = self.capacity_vertices.max(1);
        let factor = self.growth_factor;
        while new_cap < required_vertices {
            // Increase by factor, rounding up.
            new_cap = ((new_cap as f32) * factor).ceil() as usize;
        }
        // Recreate both buffers with the new capacity.
        self.staging = device.create_buffer(&BufferDescriptor {
            label: Some("rect-transfer-staging"),
            size: (new_cap * self.vertex_stride) as BufferAddress,
            usage: BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.vertex = device.create_buffer(&BufferDescriptor {
            label: Some("rect-transfer-vertex"),
            size: (new_cap * self.vertex_stride) as BufferAddress,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.capacity_vertices = new_cap;
        // Do not reset used_vertices here; we are in-frame, retaining scratch contents.
    }

    /// Append vertices; defers staging buffer map to flush() for a single map/unmap per frame.
    pub fn append_vertices<T: bytemuck::Pod>(&mut self, device: &Device, verts: &[T]) -> (usize, usize) {
        let count = verts.len();
        if count == 0 {
            return (self.used_vertices, 0);
        }
        let needed = self.used_vertices + count;
        self.ensure_capacity(device, needed);

        let offset_vertices = self.used_vertices;
        let bytes: &[u8] = bytemuck::cast_slice(verts);
        self.cpu_scratch.extend_from_slice(bytes);
        self.used_vertices += count;
        (offset_vertices, count)
    }

    /// Flushes the staging contents into the GPU vertex buffer.
    /// Returns the number of bytes written to the vertex buffer this frame.
    pub fn flush(&mut self, encoder: &mut CommandEncoder, device: &Device) -> BufferAddress {
        let used_bytes = (self.used_vertices * self.vertex_stride) as BufferAddress;
        if used_bytes == 0 {
            return 0;
        }
        // Map once and copy full CPU scratch into staging
        let slice = self.staging.slice(0..used_bytes);
        slice.map_async(wgpu::MapMode::Write, |_| {});
        let _ = device.poll(wgpu::PollType::Wait);
        {
            let mut view = slice.get_mapped_range_mut();
            debug_assert_eq!(self.cpu_scratch.len(), used_bytes as usize);
            view.copy_from_slice(&self.cpu_scratch);
        }
        self.staging.unmap();

        encoder.copy_buffer_to_buffer(&self.staging, 0, &self.vertex, 0, used_bytes);
        used_bytes
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex
    }
}
