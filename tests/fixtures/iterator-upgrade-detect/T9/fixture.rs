use vstd::prelude::*;

verus! {

pub struct FooStruct { pub seq: Vec<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

impl FooStruct {
    pub fn count(&self) -> u64
    {
        let mut n: u64 = 0;
        let it = FooIter { inner: self.seq.iter() };
        loop
            invariant
                it@.0 <= self.seq@.len(),
        {
            n = n + 1;
            break;
        }
        n
    }
}

}
