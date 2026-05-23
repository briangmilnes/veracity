use vstd::prelude::*;

verus! {

pub struct FooStruct { pub seq: Vec<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

impl FooStruct {
    pub fn at_end(&self) -> (it: FooIter<'_>)
        ensures it@.0 == self.seq@.len(),
    {
        FooIter { inner: self.seq.iter() }
    }
}

}
