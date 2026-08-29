//! Security descriptors, SID extraction, CSPRNG nonces, and process identity verification.

use crate::error::IpcError;
use crate::win32::*;
use pigtree_protocol::sha256;
use std::ffi::c_void;
use std::ptr::null_mut;

pub struct SecurityDescriptorGuard {
    sd: *mut c_void,
}

impl SecurityDescriptorGuard {
    pub fn as_ptr(&self) -> *mut c_void {
        self.sd
    }
}

impl Drop for SecurityDescriptorGuard {
    fn drop(&mut self) {
        if !self.sd.is_null() {
            unsafe {
                LocalFree(self.sd);
            }
        }
    }
}

/// Generates a cryptographically secure 256-bit (32 bytes) random nonce.
pub fn generate_nonce() -> [u8; 32] {
    let mut nonce = [0u8; 32];
    let status =
        unsafe { BCryptGenRandom(0, nonce.as_mut_ptr(), 32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status != 0 {
        panic!("BCryptGenRandom failed with status: {status:#x}");
    }
    nonce
}

/// Constant-time slice comparison helper to mitigate timing side-channel attacks on secret tokens/keys.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Constant-time comparison for fixed 32-byte nonces and keys.
pub fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Derives an ephemeral channel key hash from bootstrap nonce, client nonce, and server nonce.
pub fn derive_channel_key(
    bootstrap_nonce: &[u8],
    client_nonce: &[u8],
    server_nonce: &[u8],
) -> [u8; 32] {
    let mut data =
        Vec::with_capacity(32 + bootstrap_nonce.len() + client_nonce.len() + server_nonce.len());
    data.extend_from_slice(b"pigtree-v1-channel-key:");
    data.extend_from_slice(bootstrap_nonce);
    data.extend_from_slice(b":");
    data.extend_from_slice(client_nonce);
    data.extend_from_slice(b":");
    data.extend_from_slice(server_nonce);
    sha256(&data)
}

/// Gets the string SID of the current user.
pub fn get_current_user_sid() -> Result<String, IpcError> {
    unsafe {
        let mut h_token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut h_token) == 0 {
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "OpenProcessToken failed".to_string(),
            });
        }

        let mut return_len: DWORD = 0;
        GetTokenInformation(h_token, TOKEN_USER, null_mut(), 0, &mut return_len);
        if return_len == 0 {
            CloseHandle(h_token);
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "GetTokenInformation length query failed for TOKEN_USER".to_string(),
            });
        }

        let mut buffer = vec![0u8; return_len as usize];
        if GetTokenInformation(
            h_token,
            TOKEN_USER,
            buffer.as_mut_ptr() as *mut c_void,
            return_len,
            &mut return_len,
        ) == 0
        {
            CloseHandle(h_token);
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "GetTokenInformation failed for TOKEN_USER".to_string(),
            });
        }
        CloseHandle(h_token);

        let token_user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut str_sid: LPWSTR = null_mut();
        if ConvertSidToStringSidW(token_user.User.Sid, &mut str_sid) == 0 {
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "ConvertSidToStringSidW failed".to_string(),
            });
        }

        let mut len = 0usize;
        while *str_sid.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(str_sid, len);
        let sid_str = String::from_utf16_lossy(slice);
        LocalFree(str_sid as *mut c_void);

        Ok(sid_str)
    }
}

/// Gets the list of restricted string SIDs for the current process token, if any.
/// Returns an empty vector if the token is unrestricted or has no restricted SIDs.
pub fn get_current_token_restricted_sids() -> Result<Vec<String>, IpcError> {
    unsafe {
        let mut h_token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut h_token) == 0 {
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "OpenProcessToken failed".to_string(),
            });
        }

        let mut return_len: DWORD = 0;
        GetTokenInformation(
            h_token,
            TOKEN_RESTRICTED_SIDS,
            null_mut(),
            0,
            &mut return_len,
        );
        if return_len == 0 {
            CloseHandle(h_token);
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "GetTokenInformation length query failed for TOKEN_RESTRICTED_SIDS"
                    .to_string(),
            });
        }

        let mut buffer = vec![0u8; return_len as usize];
        if GetTokenInformation(
            h_token,
            TOKEN_RESTRICTED_SIDS,
            buffer.as_mut_ptr() as *mut c_void,
            return_len,
            &mut return_len,
        ) == 0
        {
            CloseHandle(h_token);
            return Err(IpcError::Win32 {
                code: GetLastError(),
                message: "GetTokenInformation failed for TOKEN_RESTRICTED_SIDS".to_string(),
            });
        }
        CloseHandle(h_token);

        let token_groups = &*(buffer.as_ptr() as *const TOKEN_GROUPS);
        let group_count = token_groups.GroupCount as usize;
        if group_count == 0 {
            return Ok(Vec::new());
        }

        let groups_ptr = &token_groups.Groups[0] as *const SID_AND_ATTRIBUTES;
        let mut restricted_sids = Vec::with_capacity(group_count);

        for i in 0..group_count {
            let sid_entry = &*groups_ptr.add(i);
            let mut str_sid: LPWSTR = null_mut();
            if ConvertSidToStringSidW(sid_entry.Sid, &mut str_sid) == 0 {
                return Err(IpcError::Win32 {
                    code: GetLastError(),
                    message: format!("ConvertSidToStringSidW failed for restricted SID index {i}"),
                });
            }

            let mut len = 0usize;
            while *str_sid.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(str_sid, len);
            let sid_str = String::from_utf16_lossy(slice);
            LocalFree(str_sid as *mut c_void);

            restricted_sids.push(sid_str);
        }

        restricted_sids.sort();
        restricted_sids.dedup();
        Ok(restricted_sids)
    }
}

/// Returns true if a SID string represents a broad well-known authority/group
/// (such as Everyone/World, Authenticated Users, Builtin Users, Network, etc.)
/// that must NEVER be granted access in private IPC DACLs.
pub fn is_broad_sid(sid: &str) -> bool {
    let s = sid.trim();
    if s.is_empty() {
        return true;
    }
    // Check SDDL abbreviations
    if matches!(
        s,
        "WD" | "AU" | "BU" | "BA" | "BG" | "NU" | "IU" | "RC" | "AN" | "ED"
    ) {
        return true;
    }
    // Check known broad string SIDs:
    // S-1-1-0: Everyone / World (WD)
    // S-1-5-11: Authenticated Users (AU)
    // S-1-5-32-545: Builtin Users (BU)
    // S-1-5-32-544: Builtin Administrators (BA)
    // S-1-5-32-546: Builtin Guests (BG)
    // S-1-5-7: Anonymous (AN)
    // S-1-5-2: Network (NU)
    // S-1-5-4: Interactive (IU)
    // S-1-5-15: This Organization
    if matches!(
        s,
        "S-1-1-0"
            | "S-1-5-11"
            | "S-1-5-32-545"
            | "S-1-5-32-544"
            | "S-1-5-32-546"
            | "S-1-5-7"
            | "S-1-5-2"
            | "S-1-5-4"
            | "S-1-5-15"
    ) {
        return true;
    }
    false
}

/// Builds the explicit Named Pipe SDDL string.
///
/// In standard execution, only the current user SID is granted full access:
/// `D:(A;;GA;;;<USER_SID>)S:(ML;;NW;;;ME)`
///
/// Under a Windows restricted token (such as within sandboxed runners or AppContainers),
/// kernel access checks require that the DACL satisfy BOTH the normal token check
/// (matching the user SID) and the restricted SID check (matching at least one restricted SID).
/// If restricted SIDs are present on the token, this dynamically includes exact non-broad
/// restricted SIDs:
/// `D:(A;;GA;;;<USER_SID>)(A;;GA;;;<RESTRICTED_SID_1>)...S:(ML;;NW;;;ME)`
///
/// Under no circumstances are broad groups (WD/Everyone, Authenticated Users, etc.) granted.
pub fn build_pipe_sddl(user_sid: &str, restricted_sids: &[String]) -> String {
    let mut dacl_entries = format!("(A;;GA;;;{user_sid})");

    for rsid in restricted_sids {
        let trimmed = rsid.trim();
        if trimmed.is_empty() || trimmed == user_sid || is_broad_sid(trimmed) {
            continue;
        }
        dacl_entries.push_str(&format!("(A;;GA;;;{trimmed})"));
    }

    format!("D:{dacl_entries}S:(ML;;NW;;;ME)")
}

/// Creates Named Pipe SECURITY_ATTRIBUTES applying explicit SDDL:
/// D:(A;;GA;;;<CURRENT_USER_SID>)[(A;;GA;;;<EXACT_RESTRICTED_SID>)...]S:(ML;;NW;;;ME)
pub fn create_pipe_security_attributes(
) -> Result<(SecurityDescriptorGuard, SECURITY_ATTRIBUTES), IpcError> {
    let user_sid = get_current_user_sid()?;
    let restricted_sids = get_current_token_restricted_sids()?;
    let sddl = build_pipe_sddl(&user_sid, &restricted_sids);

    let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
    let mut p_sd: *mut c_void = null_mut();
    let mut sd_size: ULONG = 0;

    let res = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide_sddl.as_ptr(),
            1, // SDDL_REVISION_1
            &mut p_sd,
            &mut sd_size,
        )
    };

    if res == 0 || p_sd.is_null() {
        return Err(IpcError::Win32 {
            code: unsafe { GetLastError() },
            message: format!(
                "ConvertStringSecurityDescriptorToSecurityDescriptorW failed for SDDL: {sddl}"
            ),
        });
    }

    let guard = SecurityDescriptorGuard { sd: p_sd };
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
        lpSecurityDescriptor: p_sd,
        bInheritHandle: FALSE,
    };

    Ok((guard, sa))
}

/// Retrieves the creation timestamp of a process.
///
/// # Safety
/// The caller must ensure `h_process` is a valid Win32 process handle.
pub unsafe fn get_process_creation_time(h_process: HANDLE) -> Result<u64, IpcError> {
    let mut creation = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exit = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut user = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };

    if GetProcessTimes(h_process, &mut creation, &mut exit, &mut kernel, &mut user) == 0 {
        return Err(IpcError::Win32 {
            code: GetLastError(),
            message: "GetProcessTimes failed".to_string(),
        });
    }

    Ok(creation.to_u64())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        let n1 = [42u8; 32];
        let mut n2 = [42u8; 32];
        assert!(constant_time_eq(&n1, &n2));
        assert!(constant_time_eq_32(&n1, &n2));

        n2[31] = 43;
        assert!(!constant_time_eq(&n1, &n2));
        assert!(!constant_time_eq_32(&n1, &n2));

        assert!(!constant_time_eq(&n1[..31], &n2));
    }

    #[test]
    fn test_generate_nonce_randomness() {
        let n1 = generate_nonce();
        let n2 = generate_nonce();
        assert_ne!(n1, n2);
        assert_eq!(n1.len(), 32);
    }

    #[test]
    fn test_derive_channel_key() {
        let b = [1u8; 32];
        let c = [2u8; 32];
        let s = [3u8; 32];
        let k1 = derive_channel_key(&b, &c, &s);
        let k2 = derive_channel_key(&b, &c, &s);
        assert_eq!(k1, k2);
        assert_ne!(k1, [0u8; 32]);
    }

    #[test]
    fn test_get_current_user_sid() {
        let sid = get_current_user_sid().expect("should retrieve current user SID");
        assert!(sid.starts_with("S-1-5-"));
    }

    #[test]
    fn test_get_current_token_restricted_sids() {
        let r_sids = get_current_token_restricted_sids().expect("should retrieve restricted SIDs");
        println!("Restricted SIDs count: {}", r_sids.len());
        for (i, sid) in r_sids.iter().enumerate() {
            println!("  [{i}]: {sid}");
        }
    }

    #[test]
    fn test_create_pipe_security_attributes() {
        let (_guard, sa) = create_pipe_security_attributes().expect("should create SA");
        assert!(!sa.lpSecurityDescriptor.is_null());
        assert_eq!(sa.bInheritHandle, FALSE);
    }

    #[test]
    fn test_build_pipe_sddl_unrestricted() {
        let user_sid = "S-1-5-21-123456789-123456789-123456789-1001";
        let restricted = vec![];
        let sddl = build_pipe_sddl(user_sid, &restricted);

        assert_eq!(
            sddl,
            "D:(A;;GA;;;S-1-5-21-123456789-123456789-123456789-1001)S:(ML;;NW;;;ME)"
        );
        assert!(sddl.contains(user_sid));
        assert!(sddl.ends_with("S:(ML;;NW;;;ME)"));
        assert!(!sddl.contains("WD"));
        assert!(!sddl.contains("AU"));
        assert!(!sddl.contains("S-1-1-0"));
        assert!(!sddl.contains("S-1-5-11"));
    }

    #[test]
    fn test_build_pipe_sddl_filters_broad_and_duplicate_sids() {
        let user_sid = "S-1-5-21-1111-2222-3333-1001";
        let restricted = vec![
            "WD".to_string(),
            "S-1-1-0".to_string(),
            "AU".to_string(),
            "S-1-5-11".to_string(),
            "BU".to_string(),
            "S-1-5-32-545".to_string(),
            "BA".to_string(),
            "S-1-5-32-544".to_string(),
            "BG".to_string(),
            "S-1-5-32-546".to_string(),
            "NU".to_string(),
            "S-1-5-2".to_string(),
            "IU".to_string(),
            "S-1-5-4".to_string(),
            "AN".to_string(),
            "S-1-5-7".to_string(),
            "S-1-5-15".to_string(),
            "".to_string(),
            "   ".to_string(),
            user_sid.to_string(), // duplicate user SID
            "S-1-4-116092966-951971648-1".to_string(),
            "S-1-5-5-0-515154".to_string(),
        ];

        let sddl = build_pipe_sddl(user_sid, &restricted);

        // Must grant user SID and exact specific restricted SIDs only
        assert_eq!(
            sddl,
            "D:(A;;GA;;;S-1-5-21-1111-2222-3333-1001)(A;;GA;;;S-1-4-116092966-951971648-1)(A;;GA;;;S-1-5-5-0-515154)S:(ML;;NW;;;ME)"
        );

        // Verify no broad groups are granted
        assert!(!sddl.contains(";;;WD)"));
        assert!(!sddl.contains(";;;AU)"));
        assert!(!sddl.contains(";;;BU)"));
        assert!(!sddl.contains(";;;BA)"));
        assert!(!sddl.contains(";;;BG)"));
        assert!(!sddl.contains(";;;NU)"));
        assert!(!sddl.contains(";;;IU)"));
        assert!(!sddl.contains(";;;AN)"));
        assert!(!sddl.contains(";;;S-1-1-0)"));
        assert!(!sddl.contains(";;;S-1-5-11)"));
        assert!(!sddl.contains(";;;S-1-5-32-545)"));
        assert!(!sddl.contains(";;;S-1-5-32-544)"));
        assert!(!sddl.contains(";;;S-1-5-32-546)"));
        assert!(!sddl.contains(";;;S-1-5-7)"));
        assert!(!sddl.contains(";;;S-1-5-2)"));
        assert!(!sddl.contains(";;;S-1-5-4)"));
        assert!(!sddl.contains(";;;S-1-5-15)"));

        // Verify user SID is not duplicated
        let user_matches: Vec<_> = sddl.match_indices(user_sid).collect();
        assert_eq!(user_matches.len(), 1);
    }

    #[test]
    fn test_live_pipe_sddl_security_properties() {
        let user_sid = get_current_user_sid().expect("get user SID");
        let restricted_sids = get_current_token_restricted_sids().expect("get restricted SIDs");
        let sddl = build_pipe_sddl(&user_sid, &restricted_sids);

        // Starts with DACL and contains SACL with medium integrity no-write-up
        assert!(sddl.starts_with("D:"));
        assert!(sddl.ends_with("S:(ML;;NW;;;ME)"));

        // Must grant user SID
        assert!(
            sddl.contains(&format!("(A;;GA;;;{user_sid})")),
            "Generated SDDL must grant the current user SID"
        );

        // Must never grant broad groups
        assert!(!sddl.contains(";;;WD)"), "Must not grant WD");
        assert!(!sddl.contains(";;;AU)"), "Must not grant AU");
        assert!(!sddl.contains(";;;BU)"), "Must not grant BU");
        assert!(!sddl.contains(";;;BA)"), "Must not grant BA");
        assert!(!sddl.contains(";;;BG)"), "Must not grant BG");
        assert!(!sddl.contains(";;;NU)"), "Must not grant NU");
        assert!(!sddl.contains(";;;IU)"), "Must not grant IU");
        assert!(!sddl.contains(";;;AN)"), "Must not grant AN");
        assert!(
            !sddl.contains(";;;S-1-1-0)"),
            "Must not grant S-1-1-0 (Everyone)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-11)"),
            "Must not grant S-1-5-11 (Authenticated Users)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-32-545)"),
            "Must not grant S-1-5-32-545 (Users)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-32-544)"),
            "Must not grant S-1-5-32-544 (Administrators)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-32-546)"),
            "Must not grant S-1-5-32-546 (Guests)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-7)"),
            "Must not grant S-1-5-7 (Anonymous)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-2)"),
            "Must not grant S-1-5-2 (Network)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-4)"),
            "Must not grant S-1-5-4 (Interactive)"
        );
        assert!(
            !sddl.contains(";;;S-1-5-15)"),
            "Must not grant S-1-5-15 (This Org)"
        );

        // Security descriptor conversion must succeed
        let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut p_sd: *mut c_void = null_mut();
        let mut sd_size: ULONG = 0;
        let res = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide_sddl.as_ptr(),
                1,
                &mut p_sd,
                &mut sd_size,
            )
        };
        assert_ne!(res, 0, "SDDL must be valid Win32 SDDL");
        assert!(!p_sd.is_null());
        unsafe {
            LocalFree(p_sd);
        }
    }
}
