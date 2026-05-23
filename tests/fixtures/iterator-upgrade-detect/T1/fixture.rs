use vstd::prelude::*;

verus! {

pub struct FooStruct { pub seq: Vec<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

impl FooStruct {
    pub fn iter(&self) -> (it: FooIter<'_>)
        ensures it@.0 == 0,
    {
        FooIter { inner: self.seq.iter() }
    }
}

}
