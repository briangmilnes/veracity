use vstd::prelude::*;

verus! {

pub struct FooStruct { pub seq: Vec<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

impl FooStruct {
    pub fn walk(&self) -> u64
    {
        let mut n: u64 = 0;
        let it = FooIter { inner: self.seq.iter() };
        for x in iter: it
            invariant
                true,
            decreases self.seq@.len() - it@.0,
        {
            n = n + 1;
        }
        n
    }
}

}
