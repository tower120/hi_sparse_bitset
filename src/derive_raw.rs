//! New type idiom wrapping for RawBitSet.

/// * `$t` Must be Self(RawBitSet)
/// * `$t` Must implement BitSetBase
macro_rules! derive_raw {
    (impl <$($generics:tt),*>
        $t:ty as 
        $raw:ty     
        where $($where_bounds:tt)*
    ) => {
        impl<$($generics),*> $t
        where
            $($where_bounds)*
        {
            #[inline]
            pub fn new() -> Self {
                Default::default()
            }
            
            /// Max usize, bitset with this `Conf` can hold.
            #[inline]
            pub const fn max_capacity() -> usize {
                <$raw>::max_capacity()
            }
            
            /// # Safety
            ///
            /// Will panic, if `index` is out of range.    
            #[inline]
            pub fn insert(&mut self, index: usize){
                self.0.insert(index)
            }
            
            /// Returns false if index is invalid/not in bitset.
            #[inline]
            pub fn remove(&mut self, index: usize) -> bool {
                self.0.remove(index)
            }
            
            /// # Safety
            ///
            /// `index` MUST exists in HiSparseBitset!
            #[inline]
            pub unsafe fn remove_unchecked(&mut self, index: usize) {
                // TODO: make sure compiler actually get rid of unused code.
                let ok = self.remove(index);
                unsafe{ $crate::assume!(ok); }
            }
        }
        
        impl<$($generics),*> Clone for $t
        where
            $($where_bounds)*
        {
            #[inline]
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }
        
        impl<$($generics),*> Default for $t
        where
            $($where_bounds)*
        {
            #[inline]
            fn default() -> Self{
                Self(Default::default())
            }
        }
    
        impl<$($generics),*> FromIterator<usize> for $t
        where
            $($where_bounds)*
        {
            #[inline]
            fn from_iter<T: IntoIterator<Item=usize>>(iter: T) -> Self {
                Self(<$raw>::from_iter(iter))
            }
        }
        
        impl<$($generics),* , const N: usize> From<[usize; N]> for $t
        where
            $($where_bounds)*
        {
            #[inline]
            fn from(value: [usize; N]) -> Self {
                Self(<$raw>::from(value))
            }
        }
        
        /// This is fastest possible way of materializing lazy bitsets
        /// into BitSet.
        impl<$($generics),* , B> From<B> for $t
        where
            B: crate::BitSetInterface<Conf = <Self as BitSetBase>::Conf>,
            $($where_bounds)*
        {
            #[inline]
            fn from(bitset: B) -> Self {
                Self(<$raw>::from(bitset))
            }
        }
        
        crate::derive_raw::derive_raw_levelmasks!(
            impl<$($generics),*> $t as $raw where $($where_bounds)*  
        );
        
        crate::internals::impl_bitset!(impl<$($generics),*> for ref $t where $($where_bounds)*);        
    }
}
pub(crate) use derive_raw;


/// * `$t` Must be Self(RawBitSet)
/// * `$t` Must implement BitSetBase
macro_rules! derive_raw_levelmasks {
    (impl <$($generics:tt),*>
        $t:ty as 
        $raw:ty     
        where $($where_bounds:tt)*
    ) => {
        impl<$($generics),*> $crate::internals::LevelMasks for $t
        where
            $($where_bounds)*
        {
            #[inline]
            fn level0_mask(&self) -> <Self::Conf as Config>::Level0BitBlock {
                self.0.level0_mask()
            }
        
            #[inline]
            unsafe fn level1_mask(&self, level0_index: usize) -> <Self::Conf as Config>::Level1BitBlock {
                self.0.level1_mask(level0_index)
            }
        
            #[inline]
            unsafe fn data_mask(&self, level0_index: usize, level1_index: usize) -> <Self::Conf as Config>::DataBitBlock {
                self.0.data_mask(level0_index, level1_index)
            }            
        }
        
        impl<$($generics),*> $crate::internals::LevelMasksIterExt for $t
        where
            $($where_bounds)*
        {
            type IterState = <$raw as $crate::internals::LevelMasksIterExt>::IterState;
            type Level1BlockData = <$raw as $crate::internals::LevelMasksIterExt>::Level1BlockData;
            
            #[inline]
            fn make_iter_state(&self) -> Self::IterState {
                self.0.make_iter_state()
            }
        
            #[inline]
            unsafe fn drop_iter_state(&self, state: &mut std::mem::ManuallyDrop<Self::IterState>) {
                self.0.drop_iter_state(state)
            }
        
            #[inline]
            unsafe fn init_level1_block_data(
                &self, 
                state: &mut Self::IterState, 
                level1_block_data: &mut std::mem::MaybeUninit<Self::Level1BlockData>, 
                level0_index: usize
            ) -> (<Self::Conf as Config>::Level1BitBlock, bool) {
                self.0.init_level1_block_data(state, level1_block_data, level0_index)
            }
        
            #[inline]
            unsafe fn data_mask_from_block_data(
                level1_block_data: &Self::Level1BlockData, 
                level1_index: usize
            ) -> <Self::Conf as Config>::DataBitBlock {
                <$raw>::data_mask_from_block_data(level1_block_data, level1_index)
            }            
        }        
    }    
}
pub(crate) use derive_raw_levelmasks;
use crate::BitSetInterface;