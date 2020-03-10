#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone, Hash)]
pub struct Cluster(u32);

impl Cluster {
    pub fn fat_address(&self) -> u32 {
        self.0
    }

    pub fn data_address(&self) -> u32 {
        self.0 - 2
    }
}

impl From<u32> for Cluster {
    fn from(raw_num: u32) -> Cluster {
        Cluster(raw_num & !(0xF << 28))
    }
}
