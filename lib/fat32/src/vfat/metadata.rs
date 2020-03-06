use core::fmt;

use alloc::string::String;

use crate::traits::{self, Metadata as MetadataTrait, Timestamp as TimestampTrait};

/// A date as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(u16);

impl Date {
    pub fn new(raw_num: u16) -> Date {
        Date(raw_num)
    }
}

/// Time as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(u16);

impl Time {
    pub fn new(raw_num: u16) -> Time {
        Time(raw_num)
    }
}

/// File attributes as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Attributes(u8);

impl Attributes {
    pub fn value(&self) -> u8 {
        self.0
    }

    pub fn new(raw_num: u8) -> Attributes {
        Attributes(raw_num)
    }
}

/// A structure containing a date and time.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub date: Date,
    pub time: Time,
}

impl Timestamp {
    pub fn new(date: Date, time: Time) -> Timestamp {
        Timestamp {
            date,
            time,
        }
    }
}
/// Metadata for a directory entry.
#[derive(Default, Debug, Copy, Clone)]
pub struct Metadata {
    attributes: Attributes,
    creation_timestamp: Timestamp,
    accessed_date: Date,
    modification_timestamp: Timestamp,
}

impl Metadata {
    pub fn new(
        attributes: Attributes,
        creation_timestamp: Timestamp,
        accessed_date: Date,
        modification_timestamp: Timestamp,
    ) -> Metadata {
        Metadata {
            attributes,
            creation_timestamp,
            accessed_date,
            modification_timestamp
        }
    }
}

impl TimestampTrait for Timestamp {
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
            month @ 1..=12 => month,
            _ => panic!("metadata month is out of range")
        }
    }

    /// The calendar day, starting at 1. Always in range [1, 31].
    fn day(&self) -> u8 {
        let day = (self.date.0 & !0b1111_1111_1110_0000) as u8;
        match day {
            day@ 1..=31 => day,
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
        let minute = ((self.time.0 & !0b1111_0000_0001_1111) >> 5) as u8;
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

impl Metadata {
    fn read_only(&self) -> bool {
        (self.attributes.0 & 0x01) !=0
    }

    /// Whether the entry should be "hidden" from directory traversals.
    fn hidden(&self) -> bool {
        ((self.attributes.0 & 0x02) >> 1) != 0
    }

    /// Whether the entry is marked as system.
    fn system(&self) -> bool {
        ((self.attributes.0 & 0x04) >> 2) != 0
    }

    /// Whether the entry is a volume ID.
    fn volume_id(&self) -> bool {
        ((self.attributes.0 & 0x08) >> 3) != 0
    }

    /// Whether the entry is a directory.
    fn directory(&self) -> bool {
        ((self.attributes.0 & 0x10) >> 4) != 0
    }

    /// Whether the entry is marked archive.
    fn archive(&self) -> bool {
        ((self.attributes.0 & 0x20) >> 5) != 0
    }

}

impl MetadataTrait for Metadata {
    type Timestamp = Timestamp;
    /// Whether the associated entry is read only.
    fn read_only(&self) -> bool {
        self.read_only()
    }

    /// Whether the entry should be "hidden" from directory traversals.
    fn hidden(&self) -> bool {
        self.hidden()
    }

    /// The timestamp when the entry was created.
    fn created(&self) -> Self::Timestamp {
        self.created()
    }

    /// The timestamp for the entry's last access.
    fn accessed(&self) -> Self::Timestamp {
        Timestamp {
            time: Time(0),
            date: self.accessed_date,
        }
    }

    /// The timestamp for the entry's last modification.
    fn modified(&self) -> Self::Timestamp {
        self.modification_timestamp
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let read_write = if self.read_only() { "r" } else { "w" };
        let hidden_visible = if self.hidden() { "h" } else { "v" };
        let system = if self.system() { "s" } else { "-" };
        let volume_id = if self.volume_id() { "i" } else { "-" };
        let filetype = if self.directory() { "d" } else { "f" };
        let archive = if self.archive() { "a" } else { "-" };
        write!(f, "{}{}{}{}{}{}", read_write, hidden_visible, system, volume_id, filetype, archive);
        write!(
            f,
            "  M{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.modified().year(),
            self.modified().month(),
            self.modified().day(),
            self.modified().hour(),
            self.modified().minute(),
            self.modified().second()
        );
        write!(
            f,
            "   C{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.created().year(),
            self.created().month(),
            self.created().day(),
            self.created().hour(),
            self.created().minute(),
            self.created().second()
        );
        Ok(())

    }
}
