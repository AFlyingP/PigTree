//! SHA-256 implementation using Windows BCrypt.

#[link(name = "bcrypt")]
extern "system" {
    fn BCryptOpenAlgorithmProvider(
        phAlgorithm: *mut usize,
        pszAlgId: *const u16,
        pszImplementation: *const u16,
        dwFlags: u32,
    ) -> i32;
    fn BCryptCloseAlgorithmProvider(hAlgorithm: usize, dwFlags: u32) -> i32;
    fn BCryptCreateHash(
        hAlgorithm: usize,
        phHash: *mut usize,
        pbHashObject: *mut u8,
        cbHashObject: u32,
        pbSecret: *const u8,
        cbSecret: u32,
        dwFlags: u32,
    ) -> i32;
    fn BCryptHashData(hHash: usize, pbInput: *const u8, cbInput: u32, dwFlags: u32) -> i32;
    fn BCryptFinishHash(hHash: usize, pbOutput: *mut u8, cbOutput: u32, dwFlags: u32) -> i32;
    fn BCryptDestroyHash(hHash: usize) -> i32;
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    unsafe {
        let mut alg = 0usize;
        let alg_id: Vec<u16> = "SHA256\0".encode_utf16().collect();
        let status = BCryptOpenAlgorithmProvider(&mut alg, alg_id.as_ptr(), std::ptr::null(), 0);
        if status != 0 {
            panic!("BCryptOpenAlgorithmProvider failed with status: {status:#x}");
        }

        let mut hash_handle = 0usize;
        let status = BCryptCreateHash(
            alg,
            &mut hash_handle,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            0,
            0,
        );
        if status != 0 {
            BCryptCloseAlgorithmProvider(alg, 0);
            panic!("BCryptCreateHash failed with status: {status:#x}");
        }

        if !data.is_empty() {
            let status = BCryptHashData(hash_handle, data.as_ptr(), data.len() as u32, 0);
            if status != 0 {
                BCryptDestroyHash(hash_handle);
                BCryptCloseAlgorithmProvider(alg, 0);
                panic!("BCryptHashData failed with status: {status:#x}");
            }
        }

        let mut out = [0u8; 32];
        let status = BCryptFinishHash(hash_handle, out.as_mut_ptr(), 32, 0);
        BCryptDestroyHash(hash_handle);
        BCryptCloseAlgorithmProvider(alg, 0);

        if status != 0 {
            panic!("BCryptFinishHash failed with status: {status:#x}");
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_known_vector() {
        let hash = sha256(b"hello");
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha256_empty() {
        let hash = sha256(b"");
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(
            hex,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
