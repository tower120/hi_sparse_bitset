#[derive(Default, Copy, Clone)]
pub struct NoCache;

#[derive(Default, Copy, Clone)]
pub struct FixedCache<const N:usize>;

#[derive(Default, Copy, Clone)]
pub struct DynamicCache;