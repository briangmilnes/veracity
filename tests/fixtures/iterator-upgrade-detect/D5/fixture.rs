use std::fmt::{Debug, Display, Formatter};
use std::fmt::Result as FmtResult;
use vstd::prelude::*;

verus! {

pub struct FooGhostIterator<'a, T> {
    pub pos: int,
    pub phantom: core::marker::PhantomData<&'a T>,
}

}

impl<'a, T> Debug for FooGhostIterator<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "FooGhostIterator")
    }
}

impl<'a, T> Display for FooGhostIterator<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "FooGhostIterator")
    }
}
