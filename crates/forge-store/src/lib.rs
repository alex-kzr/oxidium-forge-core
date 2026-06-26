pub mod definitions;
pub mod migrate;
pub mod pool;

pub use pool::Store;

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        assert!(true);
    }
}
