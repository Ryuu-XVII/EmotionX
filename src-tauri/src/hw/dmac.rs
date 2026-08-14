/// DMA Controller (DMAC) - moves data between RAM and the EE's peripherals
/// (GIF, VIF0/1, IPU, SIF, scratchpad) without CPU intervention.
///
/// Only the GIF channel is wired to an actual destination device right now
/// (see `Bus::execute_dma`); the others accept register writes and complete
/// instantly so BIOS/game code polling CHCR doesn't hang.

pub const CH_VIF0: usize = 0;
pub const CH_VIF1: usize = 1;
pub const CH_GIF: usize = 2;
pub const CH_IPU_FROM: usize = 3;
pub const CH_IPU_TO: usize = 4;
pub const CH_SIF0: usize = 5;
pub const CH_SIF1: usize = 6;
pub const CH_SIF2: usize = 7;
pub const CH_SPR_FROM: usize = 8;
pub const CH_SPR_TO: usize = 9;

const CHANNEL_BASE: [u32; 10] = [
    0x10008000, // VIF0
    0x10009000, // VIF1
    0x1000A000, // GIF
    0x1000B000, // IPU_FROM
    0x1000B400, // IPU_TO
    0x1000C000, // SIF0
    0x1000C400, // SIF1
    0x1000C800, // SIF2
    0x1000D000, // SPR_FROM
    0x1000D400, // SPR_TO
];

#[derive(Default, Clone, Copy)]
pub struct Channel {
    pub chcr: u32,
    pub madr: u32,
    pub qwc: u32,
    pub tadr: u32,
    pub asr0: u32,
    pub asr1: u32,
}

pub struct Dmac {
    pub channels: [Channel; 10],
    pub d_ctrl: u32,
    pub d_stat: u32,
    pub d_pcr: u32,
    pub d_sqwc: u32,
    pub d_rbsr: u32,
    pub d_rbor: u32,
    pub d_stadr: u32,
    pub d_enable: u32,
}

impl Dmac {
    pub fn new() -> Self {
        Self {
            channels: [Channel::default(); 10],
            d_ctrl: 0,
            d_stat: 0,
            d_pcr: 0,
            d_sqwc: 0,
            d_rbsr: 0,
            d_rbor: 0,
            d_stadr: 0,
            d_enable: 0,
        }
    }

    fn find_channel(addr: u32) -> Option<(usize, u32)> {
        for (i, &base) in CHANNEL_BASE.iter().enumerate() {
            if addr >= base && addr < base + 0x60 {
                return Some((i, addr - base));
            }
        }
        None
    }

    pub fn is_dmac_addr(addr: u32) -> bool {
        Self::find_channel(addr).is_some()
            || matches!(
                addr,
                0x1000E000 | 0x1000E010 | 0x1000E020 | 0x1000E030 | 0x1000E040 | 0x1000E050
                    | 0x1000E060 | 0x1000F520 | 0x1000F590
            )
    }

    pub fn read_reg(&self, addr: u32) -> u32 {
        if let Some((ch, off)) = Self::find_channel(addr) {
            let c = &self.channels[ch];
            return match off {
                0x00 => c.chcr,
                0x10 => c.madr,
                0x20 => c.qwc,
                0x30 => c.tadr,
                0x40 => c.asr0,
                0x50 => c.asr1,
                _ => 0,
            };
        }
        match addr {
            0x1000E000 => self.d_ctrl,
            0x1000E010 => self.d_stat,
            0x1000E020 => self.d_pcr,
            0x1000E030 => self.d_sqwc,
            0x1000E040 => self.d_rbsr,
            0x1000E050 => self.d_rbor,
            0x1000E060 => self.d_stadr,
            0x1000F520 | 0x1000F590 => self.d_enable,
            _ => 0,
        }
    }

    /// Writes a DMAC register. Returns `Some(channel)` if this write set a
    /// channel's STR (start) bit that wasn't already set, requesting a kick.
    pub fn write_reg(&mut self, addr: u32, val: u32) -> Option<usize> {
        if let Some((ch, off)) = Self::find_channel(addr) {
            let c = &mut self.channels[ch];
            match off {
                0x00 => {
                    let was_running = (c.chcr & (1 << 8)) != 0;
                    c.chcr = val;
                    let now_running = (c.chcr & (1 << 8)) != 0;
                    if now_running && !was_running {
                        return Some(ch);
                    }
                }
                0x10 => c.madr = val,
                0x20 => c.qwc = val & 0xFFFF,
                0x30 => c.tadr = val,
                0x40 => c.asr0 = val,
                0x50 => c.asr1 = val,
                _ => {}
            }
            return None;
        }
        match addr {
            0x1000E000 => self.d_ctrl = val,
            0x1000E010 => self.d_stat &= !val, // status bits are write-1-to-clear
            0x1000E020 => self.d_pcr = val,
            0x1000E030 => self.d_sqwc = val,
            0x1000E040 => self.d_rbsr = val,
            0x1000E050 => self.d_rbor = val,
            0x1000E060 => self.d_stadr = val,
            0x1000F520 | 0x1000F590 => self.d_enable = val,
            _ => {}
        }
        None
    }

    pub fn clear_str(&mut self, ch: usize) {
        self.channels[ch].chcr &= !(1 << 8);
    }
}
