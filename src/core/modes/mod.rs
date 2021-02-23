pub mod text_mode;

pub use text_mode::TextMode;

pub trait Mode {
    fn name(&self) -> &'static str;
}
