#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunMode {
    #[default]
    Verbose = 0,
    LessVerbose = 0x1000_0000,
}
