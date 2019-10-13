use chrono::{Local, DateTime};
use mosquitto_client::{MosqMessage};
use arraydeque::{ArrayDeque, Wrapping};
use std::ops::Sub;
use yaml_rust::{Yaml};

use crate::helper::average;

#[derive(new, Default, Debug, PartialEq, PartialOrd, Clone)]
pub struct Temperature
{
    pub value: f32
}

impl Sub for Temperature {
    type Output = Temperature;
    fn sub(self, other: Temperature) -> Temperature {
        Temperature { value: self.value - other.value }
    }
}


impl Temperature
{
    /*pub fn new(value: f32) -> Temperature
    {
        Temperature { value }
    }*/

    pub fn from_yaml(yaml: &Yaml) -> Option<Temperature>
    {
        Some(Temperature { value: yaml.as_f64()? as f32 })
    }

    pub fn abs(&self) -> Temperature
    {
        Temperature { value: self.value.abs() }
    }
}



#[derive(Debug, PartialEq, Clone)]
pub enum PinValue
{
    Temperature(Temperature),
    Analog(u16),
    Digital(bool)
}

impl PinValue
{
    pub fn from_string(kind: &str, message: &str) -> Result<PinValue, &'static str>
    {
        match kind {
                "digital" => {
                    let value = message.parse::<u8>().map_err(|_| "Unable to parse digital value")? > 0;
                    Ok(PinValue::Digital(value))
                },
                "analog" => {
                    let value = message.parse::<u16>().map_err(|_| "Unable to parse analog value")?;
                    Ok(PinValue::Analog(value))
                },
                "temperature" => {
                    let value = message.parse::<f32>().map_err(|_| "Unable to parse temparature value")?;
                    Ok(PinValue::Temperature(Temperature {value }))
                }
                _ => Err("Unknown pin value type")
        }
    }

    pub fn is_digital(&self) -> bool
    {
        match self { PinValue::Digital(_) => true, _ => false}
    }

    pub fn is_analog(&self) -> bool
    {
        match self { PinValue::Analog(_) => true, _ => false}
    }

    pub fn is_temperature(&self) -> bool
    {
        match self { PinValue::Temperature(_) => true, _ => false}
    }

    pub fn is_on(&self) -> bool
    {
        match self { PinValue::Analog(v) => v > &0u16, PinValue::Digital(v) => v == &true, _ => false}
    }
}

#[derive(new, Debug, Clone)]
pub struct PinState
{
    pub pin: u8,
    pub value: PinValue,
    pub dt: DateTime<Local>,
    pub until: Option<DateTime<Local>>
}

impl PinState
{
    pub fn is_on(&self)
    {
        self.value.is_on()
    }
}

#[derive(new, Debug, Clone)]
pub struct PinOperation
{
    pub pin_state: PinState,
    pub node: String,
}

impl PinOperation
{
    /**
     * node1/current/analog/3 2342
     * node1/current/digital/5 1
     * node1/current/digital/5 1
     * node1/current/temperature/5 32.23
     * node1/current/timeout/3600/analog/8 2332
     */
    pub fn from_message(message: &MosqMessage) -> Result<PinOperation, &str>
    {
        let mut paths: Vec<&str> = message.topic().split("/").collect();
        let pin = paths.pop().ok_or("Unable to read string")
            .and_then(|s: &str| s.parse::<u8>().map_err(|_| "Unable to parse integer"))?;
        let value = paths.pop().ok_or("Unknown pin").and_then(|s| PinValue::from_string(s, message.text()))?;
        let op_current = paths.pop().ok_or("Expected current")?;//.map(|s| s == "current").unwrap_or(false);
        let node = paths.pop().ok_or("Unknown node")?;

        if "current" == op_current {
           return Ok(PinOperation {pin_state: PinState { pin, value, dt: Local::now(), until: None }, node: node.to_string()});
        }

        let timeout = op_current.parse::<u32>();
        let is_time_out = paths.pop().map(|s| s == "timeout");
        let until = if is_time_out.is_some() && timeout.is_ok() { Some(Local::now() + chrono::Duration::seconds(timeout.unwrap() as i64)) } else { None };
        let node = if until.is_some() { paths.pop().ok_or("Unknown node after timeout")? } else { node };
        Ok(PinOperation {pin_state: PinState { pin, value, dt: Local::now(), until }, node: node.to_string()})
    }

}

#[derive(Default, new, Debug)]
pub struct PinCollection
{
    states: ArrayDeque<[PinState; 20], Wrapping>,
    changed: ArrayDeque<[PinState; 20], Wrapping>
}

impl PinCollection
{
    pub fn default() -> PinCollection
    {
        PinCollection {states: ArrayDeque::new(), changed: ArrayDeque::new()}
    }

    pub fn push(&mut self, state: &PinState)
    {
        if let PinValue::Digital(v) = state.value {
            let last_state = self.changed.iter().filter(|s| s.value.is_digital()).next();
            if let Some(s) = last_state {
                if let PinValue::Digital(c) = s.value {
                    if  v != c {
                        self.changed.push_front(state.clone());
                    }
                }
            } else {
                self.changed.push_front(state.clone());
            }
        } else if let PinValue::Analog(v) = state.value {
            let last_state = self.changed.iter().filter(|s| s.value.is_analog()).next();
            if let Some(s) = last_state {
                if let PinValue::Analog(c) = s.value {
                    if  (c == 0 && v > 0) || (c > 0 && v == 0) {
                        self.changed.push_front(state.clone());
                    }
                }
            } else {
                self.changed.push_front(state.clone());
            }
        }
        self.states.push_front(state.clone());
    }

    pub fn get_average_temperature(&self) -> Temperature
    {
        let vec: Vec<f32> = self.states.iter().filter(|state| state.value.is_temperature()).map(|state| if let PinValue::Temperature(v) = state.value.clone() { v.value } else { 0f32 }).collect();
        Temperature { value: average(&vec) }
    }

    pub fn is_on(&self) -> bool
    {
        self.changed.front().map(|state| state.until.map(|dt| dt > Local::now()).unwrap_or(true) && match state.value { PinValue::Digital(v) => v, PinValue::Analog(v) => v > 0, _ => false}).unwrap_or(false)
    }

    pub fn is_off(&self) -> bool
    {
        self.changed.front().map(|state| state.until.map(|dt| dt > Local::now()).unwrap_or(true) && match state.value { PinValue::Digital(v) => !v, PinValue::Analog(v) => v == 0, _ => false}).unwrap_or(false)
    }

    pub fn get_last_changed_dt(&self) -> Option<DateTime<Local>>
    {
        self.changed.front().map(|s| s.dt)
    }

    pub fn get_last_changed_value(&self) -> Option<PinValue>
    {
        self.changed.front().map(|state| state.value.clone());
        //.and_then(|state| match state.value { PinValue::Digital(v) => Some(v as u16), PinValue::Analog(v) => Some(v), _ => None})
    }

    pub fn get_last_changed(&self) -> Option<PinState>
    {
        self.changed.front();
    }
}


#[cfg(test)]
mod tests
{
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_pin_collection_is_on_off()
    {
        let mut col = PinCollection::new();
        assert_eq!(col.is_on(), false);

        col.push(&PinState {pin: 3_u8, value: PinValue::Temperature(Temperature::new(20.5_f32)), dt: Local::now(), until: None});
        assert_eq!(col.is_on(), false);
        assert_eq!(col.is_off(), false);

        col.push(&PinState {pin: 1_u8, value: PinValue::Analog(123_u16), dt: Local::now(), until: None});
        assert_eq!(col.is_on(), true);
        assert_eq!(col.is_off(), false);

        col.push(&PinState {pin: 1_u8, value: PinValue::Analog(0_u16), dt: Local::now(), until: None});
        assert_eq!(col.is_on(), false);
        assert_eq!(col.is_off(), true);

        col.push(&PinState {pin: 1_u8, value: PinValue::Analog(123_u16), dt: Local::now(), until: Some(Local::now() + Duration::seconds(3))});
        assert_eq!(col.is_on(), true);
        assert_eq!(col.is_off(), false);

        // turn off first
        col.push(&PinState {pin: 1_u8, value: PinValue::Analog(0_u16), dt: Local::now(), until: None});
        col.push(&PinState {pin: 1_u8, value: PinValue::Analog(123_u16), dt: Local::now(), until: Some(Local::now() - Duration::seconds(3))});
        assert_eq!(col.is_on(), false);
        assert_eq!(col.is_off(), false);
    }


    #[test]
    fn test_pin_collection_get_average_temperature()
    {
        let mut col = PinCollection::new();
        assert_eq!(col.get_average_temperature(), Temperature::new(0_f32));

        col.push(&PinState {pin: 3_u8, value: PinValue::Temperature(Temperature::new(20_f32)), dt: Local::now(), until: None});
        assert_eq!(col.get_average_temperature(), Temperature::new(20_f32));

        col.push(&PinState {pin: 3_u8, value: PinValue::Temperature(Temperature::new(10_f32)), dt: Local::now(), until: None});
        assert_eq!(col.get_average_temperature(), Temperature::new(15_f32));

        col.push(&PinState {pin: 3_u8, value: PinValue::Temperature(Temperature::new(18_f32)), dt: Local::now(), until: None});
        assert_eq!(col.get_average_temperature(), Temperature::new(16_f32));
    }
}
