pub use macro_impl::state_machine;

pub trait Action {}
pub trait State<W, A: Action> {
    fn next(self, action: A) -> W;
}
