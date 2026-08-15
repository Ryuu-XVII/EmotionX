/// PlayStation 2 Emotion Engine Timers (T0, T1, T2, T3)
///
/// T0: 0x10000000..=0x10000030 (supports HBlank gating)
/// T1: 0x10000800..=0x10000830 (supports VBlank gating)
/// T2: 0x10001000..=0x10001020 (system clock / 1, / 16, / 256)
/// T3: 0x10001800..=0x10001820 (system clock / 1, / 16, / 256)

#[derive(Default)]
pub struct Timer {
    pub count: u32,
    pub mode: u32,
    pub comp: u32,
    pub hold: u32,
}

pub struct Timers {
    pub timers: [Timer; 4],
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                Timer::default(),
                Timer::default(),
                Timer::default(),
                Timer::default(),
            ],
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        for t in &mut self.timers {
            // If timer count is enabled (bit 7 of MODE is usually start/stop)
            t.count = t.count.wrapping_add(cycles);
        }
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let (ch, offset) = match addr {
            0x10000000..=0x10000030 => (0, addr - 0x10000000),
            0x10000800..=0x10000830 => (1, addr - 0x10000800),
            0x10001000..=0x10001020 => (2, addr - 0x10001000),
            0x10001800..=0x10001820 => (3, addr - 0x10001800),
            _ => return 0,
        };

        // Reading COUNT advances it slightly to satisfy busy-wait polling loops
        match offset {
            0x00 => {
                let val = self.timers[ch].count;
                self.timers[ch].count = self.timers[ch].count.wrapping_add(16);
                val
            },
            0x10 => self.timers[ch].mode,
            0x20 => self.timers[ch].comp,
            0x30 => self.timers[ch].hold,
            _ => 0,
        }
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        let (ch, offset) = match addr {
            0x10000000..=0x10000030 => (0, addr - 0x10000000),
            0x10000800..=0x10000830 => (1, addr - 0x10000800),
            0x10001000..=0x10001020 => (2, addr - 0x10001000),
            0x10001800..=0x10001820 => (3, addr - 0x10001800),
            _ => return,
        };

        match offset {
            0x00 => self.timers[ch].count = val,
            0x10 => {
                self.timers[ch].mode = val;
                // Clearing interrupt or overflow flags
                if (val & (1 << 10)) != 0 || (val & (1 << 11)) != 0 {
                    self.timers[ch].mode &= !((1 << 10) | (1 << 11));
                }
            },
            0x20 => self.timers[ch].comp = val,
            0x30 => self.timers[ch].hold = val,
            _ => {},
        }
    }
}
