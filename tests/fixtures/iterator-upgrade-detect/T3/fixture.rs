use vstd::prelude::*;

verus! {

pub struct FooStruct { pub data: Seq<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

impl FooStruct {
    pub spec fn spec_data(&self) -> Seq<u64> { self.data }

    pub fn iter(&self) -> (it: FooIter<'_>)
        ensures it@.1 == self.spec_data(),
    {
        unimplemented!()
    }
}

}
