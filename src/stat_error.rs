
#[derive(Debug)]
pub enum StatError {
    BadOpp(String),
    NotFound(String),
}