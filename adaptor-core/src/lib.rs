#![feature(array_methods)]
#![allow(dead_code)]

pub mod adaptor;
pub mod errors;

pub type Result<T> = std::result::Result<T, errors::AdaptorError>;

pub use adaptor::AdaptorHandle;
pub use adaptor_common::AdaptorSettings;
pub use errors::AdaptorError;
