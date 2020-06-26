#![no_main]
use libfuzzer_sys::fuzz_target;
use parts::ProtectiveMbr;

fuzz_target!(|data: &[u8]| {
    if let Some(data) = data.get(..512) {
        let _ = ProtectiveMbr::from_bytes(data);
    }
});
