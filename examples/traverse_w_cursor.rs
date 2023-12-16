//! Example of using traverse() with cursor.

fn main(){
    use hi_sparse_bitset::prelude::*;
    use hi_sparse_bitset::iter::IndexCursor;
    use std::ops::ControlFlow::*;
    
    type BitSet = hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>;
    let bitset = BitSet::from([1,2,3,4]);

    // copy elements from bitset in 2 element sessions.
    let mut output = Vec::new();
    let mut cursor = IndexCursor::start();

    loop{
        let mut counter = 0;
        let ctrl = 
            bitset
            .iter().move_to(cursor)
            .traverse(|i|{
                if counter == 2{
                    cursor = i.into();
                    return Break(());
                }
                counter += 1;

                output.push(i);
                Continue(())
            });
        if ctrl.is_continue(){
            break;
        }
    }

    assert_eq!(output, vec![1,2,3,4]);
}