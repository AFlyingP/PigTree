//! Castagnoli CRC-32C implementation (RFC 3720 / polynomial 0x1EDC6F41).

const CRC32C_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let poly = 0x82F63B78u32; // Reversed polynomial for 0x1EDC6F41
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ poly;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Computes the CRC-32C checksum of the given byte slice using the Castagnoli polynomial.
pub fn compute_crc32c(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        let table_idx = ((crc ^ (byte as u32)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32C_TABLE[table_idx];
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_vectors() {
        assert_eq!(compute_crc32c(b"123456789"), 0xe3069283);
        assert_eq!(compute_crc32c(&[0u8; 32]), 0x8a9136aa);
        assert_eq!(compute_crc32c(&[0xffu8; 32]), 0x62a8ab43);
    }
}
