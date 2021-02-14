use byteorder::{BigEndian, ByteOrder, LittleEndian};
use std::convert::TryInto;
use std::io;
use std::io::Read;

pub trait ReadExtras {
    fn read_u8(&mut self) -> io::Result<u8>;
    fn read_u16(&mut self) -> io::Result<u16>;
    fn read_u32(&mut self) -> io::Result<u32>;
    fn read_u64(&mut self) -> io::Result<u64>;

    fn read_i32(&mut self) -> io::Result<i32>;
    fn read_i64(&mut self) -> io::Result<i64>;

    fn read_f32(&mut self) -> io::Result<f32>;

    fn read_u16_be(&mut self) -> io::Result<u16>;
    fn read_u32_be(&mut self) -> io::Result<u32>;
    fn read_u64_be(&mut self) -> io::Result<u64>;

    fn read_i32_be(&mut self) -> io::Result<i32>;
    fn read_i64_be(&mut self) -> io::Result<i64>;

    fn read_f32_be(&mut self) -> io::Result<f32>;

    fn read_var_u32(&mut self) -> io::Result<u32>;
    fn read_u8_vec(&mut self, size: usize) -> io::Result<Vec<u8>>;
    fn read_c_string(&mut self) -> io::Result<Vec<u8>>;
    fn read_fixed_c_string(&mut self, size: usize) -> std::io::Result<String>;
}

macro_rules! read_le_fn {
    ($name:ident, $read_name:ident, $t:ty) => {
        fn $name(&mut self) -> io::Result<$t> {
            let mut slice = [0u8; std::mem::size_of::<$t>()];
            self.read_exact(&mut slice)
                .map(|_| LittleEndian::$read_name(&slice))
        }
    };
}

macro_rules! read_be_fn {
    ($name:ident, $read_name:ident, $t:ty) => {
        fn $name(&mut self) -> io::Result<$t> {
            let mut slice = [0u8; std::mem::size_of::<$t>()];
            self.read_exact(&mut slice)
                .map(|_| BigEndian::$read_name(&slice))
        }
    };
}

impl<R: Read> ReadExtras for R {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut slice = [0u8; 1];
        self.read_exact(&mut slice).map(|_| slice[0])
    }
    read_le_fn!(read_u16, read_u16, u16);
    read_le_fn!(read_u32, read_u32, u32);
    read_le_fn!(read_u64, read_u64, u64);

    read_le_fn!(read_i32, read_i32, i32);
    read_le_fn!(read_i64, read_i64, i64);

    read_le_fn!(read_f32, read_f32, f32);

    read_be_fn!(read_u16_be, read_u16, u16);
    read_be_fn!(read_u32_be, read_u32, u32);
    read_be_fn!(read_u64_be, read_u64, u64);

    read_be_fn!(read_i32_be, read_i32, i32);
    read_be_fn!(read_i64_be, read_i64, i64);

    read_be_fn!(read_f32_be, read_f32, f32);

    fn read_var_u32(&mut self) -> io::Result<u32> {
        let mut res = 0;
        for byte in 0..=4 {
            let num = self.read_u8()?;
            res |= ((num as u32) & 0x7F) << (byte * 7);
            if num & 0b1000_0000 == 0 {
                return Ok(res);
            }
        }
        Ok(res)
    }
    fn read_u8_vec(&mut self, size: usize) -> io::Result<Vec<u8>> {
        let mut vec = vec![0; size];
        self.read_exact(&mut vec).map(|_| vec)
    }

    fn read_c_string(&mut self) -> io::Result<Vec<u8>> {
        let mut result = Vec::new();
        loop {
            let byte = self.read_u8()?;
            if byte == 0 {
                break;
            }
            result.push(byte);
        }
        Ok(result)
    }

    fn read_fixed_c_string(&mut self, size: usize) -> std::io::Result<String> {
        let buf = self.read_u8_vec(size)?;
        let mut buf_slice: &[u8] = &buf;
        let s = (&mut buf_slice).read_c_string()?;
        Ok(String::from_utf8_lossy(&s).to_string())
    }
}

pub struct BitReader<R: Read> {
    head: Option<(u8, u8)>,
    tail: R,
}

fn bitmask(bits: u8) -> u8 {
    (((1 << bits) as u32) - 1).try_into().unwrap()
}

impl<R: Read> Read for BitReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.head.is_none() || buf.is_empty() {
            return self.tail.read(buf);
        }
        self.ensure_head()?;

        let (bits, bits_remaining) = self.head.take().unwrap();
        let read_count = self.tail.read(buf)?;
        let last_byte = buf[buf.len() - 1];

        for i in (1..read_count).rev() {
            let hi = buf[i] << bits_remaining;
            let lo = buf[i - 1] >> (8 - bits_remaining);
            buf[i] = hi | lo;
        }

        self.head = Some((last_byte >> (8 - bits_remaining), bits_remaining));
        buf[0] = (buf[0] << bits_remaining) | bits;

        Ok(read_count)
    }
}

impl<R: Read> BitReader<R> {
    pub fn new(reader: R) -> BitReader<R> {
        BitReader {
            head: None,
            tail: reader,
        }
    }

    fn ensure_head(&mut self) -> io::Result<()> {
        if self.head.is_none() || self.head.unwrap().1 == 0 {
            let byte = &mut [0];
            self.read_exact(byte)?;
            self.head = Some((byte[0], 8));
        }
        Ok(())
    }

    pub fn read_bits_u32(&mut self, mut count: u8) -> io::Result<u32> {
        let mut result: u32 = 0;
        let mut total_shift = 0;
        while count > 0 {
            self.ensure_head()?;
            let (byte, bits_remaining): &mut (u8, u8) = self.head.as_mut().unwrap();
            let shift = u8::min(count as u8, *bits_remaining);

            let byte_part = *byte & bitmask(shift);

            result |= (byte_part as u32) << total_shift;
            total_shift += shift;
            count -= shift;
            *bits_remaining -= shift;
            if *bits_remaining == 0 {
                self.head = None;
            } else {
                *byte >>= shift;
            }
        }

        Ok(result)
    }

    pub fn read_bit(&mut self) -> io::Result<bool> {
        Ok(self.read_bits_u32(1)? == 1)
    }

    #[allow(dead_code)]
    pub fn flush_bits(&mut self) -> Option<u8> {
        Some(self.head.take()?.0)
    }
}

pub fn string_from_nilslice(s: &[u8]) -> String {
    let data = s.iter()
        .copied()
        .take_while(|c| *c != 0)
        .collect::<Vec<u8>>();
    String::from_utf8_lossy(&data).to_string()
}


#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn read_one_byte_bits() {
        let data = &[0b1001_0110u8];
        let mut r = data.as_ref();
        let mut reader = BitReader::new(&mut r);
        assert_eq!(0, reader.read_bits_u32(1).unwrap());
        assert_eq!(0b1011, reader.read_bits_u32(4).unwrap());
        assert_eq!(0b100, reader.read_bits_u32(3).unwrap());
        assert!(reader.read_bits_u32(1).is_err());
    }

    #[test]
    fn read_multibyte_bits() {
        let data: &[u8] = &[0b1001_0110, 0b0101_0101, 0b1000_1111];
        let mut r = data.as_ref();
        let mut reader = BitReader::new(&mut r);
        assert_eq!(0b0, reader.read_bits_u32(1).unwrap());
        assert_eq!(0b1011, reader.read_bits_u32(4).unwrap());
        assert_eq!(0b0_1100, reader.read_bits_u32(5).unwrap());
        assert_eq!(0b00_1111_0101_01, reader.read_bits_u32(12).unwrap());
        assert!(reader.read_bits_u32(3).is_err());
    }

    #[test]
    fn read_bits_and_byte() {
        let data: &[u8] = &[0b1001_0110, 0b0101_0101];
        let mut r = data.as_ref();
        let mut reader = BitReader::new(&mut r);
        assert_eq!(0b1001_0110, reader.read_bits_u32(8).unwrap());
        let buf = &mut [0];
        reader.read_exact(buf).unwrap();
        assert_eq!(0b0101_0101, buf[0]);
    }

    #[test]
    #[allow(unused_must_use)]
    fn unaligned_read() {
        let data: &[u8] = &[0b1001_0110, 0b0101_0101];
        let mut r = data.as_ref();
        let mut reader = BitReader::new(&mut r);
        assert_eq!(0b0110, reader.read_bits_u32(4).unwrap());
        let buf = &mut [0];
        reader.read_exact(buf).unwrap();
        assert_eq!(buf[0], 0b0101_1001);
        assert_eq!(0b0101, reader.flush_bits().unwrap());
    }

    #[test]
    fn flush_bits() {
        let data: &[u8] = &[0b1001_0110, 0b0101_0101];
        let mut r = data.as_ref();
        let mut reader = BitReader::new(&mut r);
        assert_eq!(0b0110, reader.read_bits_u32(4).unwrap());
        assert_eq!(0b1001, reader.flush_bits().unwrap());
        let buf = &mut [0];
        reader.read_exact(buf).unwrap();
        assert_eq!(0b0101_0101, buf[0]);
    }
}
