use std::fmt::{Debug, Display, Formatter};
use std::fmt::Result as FmtResult;
use vstd::prelude::*;

verus! {

pub struct FooIter<'a, T> {
    pub inner: std::slice::Iter<'a, T>,
}

}

impl<'a, T: Debug> Debug for FooIter<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "FooIter({:?})", self.inner)
    }
}

impl<'a, T> Display for FooIter<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "FooIter")
    }
}
