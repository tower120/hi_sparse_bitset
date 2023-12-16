use std::collections::{HashSet, VecDeque};
use std::iter::zip;

use itertools::assert_equal;
use rand::Rng;
use crate::binary_op::{BitAndOp, BitOrOp, BitSubOp, BitXorOp};
use crate::cache::{DynamicCache, FixedCache};
use crate::iter::{BlockCursor, IndexCursor, IndexIterator};
use crate::bitset_op::BitSetOp;
use crate::bitset_interface::BitSetInterface;
use crate::iter::BlockIterator;

use super::*;

cfg_if::cfg_if! {
    if #[cfg(hisparsebitset_test_NoCache)] {
        type DefaultCache = cache::NoCache;
    } else if #[cfg(hisparsebitset_test_FixedCache)] {
        type DefaultCache = cache::FixedCache<32>;
    } else if #[cfg(hisparsebitset_test_DynamicCache)] {
        type DefaultCache = cache::DynamicCache;
    } else {
        //type DefaultCache = cache::FixedCache<32>;
        type DefaultCache = cache::DynamicCache;
    }
}

cfg_if::cfg_if! {
    if #[cfg(hisparsebitset_test_64)] {
        type Conf = config::with_cache::_64bit<DefaultCache>;
    } else if #[cfg(hisparsebitset_test_128)] {
        type Conf = config::with_cache::_128bit<DefaultCache>;
    } else if #[cfg(hisparsebitset_test_256)] {
        type Conf = config::with_cache::_256bit<DefaultCache>;
    } else {
        type Conf = config::with_cache::_128bit<DefaultCache>;
    }
}

type HiSparseBitset = super::BitSet<Conf>;

#[test]
fn level_indices_test(){
    type Conf = config::_128bit;

    let levels = level_indices::<Conf>(0);
    assert_eq!(levels, (0,0,0));

    let levels = level_indices::<Conf>(10);
    assert_eq!(levels, (0,0,10));

    let levels = level_indices::<Conf>(128);
    assert_eq!(levels, (0,1,0));

    let levels = level_indices::<Conf>(130);
    assert_eq!(levels, (0,1,2));

    let levels = level_indices::<Conf>(130);
    assert_eq!(levels, (0,1,2));

    let levels = level_indices::<Conf>(128*128);
    assert_eq!(levels, (1,0,0));

    let levels = level_indices::<Conf>(128*128 + 50*128);
    assert_eq!(levels, (1,50,0));

    let levels = level_indices::<Conf>(128*128 + 50*128 + 4);
    assert_eq!(levels, (1,50,4));
}

#[test]
fn smoke_test(){
    let mut set = HiSparseBitset::default();

    assert!(!set.contains(0));
    set.insert(0);
    assert!(set.contains(0));
}

#[test]
fn insert_regression_test(){
    // DataBlockIndex was not large enough to address all DataBlocks.
    let insert = vec![81648, 70040, 69881, 4369, 31979, 56135, 87035, 27405, 94536, 14584, 69382, 49738, 33614, 19792, 66045, 29454, 59890, 1090, 80621, 53565, 14159, 2074, 76781, 6738, 83682, 20911, 94984, 80623, 50653, 26040, 79167, 50392, 31127, 28651, 59950, 73948, 12481, 13289, 16253, 77853, 42874, 86002, 63915, 7955, 52174, 33139, 77502, 16557, 97431, 9890, 19461, 82497, 87773, 85552, 88794, 5638, 53958, 37342, 57421, 79867, 96855, 83728, 1474, 6109, 6257, 91164, 76875, 19594, 44621, 57130, 53782, 75442, 50704, 40294, 16568, 9678, 75137, 29432, 80030, 6055, 43712, 79514, 19474, 61466, 46711, 87950, 94863, 1003, 46131, 61479, 87580, 4921, 49036, 71276, 67886, 8474, 58231, 60423, 99815, 49265, 82376, 62220, 30612, 26212, 29064, 75311, 60434, 14591, 8479, 63516, 79371, 98992, 34600, 3073, 15808, 71479, 80278, 28596, 27844, 42506, 17133, 59812, 89721, 92112, 23382, 70895, 23044, 96229, 74413, 12051, 94022, 4830, 30606, 64922, 89663, 59286, 98662, 16009, 42336, 29433, 60748, 41762, 23098, 62999, 24522, 75963, 18002, 37599, 79931, 43878, 25758, 75672, 33099, 54768, 57160, 73527, 46764, 75596, 81567, 5953, 69160, 70799, 20319, 8023, 24639, 5526, 4068, 7248, 14628, 97735, 42080, 25881, 91583, 40605, 44134, 22706, 34547, 53265, 1424, 80496, 9894, 19324, 35624, 10335, 97325, 12085, 57335, 89242, 52991, 74868, 75155, 78683, 68180, 55659, 159, 82153, 21802, 62499, 13865, 86661, 63992, 56095, 46342, 9339, 29598, 57330, 40593, 50058, 11451, 79062, 76579, 97251, 2045, 69331, 44047, 51070, 52200, 29900, 18500, 26570, 69129, 841, 88289, 78380, 23277, 27252, 80342, 98361, 41967, 76318, 46160, 49982, 42613, 44331, 54163, 32182, 46394, 63567, 25258, 84565, 18447, 16327, 68024, 95023, 55068, 59260, 24933, 4065, 5060, 81498, 89619, 7464, 60886, 22123, 87004, 25864, 17141, 34239, 10916, 5989, 91695, 24318, 82378, 32613, 16399, 50519, 54776, 87956, 84821, 74634, 7997, 86768, 34603, 69863, 94967, 50891, 70401, 83942, 85139, 8364, 8258, 99866, 21950, 63876, 32750, 5189, 75194, 34563, 78447, 14877, 83526, 26214, 1948, 68727, 49824, 55674, 4734, 48862, 26280, 75557, 96939, 30784, 66303, 69583, 44598, 82073, 54225, 81280, 8735, 32231, 60384, 2399, 5950, 96235, 42523, 20806, 26941, 73590, 32468, 4430, 76240, 34904, 24908, 31811, 71084, 99476, 48439, 13982, 24755, 38280, 81421, 66048, 72804, 5676, 22421, 88208, 43076, 3825, 45046, 17674, 52393, 70823, 82194, 25426, 53426, 34225, 68279, 98832, 29892, 38107, 64503, 24810, 99691, 82755, 56292, 21381, 96569, 33870, 50740, 10775, 43463, 10361, 79284, 21914, 33337, 93280, 27701, 12272, 35920, 35046, 82052, 75639, 1265, 90897, 99721, 27096, 75006, 92116, 12568, 11396, 64228, 85758, 8041, 30803, 37449, 83983, 33759, 75077, 22202, 90770, 29504, 40942, 134, 1661, 63615, 2465, 96964, 62333, 41605, 18746, 97835, 41262, 77397, 37877, 11084, 51122, 74987, 28024, 78981, 67489, 58293, 38546, 5753, 35035, 81375, 46964, 12780, 85476, 9278, 79564, 71250, 73450, 54928, 60383, 8784, 78604, 82215, 76524, 68112, 81080, 60635, 3313, 70818, 11515, 15039, 16401, 42245, 48242, 87192, 27965, 72971, 11937, 75718, 53388, 59647, 69358, 42201, 66701, 51628, 34994, 39815, 63946, 10996, 11503, 85880, 58792, 94111, 6673, 99802, 7823, 29570, 81137, 27800, 93920, 78610, 84695, 96901, 68661, 80431, 66087, 50296, 45463, 63353, 44284, 84585, 76471, 38385, 31463, 91744, 55237, 44637, 15091, 62018, 23315, 36266, 94985, 95107, 63600, 46176, 94696, 69953, 99624, 91338, 11665, 33243, 22048, 77125, 46785, 12688, 61284, 87989, 85715, 65754, 86959, 61316, 95721, 27272, 39746, 53254, 78023, 49197, 51300, 15061, 92589, 70761, 29260, 9711, 21532, 43802, 38809, 45438, 10105, 14774, 60125, 51371, 27479, 35448, 9152, 41521, 61589, 82456, 87647, 10689, 68592, 23324, 66377, 91867, 7881, 23242, 5566, 1650, 447, 56480, 29996, 33382, 9913, 10669, 7657, 49606, 8707, 64984, 71762, 97917, 81242, 58586, 30410, 19232, 84153, 84033, 94535, 71307, 34988, 73823, 53717, 9757, 25540, 43769, 38933, 24864, 3490, 54100, 74574, 45607, 64771, 20570, 51752, 51354, 934, 61460, 66962, 90202, 39095, 13054, 60517, 85046, 38155, 17786, 12018, 73068, 5961, 16816, 73953, 80046, 71998, 99611, 52521, 14382, 58105, 86941, 17770, 67849, 67536, 1911, 59935, 17209, 37967, 29073, 27130, 74980, 97600, 41566, 79834, 20446, 99223, 48978, 66506, 59855, 18264, 9047, 13150, 92839, 26830, 74781, 27987, 75994, 60438, 8940, 48668, 84280, 71786, 1803, 71381, 41078, 10382, 84257, 95683, 4587, 33126, 92651, 78140, 61236, 96100, 93009, 61924, 56318, 34929, 78248, 55956, 3299, 73724, 45611, 25372, 62847, 14959, 63943, 46100, 66310, 84733, 39094, 22378, 36605, 66795, 44425, 13795, 14831, 63404, 90275, 26253, 65048, 65796, 19194, 6974, 71510, 95671, 72101, 81604, 9421, 38217, 40571, 11897, 653, 73418, 78287, 12134, 41718, 95256, 50881, 38983, 31079, 34469, 20615, 56502, 18022, 34186, 82119, 61533, 89187, 79094, 88514, 98011, 50228, 28780, 23755, 18383, 17406, 36655, 15121, 39548, 95978, 95355, 5141, 20769, 49183, 16338, 11419, 19076, 64557, 62532, 74389, 62598, 14643, 69516, 32262, 20264, 78275, 8679, 90212, 53704, 90341, 41178, 99161, 32231, 40456, 17388, 81135, 88476, 12577, 21064, 51932, 31816, 97568, 90908, 58263, 28436, 47779, 49437, 99197, 4320, 72970, 67943, 85990, 60832, 71006, 42225, 22618, 3382, 5303, 327, 28724, 37180, 15129, 83399, 24183, 56464, 87398, 29180, 16049, 15357, 58042, 31234, 42819, 67983, 51088, 5282, 61580, 84463, 10900, 70287, 62423, 85788, 28859, 26614, 94292, 54912, 36618, 81500, 85987, 92179, 86629, 13646, 37609, 64161, 34435, 53043, 54794, 74341, 15869, 44322, 74946, 64581, 39531];
    let hi_set = HiSparseBitset::from_iter(insert.iter().copied());
    let c = hi_set.contains(76790);
    assert!(!c);
}

#[test]
fn fuzzy_test(){
    cfg_if::cfg_if! {
    if #[cfg(miri)] {
        const MAX_SIZE : usize = 1000;
        const MAX_RANGE: usize = 1000;
        const CONTAINS_PROBES: usize = 100;
        const REPEATS: usize = 2;
        const INNER_REPEATS: usize = 3;
        const INDEX_MUL: usize = 10;
    } else {
        const MAX_SIZE : usize = 10000;
        const MAX_RANGE: usize = 10000;
        const CONTAINS_PROBES: usize = 1000;
        const REPEATS: usize = 100;
        const INNER_REPEATS: usize = 10;
        const INDEX_MUL: usize = 10;
    }
    }
    const MAX_CURSOR_READ_SESSION: usize = MAX_SIZE;

    let mut rng = rand::thread_rng();
    for _ in 0..REPEATS{
        let mut hash_set = HashSet::new();
        let mut hi_set = HiSparseBitset::default();

        let mut inserted = Vec::new();
        let mut removed = Vec::new();

        for _ in 0..INNER_REPEATS{
            // random insert
            for _ in 0..rng.gen_range(0..MAX_SIZE){
                let index = rng.gen_range(0..MAX_RANGE)*INDEX_MUL;
                inserted.push(index);
                hash_set.insert(index);
                hi_set.insert(index);
            }

            // random remove
            if !inserted.is_empty(){
                for _ in 0..rng.gen_range(0..inserted.len()){
                    let index = rng.gen_range(0..inserted.len());
                    let value = inserted[index];
                    removed.push(value);
                    hash_set.remove(&value);
                    hi_set.remove(value);
                }
            }

            // random contains
            for _ in 0..CONTAINS_PROBES{
                let index = rng.gen_range(0..MAX_RANGE)*INDEX_MUL;
                let h1 = hash_set.contains(&index);
                let h2 = hi_set.contains(index);
                assert_eq!(h1, h2);
            }

            // existent contains
            for &index in &hash_set{
                assert!(hi_set.contains(index));
            }
            
            // block traverse contains
            hi_set.block_iter().traverse(|block|{ 
                block.traverse(|index|{
                    assert!(hash_set.contains(&index));
                    ControlFlow::Continue(())     
                })
            });

            // index traverse contains
            hi_set.iter().for_each(|index|{ 
                assert!(hash_set.contains(&index));
            });            

            // non existent does not contains
            for &index in &removed{
                let h1 = hash_set.contains(&index);
                let h2 = hi_set.contains(index);
                assert_eq!(h1, h2);
            }
            
            // eq
            {
                let other: HiSparseBitset = hi_set.iter().collect();
                assert!(hi_set == other);
            }

            let mut hash_set_vec: Vec<usize> = hash_set.iter().copied().collect();
            hash_set_vec.sort();
            
            // block traverse cursor sessions
            {
                let mut cursor = BlockCursor::start();
                let mut traversed = Vec::new();

                loop{
                    let mut session_counter = rng.gen_range(0..MAX_CURSOR_READ_SESSION) as isize;                
                    let ctrl = hi_set.block_iter().move_to(cursor).traverse(|block|{
                        if session_counter <= 0{
                            cursor = (&block).into();
                            return ControlFlow::Break(());
                        }
                        session_counter -= block.len() as isize;

                        traversed.extend(block);
                        ControlFlow::Continue(())
                    });
                    if ctrl.is_continue(){
                        break;
                    }
                }

                assert_equal(traversed, hash_set_vec.iter().copied());
            }
            
            // index traverse cursor sessions
            {
                let mut cursor = IndexCursor::start();
                let mut traversed = Vec::new();

                loop{
                    let mut session_counter = rng.gen_range(0..MAX_CURSOR_READ_SESSION);
                    let ctrl = hi_set.iter().move_to(cursor).traverse(|index|{
                        if session_counter == 0{
                            cursor = index.into();
                            return ControlFlow::Break(());
                        }
                        session_counter -= 1;

                        traversed.push(index);
                        ControlFlow::Continue(())
                    });
                    if ctrl.is_continue(){
                        break;
                    }
                }

                assert_equal(traversed, hash_set_vec.iter().copied());
            }
        }
    }
}

fn fuzzy_reduce_test<Op: BinaryOp, H>(hiset_op: Op, hashset_op: H)
where
    H: Fn(&HashSet<usize>, &HashSet<usize>) -> HashSet<usize>,
    H: Copy
{
    cfg_if::cfg_if! {
    if #[cfg(miri)] {
        const MAX_SETS : usize = 4;
        const MAX_INSERTS: usize = 100;
        const MAX_GUARANTEED_INTERSECTIONS: usize = 10;
        const MAX_REMOVES : usize = 100;
        const MAX_RANGE: usize = 1000;
        const MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME: usize = 5;
        const MAX_RESUMED_INTERSECTION_INDICES_CONSUME: usize = 30;
        const REPEATS: usize = 2;
        const INNER_REPEATS: usize = 3;
        const INDEX_MUL: usize = 20;
    } else {
        const MAX_SETS : usize = 10;
        const MAX_INSERTS: usize = 10000;
        const MAX_GUARANTEED_INTERSECTIONS: usize = 10;
        const MAX_REMOVES : usize = 10000;
        const MAX_RANGE: usize = 10000;
        const MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME: usize = 100;
        const MAX_RESUMED_INTERSECTION_INDICES_CONSUME: usize = 300;
        const REPEATS: usize = 100;
        const INNER_REPEATS: usize = 10;
        const INDEX_MUL: usize = 10;
    }
    }

    #[inline]
    fn hashset_multi_op<'a, H>(
        hash_sets: impl IntoIterator<Item = &'a HashSet<usize>>,
        hashset_op: H
    ) -> HashSet<usize>
    where
        H: Fn(&HashSet<usize>, &HashSet<usize>) -> HashSet<usize>
    {
        let mut hash_sets_iter = hash_sets.into_iter();
        let mut acc = hash_sets_iter.next().unwrap().clone();
        for set in hash_sets_iter{
            let intersection = hashset_op(&acc, &set);
            acc = intersection;
        }
        acc
    }

    let mut rng = rand::thread_rng();
    for _ in 0..REPEATS{
        let sets_count = rng.gen_range(2..MAX_SETS);
        let mut hash_sets: Vec<HashSet<usize>> = vec![Default::default(); sets_count];
        let mut hi_sets  : Vec<HiSparseBitset> = vec![Default::default(); sets_count];

        // Resumable intersection guarantee that we'll traverse at least
        // non removed initial intersection set.

        // initial insert
        let mut block_cursor = BlockCursor::default();
        let mut index_cursor = IndexCursor::default();
        let mut initial_hashsets_intersection_for_blocks;
        let mut initial_hashsets_intersection_for_indices;
        {
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_INSERTS){
                    let index = rng.gen_range(0..MAX_RANGE)*INDEX_MUL;
                    hash_set.insert(index);
                    hi_set.insert(index);
                }
            }
            initial_hashsets_intersection_for_blocks  = hashset_multi_op(&hash_sets, hashset_op);            
            initial_hashsets_intersection_for_indices = initial_hashsets_intersection_for_blocks.clone();
        }        

        for _ in 0..INNER_REPEATS{
            let mut inserted = Vec::new();
            // random insert
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_INSERTS){
                    let index = rng.gen_range(0..MAX_RANGE)*INDEX_MUL;
                    hash_set.insert(index);
                    hi_set.insert(index);
                    inserted.push(index);
                }
            }

            // guaranteed intersection (insert all)
            for _ in 0..rng.gen_range(0..MAX_GUARANTEED_INTERSECTIONS){
                let index = rng.gen_range(0..MAX_RANGE)*INDEX_MUL;
                for hash_set in &mut hash_sets{
                    hash_set.insert(index);
                }
                for hi_set in &mut hi_sets{
                    hi_set.insert(index);
                }
                inserted.push(index);
            }

            // random remove
            let mut removed = Vec::new();
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_REMOVES){
                    let index = rng.gen_range(0..MAX_RANGE)*INDEX_MUL;
                    hash_set.remove(&index);
                    hi_set.remove(index);
                    removed.push(index);
                }
            }

            // etalon intersection
            let hashsets_intersection = hashset_multi_op(&hash_sets, hashset_op);
            let mut hashsets_intersection_vec: Vec<_> = hashsets_intersection.iter().copied().collect();
            hashsets_intersection_vec.sort();


            // remove non-existent intersections from initial_hashsets_intersection
            let changed = Iterator::chain(removed.iter(), inserted.iter());
            for index in changed{
                if !hashsets_intersection.contains(index){
                    initial_hashsets_intersection_for_blocks.remove(index);
                    initial_hashsets_intersection_for_indices.remove(index);
                }
            }

            // suspend/resume blocks
            {
                let mut intersection = 
                    reduce(hiset_op, hi_sets.iter()).unwrap()
                    .into_block_iter()
                    .move_to(block_cursor);

                let mut blocks_to_consume = rng.gen_range(0..MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME);

                // through traverse
                let mut traversed_cursor = BlockCursor::end();
                let mut traversed_blocks = Vec::new();
                {
                    let mut blocks_to_consume = blocks_to_consume;
                    intersection.clone().traverse(|block|{
                        if blocks_to_consume == 0{
                            traversed_cursor = BlockCursor::from(&block);
                            return ControlFlow::Break(());
                        }
                        blocks_to_consume -= 1;
                                                 
                        traversed_blocks.push(block);
                        ControlFlow::Continue(())
                    });
                };

                // all intersections must be valid
                let mut iterated_blocks = Vec::new();
                loop{
                    if blocks_to_consume == 0{
                        break;
                    }
                    blocks_to_consume -= 1;

                    if let Some(block) = intersection.next(){
                        block.traverse(
                            |index|{
                                assert!(hashsets_intersection.contains(&index));
                                // We cannot guarantee that index will
                                // exists in initial intersection, since
                                // it could be added after initial fill.
                                initial_hashsets_intersection_for_blocks.remove(&index);
                                ControlFlow::Continue(())
                            }
                        );
                        
                        iterated_blocks.push(block);
                    } else {
                        break;
                    }
                }
                
                assert_equal(traversed_blocks, iterated_blocks);

                block_cursor = intersection.cursor();
                assert_eq!(
                    intersection.clone().move_to(block_cursor).next(),
                    intersection.clone().move_to(traversed_cursor).next()
                );
            }

            // suspend/resume indices
            {
                let mut intersection = 
                    reduce(hiset_op, hi_sets.iter()).unwrap()
                    .into_iter()
                    .move_to(index_cursor);
                
                let indices_to_consume = rng.gen_range(0..MAX_RESUMED_INTERSECTION_INDICES_CONSUME);

                // through traverse
                let mut traversed_cursor = IndexCursor::end();
                let mut traversed_indices = Vec::new();
                {
                    let mut indices_to_consume = indices_to_consume;
                    intersection.clone().traverse(|i|{
                        if indices_to_consume == 0{
                            traversed_cursor = i.into();
                            return ControlFlow::Break(());
                        }
                        indices_to_consume -= 1;

                        traversed_indices.push(i);                        
                        ControlFlow::Continue(())
                    });
                }

                // iteration
                let mut iterated_indices = Vec::new();
                for index in intersection.by_ref().take(indices_to_consume){
                    assert!(hashsets_intersection.contains(&index));
                    // We cannot guarantee that index will
                    // exists in initial intersection, since
                    // it could be added after initial fill.
                    initial_hashsets_intersection_for_indices.remove(&index);
                    iterated_indices.push(index);
                }

                index_cursor = intersection.cursor();
                assert_equal(traversed_indices, iterated_indices);
                assert_eq!(
                    intersection.clone().move_to(index_cursor).next(),
                    intersection.clone().move_to(traversed_cursor).next()
                );
            }

            // reduce test
            {
                let mut indices2 = Vec::new();
                for block in reduce(hiset_op, hi_sets.iter()).unwrap().block_iter(){
                    block.traverse(
                        |index|{
                            indices2.push(index);
                            ControlFlow::Continue(())
                        }
                    );
                }
                indices2.sort();
                assert_eq!(hashsets_intersection_vec, indices2);
            }

            // op
            {
                fn run<Op, S1, S2>(op: BitSetOp<Op, S1, S2>) -> Vec<usize>
                where
                    Op: BinaryOp,
                    S1: LevelMasksExt,
                    S2: LevelMasksExt<Conf = S1::Conf>,
                {
                    let mut indices2 = Vec::new();
                    for block in op.block_iter(){
                        block.traverse(
                            |index|{
                                indices2.push(index);
                                ControlFlow::Continue(())
                            }
                        );
                    }
                    indices2.sort();
                    indices2
                }

                let op = BitSetOp::new(hiset_op, &hi_sets[0], &hi_sets[1]);
                let indices2 = match hi_sets.len(){
                    2 => {
                        Some(run(op))
                    },
                    3 => {
                        let op = BitSetOp::new(hiset_op, op, &hi_sets[2]);
                        Some(run(op))
                    },
                    4 => {
                        let op = BitSetOp::new(hiset_op, op, &hi_sets[2]);
                        let op = BitSetOp::new(hiset_op, op, &hi_sets[3]);
                        Some(run(op))
                    },
                    5 => {
                        let op = BitSetOp::new(hiset_op, op, &hi_sets[2]);
                        let op = BitSetOp::new(hiset_op, op, &hi_sets[3]);
                        let op = BitSetOp::new(hiset_op, op, &hi_sets[4]);
                        Some(run(op))
                    },
                    _ => {
                        // Just skip all other cases, too long to type that.
                        None
                    },
                };
                if let Some(indices2) = indices2{
                    assert_eq!(hashsets_intersection_vec, indices2);
                }
            }
        }

        // consume resumable blocks leftovers
        {
            let intersection = 
                reduce(hiset_op, hi_sets.iter()).unwrap()
                .into_block_iter()
                .move_to(block_cursor);
            for block in intersection{
                block.traverse(
                    |index|{
                        initial_hashsets_intersection_for_blocks.remove(&index);
                        ControlFlow::Continue(())
                    }
                );
            }
        }

        // consume resumable indices leftovers
        {
            let intersection = 
                reduce(hiset_op, hi_sets.iter()).unwrap()
                .into_iter()
                .move_to(index_cursor);
            
            for index in intersection{
                initial_hashsets_intersection_for_indices.remove(&index);
            }
        }

        // assert that we consumed all of initial intersection set.
        assert!(initial_hashsets_intersection_for_blocks.is_empty());
        assert!(initial_hashsets_intersection_for_indices.is_empty());
    }
}

#[test]
fn fuzzy_and_test(){
    fuzzy_reduce_test(BitAndOp, |l,r| l&r);
}

#[test]
fn fuzzy_or_test(){
    fuzzy_reduce_test(BitOrOp, |l,r| l|r);
}

#[test]
fn fuzzy_xor_test(){
    fuzzy_reduce_test(BitXorOp, |l,r| l^r);
}

// Sub, probably, should not be used with reduce. But for test it will work.
#[test]
fn fuzzy_sub_test(){
    fuzzy_reduce_test(BitSubOp, |l,r| l-r);
}

#[test]
fn empty_intersection_test(){
    let reduced = reduce(BitAndOp, std::iter::empty::<&HiSparseBitset>());
    assert!(reduced.is_none());
}

#[test]
fn one_intersection_test(){
    let mut hi_set = HiSparseBitset::default();
    hi_set.insert(0);
    hi_set.insert(12300);
    hi_set.insert(8760);
    hi_set.insert(521);

    let cursor = BlockCursor::default();
    let iter = 
        reduce(BitAndOp, [&hi_set].into_iter()).unwrap()
        .into_block_iter()
        .move_to(cursor);

    let mut intersection = Vec::new();
    for block in iter{
        block.traverse(
            |index|{
                intersection.push(index);
                ControlFlow::Continue(())
            }
        );
    }
    intersection.sort();
    assert_equal(intersection, [0, 521, 8760, 12300]);
}

#[test]
fn regression_test1() {
    // worked only below 2^14=16384.
    // Probably because 128^2 = 16384.
    // Problem on switching level0 block.
    let sets_data = vec![
        vec![
            16384
        ],
        vec![
            16384
        ],
    ];

    let hash_sets: Vec<HashSet<usize>> =
        sets_data.clone().into_iter()
        .map(|data| data.into_iter().collect())
        .collect();
    let hi_sets: Vec<HiSparseBitset> =
        sets_data.clone().into_iter()
        .map(|data| data.into_iter().collect())
        .collect();

    let etalon_intersection = hash_sets[0].intersection(&hash_sets[1]);
    println!("etalon: {:?}", etalon_intersection);

    {
        let mut indices2 = Vec::new();
        let iter = 
            reduce(BitAndOp, hi_sets.iter()).unwrap()
            .into_block_iter()
            .move_to(BlockCursor::default());
        for block in iter{
            block.traverse(
                |index|{
                    indices2.push(index);
                    ControlFlow::Continue(())
                }
            );
        }
        println!("indices: {:?}", indices2);
        assert_equal(etalon_intersection, &indices2);
    }
}

#[test]
fn resume_valid_level1_index_miri_test(){
    let s1: HiSparseBitset = [1000, 2000, 3000].into();
    let s2 = s1.clone();

    let list = [s1, s2];
    let r = reduce_w_cache(BitAndOp, list.iter(), DynamicCache).unwrap();
    let cursor = {
        let mut i =  r.block_iter();
        i.next().unwrap();
        i.cursor()
    };

    let r = reduce_w_cache(BitAndOp, list.iter(), DynamicCache).unwrap();

    let mut i = r.block_iter().move_to(cursor);
    i.next();
}

#[test]
fn remove_regression_test1() {
    let mut hi_set = HiSparseBitset::new();
    hi_set.insert(10000);
    hi_set.remove(10000);
    hi_set.insert(10000);

    let c= hi_set.contains(10000);
    assert!(c);
}

#[test]
fn reduce2_test() {
    let hi_set1: HiSparseBitset = [1,2,3].into_iter().collect();
    let hi_set2: HiSparseBitset = [1,2,3].into_iter().collect();
    let hi_set3: HiSparseBitset = [1,3].into_iter().collect();

    let hi_sets = [hi_set1, hi_set2, hi_set3];
    let hi_set_refs = [&hi_sets[0], &hi_sets[1], &hi_sets[2]];

    let result = reduce(BitAndOp, hi_sets.iter()).unwrap();
    let intersections = result.iter();
    assert_equal(intersections, [1,3]);

    let result = reduce(BitAndOp, hi_set_refs.iter().copied()).unwrap();
    let intersections = result.iter();
    assert_equal(intersections, [1,3]);
}


#[test]
fn reduce_or_test(){
    type HiSparseBitset = super::BitSet<config::_64bit>;

    const BLOCK_SIZE: usize = 64;
    const LEVEL_0: usize = BLOCK_SIZE*BLOCK_SIZE;

    // Different level 0
    {
        let set1_offset = LEVEL_0 * 1;
        let hi_set1_in = [set1_offset + BLOCK_SIZE * 1, set1_offset + BLOCK_SIZE * 2];
        let hi_set1: HiSparseBitset = hi_set1_in.clone().into_iter().collect();

        let set2_offset = LEVEL_0 * 2;
        let hi_set2_in = [set2_offset + BLOCK_SIZE * 1];
        let hi_set2: HiSparseBitset = hi_set2_in.clone().into_iter().collect();

        let hi_sets = [&hi_set1, &hi_set2];
        let union = reduce(BitOrOp, hi_sets.iter().copied()).unwrap();

        let mut out = Vec::new();
        for block in union.block_iter(){
            for i in block.iter(){
                out.push(i);
                println!("{:}", i);
            }
        }
        out.sort();

        let mut etalon: Vec<_> = hi_set1_in.into_iter().collect();
        etalon.extend_from_slice(hi_set2_in.as_slice());
        etalon.sort();

        assert_equal(out, etalon);
    }
}

#[test]
fn op_or_regression_test1(){
    type HiSparseBitset = crate::BitSet<crate::config::_64bit>;
    let h1 = HiSparseBitset::from([0]);
    let h2 = HiSparseBitset::from([0]);
    let h3 = HiSparseBitset::from([4096]);
    let h4 = HiSparseBitset::from([4096]);

    let group1 = [&h1, &h2];
    let group2 = [&h3, &h4];
    let reduce1 = reduce(BitOrOp, group1.iter().copied()).unwrap();
    let reduce2 = reduce(BitOrOp, group2.iter().copied()).unwrap();

    let op = reduce1 | reduce2;
    let iter = op.block_iter();
    assert_eq!(iter.count(), 2);
}

#[test]
fn reduce_xor_test(){
    type HiSparseBitset = super::BitSet<config::_64bit>;

    const BLOCK_SIZE: usize = 64;
    const LEVEL_0: usize = BLOCK_SIZE*BLOCK_SIZE;

    // Different level 0
    {
        let set1_offset = LEVEL_0 * 1;
        let h1e1 = set1_offset + BLOCK_SIZE * 1;
        let h1e2 = set1_offset + BLOCK_SIZE * 2;

        let hi_set1_in = [h1e1, h1e2];
        let hi_set1: HiSparseBitset = hi_set1_in.clone().into_iter().collect();

        let set2_offset = LEVEL_0 * 2;
        let h2e1 = set2_offset + BLOCK_SIZE * 1;
        let hi_set2_in = [h2e1, h1e2];
        let hi_set2: HiSparseBitset = hi_set2_in.clone().into_iter().collect();

        let hi_sets = [&hi_set1, &hi_set2];
        let reduce = reduce(BitXorOp, hi_sets.iter().copied()).unwrap();

        let mut out = Vec::new();
        for block in reduce.block_iter(){
            for i in block.iter(){
                out.push(i);
                println!("{:}", i);
            }
        }
        out.sort();

        let mut etalon = vec![h1e1, h2e1];
        etalon.sort();

        assert_equal(out, etalon);
    }
}

#[test]
fn multilayer_test(){
    let seq1 = [1,2,3];
    let hi_sets1 = [
        HiSparseBitset::from_iter(seq1.into_iter()),
        HiSparseBitset::from_iter(seq1.into_iter()),
        HiSparseBitset::from_iter(seq1.into_iter()),
    ];
    let and1 = reduce(BitAndOp, hi_sets1.iter()).unwrap();

    let seq2 = [3,4,5];
    let hi_sets2 = [
        HiSparseBitset::from_iter(seq2.into_iter()),
        HiSparseBitset::from_iter(seq2.into_iter()),
        HiSparseBitset::from_iter(seq2.into_iter()),
    ];
    let and2 = reduce(BitAndOp, hi_sets2.iter()).unwrap();

    let seq3 = [5,6,7];
    let hi_sets3 = [
        HiSparseBitset::from_iter(seq3.into_iter()),
        HiSparseBitset::from_iter(seq3.into_iter()),
        HiSparseBitset::from_iter(seq3.into_iter()),
    ];
    let and3 = reduce(BitAndOp, hi_sets3.iter()).unwrap();

    let ands = [and1, and2, and3];
    let or = reduce(BitOrOp, ands.iter()).unwrap();
    let or_collected: Vec<_> = or.block_iter().flat_map(|block|block.iter()).collect();

    assert_equal(or_collected, [1,2,3,4,5,6,7]);
}

#[test]
fn multilayer_or_test(){
    type HiSparseBitset = super::BitSet<config::_64bit>;

    const BLOCK_SIZE: usize = 64;
    const LEVEL_1: usize = BLOCK_SIZE;

    let sets1 = [
        HiSparseBitset::from([1,2,3]),
        HiSparseBitset::from([3,4,5]),
    ];
    let or1 = reduce(BitOrOp, sets1.iter()).unwrap();

    let offset = LEVEL_1*2;
    let sets2 = [
        HiSparseBitset::from([offset+1,offset+2,offset+3]),
        HiSparseBitset::from([offset+3,offset+4,offset+5]),
    ];
    let or2 = reduce(BitOrOp, sets2.iter()).unwrap();

    let higher_kind = [or1, or2];
    let higher_kind_or = reduce(BitOrOp, higher_kind.iter()).unwrap();

    let or_collected: Vec<_> = higher_kind_or.block_iter().flat_map(|block|block.iter()).collect();
    assert_equal(or_collected, [1,2,3,4,5, offset+1,offset+2,offset+3,offset+4,offset+5]);
}

#[test]
fn op_or_test(){
    let seq1: HiSparseBitset = [1,2,3].into();
    let seq2: HiSparseBitset = [3,4,5].into();
    let seq3: HiSparseBitset = [5,6,7].into();

    let or = &seq1 | &seq2 | &seq3;
    let or_collected: Vec<_> = or.block_iter().flat_map(|block|block.iter()).collect();
    assert_equal(or_collected, [1,2,3,4,5,6,7]);
}

#[test]
fn multilayer_fixed_dynamic_cache(){
    let seq1: HiSparseBitset = [1,2,3].into();
    let seq2: HiSparseBitset = [3,4,5].into();
    let seq3: HiSparseBitset = [5,6,7].into();
    let seq4: HiSparseBitset = [7,8,9].into();

    let group1 = [seq1, seq2];
    let group2 = [seq3, seq4];
    let or1 = reduce_w_cache(BitOrOp, group1.iter(), DynamicCache).unwrap();
    let or2 = reduce_w_cache(BitOrOp, group2.iter(), DynamicCache).unwrap();

    let group_finale = [or1, or2];
    let and = reduce_w_cache(BitAndOp, group_finale.iter(), FixedCache::<32>).unwrap();

    assert_equal(and.iter(), [5]);
}

#[test]
fn block_cursor_test(){
    let seq: HiSparseBitset = [1000, 2000, 3000, 4000, 5000, 6000].into();
    let mut iter = seq.block_iter();

    iter.next();
    iter.next();
    assert_equal(iter.next().unwrap().iter(), [3000]);

    let c = iter.cursor();

    let mut iter = seq.block_iter().move_to(c);
    assert_equal(iter.next().unwrap().iter(), [4000]);
}

#[test]
fn block_cursor_test2(){
    type HiSparseBitset = BitSet<config::_64bit>;
    let seq: HiSparseBitset = [0, 64, 128, 192, 256].into();
    let mut iter = seq.block_iter();

    iter.next();
    iter.next();
    assert_equal(iter.next().unwrap().iter(), [128]);

    let c = iter.cursor();

    let mut iter = seq.block_iter().move_to(c);
    assert_equal(iter.next().unwrap().iter(), [192]);
}

#[test]
fn block_cursor_test_empty(){
    let seq: HiSparseBitset = Default::default();

    let iter = seq.block_iter();
    let c = iter.cursor();

    let mut iter = seq.block_iter().move_to(c);
    assert!(iter.next().is_none());
}

#[test]
fn block_cursor_test_empty2(){
    let seq: HiSparseBitset = Default::default();
    let mut iter = seq.block_iter();

    iter.next();
    iter.next();

    let c = iter.cursor();

    let mut iter = seq.block_iter().move_to(c);
    assert!(iter.next().is_none());
}

#[test]
fn index_cursor_test(){
    type HiSparseBitset = BitSet<config::_64bit>;
    let seq: HiSparseBitset = (0..4096*4).collect();
    
    let mut iter = seq.iter();
    assert_equal(iter.by_ref().take(4096*3), 0..4096*3);
    let c = iter.cursor();
    
    let mut iter = seq.iter().move_to(c);
    assert_equal(iter.by_ref().take(4096), 4096*3..4096*4);
}

#[test]
fn index_cursor_test2(){
    type HiSparseBitset = BitSet<config::_64bit>;
    let seq: HiSparseBitset = (0..4096*4).collect();
    
    let mut iter = seq.iter();
    let milestone = 4096*3 - 64;
    assert_equal(iter.by_ref().take(milestone), 0..milestone);
    let c = iter.cursor();
    
    let mut iter = seq.iter().move_to(c);
    assert_eq!(iter.next().unwrap(), milestone);
}

#[test]
fn empty_block_cursor_clone_regression() {
    let set = HiSparseBitset::new();
    let c = IndexCursor::end();
    let i = set.iter().move_to(c);
    let _ = i.clone();
}