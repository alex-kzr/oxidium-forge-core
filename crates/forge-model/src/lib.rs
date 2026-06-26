pub mod config;
pub mod error;

pub use config::Config;
pub use error::{ForgeError, StoreError};

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        assert!(true);
    }
}
