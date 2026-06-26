pub mod definitions;
pub mod deployments;
pub mod health;
pub mod router;
pub mod state;

pub use state::AppState;

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        assert!(true);
    }
}
