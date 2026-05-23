use vstd::prelude::*;

verus! {

pub struct FooStruct { pub seq: Vec<u64> }
pub struct FooIter<'a> { pub inner: std::slice::Iter<'a, u64> }

pub open spec fn iter_invariant<'a>(it: &FooIter<'a>) -> bool { true }

impl FooStruct {
    pub fn sum(&self) -> u64
    {
        let mut total: u64 = 0;
        let it = FooIter { inner: self.seq.iter() };
        loop
            invariant
                iter_invariant(&it),
        {
            total = total + 1;
            break;
        }
        total
    }
}

}
