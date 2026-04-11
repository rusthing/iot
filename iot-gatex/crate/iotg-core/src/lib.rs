pub mod driver;
mod iotg_error;
pub mod model;

pub use driver::Driver;
pub use iotg_error::IotgError;
pub use model::{Batch, DataPoint, Quality, Value};

/// channel 容量：所有驱动共用
pub const CHANNEL_CAP: usize = 8192;
