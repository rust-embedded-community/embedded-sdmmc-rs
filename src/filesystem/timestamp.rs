/// Things that impl this can tell you the current time.
pub trait TimeSource {
    /// Returns the current time
    fn get_timestamp(&self) -> Timestamp;
}

/// Represents an instant in time, in the local time zone. TODO: Consider
/// replacing this with POSIX time as a `u32`, which would save two bytes at
/// the expense of some maths.
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct Timestamp {
    /// Add 1970 to this file to get the calendar year
    pub year_since_1970: u8,
    /// Add one to this value to get the calendar month
    pub zero_indexed_month: u8,
    /// Add one to this value to get the calendar day
    pub zero_indexed_day: u8,
    /// The number of hours past midnight
    pub hours: u8,
    /// The number of minutes past the hour
    pub minutes: u8,
    /// The number of seconds past the minute
    pub seconds: u8,
}

impl Timestamp {
    /// Create a `Timestamp` from the 16-bit FAT date and time fields.
    pub fn from_fat(date: u16, time: u16) -> Timestamp {
        let year = 1980 + (date >> 9);
        let month = ((date >> 5) & 0x000F) as u8;
        let day = (date & 0x001F) as u8;
        let hours = ((time >> 11) & 0x001F) as u8;
        let minutes = ((time >> 5) & 0x0003F) as u8;
        let seconds = ((time << 1) & 0x0003F) as u8;
        // Volume labels have a zero for month/day, so tolerate that...
        Timestamp {
            year_since_1970: (year - 1970) as u8,
            zero_indexed_month: if month == 0 { 0 } else { month - 1 },
            zero_indexed_day: if day == 0 { 0 } else { day - 1 },
            hours,
            minutes,
            seconds,
        }
    }

    // TODO add tests for the method
    /// Serialize a `Timestamp` to FAT format
    pub fn serialize_to_fat(self) -> [u8; 4] {
        let mut data = [0u8; 4];

        let hours = (u16::from(self.hours) << 11) & 0xF800;
        let minutes = (u16::from(self.minutes) << 5) & 0x07E0;
        let seconds = (u16::from(self.seconds / 2)) & 0x001F;
        data[..2].copy_from_slice(&(hours | minutes | seconds).to_le_bytes()[..]);

        let year = if self.year_since_1970 < 10 {
            0
        } else {
            (u16::from(self.year_since_1970 - 10) << 9) & 0xFE00
        };
        let month = (u16::from(self.zero_indexed_month + 1) << 5) & 0x01E0;
        let day = u16::from(self.zero_indexed_day + 1) & 0x001F;
        data[2..].copy_from_slice(&(year | month | day).to_le_bytes()[..]);
        data
    }

    /// Create a `Timestamp` from year/month/day/hour/minute/second.
    ///
    /// Values should be given as you'd write then (i.e. 1980, 01, 01, 13, 30,
    /// 05) is 1980-Jan-01, 1:30:05pm.
    pub fn from_calendar(
        year: u16,
        month: u8,
        day: u8,
        hours: u8,
        minutes: u8,
        seconds: u8,
    ) -> Result<Timestamp, &'static str> {
        Ok(Timestamp {
            year_since_1970: if (1970..=(1970 + 255)).contains(&year) {
                (year - 1970) as u8
            } else {
                return Err("Bad year");
            },
            zero_indexed_month: if (1..=12).contains(&month) {
                month - 1
            } else {
                return Err("Bad month");
            },
            zero_indexed_day: if (1..=31).contains(&day) {
                day - 1
            } else {
                return Err("Bad day");
            },
            hours: if hours <= 23 {
                hours
            } else {
                return Err("Bad hours");
            },
            minutes: if minutes <= 59 {
                minutes
            } else {
                return Err("Bad minutes");
            },
            seconds: if seconds <= 59 {
                seconds
            } else {
                return Err("Bad seconds");
            },
        })
    }
}

impl core::fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Timestamp({})", self)
    }
}

impl core::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{}-{:02}-{:02} {:02}:{:02}:{:02}",
            u16::from(self.year_since_1970) + 1970,
            self.zero_indexed_month + 1,
            self.zero_indexed_day + 1,
            self.hours,
            self.minutes,
            self.seconds
        )
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
