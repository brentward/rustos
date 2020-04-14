use core::fmt;

use alloc::string::String;

use crate::traits::{self, Metadata as MetadataTrait, Timestamp as TimestampTrait};

/// A date as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(u16);

impl From<u16> for Date {
    fn from(raw_num: u16) -> Date {
        Date(raw_num)
    }
}

/// Time as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(u16);

impl From<u16> for Time {
    fn from(raw_num: u16) -> Time {
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
}

impl From<u8> for Attributes {
    fn from(raw_num: u8) -> Attributes {
        Attributes(raw_num)
    }
}

/// A structure containing a date and time.
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
/// Metadata for a directory entry.
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

impl TimestampTrait for Timestamp {
    /// The calendar year.
    ///
    /// The year is not offset. 2009 is 2009.
    fn year(&self) -> usize {
        (self.date.0 as usize >> 9) + 1980
    }
    fn set_year(&mut self, year: usize) {
        self.date.0 &= !(0x7Fu16 << 9);
        self.date.0 |= (year as u16 - 1980) << 9;
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
    fn set_month(&mut self, month: u8) {
        if month < 1 || month > 12 {
            panic!("Metadata::set_month() month out of range of 1 to 12")
        }
        self.date.0 &= !(0xFu16 << 5);
        self.date.0 |= (month as u16) << 5;
    }

    /// The calendar day, starting at 1. Always in range [1, 31].
    fn day(&self) -> u8 {
        let day = (self.date.0 & !0b1111_1111_1110_0000) as u8;
        match day {
            day@ 0..=31 => day,
            _ => panic!("metadata day is out of range")
        }
    }
    fn set_day(&mut self, day: u8) {
        if day < 1 || day > 31 {
            panic!("Metadata::set_day() day out of range of 1 to 31")
        }
        self.date.0 &= !0x1Fu16;
        self.date.0 |= day as u16;

    }

    /// The 24-hour hour. Always in range [0, 24).
    fn hour(&self) -> u8 {
        let time = (self.time.0 >> 11) as u8;
        match time {
            time @ 0..=24 => time,
            _ => panic!("metadata hour is out of range")
        }
    }
    fn set_hour(&mut self, hour: u8) {
        if hour > 24 {
            panic!("Metadata::set_hour() hour out of range of 0 to 24")
        }
        self.time.0 &= !(0x1Fu16 << 11);
        self.time.0 |= (hour as u16) << 11;

    }

    /// The minute. Always in range [0, 60).
    fn minute(&self) -> u8 {
        let minute = ((self.time.0 & !0b1111_1000_0001_1111) >> 5) as u8;
        match minute {
            minute @ 0..=60 => minute,
            _ => panic!("metadata minute is out of range")
        }
    }
    fn set_minute(&mut self, minute: u8) {
        if minute > 60 {
            panic!("Metadata::set_minute() hour out of range of 0 to 60")
        }
        self.time.0 &= !(0x3Fu16 << 5);
        self.time.0 |= (minute as u16) << 5;

    }

    /// The second. Always in range [0, 60).
    fn second(&self) -> u8 {
        let second = (self.time.0 & !0b1111_1111_1110_0000) as u8 * 2;
        match second {
            second @ 0..=60 => second,
            _ => panic!("metadata second is out of range")
        }
    }
    fn set_second(&mut self, second: u8) {
        if second > 60 {
            panic!("Metadata::set_second() second out of range of 0 to 60")
        }
        self.time.0 &= !0x1Fu16;
        self.time.0 |= second as u16 / 2;

    }
}

impl Metadata {
    fn read_only(&self) -> bool {
        (self.attributes.0 & 0x01) !=0
    }

    pub fn set_read_only(&mut self) {
        self.attributes.0 |= 0x01;
    }

    pub fn clear_read_only(&mut self) {
        self.attributes.0 &= !0x01;
    }

    /// Whether the entry should be "hidden" from directory traversals.
    fn hidden(&self) -> bool {
        ((self.attributes.0 & 0x02) >> 1) != 0
    }

    pub fn set_hidden(&mut self) {
        self.attributes.0 |= 0x02;
    }

    pub fn clear_hidden(&mut self) {
        self.attributes.0 &= !0x02;
    }

    /// Whether the entry is marked as system.
    fn system(&self) -> bool {
        ((self.attributes.0 & 0x04) >> 2) != 0
    }

    pub fn set_system(&mut self) {
        self.attributes.0 |= 0x04;
    }

    pub fn clear_system(&mut self) {
        self.attributes.0 &= !0x04;
    }

    /// Whether the entry is a volume ID.
    pub fn volume_id(&self) -> bool {
        ((self.attributes.0 & 0x08) >> 3) != 0
    }

    pub fn set_volume_id(&mut self) {
        self.attributes.0 |= 0x08;
    }

    pub fn clear_volume_id(&mut self) {
        self.attributes.0 &= !0x08;
    }

    /// Whether the entry is a directory.
    pub fn directory(&self) -> bool {
        ((self.attributes.0 & 0x10) >> 4) != 0
    }

    pub fn set_directory(&mut self) {
        self.attributes.0 |= 0x10;
    }

    pub fn clear_directory(&mut self) {
        self.attributes.0 &= !0x10;
    }

    /// Whether the entry is marked archive.
    fn archive(&self) -> bool {
        ((self.attributes.0 & 0x20) >> 5) != 0
    }

    pub fn set_archive(&mut self) {
        self.attributes.0 |= 0x20;
    }

    pub fn clear_archive(&mut self) {
        self.attributes.0 &= !0x20;
    }

    /// Set modified timestamp to provided values
    pub fn set_modified_timestamp(
        &mut self,
        year: usize,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8) {
        self.modification_timestamp.set_year(year);
        self.modification_timestamp.set_month(month);
        self.modification_timestamp.set_day(day);
        self.modification_timestamp.set_hour(hour);
        self.modification_timestamp.set_minute(minute);
        self.modification_timestamp.set_second(second);
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
        self.creation_timestamp
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
        write!(
            f,
            "  {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.created().year(),
            self.created().month(),
            self.created().day(),
            self.created().hour(),
            self.created().minute(),
            self.created().second()
        )
        ?;
        write!(
            f,
            "  {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            self.modified().year(),
            self.modified().month(),
            self.modified().day(),
            self.modified().hour(),
            self.modified().minute(),
            self.modified().second()
        )
    }
}
