
pub trait Consumer {
    fn init() -> impl Future<Output=Self>;
}