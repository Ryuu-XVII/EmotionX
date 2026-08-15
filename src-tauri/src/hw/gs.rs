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
pub const VRAM_SIZE: usize = 4 * 1024 * 1024; // 4MB GS onboard VRAM

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
    u: f32,
    v: f32,
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
    pub tex0_1: u64,
    pub tex0_2: u64,
    pub tex1_1: u64,
    pub tex1_2: u64,
    pub clamp_1: u64,
    pub clamp_2: u64,
    pub frame_1: u64,
    pub frame_2: u64,
    pub zbuf_1: u64,
    pub zbuf_2: u64,
    pub scissor_1: u64,
    pub scissor_2: u64,
    pub test_1: u64,
    pub test_2: u64,
    pub alpha_1: u64,
    pub alpha_2: u64,
    pub bitbltbuf: u64,
    pub trxpos: u64,
    pub trxreg: u64,
    pub trxdir: u64,

    current_color: u32, // cached 0xAARRGGBB from the last RGBAQ write
    current_u: f32,
    current_v: f32,
    prim_type: u32,
    vertex_queue: Vec<Vertex>,

    pub framebuffer: Vec<u32>,
    pub vram: Vec<u8>,
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
            // GS Revision 0x2E in bits 16..24, initial FIELD = 0
            csr: 0x2Eu64 << 16,
            imr: 0,
            prim: 0,
            rgbaq: 0,
            tex0_1: 0,
            tex0_2: 0,
            tex1_1: 0,
            tex1_2: 0,
            clamp_1: 0,
            clamp_2: 0,
            frame_1: 0,
            frame_2: 0,
            zbuf_1: 0,
            zbuf_2: 0,
            scissor_1: 0,
            scissor_2: 0,
            test_1: 0,
            test_2: 0,
            alpha_1: 0,
            alpha_2: 0,
            bitbltbuf: 0,
            trxpos: 0,
            trxreg: 0,
            trxdir: 0,
            current_color: 0xFFFFFFFF,
            current_u: 0.0,
            current_v: 0.0,
            prim_type: 0,
            vertex_queue: Vec::with_capacity(8),
            framebuffer: vec![0; FB_WIDTH * FB_HEIGHT],
            vram: vec![0; VRAM_SIZE],
            pixels_drawn: 0,
        }
    }

    /// Toggles the GS field bit (odd/even) and asserts VSINT on VBlank intervals.
    pub fn toggle_vblank(&mut self) {
        // Toggle FIELD (bit 13)
        self.csr ^= 1 << 13;
        // Set VSINT (bit 3)
        self.csr |= 1 << 3;
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
            0x12001000 => {
                // RESET (bit 9)
                if (val & (1 << 9)) != 0 {
                    self.prim = 0;
                    self.csr = 0x2Eu64 << 16;
                }
                // Status bits (bits 0..5: SIGNAL, FINISH, HSINT, VSINT, EDWINT) are write-1-to-clear
                self.csr &= !(val & 0x3F);
            },
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

            if flg == 2 {
                // IMAGE mode: raw pixel stream directly into GS VRAM
                // Target address determined by BITBLTBUF (DBP @ bits 32..46)
                let dbp = ((self.bitbltbuf >> 32) & 0x3FFF) as usize;
                let vram_base = dbp * 256;
                let qwords = nloop as usize;
                let bytes_to_copy = qwords * 16;
                let available = data.len().saturating_sub(pos);
                let copy_len = bytes_to_copy.min(available);
                if vram_base + copy_len <= self.vram.len() {
                    self.vram[vram_base..vram_base + copy_len].copy_from_slice(&data[pos..pos + copy_len]);
                }
                pos += copy_len;
                continue;
            } else if flg != 0 {
                // REGLIST mode: skip payload conservatively
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
            0x02 => {
                // ST (packed): S @ bits[0:32), T @ bits[32:64), Q @ bits[64:96)
                let s = f32::from_bits((qword & 0xFFFFFFFF) as u32);
                let t = f32::from_bits(((qword >> 32) & 0xFFFFFFFF) as u32);
                let q = f32::from_bits(((qword >> 64) & 0xFFFFFFFF) as u32);
                let q_denom = if q.abs() > 0.00001 { q } else { 1.0 };
                self.current_u = s / q_denom;
                self.current_v = t / q_denom;
            }
            0x03 => {
                // UV (packed): U @ bits[0:16), V @ bits[16:32), 12.4 fixed point
                let u_fixed = (qword & 0xFFFF) as u32;
                let v_fixed = ((qword >> 16) & 0xFFFF) as u32;
                self.current_u = (u_fixed as f32) / 16.0;
                self.current_v = (v_fixed as f32) / 16.0;
            }
            0x05 | 0x0C | 0x0D => {
                // XYZ2 / XYZ3 / XYZF3 (packed): X @ bits[0:16), Y @ bits[32:48), 12.4 fixed point
                let x_fixed = (qword & 0xFFFF) as u32;
                let y_fixed = ((qword >> 32) & 0xFFFF) as u32;
                let x = (x_fixed >> 4) as i32;
                let y = (y_fixed >> 4) as i32;
                self.kick_vertex(x, y);
            }
            0x06 => {
                // TEX0_1 (packed): low 64 bits
                self.tex0_1 = qword as u64;
            }
            0x07 => {
                // TEX0_2 (packed)
                self.tex0_2 = qword as u64;
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
                    0x06 => self.tex0_1 = value,
                    0x07 => self.tex0_2 = value,
                    0x14 => self.tex1_1 = value,
                    0x15 => self.tex1_2 = value,
                    0x16 => self.clamp_1 = value,
                    0x17 => self.clamp_2 = value,
                    0x40 => self.csr |= 1 << 1, // FINISH event
                    0x42 => self.alpha_1 = value,
                    0x43 => self.alpha_2 = value,
                    0x4C => self.frame_1 = value,
                    0x4D => self.frame_2 = value,
                    0x4E => self.zbuf_1 = value,
                    0x4F => self.zbuf_2 = value,
                    0x40..=0x41 => self.scissor_1 = value,
                    0x50 => self.bitbltbuf = value,
                    0x51 => self.trxpos = value,
                    0x52 => self.trxreg = value,
                    0x53 => self.trxdir = value,
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn set_prim(&mut self, value: u64) {
        self.prim = value;
        self.prim_type = (value & 0x7) as u32;
        self.vertex_queue.clear();
    }

    /// Samples a 32-bit RGBA texel from GS VRAM using the active TEX0_1 context.
    pub fn sample_texture(&self, u: f32, v: f32) -> u32 {
        let tbp0 = ((self.tex0_1 & 0x3FFF) as usize) * 256;
        let tw_exp = (((self.tex0_1 >> 26) & 0xF) as usize).min(11);
        let th_exp = (((self.tex0_1 >> 30) & 0xF) as usize).min(11);
        let tw = (1usize << tw_exp).max(1);
        let th = (1usize << th_exp).max(1);

        let tx = ((u.abs() * tw as f32) as usize) % tw;
        let ty = ((v.abs() * th as f32) as usize) % th;
        let offset = tbp0 + (ty * tw + tx) * 4;

        if offset + 4 <= self.vram.len() {
            let r = self.vram[offset] as u32;
            let g = self.vram[offset + 1] as u32;
            let b = self.vram[offset + 2] as u32;
            let a = self.vram[offset + 3] as u32;
            (a << 24) | (r << 16) | (g << 8) | b
        } else {
            0xFFFFFFFF
        }
    }

    /// Accepts a newly kicked vertex and rasterizes a primitive once enough
    /// vertices have accumulated, handling strip/fan continuation.
    fn kick_vertex(&mut self, x: i32, y: i32) {
        self.vertex_queue.push(Vertex {
            x,
            y,
            u: self.current_u,
            v: self.current_v,
            color: self.current_color,
        });

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
        let textured = (self.prim & (1 << 4)) != 0;
        let base_color = c.color; // flat shading: last vertex's color
        let min_x = a.x.min(b.x).min(c.x).max(0);
        let max_x = a.x.max(b.x).max(c.x).min(FB_WIDTH as i32 - 1);
        let min_y = a.y.min(b.y).min(c.y).max(0);
        let max_y = a.y.max(b.y).max(c.y).min(FB_HEIGHT as i32 - 1);

        let edge = |p0: Vertex, p1: Vertex, x: i32, y: i32| -> i64 {
            (p1.x - p0.x) as i64 * (y - p0.y) as i64 - (p1.y - p0.y) as i64 * (x - p0.x) as i64
        };

        let area = edge(a, b, c.x, c.y).abs().max(1) as f32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let w0 = edge(b, c, x, y);
                let w1 = edge(c, a, x, y);
                let w2 = edge(a, b, x, y);
                let inside = (w0 >= 0 && w1 >= 0 && w2 >= 0) || (w0 <= 0 && w1 <= 0 && w2 <= 0);
                if inside {
                    if textured {
                        let l0 = (w0.abs() as f32) / area;
                        let l1 = (w1.abs() as f32) / area;
                        let l2 = (w2.abs() as f32) / area;
                        let u = l0 * a.u + l1 * b.u + l2 * c.u;
                        let v = l0 * a.v + l1 * b.v + l2 * c.v;
                        let tex_col = self.sample_texture(u, v);
                        self.plot_pixel(x, y, tex_col);
                    } else {
                        self.plot_pixel(x, y, base_color);
                    }
                }
            }
        }
    }

    fn draw_sprite(&mut self, a: Vertex, b: Vertex) {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        let textured = (self.prim & (1 << 4)) != 0;
        let base_color = b.color;
        let width = (x1 - x0).max(1) as f32;
        let height = (y1 - y0).max(1) as f32;

        for y in y0..=y1 {
            let v_ratio = (y - y0) as f32 / height;
            let v = a.v + v_ratio * (b.v - a.v);
            for x in x0..=x1 {
                if textured {
                    let u_ratio = (x - x0) as f32 / width;
                    let u = a.u + u_ratio * (b.u - a.u);
                    let tex_col = self.sample_texture(u, v);
                    self.plot_pixel(x, y, tex_col);
                } else {
                    self.plot_pixel(x, y, base_color);
                }
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
