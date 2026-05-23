use vstd::prelude::*;

verus! {

pub struct FooIter<'a, T> {
    pub inner: std::slice::Iter<'a, T>,
}

pub struct FooGhostIterator<'a, T> {
    pub pos: int,
    pub phantom: core::marker::PhantomData<&'a T>,
}

impl<'a, T> vstd::pervasive::ForLoopGhostIteratorNew for FooIter<'a, T> {
    type GhostIter = FooGhostIterator<'a, T>;
    open spec fn ghost_iter(&self) -> FooGhostIterator<'a, T> {
        FooGhostIterator { pos: 0int, phantom: core::marker::PhantomData }
    }
}

}
