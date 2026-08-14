/// VU0 register file, as exposed to the EE via COP2 "macro mode".
///
/// Real VU0 macro mode is a large instruction set (~90 opcodes across FMAC,
/// integer, branch, and transcendental units). This implementation covers a
/// deliberately scoped, verified subset: register transfers (QMFC2/QMTC2/
/// CFC2/CTC2/LQC2/SQC2/BC2) plus the broadcast FMAC family (ADDbc/SUBbc/
/// MAXbc/MINIbc/MULbc/MADDbc/MSUBbc) and the non-broadcast accumulate family
/// (ADDA/SUBA/MULA/MADDA/MSUBA/OPMULA), cross-checked against DobieStation's
/// interpreter source. The combined broadcast+accumulate opcodes used by the
/// canonical `vmulax`/`vmadday`/`vmaddaz`/`vmaddw` matrix-transform idiom, and
/// the full "lower" integer/branch instruction family (VIADD, VMTIR, VLQI,
/// etc.), are NOT yet implemented - their exact macro-mode bit encoding could
/// not be confirmed against an authoritative source in this environment, so
/// they're left as logged-and-ignored rather than guessed.
pub struct Vu0 {
    pub vf: [[f32; 4]; 32], // [x, y, z, w] per register; vf[0] is hardwired to (0,0,0,1)
    pub vi: [u16; 16],      // vi[0] is hardwired to 0
    pub acc: [f32; 4],
    pub q: f32,
    pub clip_flag: bool,
}

impl Vu0 {
    pub fn new() -> Self {
        let mut vf = [[0.0f32; 4]; 32];
        vf[0] = [0.0, 0.0, 0.0, 1.0];
        Self {
            vf,
            vi: [0; 16],
            acc: [0.0; 4],
            q: 1.0,
            clip_flag: false,
        }
    }

    pub fn get_vf(&self, i: usize) -> [f32; 4] {
        self.vf[i]
    }

    /// Writes only the components enabled by `mask` (bit3=X, bit2=Y, bit1=Z, bit0=W).
    pub fn set_vf_masked(&mut self, i: usize, val: [f32; 4], mask: u32) {
        if i == 0 {
            return;
        }
        for c in 0..4 {
            if (mask & (0x8 >> c)) != 0 {
                self.vf[i][c] = val[c];
            }
        }
    }

    pub fn get_vi(&self, i: usize) -> u16 {
        self.vi[i]
    }

    pub fn set_vi(&mut self, i: usize, val: u16) {
        if i != 0 {
            self.vi[i] = val;
        }
    }
}
