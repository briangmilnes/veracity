#![no_main]

use libfuzzer_sys::fuzz_target;
use veracity::search::parse_pattern;

fuzz_target!(|data: &[u8]| {
    // Convert bytes to string (may fail for invalid UTF-8, that's fine)
    if let Ok(input) = std::str::from_utf8(data) {
        // Just try to parse - we're looking for panics, not errors
        let _ = parse_pattern(input);
    }
});
