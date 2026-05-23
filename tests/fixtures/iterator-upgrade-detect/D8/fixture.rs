use vstd::prelude::*;

verus! {

pub struct FooIter<'a, T> {
    pub inner: std::slice::Iter<'a, T>,
}

impl<'a, T> std::iter::Iterator for FooIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> (next: Option<&'a T>)
        ensures next is Some ==> true
    {
        self.inner.next()
    }
}

}
