#![warn(clippy::all)]
mod helper_buf;
mod reader;
mod writer;

pub use reader::ReadHalf;
pub use writer::WriteHalf;

#[cfg(test)]
mod test;
