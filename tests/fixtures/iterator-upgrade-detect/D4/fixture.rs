use vstd::prelude::*;

verus! {

pub struct FooIter<'a, T> { pub inner: std::slice::Iter<'a, T> }
pub struct FooGhostIterator<'a, T> {
    pub pos: int,
    pub elements: Seq<T>,
    pub phantom: core::marker::PhantomData<&'a T>,
}

impl<'a, T> vstd::pervasive::ForLoopGhostIterator for FooGhostIterator<'a, T> {
    type ExecIter = FooIter<'a, T>;
    type Item = T;
    type Decrease = int;
    open spec fn exec_invariant(&self, _e: &FooIter<'a, T>) -> bool { true }
    open spec fn ghost_invariant(&self, _i: Option<&Self>) -> bool { true }
    open spec fn ghost_ensures(&self) -> bool { true }
    open spec fn ghost_decrease(&self) -> Option<int> { Some(0int) }
    open spec fn ghost_peek_next(&self) -> Option<T> { None }
    open spec fn ghost_advance(&self, _e: &FooIter<'a, T>) -> FooGhostIterator<'a, T> {
        Self { pos: self.pos + 1int, ..*self }
    }
}

}
