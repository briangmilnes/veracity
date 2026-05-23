use vstd::prelude::*;

verus! {

pub struct FooGhostIterator<'a, T> {
    pub pos: int,
    pub elements: Seq<T>,
    pub phantom: core::marker::PhantomData<&'a T>,
}

impl<'a, T> View for FooGhostIterator<'a, T> {
    type V = (int, Seq<T>);
    open spec fn view(&self) -> (int, Seq<T>) { (self.pos, self.elements) }
}

}
