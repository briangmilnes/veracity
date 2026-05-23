use vstd::prelude::*;

verus! {

pub struct FooIter<'a, T> {
    pub inner: std::slice::Iter<'a, T>,
}

pub open spec fn iter_invariant<'a, T>(it: &FooIter<'a, T>) -> bool {
    0int <= it@.0 <= it@.1.len()
}

}
