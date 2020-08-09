use crate::*;

// #[derive(Clone, Copy, Debug)]
// pub struct SocketDescriptor(u64);
//
// impl SocketDescriptor {
//     pub fn raw(&self) -> u64 {
//         self.0
//     }
// }
//
// impl From<u64> for SocketDescriptor {
//     fn from(raw: u64) -> Self {
//         SocketDescriptor(raw)
//     }
// }

#[derive(Debug)]
pub struct SocketStatus {
    pub is_active: bool,
    pub is_listening: bool,
    pub can_send: bool,
    pub can_recv: bool,
}

pub struct IpAddr {
    pub ip: u32,
    pub port: u16,
}

impl IpAddr {
    pub fn new((ip1, ip2, ip3, ip4): (u8, u8, u8, u8), port: u16) -> Self {
        IpAddr {
            ip: u32::from_be_bytes([ip1, ip2, ip3, ip4]),
            port,
        }
    }
}

impl fmt::Debug for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.ip.to_be_bytes();
        write!(
            f,
            "IpAddr({}.{}.{}.{}:{})",
            bytes[0], bytes[1], bytes[2], bytes[3], self.port
        )
    }
}
