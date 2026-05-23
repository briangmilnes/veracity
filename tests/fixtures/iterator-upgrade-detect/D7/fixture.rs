use vstd::prelude::*;

verus! {

pub struct FooIter<'a, T> {
    pub inner: std::slice::Iter<'a, T>,
}

impl<'a, T> View for FooIter<'a, T> {
    type V = (int, Seq<T>);
    open spec fn view(&self) -> (int, Seq<T>) { self.inner@ }
}

}
