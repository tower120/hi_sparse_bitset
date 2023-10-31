use std::{collections::HashSet, hash::Hash};
use std::iter::zip;

use itertools::assert_equal;
use rand::Rng;
use crate::binary_op::{BitOrOp, BitSubOp, BitXorOp};

use super::*;

cfg_if::cfg_if! {
    if #[cfg(hisparsebitset_test_64)] {
        type Config = configs::u64s;
    } else if #[cfg(hisparsebitset_test_128)] {
        type Config = configs::simd_128;
    } else {
        type Config = configs::simd_128;
    }
}

type HiSparseBitset = super::HiSparseBitset<Config>;
type IntersectionBlocksState = super::intersection_blocks_resumable::IntersectionBlocksState<Config>;

#[test]
fn level_indices_test(){
    // assuming all levels with 128bit blocks
    type HiSparseBitset = super::HiSparseBitset<configs::simd_128>;

    let levels = HiSparseBitset::level_indices(0);
    assert_eq!(levels, (0,0,0));

    let levels = HiSparseBitset::level_indices(10);
    assert_eq!(levels, (0,0,10));

    let levels = HiSparseBitset::level_indices(128);
    assert_eq!(levels, (0,1,0));

    let levels = HiSparseBitset::level_indices(130);
    assert_eq!(levels, (0,1,2));

    let levels = HiSparseBitset::level_indices(130);
    assert_eq!(levels, (0,1,2));

    let levels = HiSparseBitset::level_indices(128*128);
    assert_eq!(levels, (1,0,0));

    let levels = HiSparseBitset::level_indices(128*128 + 50*128);
    assert_eq!(levels, (1,50,0));

    let levels = HiSparseBitset::level_indices(128*128 + 50*128 + 4);
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
    const MAX_SIZE : usize = 10000;
    const MAX_RANGE: usize = 100000;
    const CONTAINS_PROBES: usize = 1000;

    let mut rng = rand::thread_rng();
    for _ in 0..100{
        let mut hash_set = HashSet::new();
        let mut hi_set = HiSparseBitset::default();

        let mut inserted = Vec::new();
        let mut removed = Vec::new();

        for _ in 0..10{
            // random insert
            for _ in 0..rng.gen_range(0..MAX_SIZE){
                let index = rng.gen_range(0..MAX_RANGE);
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
                let index = rng.gen_range(0..MAX_RANGE);
                let h1 = hash_set.contains(&index);
                let h2 = hi_set.contains(index);
                assert_eq!(h1, h2);
            }

            // existent contains
            for &index in &hash_set{
                assert!(hi_set.contains(index));
            }

            // non existent does not contains
            for &index in &removed{
                let h1 = hash_set.contains(&index);
                let h2 = hi_set.contains(index);
                assert_eq!(h1, h2);
            }
        }
    }
}

// TODO: refactor and remove
#[test]
fn fuzzy_intersection_test(){
    const MAX_SETS : usize = 10;
    const MAX_INSERTS: usize = 10000;
    const MAX_GUARANTEED_INTERSECTIONS: usize = 10;
    const MAX_REMOVES : usize = 10000;
    const MAX_RANGE: usize = 100000;
    const MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME: usize = 100;

    fn hashset_multi_intersection<'a, T: Eq + Hash + Copy + 'a>(hash_sets: impl IntoIterator<Item = &'a HashSet<T>>) -> HashSet<T>
    {
        let mut hash_sets_iter = hash_sets.into_iter();
        let mut acc = hash_sets_iter.next().unwrap().clone();
        for set in hash_sets_iter{
            let intersection = acc.intersection(set)
                .copied()
                .collect();
            acc = intersection;
        }
        acc
    }

    let mut rng = rand::thread_rng();
    for _ in 0..100{
        let sets_count = rng.gen_range(2..MAX_SETS);
        let mut hash_sets: Vec<HashSet<usize>> = vec![Default::default(); sets_count];
        let mut hi_sets  : Vec<HiSparseBitset> = vec![Default::default(); sets_count];

        // Resumable intersection guarantee that we'll traverse at least
        // non removed initial intersection set.

        // initial insert
        let mut intersection_state = IntersectionBlocksState::default();
        let mut initial_hashsets_intersection;
        {
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_INSERTS){
                    let index = rng.gen_range(0..MAX_RANGE);
                    hash_set.insert(index);
                    hi_set.insert(index);
                }
            }
            initial_hashsets_intersection = hashset_multi_intersection(&hash_sets);
        }

        for _ in 0..10{
            // random insert
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_INSERTS){
                    let index = rng.gen_range(0..MAX_RANGE);
                    hash_set.insert(index);
                    hi_set.insert(index);
                }
            }

            // guaranteed intersection (insert all)
            for _ in 0..rng.gen_range(0..MAX_GUARANTEED_INTERSECTIONS){
                let index = rng.gen_range(0..MAX_RANGE);
                for hash_set in &mut hash_sets{
                    hash_set.insert(index);
                }
                for hi_set in &mut hi_sets{
                    hi_set.insert(index);
                }
            }

            // random remove
            let mut removed = Vec::new();
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_REMOVES){
                    let index = rng.gen_range(0..MAX_RANGE);
                    hash_set.remove(&index);
                    hi_set.remove(index);
                    removed.push(index);
                }
            }

            // etalon intersection
            let hashsets_intersection = hashset_multi_intersection(&hash_sets);

            // remove non-existent intersections from initial_hashsets_intersection
            for index in &removed{
                if !hashsets_intersection.contains(index){
                    initial_hashsets_intersection.remove(index);
                }
            }

            // intersection resume
            {
                let mut intersection = intersection_state.resume(hi_sets.iter());
                let mut blocks_to_consume = rng.gen_range(0..MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME);

                // all intersections must be valid
                loop{
                    if blocks_to_consume == 0{
                        break;
                    }
                    blocks_to_consume -= 1;

                    if let Some(block) = intersection.next(){
                        block.traverse(
                            |index|{
                                assert!(hashsets_intersection.contains(&index));
                                initial_hashsets_intersection.remove(&index);
                                ControlFlow::Continue(())
                            }
                        );
                    } else {
                        break;
                    }
                }

                intersection_state = intersection.suspend();
            }

            // intersection
            {
                let mut hi_intersection = collect_intersection(&hi_sets);

                // check that intersection_blocks = intersection_blocks_traverse
                {
                    let mut indices2 = Vec::new();
                    for block in intersection_blocks(&hi_sets){
                        block.traverse(
                            |index|{
                                indices2.push(index);
                                ControlFlow::Continue(())
                            }
                        );
                    }
                    assert_eq!(hi_intersection, indices2);
                }

                {
                    let mut indices2 = Vec::new();
                    let state = IntersectionBlocksState::default();
                    for block in state.resume(hi_sets.iter()){
                        block.traverse(
                            |index|{
                                indices2.push(index);
                                ControlFlow::Continue(())
                            }
                        );
                    }

                    if hi_intersection != indices2{
                        println!("{:?}", hash_sets);
                        panic!();
                    }
                    //assert_eq!(hi_intersection, indices2);
                }

                // reduce test
                {
                    let mut indices2 = Vec::new();
                    for block in reduce_and2(hi_sets.iter()).iter(){
                        block.traverse(
                            |index|{
                                indices2.push(index);
                                ControlFlow::Continue(())
                            }
                        );
                    }
                    assert_eq!(hi_intersection, indices2);
                }

                // reduce ext test
                {
                    let mut indices2 = Vec::new();
                    for block in reduce_and2(hi_sets.iter()).iter_ext(){
                        block.traverse(
                            |index|{
                                indices2.push(index);
                                ControlFlow::Continue(())
                            }
                        );
                    }
                    assert_eq!(hi_intersection, indices2);
                }

                let mut hashsets_intersection: Vec<usize> = hashsets_intersection.into_iter().collect();
                hashsets_intersection.sort();
                hi_intersection.sort();
                assert_equal(hi_intersection, hashsets_intersection);
            }
        }

        // consume resumable intersection leftovers
        {
            let intersection = intersection_state.resume(hi_sets.iter());
            for block in intersection{
                block.traverse(
                    |index|{
                        initial_hashsets_intersection.remove(&index);
                        ControlFlow::Continue(())
                    }
                );
            }
        }
        // assert that we consumed all initial intersection set.
        assert!(initial_hashsets_intersection.is_empty());
    }
}

fn fuzzy_reduce_test<Op: BinaryOp, H>(hiset_op: Op, hashset_op: H, repeats: usize)
where
    H: Fn(&HashSet<usize>, &HashSet<usize>) -> HashSet<usize>,
    H: Copy
{
    const MAX_SETS : usize = 10;
    const MAX_INSERTS: usize = 10000;
    const MAX_GUARANTEED_INTERSECTIONS: usize = 10;
    const MAX_REMOVES : usize = 10000;
    const MAX_RANGE: usize = 100000;
    const MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME: usize = 100;

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
    for _ in 0..repeats{
        let sets_count = rng.gen_range(2..MAX_SETS);
        let mut hash_sets: Vec<HashSet<usize>> = vec![Default::default(); sets_count];
        let mut hi_sets  : Vec<HiSparseBitset> = vec![Default::default(); sets_count];

        // Resumable intersection guarantee that we'll traverse at least
        // non removed initial intersection set.

        // initial insert
        let mut intersection_state = IntersectionBlocksState::default();
        let mut initial_hashsets_intersection;
        {
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_INSERTS){
                    let index = rng.gen_range(0..MAX_RANGE);
                    hash_set.insert(index);
                    hi_set.insert(index);
                }
            }
            initial_hashsets_intersection = hashset_multi_op(&hash_sets, hashset_op);
        }

        for _ in 0..10{
            // random insert
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_INSERTS){
                    let index = rng.gen_range(0..MAX_RANGE);
                    hash_set.insert(index);
                    hi_set.insert(index);
                }
            }

            // guaranteed intersection (insert all)
            for _ in 0..rng.gen_range(0..MAX_GUARANTEED_INTERSECTIONS){
                let index = rng.gen_range(0..MAX_RANGE);
                for hash_set in &mut hash_sets{
                    hash_set.insert(index);
                }
                for hi_set in &mut hi_sets{
                    hi_set.insert(index);
                }
            }

            // random remove
            let mut removed = Vec::new();
            for (hash_set, hi_set) in zip(hash_sets.iter_mut(), hi_sets.iter_mut()){
                for _ in 0..rng.gen_range(0..MAX_REMOVES){
                    let index = rng.gen_range(0..MAX_RANGE);
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
            for index in &removed{
                if !hashsets_intersection.contains(index){
                    initial_hashsets_intersection.remove(index);
                }
            }

            // TODO
            /*// intersection resume
            {
                let mut intersection = intersection_state.resume(hi_sets.iter());
                let mut blocks_to_consume = rng.gen_range(0..MAX_RESUMED_INTERSECTION_BLOCKS_CONSUME);

                // all intersections must be valid
                loop{
                    if blocks_to_consume == 0{
                        break;
                    }
                    blocks_to_consume -= 1;

                    if let Some(block) = intersection.next(){
                        block.traverse(
                            |index|{
                                assert!(hashsets_intersection.contains(&index));
                                initial_hashsets_intersection.remove(&index);
                                ControlFlow::Continue(())
                            }
                        );
                    } else {
                        break;
                    }
                }

                intersection_state = intersection.suspend();
            }*/

            // intersection
            {
                /*let mut hi_intersection = collect_intersection(&hi_sets);

                // check that intersection_blocks = intersection_blocks_traverse
                {
                    let mut indices2 = Vec::new();
                    for block in intersection_blocks(&hi_sets){
                        block.traverse(
                            |index|{
                                indices2.push(index);
                                ControlFlow::Continue(())
                            }
                        );
                    }
                    assert_eq!(hi_intersection, indices2);
                }

                {
                    let mut indices2 = Vec::new();
                    let state = IntersectionBlocksState::default();
                    for block in state.resume(hi_sets.iter()){
                        block.traverse(
                            |index|{
                                indices2.push(index);
                                ControlFlow::Continue(())
                            }
                        );
                    }

                    if hi_intersection != indices2{
                        println!("{:?}", hash_sets);
                        panic!();
                    }
                    //assert_eq!(hi_intersection, indices2);
                }*/

                // reduce test
                {
                    let mut indices2 = Vec::new();
                    for block in reduce(hiset_op, hi_sets.iter()).iter(){
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

                // reduce ext test
                {
                    let mut indices2 = Vec::new();
                    for block in reduce(hiset_op, hi_sets.iter()).iter_ext(){
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

                // reduce ext2 test
                {
                    let mut indices2 = Vec::new();
                    for block in reduce(hiset_op, hi_sets.iter()).iter_ext2(){
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

                // reduce ext3 test
                {
                    let mut indices2 = Vec::new();
                    for block in reduce(hiset_op, hi_sets.iter()).iter_ext3(){
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

                /*let mut hashsets_intersection: Vec<usize> = hashsets_intersection.into_iter().collect();
                hashsets_intersection.sort();
                hi_intersection.sort();
                assert_equal(hi_intersection, hashsets_intersection);*/
            }
        }

        /*// consume resumable intersection leftovers
        {
            let intersection = intersection_state.resume(hi_sets.iter());
            for block in intersection{
                block.traverse(
                    |index|{
                        initial_hashsets_intersection.remove(&index);
                        ControlFlow::Continue(())
                    }
                );
            }
        }
        // assert that we consumed all initial intersection set.
        assert!(initial_hashsets_intersection.is_empty());*/
    }
}

#[test]
fn fuzzy_and_test(){
    fuzzy_reduce_test(BitAndOp, |l,r| l&r, 100);
}

#[test]
fn fuzzy_or_test(){
    fuzzy_reduce_test(BitOrOp, |l,r| l|r, 30);
}

#[test]
fn fuzzy_xor_test(){
    fuzzy_reduce_test(BitXorOp, |l,r| l^r, 30);
}

// Sub, probably, should not be used with reduce. But for test it will work.
#[test]
fn fuzzy_sub_test(){
    fuzzy_reduce_test(BitSubOp, |l,r| l-r, 30);
}

#[test]
fn empty_intersection_test(){
    let state = IntersectionBlocksState::default();
    let mut iter = state.resume(std::iter::empty());
    let next = iter.next();
    assert!(next.is_none());
}

#[test]
fn one_intersection_test(){
    let mut hi_set = HiSparseBitset::default();
    hi_set.insert(0);
    hi_set.insert(12300);
    hi_set.insert(8760);
    hi_set.insert(521);

    let state = IntersectionBlocksState::default();
    let mut iter = state.resume([&hi_set].into_iter());

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
    let mut sets_data = vec![
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
        let state = IntersectionBlocksState::default();
        let iter = state.resume(hi_sets.iter());
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
    let mut hi_set1: HiSparseBitset = [1,2,3].into_iter().collect();
    let mut hi_set2: HiSparseBitset = [1,2,3].into_iter().collect();
    let mut hi_set3: HiSparseBitset = [1,3].into_iter().collect();

    let hi_sets = [hi_set1, hi_set2, hi_set3];
    let hi_set_refs = [&hi_sets[0], &hi_sets[1], &hi_sets[2]];

    let result = reduce_and2(hi_sets.iter());
    let intersections = result.iter().flat_map(|block|block.iter());
    assert_equal(intersections, [1,3]);

    let result = reduce_and2(hi_set_refs.iter().copied());
    let intersections = result.iter().flat_map(|block|block.iter());
    assert_equal(intersections, [1,3]);
}


#[test]
fn reduce_or_test(){
    type HiSparseBitset = super::HiSparseBitset<configs::u64s>;

    const BLOCK_SIZE: usize = 64;
    const LEVEL_0: usize = BLOCK_SIZE*BLOCK_SIZE;
    const LEVEL_1: usize = BLOCK_SIZE;

    // Different level 0
    {
        let set1_offset = LEVEL_0 * 1;
        let hi_set1_in = [set1_offset + BLOCK_SIZE * 1, set1_offset + BLOCK_SIZE * 2];
        let hi_set1: HiSparseBitset = hi_set1_in.clone().into_iter().collect();

        let set2_offset = LEVEL_0 * 2;
        let hi_set2_in = [set2_offset + BLOCK_SIZE * 1];
        let hi_set2: HiSparseBitset = hi_set2_in.clone().into_iter().collect();

        let hi_sets = [&hi_set1, &hi_set2];
        let union = reduce(BitOrOp, hi_sets.iter().copied());

        let mut out = Vec::new();
        for block in union.iter/*_ext*/(){
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
fn reduce_xor_test(){
    type HiSparseBitset = super::HiSparseBitset<configs::u64s>;

    const BLOCK_SIZE: usize = 64;
    const LEVEL_0: usize = BLOCK_SIZE*BLOCK_SIZE;
    const LEVEL_1: usize = BLOCK_SIZE;

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
        let reduce = reduce(BitXorOp, hi_sets.iter().copied());

        let mut out = Vec::new();
        for block in reduce.iter_ext3(){
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
    let and1 = reduce(BitAndOp, hi_sets1.iter());

    let seq2 = [3,4,5];
    let hi_sets2 = [
        HiSparseBitset::from_iter(seq2.into_iter()),
        HiSparseBitset::from_iter(seq2.into_iter()),
        HiSparseBitset::from_iter(seq2.into_iter()),
    ];
    let and2 = reduce(BitAndOp, hi_sets2.iter());

    let seq3 = [5,6,7];
    let hi_sets3 = [
        HiSparseBitset::from_iter(seq3.into_iter()),
        HiSparseBitset::from_iter(seq3.into_iter()),
        HiSparseBitset::from_iter(seq3.into_iter()),
    ];
    let and3 = reduce(BitAndOp, hi_sets3.iter());

    let ands = [and1, and2, and3];
    let or = reduce(BitOrOp, ands.iter());
    let or_collected: Vec<_> = or.iter_ext3().flat_map(|block|block.iter()).collect();

    assert_equal(or_collected, [1,2,3,4,5,6,7]);
}

#[test]
fn multilayer_or_test(){
    type HiSparseBitset = super::HiSparseBitset<configs::u64s>;

    const BLOCK_SIZE: usize = 64;
    const LEVEL_0: usize = BLOCK_SIZE*BLOCK_SIZE;
    const LEVEL_1: usize = BLOCK_SIZE;

    let sets1 = [
        HiSparseBitset::from([1,2,3]),
        HiSparseBitset::from([3,4,5]),
    ];
    let or1 = reduce(BitOrOp, sets1.iter());

    let offset = LEVEL_1*2;
    let sets2 = [
        HiSparseBitset::from([offset+1,offset+2,offset+3]),
        HiSparseBitset::from([offset+3,offset+4,offset+5]),
    ];
    let or2 = reduce(BitOrOp, sets2.iter());

    let higher_kind = [or1, or2];
    let higher_kind_or = reduce(BitOrOp, higher_kind.iter());

    let or_collected: Vec<_> = higher_kind_or.iter_ext3().flat_map(|block|block.iter()).collect();
    assert_equal(or_collected, [1,2,3,4,5, offset+1,offset+2,offset+3,offset+4,offset+5]);
}

#[test]
fn op_or_test(){
    let seq1: HiSparseBitset = [1,2,3].into();
    let seq2: HiSparseBitset = [3,4,5].into();
    let seq3: HiSparseBitset = [5,6,7].into();

    let or = &seq1 | &seq2 | &seq3;
    let or_collected: Vec<_> = or.iter_ext3().flat_map(|block|block.iter()).collect();
    assert_equal(or_collected, [1,2,3,4,5,6,7]);

}
