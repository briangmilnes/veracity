use vstd::prelude::*;

verus! {

pub struct FooStruct { pub seq: Vec<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

pub open spec fn iter_invariant<'a>(it: &FooIter<'a>) -> bool {
    0int <= it@.0 <= it@.1.len()
}

impl FooStruct {
    pub fn iter(&self) -> (it: FooIter<'_>)
        ensures
            it@.0 == 0,
            it@.1 == self.seq@,
            iter_invariant(&it),
    {
        FooIter { inner: self.seq.iter() }
    }
}

}
