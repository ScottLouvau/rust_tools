use std::time::Duration;

static SIZE_SCALES: [&str; 5] = ["bytes", "KiB", "MiB", "GiB", "TiB"];

pub fn to_friendly_size(byte_count: u64) -> String {
    let mut size = byte_count as f64;
    let mut scale = 0;

    while size >= 1024f64 && scale + 1 < SIZE_SCALES.len() {
        size /= 1024f64;
        scale += 1;        
    }

    let scale = SIZE_SCALES[scale];
    if size == 0f64 || size >= 100f64 { 
        return format!("{size:.0} {scale}");
    } else if size >= 10f64 {
        return format!("{size:.1} {scale}");
    } else {
        return format!("{size:.2} {scale}");
    }
}

pub fn to_friendly_duration(elapsed: Duration) -> String {
    let elapsed_seconds = elapsed.as_secs_f64();

    if elapsed_seconds < 0.01f64 {
        let elapsed_milliseconds = elapsed_seconds * 1000f64;
        return format!("{elapsed_milliseconds:.3} ms");
    } else if elapsed_seconds < 1f64 {
        let elapsed_milliseconds = elapsed_seconds * 1000f64;
        return format!("{elapsed_milliseconds:.0} ms");
    } else if elapsed_seconds < 10f64 {
        return format!("{elapsed_seconds:.1} sec");
    } else if elapsed_seconds < 120f64 {
        return format!("{elapsed_seconds:.0} sec");
    }

    let elapsed_minutes = elapsed_seconds / 60f64;
    if elapsed_minutes < 10f64 {
        return format!("{elapsed_minutes:.1} min");
    } else if elapsed_minutes < 120f64 {
        return format!("{elapsed_minutes:.0} min");
    }

    let elapsed_hours = elapsed_minutes / 60f64;
    if elapsed_hours < 10f64 {
        return format!("{elapsed_hours:.1} hours");
    } else if elapsed_hours < 48f64 {
        return format!("{elapsed_hours:.0} hours");
    }

    let elapsed_days = elapsed_hours / 24f64;
    return format!("{elapsed_days:.0} days");

}

 #[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sizes() {
        assert_eq!(to_friendly_size(946), "946 bytes");
        assert_eq!(to_friendly_size(1023), "1023 bytes");
        assert_eq!(to_friendly_size(1024), "1.00 KiB");
        assert_eq!(to_friendly_size(2 * 1024 + 512), "2.50 KiB");
        assert_eq!(to_friendly_size(16 * 1024), "16.0 KiB");
        assert_eq!(to_friendly_size(100 * 1024), "100 KiB");
        assert_eq!(to_friendly_size(1 * 1024 * 1024 - 1), "1024 KiB");
        assert_eq!(to_friendly_size(1 * 1024 * 1024), "1.00 MiB");
        assert_eq!(to_friendly_size(11 * 1024 * 1024 + 200 * 1024), "11.2 MiB");
        assert_eq!(to_friendly_size(1 * 1024 * 1024 * 1024 - 1), "1024 MiB");
        assert_eq!(to_friendly_size(1 * 1024 * 1024 * 1024), "1.00 GiB");
        assert_eq!(to_friendly_size(1 * 1024 * 1024 * 1024 * 1024 - 1), "1024 GiB");
        assert_eq!(to_friendly_size(1 * 1024 * 1024 * 1024 * 1024), "1.00 TiB");
        assert_eq!(to_friendly_size(10 * 1024 * 1024 * 1024 * 1024), "10.0 TiB");
        assert_eq!(to_friendly_size(1 * 1024 * 1024 * 1024 * 1024 * 1024), "1024 TiB");
    }

     #[test]
    fn test_durations() {
        assert_eq!(to_friendly_duration(Duration::from_micros(148)), "0.148 ms");
        assert_eq!(to_friendly_duration(Duration::from_micros(1048)), "1.048 ms");
        assert_eq!(to_friendly_duration(Duration::from_millis(15)), "15 ms");
        assert_eq!(to_friendly_duration(Duration::from_millis(999)), "999 ms");
        assert_eq!(to_friendly_duration(Duration::from_millis(1000)), "1.0 sec");
        assert_eq!(to_friendly_duration(Duration::from_millis(1570)), "1.6 sec");
        assert_eq!(to_friendly_duration(Duration::from_millis(9900)), "9.9 sec");
        assert_eq!(to_friendly_duration(Duration::from_millis(10000)), "10 sec");
        assert_eq!(to_friendly_duration(Duration::from_secs_f64(100f64)), "100 sec");
        assert_eq!(to_friendly_duration(Duration::from_secs_f64(119f64)), "119 sec");
        assert_eq!(to_friendly_duration(Duration::from_secs_f64(120f64)), "2.0 min");
        assert_eq!(to_friendly_duration(Duration::from_secs_f64(150f64)), "2.5 min");
        assert_eq!(to_friendly_duration(Duration::from_secs_f64(594f64)), "9.9 min");
        assert_eq!(to_friendly_duration(Duration::from_secs_f64(600f64)), "10 min");
        assert_eq!(to_friendly_duration(Duration::from_secs(119 * 60)), "119 min");
        assert_eq!(to_friendly_duration(Duration::from_secs(120 * 60)), "2.0 hours");
        assert_eq!(to_friendly_duration(Duration::from_secs(150 * 60)), "2.5 hours");
        assert_eq!(to_friendly_duration(Duration::from_secs(9 * 60 * 60)), "9.0 hours");
        assert_eq!(to_friendly_duration(Duration::from_secs(10 * 60 * 60)), "10 hours");
        assert_eq!(to_friendly_duration(Duration::from_secs(44 * 60 * 60)), "44 hours");
        assert_eq!(to_friendly_duration(Duration::from_secs(48 * 60 * 60 - 1)), "48 hours");
        assert_eq!(to_friendly_duration(Duration::from_secs(48 * 60 * 60)), "2 days");
        assert_eq!(to_friendly_duration(Duration::from_secs(60 * 60 * 60)), "2 days");
        assert_eq!(to_friendly_duration(Duration::from_secs(9 * 24 * 60 * 60)), "9 days");
        assert_eq!(to_friendly_duration(Duration::from_secs(10 * 24 * 60 * 60)), "10 days");

    }
}