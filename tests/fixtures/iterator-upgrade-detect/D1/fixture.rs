use vstd::prelude::*;

verus! {

pub struct FooGhostIterator<'a, T> {
    pub pos: int,
    pub elements: Seq<T>,
    pub phantom: core::marker::PhantomData<&'a T>,
}

}
