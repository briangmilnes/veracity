use vstd::prelude::*;

verus! {

pub struct FooStruct { pub seq: Vec<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

impl FooStruct {
    pub fn count(&self) -> u64
    {
        let mut n: u64 = 0;
        let it = FooIter { inner: self.seq.iter() };
        for x in iter: it
            invariant
                it@.0 < it@.1.len(),
        {
            n = n + 1;
        }
        n
    }
}

}
