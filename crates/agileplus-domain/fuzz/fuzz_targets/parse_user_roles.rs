// UserRole + UserStatus: role-based access control must not accept
// arbitrary strings and never panic.

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

use agileplus_domain::domain::user::{UserRole, UserStatus};

fuzz_target!(|data: &[u8]| {
    let s = std::str::from_utf8(data).unwrap_or("");
    let _ = UserRole::from_str(s);
    let _ = UserStatus::from_str(s);
});
