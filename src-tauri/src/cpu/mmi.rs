/// MMI (MultiMedia Instructions) - the EE's SIMD extension operating on the
/// 128-bit GPRs as packed vectors of bytes/halfwords/words. Real retail PS2
/// games use these from their very first instructions (compiler-generated
/// memcpy/memset and vector math), unlike BIOS/homebrew which mostly avoid
/// them - so this is a hard requirement for booting real game code, not an
/// optional extra.
///
/// Scope: implements the elementwise arithmetic/compare/min-max/logic family
/// (high confidence - standard, unambiguous SIMD semantics once operand
/// order is fixed) plus a best-effort implementation of the extend/pack/copy
/// shuffle family (PEXTL/PEXTU/PPAC/PCPY*, lower confidence on exact
/// operand-to-lane mapping without a reference implementation to verify
/// against). NOT implemented: the multiply/divide pipeline family (MULT1/
/// DIV1/MADD1/PMULTW/PDIVW/PMFHL/PMTHL and friends, which need a second
/// 128-bit-wide HI1/LO1 register pair) and the parallel shift family -
/// deferred to a follow-up pass.

pub fn lanes32(v: u128) -> [u32; 4] {
    let b = v.to_le_bytes();
    let mut out = [0u32; 4];
    for i in 0..4 {
        out[i] = u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
    }
    out
}

pub fn from_lanes32(lanes: [u32; 4]) -> u128 {
    let mut b = [0u8; 16];
    for i in 0..4 {
        b[i * 4..i * 4 + 4].copy_from_slice(&lanes[i].to_le_bytes());
    }
    u128::from_le_bytes(b)
}

pub fn lanes16(v: u128) -> [u16; 8] {
    let b = v.to_le_bytes();
    let mut out = [0u16; 8];
    for i in 0..8 {
        out[i] = u16::from_le_bytes(b[i * 2..i * 2 + 2].try_into().unwrap());
    }
    out
}

pub fn from_lanes16(lanes: [u16; 8]) -> u128 {
    let mut b = [0u8; 16];
    for i in 0..8 {
        b[i * 2..i * 2 + 2].copy_from_slice(&lanes[i].to_le_bytes());
    }
    u128::from_le_bytes(b)
}

pub fn lanes8(v: u128) -> [u8; 16] {
    v.to_le_bytes()
}

pub fn from_lanes8(lanes: [u8; 16]) -> u128 {
    u128::from_le_bytes(lanes)
}

pub fn lanes64(v: u128) -> [u64; 2] {
    let b = v.to_le_bytes();
    [
        u64::from_le_bytes(b[0..8].try_into().unwrap()),
        u64::from_le_bytes(b[8..16].try_into().unwrap()),
    ]
}

pub fn from_lanes64(lanes: [u64; 2]) -> u128 {
    let mut b = [0u8; 16];
    b[0..8].copy_from_slice(&lanes[0].to_le_bytes());
    b[8..16].copy_from_slice(&lanes[1].to_le_bytes());
    u128::from_le_bytes(b)
}

// --- Elementwise arithmetic/compare/logic (high confidence) ---

pub fn paddw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| a[i].wrapping_add(b[i])))
}
pub fn psubw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| a[i].wrapping_sub(b[i])))
}
pub fn paddh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| a[i].wrapping_add(b[i])))
}
pub fn psubh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| a[i].wrapping_sub(b[i])))
}
pub fn paddb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| a[i].wrapping_add(b[i])))
}
pub fn psubb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| a[i].wrapping_sub(b[i])))
}

pub fn paddsw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| (a[i] as i32).saturating_add(b[i] as i32) as u32))
}
pub fn psubsw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| (a[i] as i32).saturating_sub(b[i] as i32) as u32))
}
pub fn paddsh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| (a[i] as i16).saturating_add(b[i] as i16) as u16))
}
pub fn psubsh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| (a[i] as i16).saturating_sub(b[i] as i16) as u16))
}
pub fn paddsb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| (a[i] as i8).saturating_add(b[i] as i8) as u8))
}
pub fn psubsb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| (a[i] as i8).saturating_sub(b[i] as i8) as u8))
}

pub fn padduw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| a[i].saturating_add(b[i])))
}
pub fn psubuw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| a[i].saturating_sub(b[i])))
}
pub fn padduh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| a[i].saturating_add(b[i])))
}
pub fn psubuh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| a[i].saturating_sub(b[i])))
}
pub fn paddub(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| a[i].saturating_add(b[i])))
}
pub fn psubub(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| a[i].saturating_sub(b[i])))
}

pub fn pcgtw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| if (a[i] as i32) > (b[i] as i32) { 0xFFFFFFFF } else { 0 }))
}
pub fn pcgth(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| if (a[i] as i16) > (b[i] as i16) { 0xFFFF } else { 0 }))
}
pub fn pcgtb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| if (a[i] as i8) > (b[i] as i8) { 0xFF } else { 0 }))
}
pub fn pceqw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| if a[i] == b[i] { 0xFFFFFFFF } else { 0 }))
}
pub fn pceqh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| if a[i] == b[i] { 0xFFFF } else { 0 }))
}
pub fn pceqb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| if a[i] == b[i] { 0xFF } else { 0 }))
}

pub fn pmaxw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| ((a[i] as i32).max(b[i] as i32)) as u32))
}
pub fn pminw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32(std::array::from_fn(|i| ((a[i] as i32).min(b[i] as i32)) as u32))
}
pub fn pmaxh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| ((a[i] as i16).max(b[i] as i16)) as u16))
}
pub fn pminh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16(std::array::from_fn(|i| ((a[i] as i16).min(b[i] as i16)) as u16))
}

pub fn pabsw(rs: u128) -> u128 {
    let a = lanes32(rs);
    from_lanes32(std::array::from_fn(|i| (a[i] as i32).unsigned_abs()))
}
pub fn pabsh(rs: u128) -> u128 {
    let a = lanes16(rs);
    from_lanes16(std::array::from_fn(|i| (a[i] as i16).unsigned_abs()))
}

pub fn pand(rs: u128, rt: u128) -> u128 {
    rs & rt
}
pub fn por(rs: u128, rt: u128) -> u128 {
    rs | rt
}
pub fn pxor(rs: u128, rt: u128) -> u128 {
    rs ^ rt
}
pub fn pnor(rs: u128, rt: u128) -> u128 {
    !(rs | rt)
}

/// Parallel leading zero/one count per 32-bit lane (excluding the sign bit).
pub fn plzcw(rs: u128) -> u128 {
    let a = lanes32(rs);
    from_lanes32(std::array::from_fn(|i| {
        let v = a[i] as i32;
        let bits = if v < 0 { !v } else { v } as u32;
        if bits == 0 { 31 } else { bits.leading_zeros() - 1 }
    }))
}

// --- Extend / pack / copy shuffle family (best-effort) ---

pub fn pextlw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32([a[0], b[0], a[1], b[1]])
}
pub fn pextuw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32([a[2], b[2], a[3], b[3]])
}
pub fn pextlh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16([a[0], b[0], a[1], b[1], a[2], b[2], a[3], b[3]])
}
pub fn pextuh(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16([a[4], b[4], a[5], b[5], a[6], b[6], a[7], b[7]])
}
pub fn pextlb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| if i % 2 == 0 { a[i / 2] } else { b[i / 2] }))
}
pub fn pextub(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| if i % 2 == 0 { a[8 + i / 2] } else { b[8 + i / 2] }))
}

pub fn ppacw(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes32(rs), lanes32(rt));
    from_lanes32([b[0], b[2], a[0], a[2]])
}
pub fn ppach(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes16(rs), lanes16(rt));
    from_lanes16([b[0], b[2], b[4], b[6], a[0], a[2], a[4], a[6]])
}
pub fn ppacb(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes8(rs), lanes8(rt));
    from_lanes8(std::array::from_fn(|i| if i < 8 { b[i * 2] } else { a[(i - 8) * 2] }))
}

pub fn pcpyld(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes64(rs), lanes64(rt));
    from_lanes64([b[0], a[0]])
}
pub fn pcpyud(rs: u128, rt: u128) -> u128 {
    let (a, b) = (lanes64(rs), lanes64(rt));
    from_lanes64([a[1], b[1]])
}
pub fn pcpyh(rt: u128) -> u128 {
    let b = lanes16(rt);
    from_lanes16([b[0]; 8])
}

/// Quadword Funnel Shift Right Variable: concatenates rs (high) and rt (low),
/// then shifts right by (sa & 0xF) bytes.
pub fn qfsrv(rs: u128, rt: u128, sa: u32) -> u128 {
    let s = (sa & 0xF) as usize;
    if s == 0 {
        return rt;
    }
    let b_rs = rs.to_le_bytes();
    let b_rt = rt.to_le_bytes();
    let mut out = [0u8; 16];
    for i in 0..(16 - s) {
        out[i] = b_rt[i + s];
    }
    for i in 0..s {
        out[16 - s + i] = b_rs[i];
    }
    u128::from_le_bytes(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paddw_wraps() {
        let rs = from_lanes32([1, 0xFFFFFFFF, 100, 0]);
        let rt = from_lanes32([1, 1, 200, 0]);
        assert_eq!(lanes32(paddw(rs, rt)), [2, 0, 300, 0]);
    }

    #[test]
    fn test_padduw_saturates() {
        let rs = from_lanes32([0xFFFFFFFF, 5, 0, 0]);
        let rt = from_lanes32([1, 3, 0, 0]);
        assert_eq!(lanes32(padduw(rs, rt)), [0xFFFFFFFF, 8, 0, 0]);
    }

    #[test]
    fn test_psubsb_saturates_signed() {
        let rs = from_lanes8([0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // -128
        let rt = from_lanes8([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        // -128 - 1 saturates to -128 (i8 min)
        assert_eq!(lanes8(psubsb(rs, rt))[0], 0x80);
    }

    #[test]
    fn test_pcgtw_mask() {
        let rs = from_lanes32([5, 1, 0xFFFFFFFF, 0]); // lane2 = -1 signed
        let rt = from_lanes32([3, 1, 0, 0]);
        assert_eq!(lanes32(pcgtw(rs, rt)), [0xFFFFFFFF, 0, 0, 0]);
    }

    #[test]
    fn test_pand_por_pxor_pnor() {
        let rs = 0xFF00u128;
        let rt = 0x0FF0u128;
        assert_eq!(pand(rs, rt), 0x0F00);
        assert_eq!(por(rs, rt), 0xFFF0);
        assert_eq!(pxor(rs, rt), 0xF0F0);
        assert_eq!(pnor(rs, rt), !(0xFFF0u128));
    }

    #[test]
    fn test_pextlw_interleaves_low_lanes() {
        let rs = from_lanes32([1, 2, 3, 4]);
        let rt = from_lanes32([10, 20, 30, 40]);
        assert_eq!(lanes32(pextlw(rs, rt)), [1, 10, 2, 20]);
    }

    #[test]
    fn test_pcpyld_pcpyud() {
        let rs = from_lanes64([0x1111, 0x2222]);
        let rt = from_lanes64([0x3333, 0x4444]);
        assert_eq!(lanes64(pcpyld(rs, rt)), [0x3333, 0x1111]);
        assert_eq!(lanes64(pcpyud(rs, rt)), [0x2222, 0x4444]);
    }

    #[test]
    fn test_plzcw() {
        // 0x00000001 as i32 is positive; leading zeros of bits=1 -> 31, minus sign bit -> 30
        let rs = from_lanes32([1, 0, 0x80000000, 0xFFFFFFFF]);
        let result = lanes32(plzcw(rs));
        assert_eq!(result[0], 30); // positive 1: 31 leading zero bits before the 1, minus sign bit = 30
        assert_eq!(result[1], 31); // zero: defined as 31
        assert_eq!(result[2], 0);  // 0x80000000 is negative (-2^31); ~v = 0x7FFFFFFF has 0 leading zeros... see below
        assert_eq!(result[3], 31); // 0xFFFFFFFF = -1; ~v = 0 -> 31
    }

    #[test]
    fn test_qfsrv_funnel_shift() {
        let rs = from_lanes8([16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]);
        let rt = from_lanes8([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);

        // sa = 0 -> returns rt unmodified
        assert_eq!(lanes8(qfsrv(rs, rt, 0)), lanes8(rt));

        // sa = 4 -> shifts right by 4 bytes: lower 12 bytes from rt[4..16], top 4 bytes from rs[0..4]
        let res4 = lanes8(qfsrv(rs, rt, 4));
        assert_eq!(res4[0..12], [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        assert_eq!(res4[12..16], [16, 17, 18, 19]);
    }
}
