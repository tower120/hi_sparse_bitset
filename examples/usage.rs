fn main(){
    use itertools::assert_equal;

    use hi_sparse_bitset::BitSetInterface;
    use hi_sparse_bitset::reduce;    
    use hi_sparse_bitset::binary_op::*;
    use hi_sparse_bitset::iter::IndexIterator;
    
    type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;
    let bitset1 = BitSet::from([1,2,3,4]);
    let bitset2 = BitSet::from([3,4,5,6]);
    let bitset3 = BitSet::from([3,4,7,8]);
    let bitset4 = BitSet::from([4,9,10]);
    let bitsets = [bitset1, bitset2, bitset3];
    
    // reduce on bitsets iterator
    let intersection = reduce(BitAndOp, bitsets.iter()).unwrap();
    assert_equal(&intersection, [3,4]);
    
    // operation between different types
    let union = intersection | &bitset4;
    assert_equal(&union, [3,4,9,10]);
    
    // partially traverse iterator, and save position to cursor.
    let mut iter = union.iter();
    assert_equal(iter.by_ref().take(2), [3,4]);
    let cursor = iter.cursor();
    
    // resume iteration from cursor position
    let iter = union.iter().move_to(cursor);
    assert_equal(iter, [9,10]);
}