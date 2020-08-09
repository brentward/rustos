use core::str;

use crate::*;

pub enum Handle {
    StdIn,
    StdOut,
    StdErr,
    File(HandleDescriptor),
    Socket(HandleDescriptor),
}

impl Handle {
    pub fn raw(&self) -> u64{
        match self {
            Handle::StdIn => 0,
            Handle::StdOut => 1,
            Handle::StdErr => 2,
            Handle::File(hd) => hd.raw(),
            Handle::Socket(hd) => hd.raw(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HandleDescriptor(u64);

impl HandleDescriptor {
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl From<u64> for HandleDescriptor {
    fn from(raw: u64) -> Self {
        HandleDescriptor(raw)
    }
}

impl From<Handle> for HandleDescriptor {
    fn from(handle: Handle) -> Self {
        match handle {
            Handle::StdIn => HandleDescriptor::from(0),
            Handle::StdOut => HandleDescriptor::from(1),
            Handle::StdErr => HandleDescriptor::from(2),
            Handle::File(hd) => hd,
            Handle::Socket(hd) => hd,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DirEnt {
    d_type: DirType,
    name: [u8; 512],
    name_len: u64,
}

#[derive(Copy, Clone, Debug)]
pub enum DirType {
    File,
    Dir,
    None,
}

impl DirEnt {
    pub fn new(d_type: DirType, name: &str) -> DirEnt {
        let name_len = name.len();
        let name_bytes = name.as_bytes();
        let mut name = [0u8; 512];
        name[..name_len as usize].clone_from_slice(name_bytes);

        DirEnt {
            d_type,
            name,
            name_len: name_len as u64,
        }
    }

    pub fn name(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.name[..self.name_len as usize])
    }

    pub fn set_name(&mut self, name: &str) -> Result<(), ()> {
        let name_len = name.len();
        if name_len > 512 {
            Err(())
        } else {
            let name_bytes = name.as_bytes();
            let mut name = [0u8; 512];
            name[..name_len].clone_from_slice(name_bytes);
            self.name.clone_from_slice(&name);
            self.name_len = name_len as u64;
            Ok(())
        }
    }

    pub fn d_type(&self) -> &DirType {
        &self.d_type
    }

    pub fn set_d_type(&mut self, d_type: DirType) {
        self.d_type = d_type;
    }
}

impl Default for DirEnt {
    fn default() -> Self {
        DirEnt {
            d_type: DirType::None,
            name: [0; 512],
            name_len: 0,
        }
    }
}

#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(u16);

impl From<u16> for Date {
    fn from(raw_num: u16) -> Date {
        Date(raw_num)
    }
}

#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(u16);

impl From<u16> for Time {
    fn from(raw_num: u16) -> Time {
        Time(raw_num)
    }
}

#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Attributes(u8);

impl Attributes {
    pub fn value(&self) -> u8 {
        self.0
    }
}

impl From<u8> for Attributes {
    fn from(raw_num: u8) -> Attributes {
        Attributes(raw_num)
    }
}

#[repr(C, packed)]
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub date: Date,
    pub time: Time,
}

impl From<(u16, u16)> for Timestamp {
    fn from(date_time: (u16, u16)) -> Timestamp {
        Timestamp {
            date: Date::from(date_time.0),
            time: Time::from(date_time.1),
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct Metadata {
    attributes: Attributes,
    creation_timestamp: Timestamp,
    accessed_date: Date,
    modification_timestamp: Timestamp,
}

impl From<((u8, [u16; 5]))> for Metadata {
    fn from(metadata_tup: (u8, [u16; 5]))-> Metadata {
        Metadata {
            attributes: Attributes::from(metadata_tup.0),
            creation_timestamp: Timestamp::from((metadata_tup.1[0], metadata_tup.1[1])),
            accessed_date: Date::from(metadata_tup.1[2]),
            modification_timestamp: Timestamp::from((metadata_tup.1[3], metadata_tup.1[4])),
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct Stat {
    metadata: Metadata,
    size: u64,
}

impl Timestamp {
    /// The calendar year.
    ///
    /// The year is not offset. 2009 is 2009.
    fn year(&self) -> usize {
        (self.date.0 as usize >> 9) + 1980
    }

    /// The calendar month, starting at 1 for January. Always in range [1, 12].
    ///
    /// January is 1, Feburary is 2, ..., December is 12.
    fn month(&self) -> u8 {
        let month = (self.date.0 & !0b1111_1110_0001_1111) as u8 >> 5;
        match month {
            month @ 0..=12 => month,
            _ => panic!("metadata month is out of range")
        }
    }

    /// The calendar day, starting at 1. Always in range [1, 31].
    fn day(&self) -> u8 {
        let day = (self.date.0 & !0b1111_1111_1110_0000) as u8;
        match day {
            day@ 0..=31 => day,
            _ => panic!("metadata day is out of range")
        }
    }

    /// The 24-hour hour. Always in range [0, 24).
    fn hour(&self) -> u8 {
        let time = (self.time.0 >> 11) as u8;
        match time {
            time @ 0..=24 => time,
            _ => panic!("metadata hour is out of range")
        }
    }

    /// The minute. Always in range [0, 60).
    fn minute(&self) -> u8 {
        let minute = ((self.time.0 & !0b1111_1000_0001_1111) >> 5) as u8;
        match minute {
            minute @ 0..=60 => minute,
            _ => panic!("metadata minute is out of range")
        }
    }

    /// The second. Always in range [0, 60).
    fn second(&self) -> u8 {
        let second = (self.time.0 & !0b1111_1111_1110_0000) as u8 * 2;
        match second {
            second @ 0..=60 => second,
            _ => panic!("metadata second is out of range")
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use fmt::Write;
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.year(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}


impl Metadata {
    pub fn read_only(&self) -> bool {
        (self.attributes.0 & 0x01) !=0
    }

    /// Whether the entry should be "hidden" from directory traversals.
    pub fn hidden(&self) -> bool {
        ((self.attributes.0 & 0x02) >> 1) != 0
    }

    /// Whether the entry is marked as system.
    pub fn system(&self) -> bool {
        ((self.attributes.0 & 0x04) >> 2) != 0
    }

    /// Whether the entry is a volume ID.
    pub fn volume_id(&self) -> bool {
        ((self.attributes.0 & 0x08) >> 3) != 0
    }

    /// Whether the entry is a directory.
    pub fn directory(&self) -> bool {
        ((self.attributes.0 & 0x10) >> 4) != 0
    }

    /// Whether the entry is marked archive.
    pub fn archive(&self) -> bool {
        ((self.attributes.0 & 0x20) >> 5) != 0
    }

    /// The timestamp when the entry was created.
    pub fn created(&self) -> Timestamp {
        self.creation_timestamp
    }

    /// The timestamp for the entry's last access.
    pub fn accessed(&self) -> Timestamp {
        Timestamp {
            time: Time(0),
            date: self.accessed_date,
        }
    }

    /// The timestamp for the entry's last modification.
    pub fn modified(&self) -> Timestamp {
        self.modification_timestamp
    }
}

impl Stat {
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn metadata(&self) -> Metadata {
        self.metadata
    }

    pub fn set_metadata_from_raw(&mut self, raw: (u8, [u16; 5])) {
        self.metadata = Metadata::from(raw)
    }

    pub fn set_size(&mut self, raw: u64) {
        self.size = raw
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use fmt::Write;

        fn write_bool(to: &mut fmt::Formatter, b: bool, c: char) -> fmt::Result {
            if b {
                write!(to, "{}", c)
            } else {
                write!(to, "-")
            }
        }

        write_bool(f, self.directory(), 'd')?;
        write_bool(f, !self.directory(), 'f')?;
        write_bool(f, self.read_only(), 'r')?;
        write_bool(f, self.hidden(), 'h')?;
        write_bool(f, self.system(), 's')?;
        write_bool(f, self.volume_id(), 'i')?;
        write_bool(f, self.archive(), 'a')?;
        write!(f, "  {}", self.created())?;
        write!(f, "  {}", self.modified())
    }
}
