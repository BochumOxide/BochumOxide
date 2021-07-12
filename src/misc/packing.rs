use anyhow::{bail, Result};

#[derive(Clone, Copy)]
pub enum Endian {
    Little,
    Big,
}

// pack 8-bit integer
pub fn pack8(v: u8) -> Vec<u8> {
    let wtr = vec![v];
    wtr
}

// unpack 8-bit integer
pub fn unpack8(vec: &[u8]) -> Result<u8> {
    if vec.len() != 1 {
        bail!("Wrong vector size!");
    }
    Ok(vec[0])
}

// pack 16-bit integer
pub fn pack16(v: u16, endian: Endian) -> [u8; 2] {
    let mut wtr = [0; 2];
    match endian {
        Endian::Little => {
            for i in 0..=1 {
                wtr[i] = (v >> i * 8) as u8;
            }
        }
        Endian::Big => {
            for i in 0..=1 {
                wtr[1 - i] = (v >> i * 8) as u8;
            }
        }
    };
    wtr
}

// unpack 16-bit integer
pub fn unpack16(vec: &[u8], endian: Endian) -> Result<u16> {
    if vec.len() > 2 {
        bail!("Wrong vector size!");
    }

    let mut result: u16 = 0;
    match endian {
        Endian::Little => {
            for i in 0..=1 {
                result |= (vec[i] as u16) << (i * 8);
            }
        }
        Endian::Big => {
            for i in 0..=1 {
                result |= (vec[1 - i] as u16) << (i * 8);
            }
        }
    };
    Ok(result)
}

// pack 32-bit integer
pub fn pack32(v: u32, endian: Endian) -> [u8; 4] {
    let mut wtr = [0; 4];
    match endian {
        Endian::Little => {
            for i in 0..=3 {
                wtr[i] = (v >> i * 8) as u8;
            }
        }
        Endian::Big => {
            for i in 0..=3 {
                wtr[3 - i] = (v >> i * 8) as u8;
            }
        }
    };
    wtr
}

// unpack 32-bit integer
pub fn unpack32(vec: &[u8], endian: Endian) -> Result<u32> {
    if vec.len() > 4 {
        bail!("Wrong vector size!");
    }

    let mut result: u32 = 0;
    match endian {
        Endian::Little => {
            for i in 0..=3 {
                result |= (vec[i] as u32) << (i * 8);
            }
        }
        Endian::Big => {
            for i in 0..=3 {
                result |= (vec[3 - i] as u32) << (i * 8);
            }
        }
    };
    Ok(result)
}

// pack 64-bit integer
pub fn pack64(v: u64, endian: Endian) -> [u8; 8] {
    let mut wtr = [0; 8];
    match endian {
        Endian::Little => {
            for i in 0..=7 {
                wtr[i] = (v >> i * 8) as u8;
            }
        }
        Endian::Big => {
            for i in 0..=7 {
                wtr[7 - i] = (v >> i * 8) as u8;
            }
        }
    };
    wtr
}

// unpack 64-bit integer
pub fn unpack64(vec: &[u8], endian: Endian) -> Result<u64> {
    if vec.len() > 8 {
        bail!("Wrong vector size!");
    }

    let mut result: u64 = 0;
    match endian {
        Endian::Little => {
            for i in 0..=7 {
                result |= (vec[i] as u64) << (i * 8);
            }
        }
        Endian::Big => {
            for i in 0..=7 {
                result |= (vec[7 - i] as u64) << (i * 8);
            }
        }
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack8() {
        assert_eq!(pack8(5), b"\x05");
    }
    #[test]
    fn test_unpack8() {
        assert_eq!(unpack8(b"\x05").unwrap(), 5);
    }

    #[test]
    #[should_panic]
    fn test_unpack8_panic() {
        unpack8(b"\x05\x04").unwrap(); // Err
    }

    #[test]
    fn test_pack16() {
        assert_eq!(&pack16(517, Endian::Little), b"\x05\x02");
        assert_eq!(&pack16(517, Endian::Big), b"\x02\x05");
    }

    #[test]
    fn test_unpack16() {
        assert_eq!(unpack16(b"\x05\x02", Endian::Little).unwrap(), 517);
        assert_eq!(unpack16(b"\x02\x05", Endian::Big).unwrap(), 517);
    }

    #[test]
    #[should_panic]
    fn test_unpack16_panic_little_endian() {
        unpack16(b"\x05\x02\x03", Endian::Little).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_unpack16_panic_big_endian() {
        unpack16(b"\x05\x02\x03", Endian::Big).unwrap();
    }

    #[test]
    fn test_pack32() {
        assert_eq!(&pack32(4294967281, Endian::Little), b"\xf1\xff\xff\xff");
        assert_eq!(&pack32(4294967281, Endian::Big), b"\xff\xff\xff\xf1")
    }

    #[test]
    fn test_unpack32() {
        assert_eq!(
            unpack32(b"\xf1\xff\xff\xff", Endian::Little).unwrap(),
            4294967281
        );
        assert_eq!(
            unpack32(b"\xff\xff\xff\xf1", Endian::Big).unwrap(),
            4294967281
        );
    }

    #[test]
    #[should_panic]
    fn test_unpack32_panic_little_endian() {
        unpack32(b"\xf1\xff\xff\xff\x11", Endian::Little).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_unpack32_panic_big_endian() {
        unpack32(b"\xff\xff\xff\xf1\x11", Endian::Big).unwrap();
    }

    #[test]
    fn test_pack64() {
        assert_eq!(
            &pack64(918733457491587, Endian::Little),
            b"\x83\x86\x60\x4d\x95\x43\x03\x00"
        );
        assert_eq!(
            &pack64(918733457491587, Endian::Big),
            b"\x00\x03\x43\x95\x4d\x60\x86\x83"
        );
    }

    #[test]
    fn test_unpack64() {
        assert_eq!(
            unpack64(b"\x83\x86\x60\x4d\x95\x43\x03\x00", Endian::Little).unwrap(),
            918733457491587
        );
        assert_eq!(
            unpack64(b"\x00\x03\x43\x95\x4d\x60\x86\x83", Endian::Big).unwrap(),
            918733457491587
        );
    }

    #[test]
    #[should_panic]
    fn test_unpack64_panic_little_endian() {
        unpack64(b"\x83\x86\x60\x4d\x95\x43\x03\x00\x11", Endian::Little).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_unpack64_panic_big_endian() {
        unpack64(b"\x00\x03\x43\x95\x4d\x60\x86\x83\x11", Endian::Big).unwrap();
    }
}
