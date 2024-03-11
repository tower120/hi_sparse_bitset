use crate::config::Config;
use crate::block::Block;
use crate::derive_raw::derive_raw;
use crate::{BitSetBase, raw};

type Level0Block<Conf> = Block<
    <Conf as Config>::Level0BitBlock, 
    <Conf as Config>::Level0BlockIndices
>;
type Level1Block<Conf> = Block<
    <Conf as Config>::Level1BitBlock,
    <Conf as Config>::Level1BlockIndices
>;
type LevelDataBlock<Conf> = Block<
    <Conf as Config>::DataBitBlock, [usize;0]
>;

type RawBitSet<Conf> = raw::RawBitSet<
    Conf,
    Level0Block<Conf>,
    Level1Block<Conf>,
    LevelDataBlock<Conf>
>;

/// Hierarchical sparse bitset.
///
/// Tri-level hierarchy. Highest uint it can hold
/// is [Level0BitBlock]::size() * [Level1BitBlock]::size() * [DataBitBlock]::size().
///
/// Only last level contains blocks of actual data. Empty(skipped) data blocks
/// are not allocated.
///
/// Structure optimized for intersection speed. 
/// _(Other inter-bitset operations are in fact fast too - but intersection has lowest algorithmic complexity.)_
/// Insert/remove/contains is fast O(1) too.
/// 
/// [Level0BitBlock]: crate::config::Config::Level0BitBlock
/// [Level1BitBlock]: crate::config::Config::Level1BitBlock
/// [DataBitBlock]: crate::config::Config::DataBitBlock
pub struct BitSet<Conf: Config>(
    RawBitSet<Conf>
);
impl<Conf: Config> BitSetBase for BitSet<Conf> {
    type Conf = Conf;
    const TRUSTED_HIERARCHY: bool = true;
}
derive_raw!(
    impl<Conf> BitSet<Conf> as RawBitSet<Conf> where Conf: Config  
);