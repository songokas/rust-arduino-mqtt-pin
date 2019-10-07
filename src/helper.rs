use chrono::{Local, DateTime};
use std::ops::Add;
use num::{ToPrimitive, Zero};

pub fn average<T>(numbers: &[T]) -> f32
    where T: Add<T, Output=T> + Copy + Zero + ToPrimitive
{
    if numbers.len() > 0 {
        return numbers.iter().fold(T::zero(), |sum, &value| sum + value).to_f32()
            .map(|n| n / numbers.len() as f32 ).unwrap_or(0_f32);
    }
    0_f32
}

pub fn more_resent_date(dt1: Option<DateTime<Local>>, dt2: Option<DateTime<Local>>) -> Option<DateTime<Local>>
{
    if let Some(d1) = dt1 {
        if let Some(d2) = dt2 {
            return if d1 < d2 { dt2 } else { dt1 };
        } else {
            return dt1;
        }
    }
    dt2
}

pub fn percent_to_analog(num: u8) -> u16
{
    if num >= 100 { 1023_u16 } else { (num as u32 * 1023_u32 / 100) as u16}
}


#[cfg(test)]
mod tests
{
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_percent_to_analog()
    {
        assert_eq!(percent_to_analog(100), 1023);
        assert_eq!(percent_to_analog(200), 1023);
        assert_eq!(percent_to_analog(0), 0);
        assert_eq!(percent_to_analog(50), 511);
    }

    #[test]
    fn test_average()
    {
        assert_eq!(average(&[2, 3, 7]), 4_f32);
        assert_eq!(average(&[0_u8]), 0_f32);
        let vec: Vec<u8> = Vec::new();
        assert_eq!(average(&vec), 0_f32);
        assert_eq!(average(&[3.35_f32, 1.45_f32]), 2.4_f32);
    }


    #[test]
    fn test_more_recent_date()
    {
        let dt1 = Some(Local::now());
        let dt2 = Some(Local::now() + Duration::seconds(3));
        let dt3 = Some(Local::now() - Duration::seconds(3));
        let dt4 = None;
        assert_eq!(more_resent_date(dt1, dt2), dt2);
        assert_eq!(more_resent_date(dt2, dt1), dt2);
        assert_eq!(more_resent_date(dt1, dt3), dt1);
        assert_eq!(more_resent_date(dt2, dt3), dt2);
        assert_eq!(more_resent_date(dt1, dt4), dt1);
        assert_eq!(more_resent_date(dt2, dt4), dt2);
        assert_eq!(more_resent_date(dt3, dt4), dt3);
        assert_eq!(more_resent_date(dt4, dt4), dt4);
        assert_eq!(more_resent_date(dt4, dt1), dt1);
    }
}
