/// A minimal ISO9660 reader sufficient to boot a PS2 disc image: find and
/// read `SYSTEM.CNF` from the root directory, parse its `BOOT2=` entry, and
/// extract the referenced boot ELF's raw bytes.
///
/// Scope: standard 2048-byte-per-sector .iso images only (no raw 2352-byte
/// CD sector formats like some .bin/.cue rips, no Joliet/Rock Ridge - PS2
/// discs use plain ISO9660 for SYSTEM.CNF and the boot executable, so this
/// is sufficient for that purpose).
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const SECTOR_SIZE: u64 = 2048;
const PVD_SECTOR: u64 = 16;

/// A random-access byte source backing an `Iso9660` reader: either a raw
/// `.iso` file, or a CHD-compressed image (see `chd_source::ChdSource`).
pub trait DiscSource {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), String>;
}

impl DiscSource for File {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), String> {
        self.seek(SeekFrom::Start(offset)).map_err(|e| format!("ISO seek failed: {}", e))?;
        self.read_exact(buf).map_err(|e| format!("ISO read failed: {}", e))
    }
}

pub struct Iso9660 {
    source: Box<dyn DiscSource>,
}

struct DirEntry {
    name: String,
    lba: u32,
    size: u32,
    is_dir: bool,
}

impl Iso9660 {
    /// Opens a disc image, auto-detecting CHD-compressed images (by their
    /// `MComprHD` magic) versus plain raw `.iso` files.
    pub fn open(path: &str) -> Result<Self, String> {
        let mut probe = File::open(path).map_err(|e| format!("Failed to open disc image: {}", e))?;
        let mut magic = [0u8; 8];
        let is_chd = probe.read_exact(&mut magic).is_ok() && &magic == b"MComprHD";

        let source: Box<dyn DiscSource> = if is_chd {
            Box::new(crate::chd_source::ChdSource::open(path)?)
        } else {
            Box::new(File::open(path).map_err(|e| format!("Failed to open disc image: {}", e))?)
        };
        Ok(Self { source })
    }

    fn read_sector(&mut self, lba: u32) -> Result<[u8; SECTOR_SIZE as usize], String> {
        let mut buf = [0u8; SECTOR_SIZE as usize];
        self.source.read_at(lba as u64 * SECTOR_SIZE, &mut buf)?;
        Ok(buf)
    }

    fn root_dir(&mut self) -> Result<DirEntry, String> {
        let pvd = self.read_sector(PVD_SECTOR as u32)?;
        if &pvd[1..6] != b"CD001" {
            return Err("Not a valid ISO9660 image (missing CD001 signature)".to_string());
        }
        let record = &pvd[156..156 + 34];
        let (entry, _) = parse_dir_record(record).ok_or("Failed to parse root directory record")?;
        Ok(entry)
    }

    fn list_dir(&mut self, lba: u32, size: u32) -> Result<Vec<DirEntry>, String> {
        let sectors = (size as u64 + SECTOR_SIZE - 1) / SECTOR_SIZE;
        let mut entries = Vec::new();
        for s in 0..sectors {
            let sector = self.read_sector(lba + s as u32)?;
            let mut pos = 0usize;
            while pos < sector.len() {
                if sector[pos] == 0 {
                    break; // rest of this sector is padding
                }
                match parse_dir_record(&sector[pos..]) {
                    Some((entry, len)) => {
                        // Skip the "." and ".." self/parent entries (name is a single 0x00/0x01 byte)
                        if entry.name.as_bytes() != [0u8] && entry.name.as_bytes() != [1u8] {
                            entries.push(entry);
                        }
                        pos += len;
                    }
                    None => break,
                }
            }
        }
        Ok(entries)
    }

    /// Resolves a "\"- or "/"-separated path (with or without ";N" version
    /// suffixes) starting from the root directory.
    fn find_entry(&mut self, path: &str) -> Result<DirEntry, String> {
        let components: Vec<&str> = path
            .split(|c| c == '\\' || c == '/')
            .filter(|c| !c.is_empty())
            .collect();
        if components.is_empty() {
            return Err("Empty path".to_string());
        }

        let mut current = self.root_dir()?;
        for (i, component) in components.iter().enumerate() {
            let is_last = i == components.len() - 1;
            let entries = self.list_dir(current.lba, current.size)?;
            let target = strip_version(component).to_uppercase();
            let found = entries
                .into_iter()
                .find(|e| strip_version(&e.name).to_uppercase() == target)
                .ok_or_else(|| format!("Path component '{}' not found on ISO", component))?;
            if !is_last && !found.is_dir {
                return Err(format!("'{}' is not a directory", component));
            }
            current = found;
        }
        Ok(current)
    }

    /// Reads a file's full contents given its path on the disc.
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, String> {
        let entry = self.find_entry(path)?;
        if entry.is_dir {
            return Err(format!("'{}' is a directory, not a file", path));
        }
        let sectors = (entry.size as u64 + SECTOR_SIZE - 1) / SECTOR_SIZE;
        let mut data = Vec::with_capacity(entry.size as usize);
        for s in 0..sectors {
            let sector = self.read_sector(entry.lba + s as u32)?;
            data.extend_from_slice(&sector);
        }
        data.truncate(entry.size as usize);
        Ok(data)
    }
}

fn parse_dir_record(data: &[u8]) -> Option<(DirEntry, usize)> {
    if data.is_empty() {
        return None;
    }
    let len = data[0] as usize;
    if len == 0 || data.len() < len || len < 33 {
        return None;
    }
    let lba = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
    let size = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);
    let flags = data[25];
    let is_dir = (flags & 0x02) != 0;
    let name_len = data[32] as usize;
    if 33 + name_len > data.len() {
        return None;
    }
    let name = String::from_utf8_lossy(&data[33..33 + name_len]).to_string();
    Some((DirEntry { name, lba, size, is_dir }, len))
}

fn strip_version(name: &str) -> &str {
    match name.find(';') {
        Some(idx) => &name[..idx],
        None => name,
    }
}

/// Parses a `SYSTEM.CNF` file and returns the path referenced by its `BOOT2`
/// entry (e.g. `BOOT2 = cdrom0:\SLUS_200.36;1` -> `SLUS_200.36;1`).
pub fn parse_boot_path(system_cnf: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(system_cnf);
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("BOOT2") {
            let rest = rest.trim_start_matches([' ', '\t', '=']).trim();
            let rest = rest.strip_prefix("cdrom0:").or_else(|| rest.strip_prefix("cdrom:")).unwrap_or(rest);
            let rest = rest.trim_start_matches(['\\', '/']);
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_boot_path() {
        let cnf = b"BOOT2 = cdrom0:\\SLUS_200.36;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n";
        assert_eq!(parse_boot_path(cnf), Some("SLUS_200.36;1".to_string()));
    }

    #[test]
    fn test_strip_version() {
        assert_eq!(strip_version("SLUS_200.36;1"), "SLUS_200.36");
        assert_eq!(strip_version("SYSTEM.CNF"), "SYSTEM.CNF");
    }

    fn build_dir_record(name: &str, lba: u32, size: u32, is_dir: bool) -> Vec<u8> {
        let name_bytes = name.as_bytes();
        let len = 33 + name_bytes.len();
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[6..10].copy_from_slice(&lba.to_be_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[14..18].copy_from_slice(&size.to_be_bytes());
        r[25] = if is_dir { 0x02 } else { 0x00 };
        r[28..30].copy_from_slice(&1u16.to_le_bytes());
        r[30..32].copy_from_slice(&1u16.to_be_bytes());
        r[32] = name_bytes.len() as u8;
        r[33..33 + name_bytes.len()].copy_from_slice(name_bytes);
        r
    }

    /// Builds a minimal but structurally real ISO9660 image (PVD + root
    /// directory + two files) at `path`, to exercise the actual sector/
    /// directory-record parsing logic rather than just the SYSTEM.CNF parser.
    fn build_synthetic_iso(path: &std::path::Path, elf_contents: &[u8]) {
        use std::io::Write;
        const ROOT_DIR_LBA: u32 = 17;
        const CNF_LBA: u32 = 18;
        const ELF_LBA: u32 = 19;

        let cnf_contents = b"BOOT2 = cdrom0:\\TEST.ELF;1\r\nVER = 1.00\r\n".to_vec();

        let mut root_dir_data = vec![0u8; SECTOR_SIZE as usize];
        let self_entry = build_dir_record("\0", ROOT_DIR_LBA, SECTOR_SIZE as u32, true);
        let parent_entry = build_dir_record("\u{1}", ROOT_DIR_LBA, SECTOR_SIZE as u32, true);
        let cnf_entry = build_dir_record("SYSTEM.CNF;1", CNF_LBA, cnf_contents.len() as u32, false);
        let elf_entry = build_dir_record("TEST.ELF;1", ELF_LBA, elf_contents.len() as u32, false);
        let mut pos = 0;
        for rec in [&self_entry, &parent_entry, &cnf_entry, &elf_entry] {
            root_dir_data[pos..pos + rec.len()].copy_from_slice(rec);
            pos += rec.len();
        }

        let mut pvd = vec![0u8; SECTOR_SIZE as usize];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        let root_record = build_dir_record("\0", ROOT_DIR_LBA, SECTOR_SIZE as u32, true);
        pvd[156..156 + root_record.len()].copy_from_slice(&root_record);

        let mut file = File::create(path).unwrap();
        // Sectors 0-15: unused system area
        file.write_all(&vec![0u8; PVD_SECTOR as usize * SECTOR_SIZE as usize]).unwrap();
        file.write_all(&pvd).unwrap(); // sector 16
        file.write_all(&root_dir_data).unwrap(); // sector 17
        let mut cnf_sector = cnf_contents.clone();
        cnf_sector.resize(SECTOR_SIZE as usize, 0);
        file.write_all(&cnf_sector).unwrap(); // sector 18
        let mut elf_sector = elf_contents.to_vec();
        elf_sector.resize(SECTOR_SIZE as usize, 0);
        file.write_all(&elf_sector).unwrap(); // sector 19
    }

    #[test]
    fn test_synthetic_iso_boot_extraction() {
        let path = std::env::temp_dir().join("emotionx_test_synthetic.iso");
        let fake_elf = b"NOT_A_REAL_ELF_JUST_TEST_BYTES_1234567890";
        build_synthetic_iso(&path, fake_elf);

        let mut iso = Iso9660::open(path.to_str().unwrap()).unwrap();
        let cnf = iso.read_file("SYSTEM.CNF").unwrap();
        let boot_path = parse_boot_path(&cnf).unwrap();
        assert_eq!(boot_path, "TEST.ELF;1");

        let elf_data = iso.read_file(&boot_path).unwrap();
        assert_eq!(elf_data, fake_elf);

        let _ = std::fs::remove_file(&path);
    }
}
