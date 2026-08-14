/// Graphics Synthesizer (GS) - minimal fundamentals.
///
/// Implements the privileged register file (memory-mapped 64-bit registers at
/// 0x12000000-0x12001080, used to configure display output) and a PACKED-mode
/// GIFtag parser that decodes the handful of registers needed to draw: PRIM,
/// RGBAQ, and XYZ2 (also accepted for XYZ3/XYZF3). Vertices accumulate per the
/// primitive type selected by PRIM and are rasterized (flat-shaded, using the
/// last vertex's color) into a software framebuffer. REGLIST/IMAGE transfer
/// modes and texturing are not implemented yet.

pub const FB_WIDTH: usize = 640;
pub const FB_HEIGHT: usize = 448;

// PRIM register primitive types (bits 0-2)
const PRIM_POINT: u32 = 0;
const PRIM_LINE: u32 = 1;
const PRIM_LINE_STRIP: u32 = 2;
const PRIM_TRIANGLE: u32 = 3;
const PRIM_TRIANGLE_STRIP: u32 = 4;
const PRIM_TRIANGLE_FAN: u32 = 5;
const PRIM_SPRITE: u32 = 6;

#[derive(Clone, Copy)]
struct Vertex {
    x: i32,
    y: i32,
    color: u32,
}

pub struct Gs {
    // Privileged registers (0x12000000 range), 64-bit, EE-accessible via LD/SD.
    pub pmode: u64,
    pub smode1: u64,
    pub smode2: u64,
    pub dispfb1: u64,
    pub display1: u64,
    pub dispfb2: u64,
    pub display2: u64,
    pub bgcolor: u64,
    pub csr: u64,
    pub imr: u64,

    // Drawing context, set via GIF register writes.
    pub prim: u64,
    pub rgbaq: u64,
    current_color: u32, // cached 0xAARRGGBB from the last RGBAQ write
    prim_type: u32,
    vertex_queue: Vec<Vertex>,

    pub framebuffer: Vec<u32>,
    pub pixels_drawn: u64,
}

impl Gs {
    pub fn new() -> Self {
        Self {
            pmode: 0,
            smode1: 0,
            smode2: 0,
            dispfb1: 0,
            display1: 0,
            dispfb2: 0,
            display2: 0,
            bgcolor: 0,
            csr: 0,
            imr: 0,
            prim: 0,
            rgbaq: 0,
            current_color: 0xFF000000,
            prim_type: PRIM_POINT,
            vertex_queue: Vec::with_capacity(3),
            framebuffer: vec![0; FB_WIDTH * FB_HEIGHT],
            pixels_drawn: 0,
        }
    }

    pub fn read64(&self, addr: u32) -> u64 {
        match addr {
            0x12000000 => self.pmode,
            0x12000010 => self.smode1,
            0x12000020 => self.smode2,
            0x12000070 => self.dispfb1,
            0x12000080 => self.display1,
            0x12000090 => self.dispfb2,
            0x120000A0 => self.display2,
            0x120000E0 => self.bgcolor,
            0x12001000 => self.csr,
            0x12001010 => self.imr,
            _ => 0,
        }
    }

    pub fn write64(&mut self, addr: u32, val: u64) {
        match addr {
            0x12000000 => self.pmode = val,
            0x12000010 => self.smode1 = val,
            0x12000020 => self.smode2 = val,
            0x12000070 => self.dispfb1 = val,
            0x12000080 => self.display1 = val,
            0x12000090 => self.dispfb2 = val,
            0x120000A0 => self.display2 = val,
            0x120000E0 => self.bgcolor = val,
            0x12001000 => self.csr &= !val, // status bits are write-1-to-clear
            0x12001010 => self.imr = val,
            _ => {}
        }
    }

    /// Processes a raw byte stream of GIFtag + register/vertex data, as
    /// delivered by a completed GIF-channel DMA transfer.
    pub fn receive_gif_data(&mut self, data: &[u8]) {
        let mut pos = 0usize;
        while pos + 16 <= data.len() {
            let tag = u128::from_le_bytes(data[pos..pos + 16].try_into().unwrap());
            pos += 16;

            let nloop = (tag & 0x7FFF) as u32;
            let flg = ((tag >> 58) & 0x3) as u32;
            let nreg_raw = ((tag >> 60) & 0xF) as u32;
            let nreg = if nreg_raw == 0 { 16 } else { nreg_raw };
            let regs = (tag >> 64) as u64;

            if flg != 0 {
                // REGLIST/IMAGE modes aren't implemented; skip their payload conservatively.
                let qwords_to_skip = (nloop as usize).saturating_mul(nreg as usize);
                pos += qwords_to_skip.saturating_mul(16).min(data.len().saturating_sub(pos));
                continue;
            }

            for _ in 0..nloop {
                for reg_i in 0..nreg {
                    if pos + 16 > data.len() {
                        return;
                    }
                    let qword = u128::from_le_bytes(data[pos..pos + 16].try_into().unwrap());
                    pos += 16;

                    let reg_code = (regs >> (reg_i * 4)) & 0xF;
                    self.write_packed_register(reg_code as u32, qword);
                }
            }
        }
    }

    fn write_packed_register(&mut self, reg: u32, qword: u128) {
        match reg {
            0x00 => self.set_prim(qword as u64),
            0x01 => {
                // RGBAQ (packed): R, G, B, A each occupy one 32-bit lane, low byte significant
                let r = (qword & 0xFF) as u32;
                let g = ((qword >> 32) & 0xFF) as u32;
                let b = ((qword >> 64) & 0xFF) as u32;
                let a = ((qword >> 96) & 0xFF) as u32;
                self.rgbaq = qword as u64;
                self.current_color = (a << 24) | (r << 16) | (g << 8) | b;
            }
            0x05 | 0x0C | 0x0D => {
                // XYZ2 / XYZ3 / XYZF3 (packed): X @ bits[0:16), Y @ bits[32:48), 12.4 fixed point
                let x_fixed = (qword & 0xFFFF) as u32;
                let y_fixed = ((qword >> 32) & 0xFFFF) as u32;
                let x = (x_fixed >> 4) as i32;
                let y = (y_fixed >> 4) as i32;
                self.kick_vertex(x, y);
            }
            0x0E => {
                // A+D: direct register write. Data = low 64 bits, target register = bits[64:72).
                let value = qword as u64;
                let target = ((qword >> 64) & 0xFF) as u32;
                match target {
                    0x00 => self.set_prim(value),
                    0x01 => {
                        // Native RGBAQ layout: R[0:8) G[8:16) B[16:24) A[24:32) Q[32:64) as f32
                        let r = (value & 0xFF) as u32;
                        let g = ((value >> 8) & 0xFF) as u32;
                        let b = ((value >> 16) & 0xFF) as u32;
                        let a = ((value >> 24) & 0xFF) as u32;
                        self.rgbaq = value;
                        self.current_color = (a << 24) | (r << 16) | (g << 8) | b;
                    }
                    _ => {}
                }
            }
            _ => {} // NOP, ST/UV/TEX0/CLAMP/FOG - not needed yet (no texturing)
        }
    }

    fn set_prim(&mut self, value: u64) {
        self.prim = value;
        self.prim_type = (value & 0x7) as u32;
        self.vertex_queue.clear();
    }

    /// Accepts a newly kicked vertex and rasterizes a primitive once enough
    /// vertices have accumulated, handling strip/fan continuation.
    fn kick_vertex(&mut self, x: i32, y: i32) {
        self.vertex_queue.push(Vertex { x, y, color: self.current_color });

        let needed = match self.prim_type {
            PRIM_POINT => 1,
            PRIM_LINE | PRIM_LINE_STRIP => 2,
            PRIM_TRIANGLE | PRIM_TRIANGLE_STRIP | PRIM_TRIANGLE_FAN => 3,
            PRIM_SPRITE => 2,
            _ => return, // unsupported primitive type
        };
        if self.vertex_queue.len() < needed {
            return;
        }

        let n = self.vertex_queue.len();
        match self.prim_type {
            PRIM_POINT => {
                let v = self.vertex_queue[0];
                self.plot_pixel(v.x, v.y, v.color);
                self.vertex_queue.clear();
            }
            PRIM_LINE => {
                let (a, b) = (self.vertex_queue[0], self.vertex_queue[1]);
                self.draw_line(a, b);
                self.vertex_queue.clear();
            }
            PRIM_LINE_STRIP => {
                let (a, b) = (self.vertex_queue[n - 2], self.vertex_queue[n - 1]);
                self.draw_line(a, b);
                self.vertex_queue.remove(0);
            }
            PRIM_TRIANGLE => {
                let (a, b, c) = (self.vertex_queue[0], self.vertex_queue[1], self.vertex_queue[2]);
                self.draw_triangle(a, b, c);
                self.vertex_queue.clear();
            }
            PRIM_TRIANGLE_STRIP => {
                let (a, b, c) = (self.vertex_queue[n - 3], self.vertex_queue[n - 2], self.vertex_queue[n - 1]);
                self.draw_triangle(a, b, c);
                self.vertex_queue.remove(0);
            }
            PRIM_TRIANGLE_FAN => {
                let (a, b, c) = (self.vertex_queue[0], self.vertex_queue[n - 2], self.vertex_queue[n - 1]);
                self.draw_triangle(a, b, c);
                let anchor = self.vertex_queue[0];
                let last = self.vertex_queue[n - 1];
                self.vertex_queue = vec![anchor, last];
            }
            PRIM_SPRITE => {
                let (a, b) = (self.vertex_queue[0], self.vertex_queue[1]);
                self.draw_sprite(a, b);
                self.vertex_queue.clear();
            }
            _ => {}
        }
    }

    fn draw_line(&mut self, a: Vertex, b: Vertex) {
        let (mut x0, mut y0) = (a.x, a.y);
        let (x1, y1) = (b.x, b.y);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.plot_pixel(x0, y0, b.color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn draw_triangle(&mut self, a: Vertex, b: Vertex, c: Vertex) {
        let color = c.color; // flat shading: last vertex's color
        let min_x = a.x.min(b.x).min(c.x).max(0);
        let max_x = a.x.max(b.x).max(c.x).min(FB_WIDTH as i32 - 1);
        let min_y = a.y.min(b.y).min(c.y).max(0);
        let max_y = a.y.max(b.y).max(c.y).min(FB_HEIGHT as i32 - 1);

        let edge = |p0: Vertex, p1: Vertex, x: i32, y: i32| -> i64 {
            (p1.x - p0.x) as i64 * (y - p0.y) as i64 - (p1.y - p0.y) as i64 * (x - p0.x) as i64
        };

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let w0 = edge(b, c, x, y);
                let w1 = edge(c, a, x, y);
                let w2 = edge(a, b, x, y);
                let inside = (w0 >= 0 && w1 >= 0 && w2 >= 0) || (w0 <= 0 && w1 <= 0 && w2 <= 0);
                if inside {
                    self.plot_pixel(x, y, color);
                }
            }
        }
    }

    fn draw_sprite(&mut self, a: Vertex, b: Vertex) {
        // Axis-aligned filled rectangle; per GS convention, only the second vertex's color matters.
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        let color = b.color;
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.plot_pixel(x, y, color);
            }
        }
    }

    fn plot_pixel(&mut self, x: i32, y: i32, color: u32) {
        if x < 0 || y < 0 || x as usize >= FB_WIDTH || y as usize >= FB_HEIGHT {
            return;
        }
        self.framebuffer[y as usize * FB_WIDTH + x as usize] = color;
        self.pixels_drawn += 1;
    }
}
