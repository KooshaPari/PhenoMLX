// WpState + PrState: work-package + PR lifecycle states used in
// the traceability bridge. These types currently lack FromStr / Display
// impls, so we exercise serde_json deserialize to ensure malformed
// input never panics.

#![no_main]
use std::str::FromStr;
use libfuzzer_sys::fuzz_target;

use agileplus_domain::domain::work_package::{PrState, WpState};

fuzz_target!(|data: &[u8]| {
    let _ = serde_json::from_slice::<WpState>(data);
    let _ = serde_json::from_slice::<PrState>(data);
});
