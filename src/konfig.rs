use once_cell::sync::Lazy;
use std::sync::RwLock;

pub struct Konfig {
    // Add fields here as needed
    pub example_field: String,
}

impl Konfig {
    fn new() -> Self {
        Konfig {
            example_field: "default_value".to_string(),
        }
    }
}

pub static KONFIG: Lazy<RwLock<Konfig>> = Lazy::new(|| {
    RwLock::new(Konfig::new())
});

// Example usage (can be removed or moved to tests)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_konfig_access() {
        {
            let konfig_read = KONFIG.read().unwrap();
            assert_eq!(konfig_read.example_field, "default_value");
        }
        {
            let mut konfig_write = KONFIG.write().unwrap();
            konfig_write.example_field = "new_value".to_string();
        }
        {
            let konfig_read_again = KONFIG.read().unwrap();
            assert_eq!(konfig_read_again.example_field, "new_value");
        }
    }
} 