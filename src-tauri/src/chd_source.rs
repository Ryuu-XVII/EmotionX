/// Thin adapter over the `chd` crate exposing a flat, randomly-addressable
/// 2048-byte-sector byte stream, so `Iso9660` can read a CHD-compressed disc
/// image the same way it reads a raw `.iso` file.
///
/// CHDs come in two relevant layouts:
/// - **DVD-style**: each hunk's decompressed bytes map 1:1 onto the disc's
///   logical byte stream (`unit_bytes == 2048`). Common for PS2 DVD rips
///   made with `chdman createdvd`.
/// - **CD-style**: each "unit" is `unit_bytes` long (2352 or 2448, the
///   latter including 96 bytes of trailing subchannel data) but holds the
///   2048 bytes of user data starting at the very beginning of the unit,
///   followed by padding/ECC remnants (and subchannel, if present) that
///   this adapter ignores. Confirmed empirically against a real PS2 CHD
///   rip (`Iso9660`'s search for the ISO9660 `CD001` signature landed
///   exactly at unit-local offset 1, matching the standard's sector-local
///   offset 1 with zero prefix skip) - unlike a literal raw MODE1 CD sector
///   dump, no 16-byte sync+header prefix is stored per unit here.
use std::fs::File;
use std::io::BufReader;
use crate::iso9660::DiscSource;

const LOGICAL_SECTOR: u64 = 2048;
const CD_SECTOR_DATA_OFFSET: u64 = 0;

pub struct ChdSource {
    chd: chd::Chd<BufReader<File>>,
    hunk_size: u64,
    unit_bytes: u64,
    units_per_hunk: u64,
    cd_mode: bool,
    cached_hunk_index: Option<u32>,
    cached_hunk_buf: Vec<u8>,
    compressed_scratch: Vec<u8>,
}

impl ChdSource {
    pub fn open(path: &str) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("Failed to open CHD: {}", e))?;
        let reader = BufReader::new(file);
        let chd = chd::Chd::open(reader, None).map_err(|e| format!("Failed to parse CHD: {}", e))?;
        let hunk_size = chd.header().hunk_size() as u64;
        let unit_bytes = chd.header().unit_bytes() as u64;
        let cached_hunk_buf = vec![0u8; hunk_size as usize];
        Ok(Self {
            chd,
            hunk_size,
            unit_bytes,
            units_per_hunk: hunk_size / unit_bytes,
            cd_mode: unit_bytes != LOGICAL_SECTOR,
            cached_hunk_index: None,
            cached_hunk_buf,
            compressed_scratch: Vec::new(),
        })
    }

    fn ensure_hunk_cached(&mut self, hunk_index: u32) -> Result<(), String> {
        if self.cached_hunk_index != Some(hunk_index) {
            let mut hunk = self.chd.hunk(hunk_index).map_err(|e| format!("CHD hunk {} out of range: {}", hunk_index, e))?;
            hunk.read_hunk_in(&mut self.compressed_scratch, &mut self.cached_hunk_buf)
                .map_err(|e| format!("CHD hunk {} decompression failed: {}", hunk_index, e))?;
            self.cached_hunk_index = Some(hunk_index);
        }
        Ok(())
    }
}

impl DiscSource for ChdSource {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), String> {
        if !self.cd_mode {
            let mut buf_pos = 0usize;
            let mut cur_offset = offset;
            while buf_pos < buf.len() {
                let hunk_index = (cur_offset / self.hunk_size) as u32;
                let hunk_local_offset = (cur_offset % self.hunk_size) as usize;
                self.ensure_hunk_cached(hunk_index)?;

                let available = self.hunk_size as usize - hunk_local_offset;
                let remaining = buf.len() - buf_pos;
                let to_copy = available.min(remaining);
                buf[buf_pos..buf_pos + to_copy]
                    .copy_from_slice(&self.cached_hunk_buf[hunk_local_offset..hunk_local_offset + to_copy]);
                buf_pos += to_copy;
                cur_offset += to_copy as u64;
            }
            return Ok(());
        }

        // CD-style: offset/buf are in logical (2048-byte-sector) space; each
        // logical sector corresponds 1:1 to one raw CD sector ("unit").
        let mut buf_pos = 0usize;
        let mut cur_offset = offset;
        while buf_pos < buf.len() {
            let logical_sector = cur_offset / LOGICAL_SECTOR;
            let sector_off = cur_offset % LOGICAL_SECTOR;

            let unit_index = logical_sector;
            let hunk_index = (unit_index / self.units_per_hunk) as u32;
            let unit_in_hunk = unit_index % self.units_per_hunk;
            self.ensure_hunk_cached(hunk_index)?;

            let unit_start = (unit_in_hunk * self.unit_bytes) as usize;
            let data_start = unit_start + (CD_SECTOR_DATA_OFFSET + sector_off) as usize;

            let available_in_sector = (LOGICAL_SECTOR - sector_off) as usize;
            let remaining = buf.len() - buf_pos;
            let to_copy = available_in_sector.min(remaining);
            buf[buf_pos..buf_pos + to_copy]
                .copy_from_slice(&self.cached_hunk_buf[data_start..data_start + to_copy]);

            buf_pos += to_copy;
            cur_offset += to_copy as u64;
        }
        Ok(())
    }
}
